use super::prelude::*;
use super::resources::SharedGpuImage;
use super::*;
use crate::built_in_shaders::*;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct GpuInstance {
    pub(crate) rect: [f32; 4],
    pub(crate) color: [f32; 4],
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct GpuGlyphInstance {
    pub(crate) rect: [f32; 4],
    pub(crate) affine: [f32; 4],
    pub(crate) translation: [f32; 4],
    pub(crate) surface: [f32; 4],
    pub(crate) uv: [f32; 4],
    pub(crate) color: [f32; 4],
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct GpuImageInstance {
    pub(crate) rect: [f32; 4],
    pub(crate) affine: [f32; 4],
    pub(crate) translation: [f32; 4],
    pub(crate) surface: [f32; 4],
    pub(crate) uv: [f32; 4],
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct GpuRRectInstance {
    pub(crate) rect: [f32; 4],
    pub(crate) affine: [f32; 4],
    pub(crate) translation: [f32; 4],
    pub(crate) surface: [f32; 4],
    /// Radii in physical pixels, TL/TR/BR/BL.
    pub(crate) radii: [f32; 4],
    pub(crate) color_a: [f32; 4],
    pub(crate) color_b: [f32; 4],
    /// Gradient start/end in physical pixels, or radial center/radius.
    pub(crate) gradient: [f32; 4],
    /// x=kind (0 solid, 1 linear, 2 radial), y=inside border width.
    pub(crate) options: [f32; 4],
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct GpuPathInstance {
    /// Physical-space affine linear matrix `[a, b, c, d]`.
    pub(crate) affine: [f32; 4],
    /// Physical-space affine translation `[e, f]`.
    pub(crate) translation: [f32; 4],
    pub(crate) surface: [f32; 4],
    pub(crate) color: [f32; 4],
    pub(crate) gradient: [f32; 4],
    pub(crate) options: [f32; 4],
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct GpuCompositeInstance {
    pub(crate) rect: [f32; 4],
    pub(crate) uv: [f32; 4],
    pub(crate) alpha: [f32; 4],
    /// Straight shadow color; the shader converts it to premultiplied output
    /// using the sampled blurred alpha. Zero means ordinary offscreen draw.
    pub(crate) color: [f32; 4],
    pub(crate) options: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct GpuBlurParams {
    /// Source coordinate of the first output pixel, in input physical pixels.
    pub(crate) source_origin: [f32; 2],
    /// Input source dimensions in physical pixels.
    pub(crate) source_size: [f32; 2],
    /// Output dimensions in physical pixels.
    pub(crate) output_size: [f32; 2],
    /// For direct blur this is a unit axis; for resampling it is the input
    /// pixels traversed by one output pixel.
    pub(crate) direction: [f32; 2],
    pub(crate) radius: u32,
    pub(crate) mode: u32,
    pub(crate) _padding: [u32; 2],
    pub(crate) weights: [f32; BLUR_WEIGHT_SLOTS],
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct GpuColorMatrixParams {
    pub(crate) matrix: [f32; 20],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct BlurKernelKey {
    pub(crate) sigma_bits: u32,
    pub(crate) downsample_factor: u32,
}
#[derive(Clone, Debug)]
pub(crate) struct BlurKernel {
    pub(crate) radius: u32,
    pub(crate) weights: [f32; BLUR_WEIGHT_SLOTS],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum PathMeshKind {
    Fill(FillRule),
    Stroke {
        width: u32,
        cap: LineCap,
        join: LineJoin,
        miter_limit: u32,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct PathMeshKey {
    pub(crate) path: PathId,
    pub(crate) kind: PathMeshKind,
}
pub(crate) struct GpuPathMesh {
    pub(crate) vertices: wgpu::Buffer,
    pub(crate) indices: wgpu::Buffer,
    pub(crate) index_count: u32,
    pub(crate) last_used_frame: u64,
}
#[derive(Clone)]
pub(crate) struct GpuGradient {
    pub(crate) _texture: wgpu::Texture,
    pub(crate) bind_group: wgpu::BindGroup,
}

pub(crate) struct GpuAtlasPage {
    pub(crate) _texture: wgpu::Texture,
    pub(crate) bind_group: wgpu::BindGroup,
}
pub(crate) struct GpuImage {
    pub(crate) resource: Arc<SharedGpuImage>,
    pub(crate) bind_groups: HashMap<ImageSampling, wgpu::BindGroup>,
}
/// Resources that are immutable for a target format and can therefore be
/// cloned by every surface using that format. A clone is another handle to the
/// same `wgpu` object, not a duplicate GPU allocation.
#[derive(Clone)]
pub(crate) struct SharedPipelineResources {
    pub(crate) rectangle_pipeline: DeferredPipeline,
    pub(crate) text_pipeline: DeferredPipeline,
    pub(crate) image_pipeline: DeferredPipeline,
    pub(crate) rounded_rect_pipeline: DeferredPipeline,
    pub(crate) path_pipeline: DeferredPipeline,
    pub(crate) composite_pipeline: DeferredPipeline,
    pub(crate) straight_alpha_present_pipeline: DeferredPipeline,
    pub(crate) fixed_blend_pipelines: Vec<DeferredPipeline>,
    pub(crate) blur_pipeline: DeferredPipeline,
    pub(crate) resample_pipeline: DeferredPipeline,
    pub(crate) color_matrix_pipeline: DeferredPipeline,
    pub(crate) blend_pipeline: DeferredPipeline,
    pub(crate) stencil_rrect_increment_pipeline: DeferredPipeline,
    pub(crate) stencil_rrect_decrement_pipeline: DeferredPipeline,
    pub(crate) stencil_path_increment_pipeline: DeferredPipeline,
    pub(crate) stencil_path_decrement_pipeline: DeferredPipeline,
    pub(crate) mesh: wgpu::Buffer,
    pub(crate) gradient_bind_group_layout: wgpu::BindGroupLayout,
    pub(crate) gradient_sampler: wgpu::Sampler,
    pub(crate) solid_gradient: GpuGradient,
    pub(crate) atlas_bind_group_layout: wgpu::BindGroupLayout,
    pub(crate) atlas_sampler: wgpu::Sampler,
    pub(crate) image_bind_group_layout: wgpu::BindGroupLayout,
    pub(crate) image_samplers: HashMap<ImageSampling, wgpu::Sampler>,
    pub(crate) composite_bind_group_layout: wgpu::BindGroupLayout,
    pub(crate) composite_sampler: wgpu::Sampler,
    pub(crate) blur_bind_group_layout: wgpu::BindGroupLayout,
    pub(crate) blur_sampler: wgpu::Sampler,
    pub(crate) color_matrix_bind_group_layout: wgpu::BindGroupLayout,
    pub(crate) color_matrix_sampler: wgpu::Sampler,
    pub(crate) blend_bind_group_layout: wgpu::BindGroupLayout,
    pub(crate) blend_sampler: wgpu::Sampler,
}
impl SharedPipelineResources {
    pub(crate) fn created_count(&self) -> usize {
        [
            &self.rectangle_pipeline,
            &self.text_pipeline,
            &self.image_pipeline,
            &self.rounded_rect_pipeline,
            &self.path_pipeline,
            &self.composite_pipeline,
            &self.straight_alpha_present_pipeline,
            &self.blur_pipeline,
            &self.resample_pipeline,
            &self.color_matrix_pipeline,
            &self.blend_pipeline,
            &self.stencil_rrect_increment_pipeline,
            &self.stencil_rrect_decrement_pipeline,
            &self.stencil_path_increment_pipeline,
            &self.stencil_path_decrement_pipeline,
        ]
        .into_iter()
        .chain(self.fixed_blend_pipelines.iter())
        .filter(|pipeline| pipeline.is_created())
        .count()
    }
}
/// Direction of a clip mask's stencil write.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ClipStencilDirection {
    Increment,
    Decrement,
}
impl From<ClipStencilDirection> for wgpu::StencilOperation {
    fn from(direction: ClipStencilDirection) -> Self {
        match direction {
            ClipStencilDirection::Increment => Self::IncrementClamp,
            ClipStencilDirection::Decrement => Self::DecrementClamp,
        }
    }
}

/// Identifies exactly one production render pipeline class. This enum is the
/// registry of deferred descriptors the renderer retains per target format.
/// Each entry in [`pipeline_contracts`] supplies the complete GPU state used
/// when that pipeline first renders.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum PipelineClass {
    Rectangle,
    Text,
    Image,
    RoundedRect,
    /// Retained tessellated geometry: Kurbo paths filled/stroked through Lyon.
    Path,
    /// Offscreen group composite (opacity, drop shadows).
    Composite,
    /// Premultiplied retained scene -> straight-alpha native presentation.
    StraightAlphaPresent,
    /// Fixed-function Porter-Duff blending over offscreen groups.
    FixedBlend(BlendMode),
    /// Direct separable Gaussian blur pass.
    Blur,
    /// Multi-scale blur resampling pass (downsample/upsample).
    Resample,
    /// Color-matrix filter pass.
    ColorMatrix,
    /// Shader-based artistic blend modes reading source and destination.
    DestinationBlend,
    RoundedClipMask(ClipStencilDirection),
    PathClipMask(ClipStencilDirection),
}

/// Which shared bind-group layout the pipeline binds at `@group(0)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ResourceSet {
    None,
    GlyphAtlas,
    Images,
    Gradients,
    Composite,
    BlurUniforms,
    ColorMatrixUniforms,
    DestinationBlend,
}

/// Which vertex/instance streams feed the vertex stage. Every pipeline binds
/// the unit quad at slot zero; the variant names the additional
/// instance-stream family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum VertexStreams {
    EffectQuad,
    RectInstances,
    GlyphInstances,
    ImageInstances,
    RRectInstances,
    PathInstances,
    CompositeInstances,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ColorBlend {
    /// Straight-alpha source-over (`ALPHA_BLENDING`).
    StraightAlpha,
    /// Premultiplied-alpha source-over for offscreen groups.
    PremultipliedAlpha,
    /// No hardware blending; the fragment result replaces the target.
    Replace,
    /// Fixed-function Porter-Duff factors selected per mode.
    FixedPorterDuff(BlendMode),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StencilRequirement {
    /// Draw only where the clip stencil equals the current clip depth.
    ContentEqualKeep,
    Mask(ClipStencilDirection),
    Disabled,
}

/// The complete declarative description of one production render pipeline.
///
/// Production creation ([`create_shared_pipeline_resources`]) consumes the
/// list produced by [`pipeline_contracts`], the source of truth for shader
/// entry points, vertex/instance layouts, bindings, blending, and stencil
/// state.
pub(crate) struct PipelineContract {
    pub(crate) class: PipelineClass,
    pub(crate) label: &'static str,
    pub(crate) shader_module_label: &'static str,
    pub(crate) shader: &'static [u8],
    pub(crate) resources: ResourceSet,
    pub(crate) streams: VertexStreams,
    pub(crate) blend: ColorBlend,
    pub(crate) stencil: StencilRequirement,
    pub(crate) color_writes: wgpu::ColorWrites,
}
impl PipelineContract {
    pub(crate) const VERTEX_ENTRY: &'static str = "vs_main";
    pub(crate) const FRAGMENT_ENTRY: &'static str = "fs_main";
}

/// Porter-Duff modes implemented by fixed-function blend pipelines; artistic
/// modes route through `BLEND_SHADER` instead.
pub(crate) const PORTER_DUFF_BLEND_MODES: [BlendMode; 11] = [
    BlendMode::SrcOver,
    BlendMode::Src,
    BlendMode::DstOver,
    BlendMode::SrcIn,
    BlendMode::DstIn,
    BlendMode::SrcOut,
    BlendMode::DstOut,
    BlendMode::SrcAtop,
    BlendMode::DstAtop,
    BlendMode::Xor,
    BlendMode::Plus,
];

pub(crate) const CLIP_STENCIL_DIRECTIONS: [ClipStencilDirection; 2] = [
    ClipStencilDirection::Increment,
    ClipStencilDirection::Decrement,
];

/// Every render pipeline the retained renderer creates for one target format.
/// Entries describe deferred pipelines. The first draw using an entry compiles
/// and validates it; unused effects do not allocate driver pipeline resources.
pub(crate) fn pipeline_contracts() -> Vec<PipelineContract> {
    let mut contracts = vec![
        PipelineContract {
            class: PipelineClass::Rectangle,
            label: "incular rectangle pipeline",
            shader_module_label: "incular rectangle pipeline",
            shader: RECT_SHADER,
            resources: ResourceSet::None,
            streams: VertexStreams::RectInstances,
            blend: ColorBlend::StraightAlpha,
            stencil: StencilRequirement::ContentEqualKeep,
            color_writes: wgpu::ColorWrites::ALL,
        },
        PipelineContract {
            class: PipelineClass::Text,
            label: "incular text pipeline",
            shader_module_label: "incular text pipeline",
            shader: TEXT_SHADER,
            resources: ResourceSet::GlyphAtlas,
            streams: VertexStreams::GlyphInstances,
            blend: ColorBlend::StraightAlpha,
            stencil: StencilRequirement::ContentEqualKeep,
            color_writes: wgpu::ColorWrites::ALL,
        },
        PipelineContract {
            class: PipelineClass::Image,
            label: "incular image pipeline",
            shader_module_label: "incular image pipeline",
            shader: IMAGE_SHADER,
            resources: ResourceSet::Images,
            streams: VertexStreams::ImageInstances,
            blend: ColorBlend::StraightAlpha,
            stencil: StencilRequirement::ContentEqualKeep,
            color_writes: wgpu::ColorWrites::ALL,
        },
        PipelineContract {
            class: PipelineClass::RoundedRect,
            label: "incular analytic rounded rectangle pipeline",
            shader_module_label: "incular analytic rounded rectangle pipeline",
            shader: RRECT_SHADER,
            resources: ResourceSet::Gradients,
            streams: VertexStreams::RRectInstances,
            blend: ColorBlend::StraightAlpha,
            stencil: StencilRequirement::ContentEqualKeep,
            color_writes: wgpu::ColorWrites::ALL,
        },
        PipelineContract {
            class: PipelineClass::Path,
            label: "incular retained path pipeline",
            shader_module_label: "incular retained path shader",
            shader: PATH_SHADER,
            resources: ResourceSet::Gradients,
            streams: VertexStreams::PathInstances,
            blend: ColorBlend::StraightAlpha,
            stencil: StencilRequirement::ContentEqualKeep,
            color_writes: wgpu::ColorWrites::ALL,
        },
        PipelineContract {
            class: PipelineClass::Composite,
            label: "incular opacity composite pipeline",
            shader_module_label: "incular opacity composite shader",
            shader: COMPOSITE_SHADER,
            resources: ResourceSet::Composite,
            streams: VertexStreams::CompositeInstances,
            blend: ColorBlend::PremultipliedAlpha,
            stencil: StencilRequirement::ContentEqualKeep,
            color_writes: wgpu::ColorWrites::ALL,
        },
        PipelineContract {
            class: PipelineClass::StraightAlphaPresent,
            label: "incular straight-alpha presentation pipeline",
            shader_module_label: "incular straight-alpha presentation shader",
            shader: STRAIGHT_ALPHA_PRESENT_SHADER,
            resources: ResourceSet::Composite,
            streams: VertexStreams::CompositeInstances,
            blend: ColorBlend::Replace,
            stencil: StencilRequirement::Disabled,
            color_writes: wgpu::ColorWrites::ALL,
        },
        PipelineContract {
            class: PipelineClass::Blur,
            label: "incular separable gaussian blur pipeline",
            shader_module_label: "incular separable gaussian blur pipeline",
            shader: BLUR_SHADER,
            resources: ResourceSet::BlurUniforms,
            streams: VertexStreams::EffectQuad,
            blend: ColorBlend::Replace,
            stencil: StencilRequirement::Disabled,
            color_writes: wgpu::ColorWrites::ALL,
        },
        PipelineContract {
            class: PipelineClass::Resample,
            label: "incular effect resample pipeline",
            shader_module_label: "incular effect resample pipeline",
            shader: RESAMPLE_SHADER,
            resources: ResourceSet::BlurUniforms,
            streams: VertexStreams::EffectQuad,
            blend: ColorBlend::Replace,
            stencil: StencilRequirement::Disabled,
            color_writes: wgpu::ColorWrites::ALL,
        },
        PipelineContract {
            class: PipelineClass::ColorMatrix,
            label: "incular color matrix pipeline",
            shader_module_label: "incular color matrix pipeline",
            shader: COLOR_MATRIX_SHADER,
            resources: ResourceSet::ColorMatrixUniforms,
            streams: VertexStreams::EffectQuad,
            blend: ColorBlend::Replace,
            stencil: StencilRequirement::Disabled,
            color_writes: wgpu::ColorWrites::ALL,
        },
        PipelineContract {
            class: PipelineClass::DestinationBlend,
            label: "incular destination blend pipeline",
            shader_module_label: "incular destination blend shader",
            shader: BLEND_SHADER,
            resources: ResourceSet::DestinationBlend,
            streams: VertexStreams::CompositeInstances,
            blend: ColorBlend::Replace,
            stencil: StencilRequirement::ContentEqualKeep,
            color_writes: wgpu::ColorWrites::ALL,
        },
    ];
    contracts.extend(PORTER_DUFF_BLEND_MODES.map(|mode| PipelineContract {
        class: PipelineClass::FixedBlend(mode),
        label: "incular fixed-function blend pipeline",
        shader_module_label: "incular fixed-function blend shader",
        shader: FIXED_BLEND_SHADER,
        resources: ResourceSet::Composite,
        streams: VertexStreams::CompositeInstances,
        blend: ColorBlend::FixedPorterDuff(mode),
        stencil: StencilRequirement::ContentEqualKeep,
        color_writes: wgpu::ColorWrites::ALL,
    }));
    contracts.extend(CLIP_STENCIL_DIRECTIONS.map(|direction| PipelineContract {
        class: PipelineClass::RoundedClipMask(direction),
        label: match direction {
            ClipStencilDirection::Increment => "incular rounded clip mask increment",
            ClipStencilDirection::Decrement => "incular rounded clip mask decrement",
        },
        shader_module_label: "incular rounded clip mask",
        shader: RRECT_SHADER,
        resources: ResourceSet::Gradients,
        streams: VertexStreams::RRectInstances,
        blend: ColorBlend::StraightAlpha,
        stencil: StencilRequirement::Mask(direction),
        color_writes: wgpu::ColorWrites::empty(),
    }));
    contracts.extend(CLIP_STENCIL_DIRECTIONS.map(|direction| PipelineContract {
        class: PipelineClass::PathClipMask(direction),
        label: match direction {
            ClipStencilDirection::Increment => "incular path clip mask pipeline increment",
            ClipStencilDirection::Decrement => "incular path clip mask pipeline decrement",
        },
        shader_module_label: "incular path clip mask shader",
        shader: PATH_SHADER,
        resources: ResourceSet::Gradients,
        streams: VertexStreams::PathInstances,
        blend: ColorBlend::Replace,
        stencil: StencilRequirement::Mask(direction),
        color_writes: wgpu::ColorWrites::empty(),
    }));
    contracts
}

/// Bind-group layouts every pipeline variant can reference, created once per
/// device and reused across target formats and windows.
pub(crate) struct SharedBindGroupLayouts<'a> {
    pub(crate) atlas: &'a wgpu::BindGroupLayout,
    pub(crate) images: &'a wgpu::BindGroupLayout,
    pub(crate) gradients: &'a wgpu::BindGroupLayout,
    pub(crate) composite: &'a wgpu::BindGroupLayout,
    pub(crate) blur: &'a wgpu::BindGroupLayout,
    pub(crate) color_matrix: &'a wgpu::BindGroupLayout,
    pub(crate) destination_blend: &'a wgpu::BindGroupLayout,
}

/// Owned bind-group layouts created once per device. Extracted from renderer
/// initialization so headless validation tests construct identical GPU
/// resources without needing a window surface.
#[derive(Clone)]
pub(crate) struct SharedBindGroupLayoutsOwned {
    pub(crate) atlas: wgpu::BindGroupLayout,
    pub(crate) images: wgpu::BindGroupLayout,
    pub(crate) gradients: wgpu::BindGroupLayout,
    pub(crate) composite: wgpu::BindGroupLayout,
    pub(crate) blur: wgpu::BindGroupLayout,
    pub(crate) color_matrix: wgpu::BindGroupLayout,
    pub(crate) destination_blend: wgpu::BindGroupLayout,
}
impl SharedBindGroupLayoutsOwned {
    pub(crate) fn create(device: &wgpu::Device) -> Self {
        let texture_float_entry = |binding: u32| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let sampler_entry = |binding: u32| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        };
        let uniform_entry = |binding: u32| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let create = |label: &'static str, entries: &[wgpu::BindGroupLayoutEntry]| {
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some(label),
                entries,
            })
        };
        Self {
            atlas: create(
                "incular glyph atlas layout",
                &[texture_float_entry(0), sampler_entry(1)],
            ),
            images: create(
                "incular image layout",
                &[texture_float_entry(0), sampler_entry(1)],
            ),
            gradients: create(
                "incular gradient lookup layout",
                &[texture_float_entry(0), sampler_entry(1)],
            ),
            composite: create(
                "incular opacity composite layout",
                &[texture_float_entry(0), sampler_entry(1)],
            ),
            blur: create(
                "incular gaussian effect layout",
                &[texture_float_entry(0), sampler_entry(1), uniform_entry(2)],
            ),
            color_matrix: create(
                "incular color matrix layout",
                &[texture_float_entry(0), sampler_entry(1), uniform_entry(2)],
            ),
            destination_blend: create(
                "incular destination blend layout",
                &[
                    texture_float_entry(0),
                    texture_float_entry(1),
                    sampler_entry(2),
                ],
            ),
        }
    }
    pub(crate) fn borrowed(&self) -> SharedBindGroupLayouts<'_> {
        SharedBindGroupLayouts {
            atlas: &self.atlas,
            images: &self.images,
            gradients: &self.gradients,
            composite: &self.composite,
            blur: &self.blur,
            color_matrix: &self.color_matrix,
            destination_blend: &self.destination_blend,
        }
    }
}

pub(crate) fn stream_layouts(
    streams: VertexStreams,
) -> Vec<Option<wgpu::VertexBufferLayout<'static>>> {
    match streams {
        VertexStreams::EffectQuad => vec![Some(quad_layout())],
        VertexStreams::RectInstances => {
            vec![Some(quad_layout()), Some(rectangle_layout())]
        }
        VertexStreams::GlyphInstances => vec![Some(quad_layout()), Some(glyph_layout())],
        VertexStreams::ImageInstances => vec![Some(quad_layout()), Some(image_layout())],
        VertexStreams::RRectInstances => {
            vec![Some(quad_layout()), Some(rrect_layout())]
        }
        VertexStreams::PathInstances => vec![Some(quad_layout()), Some(path_instance_layout())],
        VertexStreams::CompositeInstances => {
            vec![Some(quad_layout()), Some(composite_layout())]
        }
    }
}

pub(crate) fn porter_duff_factors(mode: BlendMode) -> (wgpu::BlendComponent, wgpu::BlendComponent) {
    let component =
        |source: wgpu::BlendFactor, destination: wgpu::BlendFactor| wgpu::BlendComponent {
            src_factor: source,
            dst_factor: destination,
            operation: wgpu::BlendOperation::Add,
        };
    let factors = |source, destination| {
        (
            component(source, destination),
            component(source, destination),
        )
    };
    match mode {
        BlendMode::SrcOver => factors(wgpu::BlendFactor::One, wgpu::BlendFactor::OneMinusSrcAlpha),
        BlendMode::Src => factors(wgpu::BlendFactor::One, wgpu::BlendFactor::Zero),
        BlendMode::DstOver => factors(wgpu::BlendFactor::OneMinusDstAlpha, wgpu::BlendFactor::One),
        BlendMode::SrcIn => factors(wgpu::BlendFactor::DstAlpha, wgpu::BlendFactor::Zero),
        BlendMode::DstIn => factors(wgpu::BlendFactor::Zero, wgpu::BlendFactor::SrcAlpha),
        BlendMode::SrcOut => factors(wgpu::BlendFactor::OneMinusDstAlpha, wgpu::BlendFactor::Zero),
        BlendMode::DstOut => factors(wgpu::BlendFactor::Zero, wgpu::BlendFactor::OneMinusSrcAlpha),
        BlendMode::SrcAtop => factors(
            wgpu::BlendFactor::DstAlpha,
            wgpu::BlendFactor::OneMinusSrcAlpha,
        ),
        BlendMode::DstAtop => factors(
            wgpu::BlendFactor::OneMinusDstAlpha,
            wgpu::BlendFactor::SrcAlpha,
        ),
        BlendMode::Xor => factors(
            wgpu::BlendFactor::OneMinusDstAlpha,
            wgpu::BlendFactor::OneMinusSrcAlpha,
        ),
        BlendMode::Plus => factors(wgpu::BlendFactor::One, wgpu::BlendFactor::One),
        _ => unreachable!("fixed blend pipelines only support Porter-Duff modes"),
    }
}

pub(crate) fn color_target_state(
    format: wgpu::TextureFormat,
    blend: ColorBlend,
    write_mask: wgpu::ColorWrites,
) -> wgpu::ColorTargetState {
    let hardware_blend = match blend {
        ColorBlend::StraightAlpha => Some(wgpu::BlendState::ALPHA_BLENDING),
        ColorBlend::PremultipliedAlpha => Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
        ColorBlend::Replace => None,
        ColorBlend::FixedPorterDuff(mode) => {
            let (color, alpha) = porter_duff_factors(mode);
            Some(wgpu::BlendState { color, alpha })
        }
    };
    wgpu::ColorTargetState {
        format,
        blend: hardware_blend,
        write_mask,
    }
}

pub(crate) fn depth_stencil_state(stencil: StencilRequirement) -> Option<wgpu::DepthStencilState> {
    match stencil {
        StencilRequirement::ContentEqualKeep => content_stencil(),
        StencilRequirement::Mask(direction) => Some(stencil_state(direction.into())),
        StencilRequirement::Disabled => None,
    }
}

/// Builds one production pipeline from its contract. Pure descriptor
/// assembly; error scopes are applied by the caller.
pub(crate) fn create_contract_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    contract: &PipelineContract,
    layouts: &SharedBindGroupLayouts<'_>,
) -> wgpu::RenderPipeline {
    let bind_group = match contract.resources {
        ResourceSet::None => None,
        ResourceSet::GlyphAtlas => Some(layouts.atlas),
        ResourceSet::Images => Some(layouts.images),
        ResourceSet::Gradients => Some(layouts.gradients),
        ResourceSet::Composite => Some(layouts.composite),
        ResourceSet::BlurUniforms => Some(layouts.blur),
        ResourceSet::ColorMatrixUniforms => Some(layouts.color_matrix),
        ResourceSet::DestinationBlend => Some(layouts.destination_blend),
    };
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(contract.label),
        bind_group_layouts: &[bind_group],
        immediate_size: 0,
    });
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(contract.shader_module_label),
        source: crate::built_in_shaders::decode(contract.shader),
    });
    let buffers = stream_layouts(contract.streams);
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(contract.label),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some(PipelineContract::VERTEX_ENTRY),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &buffers,
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: depth_stencil_state(contract.stencil),
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some(PipelineContract::FRAGMENT_ENTRY),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(color_target_state(
                format,
                contract.blend,
                contract.color_writes,
            ))],
        }),
        multiview_mask: None,
        cache: None,
    })
}

