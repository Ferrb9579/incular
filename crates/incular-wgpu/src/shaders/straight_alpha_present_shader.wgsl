
struct Out { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32> };
@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var source_sampler: sampler;
@vertex fn vs_main(
  @location(0) quad: vec2<f32>,
  @location(1) rect: vec4<f32>,
  @location(2) uv: vec4<f32>,
  @location(3) alpha: vec4<f32>,
  @location(4) color: vec4<f32>,
  @location(5) options: vec4<f32>,
) -> Out {
  var out: Out;
  out.position = vec4<f32>(rect.xy + quad * rect.zw, 0., 1.);
  out.uv = uv.xy + quad * (uv.zw - uv.xy);
  return out;
}
@fragment fn fs_main(input: Out) -> @location(0) vec4<f32> {
  let sample = textureSampleLevel(source, source_sampler, input.uv, 0.);
  let alpha = clamp(sample.a, 0., 1.);
  let rgb = clamp(sample.rgb / max(alpha, .000001), vec3<f32>(0.), vec3<f32>(1.));
  return select(vec4<f32>(0.), vec4<f32>(rgb, alpha), alpha > .000001);
}
