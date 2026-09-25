use crate::{RendererError, SurfaceAlphaPlan};
use incular_config::TransparencyMode;

/// Narrowly scoped to the driver/device combination reproduced by the idle probe.
/// Do not infer that every AMD GPU or every Windows driver needs this work.
pub(crate) fn needs_present_bootstrap(info: &wgpu::AdapterInfo) -> bool {
    info.vendor == 0x1002
        && info.device == 0x164e
        && match info.backend {
            wgpu::Backend::Dx12 => info.driver == "32.0.21036.11002",
            wgpu::Backend::Vulkan => info.driver_info.starts_with("25.10.36.11 "),
            _ => false,
        }
}

/// AMD's presentation worker can busy-spin indefinitely after a first draw.
/// A finite series of presentations lets it finish initialization and sleep.
/// See GPUOpen-Drivers/AMD-Gfx-Drivers#106 and the repository's idle probe.
/// This runs once per device, before the application renders, on its temporary
/// adapter-selection surface. No timer or ongoing redraw loop is installed.
pub(crate) async fn bootstrap_presentation(
    surface: &wgpu::Surface<'_>,
    adapter: &wgpu::Adapter,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    transparency: TransparencyMode,
) -> Result<(), RendererError> {
    if !cfg!(target_os = "windows") || !needs_present_bootstrap(&adapter.get_info()) {
        return Ok(());
    }
    let capabilities = surface.get_capabilities(adapter);
    let mut config = surface
        .get_default_config(adapter, 1, 1)
        .ok_or(RendererError::SurfaceConfigurationUnsupported)?;
    config.alpha_mode = SurfaceAlphaPlan::select(transparency, &capabilities.alpha_modes)
        .map_err(RendererError::SurfaceAlpha)?
        .composite_mode();
    // Use the adapter's default presentation mode, as in the reproducer.
    surface.configure(device, &config);
    let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("AMD presentation bootstrap"),
        source: crate::built_in_shaders::decode(crate::built_in_shaders::BOOTSTRAP_SHADER),
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("AMD presentation bootstrap"),
        layout: None,
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(config.format.into())],
        }),
        multiview_mask: None,
        cache: None,
    });
    if let Some(error) = scope.pop().await {
        return Err(RendererError::PipelineCreation {
            label: "AMD presentation bootstrap".into(),
            reason: error.to_string(),
        });
    }
    // 128 still spins on the affected driver; 256 settles both Vulkan and DX12.
    // Render only one pixel and cap queued submissions, keeping startup bounded.
    for index in 0..256 {
        let frame = match surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            other => {
                return Err(RendererError::DriverInitialization(format!(
                    "AMD presentation bootstrap acquisition: {other:?}"
                )));
            }
        };
        let view = frame.texture.create_view(&Default::default());
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&pipeline);
            pass.draw(0..3, 0..1);
        }
        queue.submit([encoder.finish()]);
        queue.present(frame);
        if index % 8 == 7 {
            device
                .poll(wgpu::PollType::Wait {
                    submission_index: None,
                    timeout: Some(std::time::Duration::from_secs(2)),
                })
                .map_err(|error| RendererError::DriverInitialization(error.to_string()))?;
        }
    }
    Ok(())
}