/// Creates every device-level render resource for one target format: shared
/// bind-group layouts, samplers, the unit quad mesh, the solid-gradient LUT,
/// and deferred production pipeline descriptors. Shader modules and pipelines
/// are compiled on first use and shared by all windows using this format.
pub(crate) async fn create_shared_pipeline_resources(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    format: wgpu::TextureFormat,
) -> Result<SharedPipelineResources, RendererError> {
    let mesh = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("incular unit quad mesh"),
        contents: bytemuck::cast_slice(&[
            [0_f32, 0_f32],
            [1., 0.],
            [0., 1.],
            [0., 1.],
            [1., 0.],
            [1., 1.],
        ]),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let owned_bind_group_layouts = SharedBindGroupLayoutsOwned::create(device);
    let SharedBindGroupLayoutsOwned {
        atlas: atlas_bind_group_layout,
        images: image_bind_group_layout,
        gradients: gradient_bind_group_layout,
        composite: composite_bind_group_layout,
        blur: blur_bind_group_layout,
        color_matrix: color_matrix_bind_group_layout,
        destination_blend: blend_bind_group_layout,
    } = owned_bind_group_layouts.clone();
    let atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("incular glyph atlas sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        // Coverage masks can be positioned at fractional physical pixels.
        // Bilinear filtering preserves grayscale antialiasing; each atlas
        // allocation has a zero-coverage border to prevent glyph bleed.
        mag_filter: GLYPH_ATLAS_FILTER,
        min_filter: GLYPH_ATLAS_FILTER,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        ..Default::default()
    });
    let composite_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("incular opacity composite sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        ..Default::default()
    });
    let blur_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("incular gaussian linear sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        ..Default::default()
    });
    let color_matrix_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("incular color matrix sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        ..Default::default()
    });
    let blend_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("incular destination blend sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        ..Default::default()
    });
    let gradient_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("incular gradient lookup sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    let solid_gradient = create_gradient_resource(
        device,
        queue,
        &gradient_bind_group_layout,
        &gradient_sampler,
        &[[255, 255, 255, 255]; 1],
    );
    let mut image_samplers = HashMap::new();
    for (sampling, filter) in [
        (ImageSampling::Linear, wgpu::FilterMode::Linear),
        (ImageSampling::Nearest, wgpu::FilterMode::Nearest),
    ] {
        image_samplers.insert(
            sampling,
            device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("incular retained image sampler"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: filter,
                min_filter: filter,
                mipmap_filter: wgpu::MipmapFilterMode::Nearest,
                ..Default::default()
            }),
        );
    }
    let layouts = Arc::new(owned_bind_group_layouts);
    let mut created_pipelines: HashMap<_, _> = pipeline_contracts()
        .into_iter()
        .map(|contract| {
            let class = contract.class;
            let label = contract.label;
            let pipeline_device = device.clone();
            let layouts = layouts.clone();
            let pipeline = DeferredPipeline::new(device, label, move || {
                create_contract_pipeline(&pipeline_device, format, &contract, &layouts.borrowed())
            });
            (class, pipeline)
        })
        .collect();
    let mut take_pipeline = |class: PipelineClass| -> DeferredPipeline {
        created_pipelines
            .remove(&class)
            .expect("every pipeline class has a deferred contract")
    };
    let rectangle_pipeline = take_pipeline(PipelineClass::Rectangle);
    let text_pipeline = take_pipeline(PipelineClass::Text);
    let image_pipeline = take_pipeline(PipelineClass::Image);
    let rounded_rect_pipeline = take_pipeline(PipelineClass::RoundedRect);
    let path_pipeline = take_pipeline(PipelineClass::Path);
    let composite_pipeline = take_pipeline(PipelineClass::Composite);
    let straight_alpha_present_pipeline = take_pipeline(PipelineClass::StraightAlphaPresent);
    let fixed_blend_pipelines: Vec<_> = PORTER_DUFF_BLEND_MODES
        .map(|mode| take_pipeline(PipelineClass::FixedBlend(mode)))
        .into_iter()
        .collect();
    let blur_pipeline = take_pipeline(PipelineClass::Blur);
    let resample_pipeline = take_pipeline(PipelineClass::Resample);
    let color_matrix_pipeline = take_pipeline(PipelineClass::ColorMatrix);
    let blend_pipeline = take_pipeline(PipelineClass::DestinationBlend);
    let stencil_rrect_increment_pipeline = take_pipeline(PipelineClass::RoundedClipMask(
        ClipStencilDirection::Increment,
    ));
    let stencil_rrect_decrement_pipeline = take_pipeline(PipelineClass::RoundedClipMask(
        ClipStencilDirection::Decrement,
    ));
    let stencil_path_increment_pipeline =
        take_pipeline(PipelineClass::PathClipMask(ClipStencilDirection::Increment));
    let stencil_path_decrement_pipeline =
        take_pipeline(PipelineClass::PathClipMask(ClipStencilDirection::Decrement));
    Ok(SharedPipelineResources {
        rectangle_pipeline,
        text_pipeline,
        image_pipeline,
        rounded_rect_pipeline,
        path_pipeline,
        composite_pipeline,
        straight_alpha_present_pipeline,
        fixed_blend_pipelines,
        blur_pipeline,
        resample_pipeline,
        color_matrix_pipeline,
        blend_pipeline,
        stencil_rrect_increment_pipeline,
        stencil_rrect_decrement_pipeline,
        stencil_path_increment_pipeline,
        stencil_path_decrement_pipeline,
        mesh,
        gradient_bind_group_layout,
        gradient_sampler,
        solid_gradient,
        atlas_bind_group_layout,
        atlas_sampler,
        image_bind_group_layout,
        image_samplers,
        composite_bind_group_layout,
        composite_sampler,
        blur_bind_group_layout,
        blur_sampler,
        color_matrix_bind_group_layout,
        color_matrix_sampler,
        blend_bind_group_layout,
        blend_sampler,
    })
}

