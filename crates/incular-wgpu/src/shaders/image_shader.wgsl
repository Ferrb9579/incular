
struct Out { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32> };
@group(0) @binding(0) var image: texture_2d<f32>;
@group(0) @binding(1) var image_sampler: sampler;
@vertex fn vs_main(@location(0) quad: vec2<f32>, @location(1) rect: vec4<f32>, @location(2) affine: vec4<f32>, @location(3) translation: vec4<f32>, @location(4) surface: vec4<f32>, @location(5) uv: vec4<f32>) -> Out { var out: Out; let local=rect.xy+quad*rect.zw; let p=vec2<f32>(affine.x*local.x+affine.z*local.y+translation.x,affine.y*local.x+affine.w*local.y+translation.y); out.position=vec4<f32>(p.x/surface.x*2.-1.,1.-p.y/surface.y*2.,0.,1.); out.uv=uv.xy+quad*(uv.zw-uv.xy); return out; }
// The image texture decodes sRGB into linear sample values. Source pixels are
// straight alpha, and ALPHA_BLENDING is straight source-over blending.
@fragment fn fs_main(input: Out) -> @location(0) vec4<f32> { return textureSample(image, image_sampler, input.uv); }
