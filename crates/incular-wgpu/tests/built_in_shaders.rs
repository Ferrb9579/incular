use std::path::Path;

#[allow(dead_code)]
#[path = "../src/built_in_shaders.rs"]
mod built_in_shaders;

#[test]
fn embedded_modules_match_sources_and_validate_on_gpu() {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()));
    let device = adapter.ok().map(|adapter| {
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .expect("test device")
            .0
    });
    if std::env::var("INCULAR_WGPU_REQUIRE_GPU").as_deref() == Ok("1") {
        assert!(device.is_some(), "GPU validation required");
    }
    let shaders = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/shaders");
    let mut count = 0;
    for entry in std::fs::read_dir(shaders).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|extension| extension != "wgsl") {
            continue;
        }
        let source = std::fs::read_to_string(&path).unwrap();
        let parsed = naga::front::wgsl::parse_str(&source).unwrap();
        let stem = path.file_stem().unwrap().to_str().unwrap();
        let bytes = std::fs::read(Path::new(env!("OUT_DIR")).join(format!("{stem}.naga"))).unwrap();
        let decoded: naga::Module = postcard::from_bytes(&bytes).unwrap();
        assert_eq!(
            postcard::to_allocvec(&parsed).unwrap(),
            postcard::to_allocvec(&decoded).unwrap(),
            "{stem}: embedded IR must retain the full source module"
        );
        if let Some(device) = &device {
            let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
            let _module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(stem),
                source: wgpu::ShaderSource::Naga(std::borrow::Cow::Owned(decoded)),
            });
            assert!(pollster::block_on(scope.pop()).is_none(), "{stem}");
        }
        count += 1;
    }
    assert_eq!(count, 13, "all renderer and bootstrap shaders are covered");
    // Exercise the production decoder as well as the file-by-file checks.
    assert!(matches!(
        built_in_shaders::decode(built_in_shaders::BOOTSTRAP_SHADER),
        wgpu::ShaderSource::Naga(_)
    ));
}
