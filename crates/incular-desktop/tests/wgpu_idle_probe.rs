//! Opt-in minimal native GPU probe, with no Incular runtime or renderer.
//! INCULAR_GPU_PROBE_STAGE selects window, instance, surface, adapter, device,
//! configure, submit, or present. Each process stops at that stage and idles.
use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

#[derive(Default)]
struct Probe {
    window: Option<Arc<Window>>,
    instance: Option<wgpu::Instance>,
    surface: Option<wgpu::Surface<'static>>,
    adapter: Option<wgpu::Adapter>,
    device: Option<wgpu::Device>,
    queue: Option<wgpu::Queue>,
    pipeline: Option<wgpu::RenderPipeline>,
    pending_frame: Option<wgpu::SurfaceTexture>,
}

fn ready() {
    if let Ok(path) = std::env::var("INCULAR_BENCH_READY") {
        std::fs::write(path, "minimal GPU probe ready").unwrap();
    }
    eprintln!("PROBE READY");
}

impl ApplicationHandler for Probe {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        event_loop.set_control_flow(ControlFlow::Wait);
        let stage = std::env::var("INCULAR_GPU_PROBE_STAGE").unwrap();
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title(format!("WGPU idle probe: {stage}"))
                        .with_inner_size(winit::dpi::LogicalSize::new(1100., 720.)),
                )
                .unwrap(),
        );
        self.window = Some(window.clone());
        if stage == "window" {
            ready();
            return;
        }
        self.instance = Some(wgpu::Instance::new(
            wgpu::InstanceDescriptor::new_without_display_handle_from_env(),
        ));
        if stage == "instance" {
            ready();
            return;
        }
        self.surface = Some(
            self.instance
                .as_ref()
                .unwrap()
                .create_surface(window.clone())
                .unwrap(),
        );
        if stage == "surface" {
            ready();
            return;
        }
        self.adapter = Some(
            pollster::block_on(self.instance.as_ref().unwrap().request_adapter(
                &wgpu::RequestAdapterOptions {
                    compatible_surface: self.surface.as_ref(),
                    ..Default::default()
                },
            ))
            .unwrap(),
        );
        eprintln!("ADAPTER {:?}", self.adapter.as_ref().unwrap().get_info());
        if stage == "adapter" {
            ready();
            return;
        }
        let (device, queue) = pollster::block_on(
            self.adapter
                .as_ref()
                .unwrap()
                .request_device(&wgpu::DeviceDescriptor {
                    required_limits: wgpu::Limits {
                        max_non_sampler_bindings: std::env::var("INCULAR_GPU_PROBE_BINDINGS")
                            .ok()
                            .and_then(|value| value.parse().ok())
                            .unwrap_or(1_000_000),
                        ..Default::default()
                    },
                    memory_hints: if std::env::var("INCULAR_GPU_PROBE_SMALL_ALLOCATIONS").as_deref()
                        == Ok("1")
                    {
                        wgpu::MemoryHints::Manual {
                            suballocated_device_memory_block_size: (4 << 20)..(64 << 20),
                        }
                    } else {
                        wgpu::MemoryHints::MemoryUsage
                    },
                    ..Default::default()
                }),
        )
        .unwrap();
        self.device = Some(device);
        self.queue = Some(queue);
        if stage == "device" {
            ready();
            return;
        }
        let device = self.device.as_ref().unwrap();
        let queue = self.queue.as_ref().unwrap();
        if stage == "submit" {
            queue.submit([device.create_command_encoder(&Default::default()).finish()]);
            ready();
            return;
        }
        let mut surface = self.surface.take().unwrap();
        let size = window.inner_size();
        let mut config = surface
            .get_default_config(self.adapter.as_ref().unwrap(), size.width, size.height)
            .unwrap();
        if stage.ends_with("-copy") {
            config.usage |= wgpu::TextureUsages::COPY_SRC;
        }
        if stage == "draw-offscreen-present" {
            config.usage |= wgpu::TextureUsages::COPY_DST;
        }
        if std::env::var("INCULAR_GPU_PROBE_FIFO").as_deref() == Ok("1") {
            config.present_mode = wgpu::PresentMode::Fifo;
        }
        if std::env::var("INCULAR_GPU_PROBE_TINY").as_deref() == Ok("1") {
            config.width = 1;
            config.height = 1;
        }
        eprintln!("CONFIG {config:?}");
        surface.configure(device, &config);
        if stage == "configure" {
            self.surface = Some(surface);
            ready();
            return;
        }
        assert!(
            matches!(
                stage.as_str(),
                "present"
                    | "present-copy"
                    | "draw"
                    | "draw-copy"
                    | "compile"
                    | "compile-present"
                    | "draw-then-clear"
                    | "draw-no-present"
                    | "draw-offscreen-present"
                    | "draw-drop-surface"
            ),
            "unknown probe stage"
        );
        let pipeline = (stage.starts_with("draw") || stage.starts_with("compile")).then(|| {
            let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("minimal triangle"),
                source: wgpu::ShaderSource::Wgsl(r#"
                    @vertex fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4f {
                        let positions = array<vec2f, 3>(vec2f(-0.8, -0.8), vec2f(0.8, -0.8), vec2f(0.0, 0.8));
                        return vec4f(positions[index], 0.0, 1.0);
                    }
                    @fragment fn fs_main() -> @location(0) vec4f { return vec4f(0.2, 0.7, 0.4, 1.0); }
                "#.into()),
            });
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("minimal triangle"), layout: None,
                vertex: wgpu::VertexState { module: &module, entry_point: Some("vs_main"), compilation_options: Default::default(), buffers: &[] },
                primitive: Default::default(), depth_stencil: None, multisample: Default::default(),
                fragment: Some(wgpu::FragmentState { module: &module, entry_point: Some("fs_main"), compilation_options: Default::default(), targets: &[Some(config.format.into())] }),
                multiview_mask: None, cache: None,
            })
        });
        if stage == "compile" {
            self.surface = Some(surface);
            self.pipeline = pipeline;
            ready();
            return;
        }
        let offscreen = (stage == "draw-offscreen-present").then(|| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some("offscreen triangle"),
                size: wgpu::Extent3d {
                    width: config.width,
                    height: config.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: config.format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            })
        });
        let warmup = std::env::var("INCULAR_GPU_PROBE_WARMUP")
            .ok()
            .and_then(|value| value.parse::<u32>().ok());
        let frame_count = if let Some(frames) = warmup {
            frames
        } else if stage == "draw-then-clear" {
            4
        } else {
            3
        };
        let restore = std::env::var("INCULAR_GPU_PROBE_RESTORE").as_deref() == Ok("1");
        for frame_index in 0..frame_count + if restore { 3 } else { 0 } {
            if restore && frame_index == frame_count {
                device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
                std::thread::sleep(std::time::Duration::from_secs(2));
                if std::env::var("INCULAR_GPU_PROBE_RECREATE").as_deref() == Ok("1") {
                    drop(surface);
                    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
                    surface = self
                        .instance
                        .as_ref()
                        .unwrap()
                        .create_surface(window.clone())
                        .unwrap();
                }
                config.width = window.inner_size().width;
                config.height = window.inner_size().height;
                surface.configure(device, &config);
            }
            let frame = match surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(frame)
                | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
                other => panic!("surface acquisition failed: {other:?}"),
            };
            let view = offscreen
                .as_ref()
                .unwrap_or(&frame.texture)
                .create_view(&Default::default());
            let mut encoder = device.create_command_encoder(&Default::default());
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    ..Default::default()
                });
                if let Some(pipeline) = &pipeline
                    && stage.starts_with("draw")
                    && (stage != "draw-then-clear" || frame_index < 3 || frame_index >= frame_count)
                    && (std::env::var("INCULAR_GPU_PROBE_CLEAR_FIRST").as_deref() != Ok("1")
                        || frame_index >= frame_count)
                    && (warmup.is_some() || frame_index < 3)
                {
                    pass.set_pipeline(pipeline);
                    pass.draw(0..3, 0..1);
                }
            }
            if let Some(texture) = &offscreen {
                encoder.copy_texture_to_texture(
                    texture.as_image_copy(),
                    frame.texture.as_image_copy(),
                    texture.size(),
                );
            }
            queue.submit([encoder.finish()]);
            if warmup.is_some() {
                std::thread::sleep(std::time::Duration::from_millis(
                    std::env::var("INCULAR_GPU_PROBE_PACE_MS")
                        .ok()
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(16),
                ));
            }
            window.pre_present_notify();
            if stage != "draw-no-present" {
                queue.present(frame);
            }
            if std::env::var("INCULAR_GPU_PROBE_DRAIN").as_deref() == Ok("1")
                && frame_index % 8 == 7
            {
                device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            }
        }
        if std::env::var("INCULAR_GPU_PROBE_PREFETCH").as_deref() == Ok("1") {
            self.pending_frame = match surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(frame)
                | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => Some(frame),
                other => panic!("prefetch failed: {other:?}"),
            };
        }
        if std::env::var("INCULAR_GPU_PROBE_NATIVE_IDLE").as_deref() == Ok("1") {
            // Diagnostic only: this probe owns all submissions and no other thread uses its queue.
            unsafe {
                if let Some(native) = device.as_hal::<wgpu::hal::api::Vulkan>() {
                    native.raw_device().device_wait_idle().unwrap();
                }
            }
        }
        self.pipeline = pipeline;
        if stage != "draw-drop-surface" {
            self.surface = Some(surface);
        }
        ready();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        if matches!(event, WindowEvent::CloseRequested) {
            event_loop.exit();
        }
    }
}

fn main() {
    if std::env::var_os("INCULAR_GPU_PROBE_STAGE").is_none() {
        eprintln!("wgpu_idle_probe: opt-in diagnostic; set INCULAR_GPU_PROBE_STAGE");
        return;
    }
    EventLoop::new()
        .unwrap()
        .run_app(&mut Probe::default())
        .unwrap();
}
