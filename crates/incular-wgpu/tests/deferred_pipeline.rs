use incular_wgpu::RendererError;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[path = "../src/deferred_pipeline.rs"]
mod deferred_pipeline;
use deferred_pipeline::DeferredPipeline;

const SHADER: &str = r#"
@vertex fn vs_main(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    return vec4<f32>(f32(i), 0.0, 0.0, 1.0);
}
@fragment fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0);
}
"#;

fn pipeline(device: &wgpu::Device, shader: &str) -> wgpu::RenderPipeline {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("deferred pipeline fixture"),
        source: wgpu::ShaderSource::Wgsl(shader.into()),
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("deferred fixture"),
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
            targets: &[Some(wgpu::TextureFormat::Rgba8Unorm.into())],
        }),
        multiview_mask: None,
        cache: None,
    })
}

#[test]
fn first_use_is_shared_and_validation_failures_are_cached_without_panicking() {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default()))
        .expect("headless adapter");
    let (device, _) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        memory_hints: wgpu::MemoryHints::MemoryUsage,
        ..Default::default()
    }))
    .expect("headless device");
    let creations = Arc::new(AtomicUsize::new(0));
    let count = creations.clone();
    let pipeline_device = device.clone();
    let deferred = DeferredPipeline::new(&device, "valid fixture", move || {
        count.fetch_add(1, Ordering::SeqCst);
        pipeline(&pipeline_device, SHADER)
    });
    let second_window = deferred.clone();
    assert!(!deferred.is_created());
    assert_eq!(creations.load(Ordering::SeqCst), 0);
    std::thread::scope(|scope| {
        for _ in 0..4 {
            scope.spawn(|| {
                second_window.get().expect("shared compilation");
            });
        }
    });
    assert!(deferred.is_created());
    assert_eq!(creations.load(Ordering::SeqCst), 1);
    assert!(std::ptr::eq(
        deferred.get().unwrap(),
        second_window.get().unwrap()
    ));

    let failures = Arc::new(AtomicUsize::new(0));
    let count = failures.clone();
    let pipeline_device = device.clone();
    let invalid = DeferredPipeline::new(&device, "invalid fixture", move || {
        count.fetch_add(1, Ordering::SeqCst);
        pipeline(&pipeline_device, "invalid WGSL")
    });
    for entry in [&invalid, &invalid.clone()] {
        match entry.get() {
            Err(RendererError::PipelineCreation { label, reason }) => {
                assert_eq!(label, "invalid fixture");
                assert!(!reason.is_empty());
            }
            other => panic!("expected labeled validation error: {other:?}"),
        }
    }
    assert!(!invalid.is_created());
    assert_eq!(failures.load(Ordering::SeqCst), 1);
}
