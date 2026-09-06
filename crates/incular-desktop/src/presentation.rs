//! Per-window GPU submission and observable frame completion.
use crate::{runtime_render_metrics, window_host::NativeWindowState};
use incular_rendering::DisplayList;
use incular_runtime::{Application, GpuSample, Screenshot};
use incular_wgpu::RendererError;

pub(crate) fn present_window(
    application: &mut Application,
    state: &mut NativeWindowState,
    list: &DisplayList,
    #[cfg(feature = "devtools")] frame: &incular_runtime::FrameStats,
    #[cfg(feature = "devtools")] devtools: &mut crate::devtools_runner::DevToolsState,
) {
    let id = state.id;
    match state.renderer.render(list, state.metrics.scale_factor) {
        Ok(stats) => {
            application.note_presented(id, stats.presented);
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
            application.complete_simulation_frame(id, stats.presented, capture);
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
            if !stats.presented {
                state.window.request_redraw();
            }
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
