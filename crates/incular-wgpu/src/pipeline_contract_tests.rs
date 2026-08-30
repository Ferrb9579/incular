use super::prelude::*;
use super::resources::{SharedGpuContextInner, SharedGpuResources};
use super::*;

/// Renderable everywhere; shaders do not depend on the target format.
const VALIDATION_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// One user-defined stage-interface slot: a single `@location(N)` value
/// with enough information to compare WGSL declarations against Rust
/// vertex-buffer layouts and against the opposite shader stage.
#[derive(Clone, Copy, Debug, PartialEq)]
struct InterfaceSlot {
    location: u32,
    kind: naga::ScalarKind,
    width: u8,
    components: u32,
    interpolation: Option<naga::Interpolation>,
    sampling: Option<naga::Sampling>,
}

fn validated_module(shader: &str, label: &str) -> naga::Module {
    let module = naga::front::wgsl::parse_str(shader)
        .unwrap_or_else(|error| panic!("{label}: WGSL failed to parse: {error}"));
    let mut validator = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    );
    validator
        .validate(&module)
        .unwrap_or_else(|error| panic!("{label}: Naga rejected the module: {error}"));
    module
}

fn entry_function<'m>(
    module: &'m naga::Module,
    stage: naga::ShaderStage,
    name: &str,
    label: &str,
) -> &'m naga::Function {
    module
        .entry_points
        .iter()
        .find(|entry| entry.stage == stage && entry.name == name)
        .map(|entry| &entry.function)
        .unwrap_or_else(|| panic!("{label}: missing {stage:?} entry point '{name}'"))
}

fn collect_slots(
    module: &naga::Module,
    ty: naga::Handle<naga::Type>,
    binding: Option<&naga::Binding>,
    out: &mut Vec<InterfaceSlot>,
) {
    match binding {
        Some(naga::Binding::Location {
            location,
            interpolation,
            sampling,
            ..
        }) => {
            let (kind, width, components) = match &module.types[ty].inner {
                naga::TypeInner::Scalar(scalar) => (scalar.kind, scalar.width, 1),
                naga::TypeInner::Vector { size, scalar } => {
                    (scalar.kind, scalar.width, *size as u32)
                }
                other => panic!("unexpected varying type in interface: {other:?}"),
            };
            out.push(InterfaceSlot {
                location: *location,
                kind,
                width,
                components,
                interpolation: *interpolation,
                sampling: *sampling,
            });
        }
        _ => {
            // Entry-point arguments/results may be I/O structs whose
            // members carry their own location bindings.
            if let naga::TypeInner::Struct { members, .. } = &module.types[ty].inner {
                for member in members {
                    collect_slots(module, member.ty, member.binding.as_ref(), out);
                }
            }
        }
    }
}

fn input_slots(module: &naga::Module, function: &naga::Function) -> Vec<InterfaceSlot> {
    let mut slots = Vec::new();
    for argument in &function.arguments {
        collect_slots(module, argument.ty, argument.binding.as_ref(), &mut slots);
    }
    slots
}

fn output_slots(module: &naga::Module, function: &naga::Function) -> Vec<InterfaceSlot> {
    let mut slots = Vec::new();
    if let Some(result) = &function.result {
        collect_slots(module, result.ty, result.binding.as_ref(), &mut slots);
    }
    slots
}

/// The `wgpu::VertexFormat` a `VertexBufferLayout` must declare so the
/// GPU hands the shader exactly the type its WGSL signature requests.
fn required_vertex_format(slot: &InterfaceSlot) -> wgpu::VertexFormat {
    assert_eq!(slot.width, 4, "renderer varyings must be 32-bit");
    match (slot.kind, slot.components) {
        (naga::ScalarKind::Float, 1) => wgpu::VertexFormat::Float32,
        (naga::ScalarKind::Float, 2) => wgpu::VertexFormat::Float32x2,
        (naga::ScalarKind::Float, 3) => wgpu::VertexFormat::Float32x3,
        (naga::ScalarKind::Float, 4) => wgpu::VertexFormat::Float32x4,
        (naga::ScalarKind::Uint, 1) => wgpu::VertexFormat::Uint32,
        (naga::ScalarKind::Uint, 2) => wgpu::VertexFormat::Uint32x2,
        (naga::ScalarKind::Uint, 3) => wgpu::VertexFormat::Uint32x3,
        (naga::ScalarKind::Uint, 4) => wgpu::VertexFormat::Uint32x4,
        (naga::ScalarKind::Sint, 1) => wgpu::VertexFormat::Sint32,
        (naga::ScalarKind::Sint, 2) => wgpu::VertexFormat::Sint32x2,
        (naga::ScalarKind::Sint, 3) => wgpu::VertexFormat::Sint32x3,
        (naga::ScalarKind::Sint, 4) => wgpu::VertexFormat::Sint32x4,
        other => panic!("unsupported varying shape {other:?}"),
    }
}

