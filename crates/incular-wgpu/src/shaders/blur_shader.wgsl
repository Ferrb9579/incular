
struct Params {
  source_origin: vec2<f32>,
  source_size: vec2<f32>,
  output_size: vec2<f32>,
  direction: vec2<f32>,
  radius: u32,
  mode: u32,
  _padding: vec2<u32>,
  weights: array<vec4<f32>, 16>,
};
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
fn sample_transparent(px: vec2<f32>) -> vec4<f32> {
  if (px.x < 0. || px.y < 0. || px.x >= params.source_size.x || px.y >= params.source_size.y) {
    return vec4<f32>(0.);
  }
  return textureSampleLevel(source, source_sampler, (px + vec2<f32>(0.5)) / params.source_size, 0.);
}
@fragment fn fs_main(input: Out) -> @location(0) vec4<f32> {
  let out_px = input.uv * params.output_size;
  var result = vec4<f32>(0.);
  for (var i: i32 = -48; i <= 48; i = i + 1) {
    if (abs(i) <= i32(params.radius)) {
      let px = out_px + params.direction * f32(i) - params.source_origin;
      let weight_index = abs(i);
      result += sample_transparent(px) * params.weights[weight_index / 4][weight_index % 4];
    }
  }
  return result;
}
