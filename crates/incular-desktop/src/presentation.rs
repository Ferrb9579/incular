//! Per-window GPU submission and observable frame completion.
use crate::{runtime_render_metrics, window_host::NativeWindowState};
use incular_rendering::DisplayList;
use incular_runtime::{Application, GpuSample, RenderFrameMetrics, Screenshot};
use incular_wgpu::{FrameOutcome, RendererError};

/// Delay before the next attempt after this many consecutive retryable
/// skips. The first retry is immediate: a single transient timeout or an
/// inline surface recovery usually succeeds at once. Further consecutive
/// failures back off exponentially to a quarter-second cap, so repeated
/// failures stay responsive without spinning full-frame work.
fn retry_delay(consecutive_skips: u32) -> std::time::Duration {
    match consecutive_skips {
        0 | 1 => std::time::Duration::ZERO,
        2 => std::time::Duration::from_millis(16),
        3 => std::time::Duration::from_millis(32),
        4 => std::time::Duration::from_millis(64),
        5 => std::time::Duration::from_millis(128),
        _ => std::time::Duration::from_millis(250),
    }
}

/// Host-owned pacing for presentation retries: one per window (normal and
/// transient alike), driven by settled frame outcomes and explicit clock
/// readings. The event loop owns deadlines and window lifetime; the
/// runtime and renderer hold no retry state. All decisions are pure over
/// the injected `now`, so headless tests drive exact dispatch times with
/// a fake clock.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PresentationRetry {
    consecutive_skips: u32,
    retry_at: Option<std::time::Instant>,
    dormant: bool,
}

impl PresentationRetry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Settles one frame outcome at `now`. Presentation resets to idle;
    /// retryable skips (timeouts, inline recoveries) arm a deadline —
    /// immediate for the first, then backing off; dormant reasons
    /// (unconfigured, occluded) park without deadlines until an explicit
    /// recovery event. Skips never settle waiters: pending work stays
    /// eligible for the armed attempt.
    pub fn note_outcome(&mut self, outcome: &FrameOutcome, now: std::time::Instant) {
        match outcome {
            FrameOutcome::Presented(_) => {
                *self = Self::new();
            }
            FrameOutcome::Skipped(reason) => {
                if reason.should_request_retry() {
                    self.dormant = false;
                    self.consecutive_skips = self.consecutive_skips.saturating_add(1);
                    self.retry_at = Some(now + retry_delay(self.consecutive_skips));
                } else {
                    self.consecutive_skips = 0;
                    self.retry_at = None;
                    self.dormant = true;
                }
            }
        }
    }

    /// Recovery events that make attempts useful again — non-zero resize,
    /// unocclusion: dormant or backed-off windows become immediately
    /// eligible. New application demand deliberately does not reset: gated
    /// demand waits for the armed deadline instead of bypassing it.
    pub fn note_recovered(&mut self) {
        *self = Self::new();
    }

    /// Armed retry deadline, if any. `None` means idle (demand decides) or
    /// dormant (recovery events decide).
    #[must_use]
    pub const fn retry_at(&self) -> Option<std::time::Instant> {
        self.retry_at
    }

    /// Whether an attempt may run now: idle always, armed deadlines once
    /// due, never while dormant. Gates both demand scheduling and redraw
    /// handling, so no path can recreate an unrestricted cycle.
    #[must_use]
    pub fn attempt_due(&self, now: std::time::Instant) -> bool {
        !self.dormant && self.retry_at.is_none_or(|deadline| now >= deadline)
    }

    /// Whether a paced retry is owed now: an armed deadline has passed.
    /// Unlike [`Self::attempt_due`], idle windows report false — the host
    /// dispatches redraws from this, so idle windows are never prodded.
    #[must_use]
    pub fn retry_due(&self, now: std::time::Instant) -> bool {
        !self.dormant && self.retry_at.is_some_and(|deadline| now >= deadline)
    }
}

/// Earliest armed deadline across live windows, normal and transient:
/// pass each window's [`PresentationRetry::retry_at`]. `None` means no
/// window owes a paced retry and the loop may wait indefinitely. Closing
/// a window drops its policy with it, which simply stops contributing —
/// pending retries cancel without further action.
#[must_use]
pub fn earliest_retry_after(
    deadlines: impl IntoIterator<Item = Option<std::time::Instant>>,
) -> Option<std::time::Instant> {
    deadlines.into_iter().flatten().min()
}