fn contract_shader(contract: &PipelineContract, stage: naga::ShaderStage) -> naga::Module {
    let name = format!("{} ({stage:?})", contract.label);
    validated_module(contract.shader, &name)
}

/// `(shader_location, format, byte offset, buffer stride)` for every
/// declared attribute of the contract's vertex streams.
fn contract_attributes(contract: &PipelineContract) -> Vec<(u32, wgpu::VertexFormat, u64, u64)> {
    stream_layouts(contract.streams)
        .into_iter()
        .flatten()
        .flat_map(|layout| {
            layout.attributes.iter().map(move |attribute| {
                (
                    attribute.shader_location,
                    attribute.format,
                    attribute.offset,
                    layout.array_stride,
                )
            })
        })
        .collect()
}

#[test]
fn production_pipeline_inventory_matches_contracts() {
    // The registry must stay complete and duplicate-free: every class in
    // `SharedPipelineResources` has exactly one contract.
    let contracts = pipeline_contracts();
    assert_eq!(contracts.len(), 25, "renderer pipeline inventory changed");
    let mut classes: Vec<PipelineClass> = contracts.iter().map(|c| c.class).collect();
    classes.sort_by_key(|class| format!("{class:?}"));
    let count = classes.len();
    classes.dedup_by_key(|class| format!("{class:?}"));
    assert_eq!(count, classes.len(), "duplicate pipeline classes");
    for contract in &contracts {
        assert!(!contract.label.is_empty());
        assert!(contract.shader.contains("@vertex"));
        assert!(contract.shader.contains("@fragment"));
    }
}

#[test]
fn all_production_shaders_parse_and_validate_with_naga() {
    let mut unique: HashMap<&'static str, &'static str> = HashMap::new();
    for contract in pipeline_contracts() {
        unique.insert(contract.shader_module_label, contract.shader);
    }
    // rect, text, image, rrect, path, composite, fixed blend, blur,
    // resample, color matrix, destination blend.
    assert!(
        unique.len() >= 11,
        "expected at least eleven distinct shader modules"
    );
    for (module_label, shader) in unique {
        let module = validated_module(shader, module_label);
        entry_function(&module, naga::ShaderStage::Vertex, "vs_main", module_label);
        entry_function(
            &module,
            naga::ShaderStage::Fragment,
            "fs_main",
            module_label,
        );
    }
}

/// Regression test for the Task 13 sweep-gradient breakage: the retained
/// path vertex shader gained an `options` input (`@location(6)` selecting
/// solid/linear/radial/sweep brushes) but the Rust instance layout kept
/// only locations 1-5, so every `create_render_pipeline` call failed
/// validation. Asserts the exact final interface instead of merely
/// checking that *some* error went away.
#[test]
fn retained_path_pipeline_vertex_interface_matches_layout() {
    let contract = pipeline_contracts()
        .into_iter()
        .find(|contract| contract.class == PipelineClass::Path)
        .expect("path pipeline contract");
    let module = contract_shader(&contract, naga::ShaderStage::Vertex);
    let vertex = entry_function(
        &module,
        naga::ShaderStage::Vertex,
        "vs_main",
        contract.label,
    );

    let inputs = input_slots(&module, vertex);
    let mut locations: Vec<u32> = inputs.iter().map(|slot| slot.location).collect();
    locations.sort_unstable();
    assert_eq!(locations, vec![0, 1, 2, 3, 4, 5, 6]);
    assert_eq!(inputs[0].components, 2, "@location(0) is the quad xy");

    let attributes = contract_attributes(&contract);
    let mut provided: Vec<u32> = attributes.iter().map(|(location, ..)| *location).collect();
    provided.sort_unstable();
    assert_eq!(provided, vec![0, 1, 2, 3, 4, 5, 6]);

    for slot in &inputs {
        let expected = required_vertex_format(slot);
        let found = attributes
            .iter()
            .find(|(location, ..)| *location == slot.location)
            .map(|(_, format, ..)| *format);
        assert_eq!(
            found,
            Some(expected),
            "{}: @location({}) requires {:?} from the vertex layout",
            contract.label,
            slot.location,
            expected
        );
    }

    // The instance stride must equal the exact `#[repr(C)]` upload size,
    // and location 6 must read the brush-kind field (`options.x`: solid,
    // linear, radial, sweep) rather than any dummy byte range.
    let layouts = stream_layouts(contract.streams);
    let instance = layouts
        .iter()
        .flatten()
        .find(|layout| layout.step_mode == wgpu::VertexStepMode::Instance)
        .expect("path pipeline has an instance stream");
    assert_eq!(
        instance.array_stride as usize,
        std::mem::size_of::<GpuPathInstance>()
    );
    let options_attribute = attributes
        .iter()
        .find(|(location, ..)| *location == 6)
        .expect("location 6 is supplied");
    assert_eq!(options_attribute.1, wgpu::VertexFormat::Float32x4);
    assert_eq!(
        options_attribute.2,
        std::mem::offset_of!(GpuPathInstance, options) as u64
    );
    assert_eq!(
        options_attribute.3 as usize,
        std::mem::size_of::<GpuPathInstance>()
    );
}

