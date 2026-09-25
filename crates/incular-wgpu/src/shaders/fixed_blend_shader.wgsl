
struct Out { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32>, @location(1) alpha: f32 };
@group(0) @binding(0) var group_texture: texture_2d<f32>;
@group(0) @binding(1) var target_sampler: sampler;
@vertex fn vs_main(@location(0) quad: vec2<f32>, @location(1) rect: vec4<f32>, @location(2) uv: vec4<f32>, @location(3) alpha: vec4<f32>, @location(4) color: vec4<f32>, @location(5) options: vec4<f32>) -> Out {
  var out: Out;
  out.position = vec4<f32>(rect.xy + quad * rect.zw, 0., 1.);
  out.uv = uv.xy + quad * (uv.zw - uv.xy);
  out.alpha = alpha.x;
  return out;
}
@fragment fn fs_main(input: Out) -> @location(0) vec4<f32> {
  let sample = textureSample(group_texture, target_sampler, input.uv);
  return sample * input.alpha;
}
