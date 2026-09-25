//! Build-time parsed, backend-independent built-in shaders.
//!
//! The producer and consumer share Cargo's resolved Naga version. These bytes
//! are embedded in this binary, never loaded from application or external data.
//! WGPU still validates the decoded module and compiles it for the selected GPU.
//! The straight-alpha presentation shader unpremultiplies the retained scene
//! exactly once for native surfaces requiring postmultiplied RGB.

include!(concat!(env!("OUT_DIR"), "/built_in_shaders.rs"));

pub(crate) fn decode(bytes: &'static [u8]) -> wgpu::ShaderSource<'static> {
    let module = postcard::from_bytes(bytes).expect("build-validated embedded Naga module");
    wgpu::ShaderSource::Naga(std::borrow::Cow::Owned(module))
}