/// Generalizes the regression above: for every production pipeline, every
/// `@location(N)` input of `vs_main` must be supplied by exactly one
/// declared attribute whose format matches the WGSL type, and attribute
/// reads must stay inside their buffer stride without duplicate locations.
#[test]
fn every_contract_vertex_input_is_supplied_by_rust_layouts() {
    for contract in pipeline_contracts() {
        let module = contract_shader(&contract, naga::ShaderStage::Vertex);
        let vertex = entry_function(
            &module,
            naga::ShaderStage::Vertex,
            "vs_main",
            contract.label,
        );
        let inputs = input_slots(&module, vertex);
        let attributes = contract_attributes(&contract);

        let mut seen_locations: Vec<u32> =
            attributes.iter().map(|(location, ..)| *location).collect();
        seen_locations.sort_unstable();
        seen_locations.dedup();
        assert_eq!(
            seen_locations.len(),
            attributes.len(),
            "{}: duplicate shader_location across vertex buffers",
            contract.label
        );

        for slot in &inputs {
            let expected = required_vertex_format(slot);
            let found = attributes
                .iter()
                .find(|(location, ..)| *location == slot.location)
                .map(|(_, format, offset, stride)| (*format, *offset, *stride));
            let Some((format, offset, stride)) = found else {
                panic!(
                    "pipeline validation failed:\n  {}:\n    vertex shader requires @location({}) {:?}\n    layout does not provide it",
                    contract.label, slot.location, expected
                );
            };
            assert_eq!(
                format, expected,
                "{}: @location({}) is declared as {format:?} but the shader requires {expected:?}",
                contract.label, slot.location
            );
            // An attribute read must stay inside its own buffer record.
            let format_size = match format {
                wgpu::VertexFormat::Float32 => 4,
                wgpu::VertexFormat::Float32x2 => 8,
                wgpu::VertexFormat::Float32x3 => 12,
                wgpu::VertexFormat::Float32x4 => 16,
                wgpu::VertexFormat::Uint32 => 4,
                wgpu::VertexFormat::Uint32x2 => 8,
                wgpu::VertexFormat::Uint32x3 => 12,
                wgpu::VertexFormat::Uint32x4 => 16,
                wgpu::VertexFormat::Sint32 => 4,
                wgpu::VertexFormat::Sint32x2 => 8,
                wgpu::VertexFormat::Sint32x3 => 12,
                wgpu::VertexFormat::Sint32x4 => 16,
                other => panic!("{}: unhandled format {other:?}", contract.label),
            };
            assert!(
                offset + format_size <= stride,
                "{}: @location({}) reads [{offset}, {}) beyond stride {stride}",
                contract.label,
                slot.location,
                offset + format_size
            );
        }
    }
}

/// Prevents the fragment-side equivalent of the Task 13 regression: every
/// `@location(N)` the fragment stage reads must be written by the paired
/// vertex stage with a matching type and interpolation setup.
#[test]
fn vertex_outputs_satisfy_fragment_inputs_for_every_pipeline() {
    for contract in pipeline_contracts() {
        let module = validated_module(contract.shader, contract.label);
        let vertex = entry_function(
            &module,
            naga::ShaderStage::Vertex,
            PipelineContract::VERTEX_ENTRY,
            contract.label,
        );
        let fragment = entry_function(
            &module,
            naga::ShaderStage::Fragment,
            PipelineContract::FRAGMENT_ENTRY,
            contract.label,
        );
        let outputs = output_slots(&module, vertex);
        let mut outputs: Vec<InterfaceSlot> = outputs;
        outputs.sort_by_key(|slot| slot.location);

        for input in &input_slots(&module, fragment) {
            let output = outputs
                    .iter()
                    .find(|slot| slot.location == input.location)
                    .unwrap_or_else(|| {
                        panic!(
                            "pipeline validation failed:\n  {}:\n    fragment shader requires @location({})\n    vertex stage does not provide it",
                            contract.label, input.location
                        )
                    });
            assert_eq!(
                (output.kind, output.components, output.width),
                (input.kind, input.components, input.width),
                "{}: @location({}) type mismatch between vertex and fragment stages",
                contract.label,
                input.location
            );
            assert_eq!(
                (output.interpolation, output.sampling),
                (input.interpolation, input.sampling),
                "{}: @location({}) interpolation mismatch between stages",
                contract.label,
                input.location
            );
        }
    }
}