pub(crate) fn present_window(
    application: &mut Application,
    state: &mut NativeWindowState,
    list: &DisplayList,
    #[cfg(feature = "devtools")] frame: &incular_runtime::FrameStats,
    #[cfg(feature = "devtools")] devtools: &mut crate::devtools_runner::DevToolsState,
) {
    let id = state.id;
    match state.renderer.render(list, state.metrics.scale_factor) {
        Ok(outcome) => {
            // Settle pacing first: skips arm a deadline (or park dormant)
            // instead of requesting an immediate redraw, so no path can
            // spin an unrestricted cycle. The armed deadline fires from the
            // event loop's wait control.
            state
                .retry
                .note_outcome(&outcome, std::time::Instant::now());
            let stats = match outcome {
                FrameOutcome::Presented(stats) => {
                    application.note_presented(id, true);
                    stats
                }
                FrameOutcome::Skipped(_) => {
                    // Skipped attempts record no presentation and no frame
                    // metrics beyond an empty sample; simulator waiters stay
                    // pending for the armed attempt.
                    application.note_presented(id, false);
                    application.note_render_metrics(id, RenderFrameMetrics::default(), None);
                    application.complete_simulation_frame(id, false, None);
                    return;
                }
            };
            let gpu = state.renderer.gpu_frame_timings().map(|timing| GpuSample {
                supported: true,
                frame: timing.frame,
                main_pass_us: timing.main_pass_us,
            });
            application.note_render_metrics(id, runtime_render_metrics(&stats), gpu);
            let capture = state.renderer.take_capture().map(|result| {
                result.and_then(|frame| {
                    Screenshot::from_rgba8(frame.width, frame.height, frame.rgba8)
                        .map_err(|error| error.to_string())
                })
            });
            application.complete_simulation_frame(id, true, capture);
            #[cfg(feature = "devtools")]
            {
                let budget_us = state
                    .window
                    .current_monitor()
                    .and_then(|monitor| monitor.refresh_rate_millihertz())
                    .filter(|rate| *rate > 0)
                    .map(|rate| {
                        u32::try_from(1_000_000_000_u64 / u64::from(rate)).unwrap_or(u32::MAX)
                    });
                let cpu_total = frame
                    .timings
                    .cpu_total
                    .saturating_add(stats.prepare_us)
                    .saturating_add(stats.encode_us)
                    .saturating_add(stats.submit_us);
                let frame_id = devtools.next_frame();
                devtools.push_frame(incular_devtools_protocol::TargetEvent::FrameRecord(
                    incular_devtools_protocol::FrameRecordEvent {
                        window: incular_devtools_protocol::DevWindowId::new(
                            u64::from(id.index()) + 1,
                            u64::from(id.generation()),
                        ),
                        frame: frame_id,
                        timings: incular_devtools_protocol::FrameTimingsWire {
                            event_processing: frame.timings.event_processing,
                            runtime_messages: frame.timings.runtime_messages,
                            build: frame.timings.build,
                            layout: frame.timings.layout,
                            composite: frame.timings.composite,
                            semantics: frame.timings.semantics,
                            paint: frame.timings.paint,
                            cpu_total,
                            prepare: stats.prepare_us,
                            encode: stats.encode_us,
                            submit: stats.submit_us,
                            gpu_us: state
                                .renderer
                                .gpu_frame_timings()
                                .map(|timing| timing.main_pass_us),
                        },
                        budget_us,
                        over_budget: budget_us.is_some_and(|budget| cpu_total > budget),
                        draw_calls: stats.draw_calls,
                        instances: stats.total_instances(),
                        upload_bytes: stats.upload_bytes,
                        pipelines_created: stats.pipelines_created,
                    },
                ));
                if devtools.note_recorded_frame()
                    && devtools.deep_recording()
                    && let Some(trace) = application.devtools_take_deep_trace(id, frame_id)
                {
                    devtools.push_frame(incular_devtools_protocol::TargetEvent::DeepTrace(trace));
                }
            }
            // Presented frames never re-request: the next frame comes from
            // normal application demand. Skipped attempts return early
            // above with reason-gated retries.
        }
        Err(RendererError::OutOfMemory) => {
            eprintln!("Incular renderer stopped: out of GPU memory");
            application.fail_simulation_frame(id, "renderer stopped: out of GPU memory");
            #[cfg(feature = "devtools")]
            devtools.push_frame(incular_devtools_protocol::TargetEvent::Log {
                level: "error".into(),
                target: "incular::renderer".into(),
                message: "renderer stopped: out of GPU memory".into(),
            });
        }
        Err(error) => {
            eprintln!("Incular renderer error: {error}");
            application.fail_simulation_frame(id, error.to_string());
            #[cfg(feature = "devtools")]
            devtools.push_frame(incular_devtools_protocol::TargetEvent::Log {
                level: "error".into(),
                target: "incular::renderer".into(),
                message: error.to_string(),
            });
        }
    }
}
