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

/// Mutually exclusive scheduling states for one window's presentation
/// retries. There is deliberately no separate "armed" boolean: the state
/// itself says whether a retry is owed, queued, or parked.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum RetryState {
    /// Nothing owed; demand decides.
    #[default]
    Idle,
    /// A retry is owed at the deadline.
    Waiting { deadline: std::time::Instant },
    /// A redraw was dispatched for the owed retry and the attempt has not
    /// run yet. Holds no deadline: repeated maintenance must neither
    /// re-dispatch nor wake the loop for it.
    Dispatched,
    /// Parked without a deadline (unconfigured, occluded). Only an
    /// explicit recovery event leaves this state.
    Dormant,
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
    state: RetryState,
}

impl PresentationRetry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Settles one frame outcome at `now`: the attempt ran, so any queued
    /// or waiting retry resolves. Presentation resets to idle; retryable
    /// skips (timeouts, inline recoveries) arm a deadline — immediate for
    /// the first, then backing off; dormant reasons (unconfigured,
    /// occluded) park without deadlines until an explicit recovery event.
    /// Skips never settle waiters: pending work stays eligible for the
    /// armed attempt.
    pub fn note_outcome(&mut self, outcome: &FrameOutcome, now: std::time::Instant) {
        match outcome {
            FrameOutcome::Presented(_) => {
                *self = Self::new();
            }
            FrameOutcome::Skipped(reason) => {
                if reason.should_request_retry() {
                    self.consecutive_skips = self.consecutive_skips.saturating_add(1);
                    self.state = RetryState::Waiting {
                        deadline: now + retry_delay(self.consecutive_skips),
                    };
                } else {
                    self.consecutive_skips = 0;
                    self.state = RetryState::Dormant;
                }
            }
        }
    }

    /// Recovery events that make attempts useful again — non-zero resize,
    /// unocclusion: dormant, waiting, or dispatched windows become
    /// immediately eligible. New application demand deliberately does not
    /// reset: gated demand waits for the armed deadline instead of
    /// bypassing it.
    pub fn note_recovered(&mut self) {
        *self = Self::new();
    }

    /// Armed retry deadline, if any. `Some` exactly while a retry is
    /// owed and undispatched; idle, dispatched, and dormant windows
    /// report `None`.
    #[must_use]
    pub const fn retry_at(&self) -> Option<std::time::Instant> {
        match self.state {
            RetryState::Waiting { deadline } => Some(deadline),
            RetryState::Idle | RetryState::Dispatched | RetryState::Dormant => None,
        }
    }

    /// Whether a requested attempt may run now: idle always, an owed retry
    /// once due, a dispatched redraw already in flight, never while
    /// dormant. Gates demand scheduling and redraw handling. Visibility is
    /// enforced separately by the host's own checks plus [`Self::poll`];
    /// this gate only answers for the retry lifecycle itself.
    #[must_use]
    pub fn attempt_due(&self, now: std::time::Instant) -> bool {
        match self.state {
            RetryState::Idle | RetryState::Dispatched => true,
            RetryState::Dormant => false,
            RetryState::Waiting { deadline } => now >= deadline,
        }
    }

    /// Advances scheduling at `now` for a window the host reports as
    /// runnable (visible and live). This is the one authoritative decision
    /// for both retry dispatch and wake deadlines: it returns whether the
    /// host must request a redraw now, plus the next wake deadline the
    /// loop should wait on, if any.
    ///
    /// - `Idle` / `Dormant`: nothing owed — `(false, None)`. Dormant stays
    ///   parked even past any clock reading; only [`Self::note_recovered`]
    ///   wakes it.
    /// - `Waiting`: hidden windows contribute neither a redraw nor a
    ///   deadline — the owed retry and its backoff progress are preserved
    ///   silently instead of waking the loop for nothing. Visible windows
    ///   dispatch once due (transitioning to `Dispatched`, consuming the
    ///   deadline) and otherwise contribute their deadline.
    /// - `Dispatched`: the redraw is already queued — `(false, None)`,
    ///   however often maintenance polls. If the window stops being
    ///   runnable before the attempt runs, the retry falls back to
    ///   `Waiting` due immediately, so restoring runnability dispatches
    ///   one prompt attempt; the attempt's outcome then settles normally.
    pub fn poll(
        &mut self,
        now: std::time::Instant,
        runnable: bool,
    ) -> (bool, Option<std::time::Instant>) {
        match self.state {
            RetryState::Idle | RetryState::Dormant => (false, None),
            RetryState::Waiting { deadline } => {
                if !runnable {
                    (false, None)
                } else if now >= deadline {
                    self.state = RetryState::Dispatched;
                    (true, None)
                } else {
                    (false, Some(deadline))
                }
            }
            RetryState::Dispatched => {
                if !runnable {
                    self.state = RetryState::Waiting { deadline: now };
                }
                (false, None)
            }
        }
    }
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
