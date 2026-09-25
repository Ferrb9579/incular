
struct Out { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32>, @location(1) color: vec4<f32> };
@group(0) @binding(0) var atlas: texture_2d<f32>;
@group(0) @binding(1) var atlas_sampler: sampler;
@vertex fn vs_main(@location(0) quad: vec2<f32>, @location(1) rect: vec4<f32>, @location(2) affine: vec4<f32>, @location(3) translation: vec4<f32>, @location(4) surface: vec4<f32>, @location(5) uv: vec4<f32>, @location(6) color: vec4<f32>) -> Out { var out: Out; let local=rect.xy+quad*rect.zw; let p=vec2<f32>(affine.x*local.x+affine.z*local.y+translation.x,affine.y*local.x+affine.w*local.y+translation.y); out.position=vec4<f32>(p.x/surface.x*2.-1.,1.-p.y/surface.y*2.,0.,1.); out.uv=uv.xy+quad*(uv.zw-uv.xy); out.color=color; return out; }
@fragment fn fs_main(input: Out) -> @location(0) vec4<f32> { let coverage = textureSample(atlas, atlas_sampler, input.uv).r; return vec4<f32>(input.color.rgb, input.color.a * coverage); }