pub(crate) fn stencil_state(operation: wgpu::StencilOperation) -> wgpu::DepthStencilState {
    wgpu::DepthStencilState {
        format: wgpu::TextureFormat::Depth24PlusStencil8,
        depth_write_enabled: Some(false),
        depth_compare: Some(wgpu::CompareFunction::Always),
        stencil: wgpu::StencilState {
            front: wgpu::StencilFaceState {
                compare: wgpu::CompareFunction::Equal,
                fail_op: wgpu::StencilOperation::Keep,
                depth_fail_op: wgpu::StencilOperation::Keep,
                pass_op: operation,
            },
            back: wgpu::StencilFaceState {
                compare: wgpu::CompareFunction::Equal,
                fail_op: wgpu::StencilOperation::Keep,
                depth_fail_op: wgpu::StencilOperation::Keep,
                pass_op: operation,
            },
            read_mask: u32::from(u8::MAX),
            write_mask: u32::from(u8::MAX),
        },
        bias: wgpu::DepthBiasState::default(),
    }
}
pub(crate) fn content_stencil() -> Option<wgpu::DepthStencilState> {
    Some(stencil_state(wgpu::StencilOperation::Keep))
}
pub(crate) fn create_stencil_attachment(
    device: &wgpu::Device,
    width: u32,
    height: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("incular retained stencil attachment"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth24PlusStencil8,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}
/// One `vec4<f32>` attribute read from field `$field` of the actual
/// `#[repr(C)]` instance struct at shader location `$location`. Deriving the
/// byte offset from the struct keeps WGSL locations, `bytemuck` uploads, and
/// vertex-buffer strides in lockstep; hand-written offsets have already drifted
/// once (Task 13 sweep-gradient `options`), so they are no longer used.
macro_rules! f32x4_attr {
    ($instance:ty, $field:ident, $location:expr) => {
        wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x4,
            offset: ::std::mem::offset_of!($instance, $field) as u64,
            shader_location: $location,
        }
    };
}
pub(crate) fn quad_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<[f32; 2]>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x2,
            offset: 0,
            shader_location: 0,
        }],
    }
}
pub(crate) fn rectangle_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<GpuInstance>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &[
            f32x4_attr!(GpuInstance, rect, 1),
            f32x4_attr!(GpuInstance, color, 2),
        ],
    }
}
pub(crate) fn glyph_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<GpuGlyphInstance>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &[
            f32x4_attr!(GpuGlyphInstance, rect, 1),
            f32x4_attr!(GpuGlyphInstance, affine, 2),
            f32x4_attr!(GpuGlyphInstance, translation, 3),
            f32x4_attr!(GpuGlyphInstance, surface, 4),
            f32x4_attr!(GpuGlyphInstance, uv, 5),
            f32x4_attr!(GpuGlyphInstance, color, 6),
        ],
    }
}
pub(crate) fn image_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<GpuImageInstance>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &[
            f32x4_attr!(GpuImageInstance, rect, 1),
            f32x4_attr!(GpuImageInstance, affine, 2),
            f32x4_attr!(GpuImageInstance, translation, 3),
            f32x4_attr!(GpuImageInstance, surface, 4),
            f32x4_attr!(GpuImageInstance, uv, 5),
        ],
    }
}
pub(crate) fn rrect_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<GpuRRectInstance>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &[
            f32x4_attr!(GpuRRectInstance, rect, 1),
            f32x4_attr!(GpuRRectInstance, affine, 2),
            f32x4_attr!(GpuRRectInstance, translation, 3),
            f32x4_attr!(GpuRRectInstance, surface, 4),
            f32x4_attr!(GpuRRectInstance, radii, 5),
            f32x4_attr!(GpuRRectInstance, color_a, 6),
            f32x4_attr!(GpuRRectInstance, color_b, 7),
            f32x4_attr!(GpuRRectInstance, gradient, 8),
            f32x4_attr!(GpuRRectInstance, options, 9),
        ],
    }
}
/// Instance streams for [`PATH_SHADER`] must supply every brush parameter the
/// vertex stage forwards to the fragment stage: `options.x` selects solid /
/// linear / radial / sweep lookup, so omitting it breaks validation even when
/// only solid fills are drawn.
pub(crate) fn path_instance_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<GpuPathInstance>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &[
            f32x4_attr!(GpuPathInstance, affine, 1),
            f32x4_attr!(GpuPathInstance, translation, 2),
            f32x4_attr!(GpuPathInstance, surface, 3),
            f32x4_attr!(GpuPathInstance, color, 4),
            f32x4_attr!(GpuPathInstance, gradient, 5),
            f32x4_attr!(GpuPathInstance, options, 6),
        ],
    }
}
pub(crate) fn composite_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<GpuCompositeInstance>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &[
            f32x4_attr!(GpuCompositeInstance, rect, 1),
            f32x4_attr!(GpuCompositeInstance, uv, 2),
            f32x4_attr!(GpuCompositeInstance, alpha, 3),
            f32x4_attr!(GpuCompositeInstance, color, 4),
            f32x4_attr!(GpuCompositeInstance, options, 5),
        ],
    }
}
pub(crate) fn create_instance_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("incular rectangle instances"),
        size: (capacity * std::mem::size_of::<GpuInstance>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}