#[test]
fn path_pipeline_shares_one_shader_between_paint_and_clip_masks() {
    // Paint path and clip-mask pipelines intentionally reuse PATH_SHADER;
    // they must therefore also share one vertex layout declaration rather
    // than drifting into two formats.
    let contracts = pipeline_contracts();
    let paint = contracts
        .iter()
        .find(|contract| contract.class == PipelineClass::Path)
        .expect("path contract");
    let mask = contracts
        .iter()
        .find(|contract| {
            contract.class == PipelineClass::PathClipMask(ClipStencilDirection::Increment)
        })
        .expect("path clip mask contract");
    assert_eq!(paint.shader, mask.shader);
    assert_eq!(
        format!("{:?}", stream_layouts(paint.streams)),
        format!("{:?}", stream_layouts(mask.streams))
    );
}

/// Real GPU-backed smoke coverage: creates every production pipeline on a
/// headless adapter with no surface, no Winit window, and no compositor.
///
/// Bind-group layouts, blend states, depth/stencil requirements, and
/// target formats are all validated by `wgpu` itself here — that class of
/// error is deliberately not duplicated by the Naga tests. Each pipeline
/// is created inside a validation error scope, so failures are reported
/// with the pipeline label instead of an uncaptured-error panic.
///
/// If no headless adapter exists (some CI environments), this test prints
/// an explicit SKIPPED notice; absence of a GPU must never masquerade as
/// a passing run.
#[test]
fn headless_create_all_production_render_pipelines() {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let Ok(adapter) = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        ..Default::default()
    })) else {
        println!("SKIPPED: no compatible headless adapter");
        return;
    };
    let (device, _queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("incular headless pipeline validation"),
        ..Default::default()
    }))
    .expect("headless device creation");

    let layouts = SharedBindGroupLayoutsOwned::create(&device);
    let created = pollster::block_on(create_validated_contract_pipelines(
        &device,
        VALIDATION_FORMAT,
        &layouts.borrowed(),
    ));
    match created {
        Ok(pipelines) => {
            assert_eq!(pipelines.len(), 25, "every production pipeline created");
        }
        Err(RendererError::PipelineCreation { label, reason }) => {
            panic!("pipeline validation failed:\n  {label}:\n    {reason}");
        }
        Err(error) => panic!("pipeline creation failed: {error}"),
    }
}

/// The same headless context must reuse one set of pipelines per target
/// format across windows instead of duplicating device-level pipelines.
#[test]
fn shared_pipeline_resources_are_reused_per_format() {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let Ok(adapter) = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        ..Default::default()
    })) else {
        println!("SKIPPED: no compatible headless adapter");
        return;
    };
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .expect("headless device creation");

    let make_inner = || SharedGpuContextInner {
        instance: instance.clone(),
        adapter: adapter.clone(),
        device: device.clone(),
        queue: queue.clone(),
        device_generation: 1,
        pipelines: Mutex::new(HashMap::new()),
        resources: Mutex::new(SharedGpuResources {
            registry: SharedGpuResourceRegistry::default(),
            images: HashMap::new(),
            gradients: HashMap::new(),
            glyph_atlas: GlyphAtlas::new(),
            glyph_pages: Vec::new(),
        }),
        texture_upload_bytes: std::sync::atomic::AtomicU64::new(0),
    };
    // Two "windows" on one device context.
    let context_a = SharedGpuContext {
        inner: Arc::new(make_inner()),
    };
    let context_b = context_a.clone();

    let resources = pollster::block_on(create_shared_pipeline_resources(
        &device,
        &queue,
        VALIDATION_FORMAT,
    ))
    .expect("shared format pipelines");
    context_a.register_pipeline_resources(VALIDATION_FORMAT, resources);
    let from_window_a = context_a
        .pipeline_resources(VALIDATION_FORMAT)
        .expect("window A pipelines");
    let from_window_b = context_b
        .pipeline_resources(VALIDATION_FORMAT)
        .expect("pipelines registered by one window are visible to another");
    assert_eq!(Arc::as_ptr(&from_window_a), Arc::as_ptr(&from_window_b));
}
