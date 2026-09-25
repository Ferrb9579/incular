
struct Params { matrix: array<vec4<f32>, 5> };
struct Out { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32> };
@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var source_sampler: sampler;
@group(0) @binding(2) var<uniform> params: Params;
@vertex fn vs_main(@location(0) quad: vec2<f32>) -> Out {
  var out: Out;
  out.position = vec4<f32>(quad * 2. - 1., 0., 1.);
  out.uv = quad;
  return out;
}
@fragment fn fs_main(input: Out) -> @location(0) vec4<f32> {
  let sample = textureSampleLevel(source, source_sampler, input.uv, 0.);
  let a = clamp(sample.a, 0., 1.);
  let straight_rgb = clamp(sample.rgb / max(a, .000001), vec3<f32>(0.), vec3<f32>(1.));
  let straight = select(vec4<f32>(0., 0., 0., a), vec4<f32>(straight_rgb, a), a > .000001);
  let c0 = params.matrix[0];
  let c1 = params.matrix[1];
  let c2 = params.matrix[2];
  let c3 = params.matrix[3];
  let c4 = params.matrix[4];
  let filtered = vec4<f32>(
    dot(c0, straight) + c1.x,
    c1.y * straight.x + c1.z * straight.y + c1.w * straight.z + c2.x * straight.w + c2.y,
    c2.z * straight.x + c2.w * straight.y + c3.x * straight.z + c3.y * straight.w + c3.z,
    c3.w * straight.x + c4.x * straight.y + c4.y * straight.z + c4.z * straight.w + c4.w
  );
  let out_a = clamp(filtered.a, 0., 1.);
  let out_rgb = clamp(filtered.rgb, vec3<f32>(0.), vec3<f32>(1.)) * out_a;
  return vec4<f32>(out_rgb, out_a);
}