pub(crate) fn create_glyph_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("incular glyph instances"),
        size: (capacity * std::mem::size_of::<GpuGlyphInstance>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}
pub(crate) fn create_image_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("incular image instances"),
        size: (capacity * std::mem::size_of::<GpuImageInstance>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}
pub(crate) fn create_rrect_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("incular rounded rectangle instances"),
        size: (capacity * std::mem::size_of::<GpuRRectInstance>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}
pub(crate) fn create_path_instance_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("incular path paint instances"),
        size: (capacity * std::mem::size_of::<GpuPathInstance>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}
pub(crate) fn create_composite_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("incular opacity composite instances"),
        size: (capacity * std::mem::size_of::<GpuCompositeInstance>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}
pub(crate) fn create_gradient_resource(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    pixels: &[[u8; 4]],
) -> GpuGradient {
    let width = pixels.len().max(1) as u32;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("incular retained gradient lookup"),
        size: wgpu::Extent3d {
            width,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        bytemuck::cast_slice(pixels),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * 4),
            rows_per_image: Some(1),
        },
        wgpu::Extent3d {
            width,
            height: 1,
            depth_or_array_layers: 1,
        },
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("incular retained gradient bind group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    });
    GpuGradient {
        _texture: texture,
        bind_group,
    }
}
pub(crate) fn gradient_lut_pixels(stops: &incular_painting::GradientStops) -> Vec<[u8; 4]> {
    (0..GRADIENT_LUT_SAMPLES)
        .map(|index| {
            let t = index as f32 / (GRADIENT_LUT_SAMPLES - 1) as f32;
            sample_gradient_stops(stops, t)
                .map(|channel| (channel.clamp(0., 1.) * 255.).round() as u8)
        })
        .collect()
}
