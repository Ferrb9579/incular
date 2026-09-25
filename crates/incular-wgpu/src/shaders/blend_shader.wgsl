
struct Out { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32>, @location(1) alpha: f32, @location(2) color: vec4<f32>, @location(3) options: vec4<f32> };
@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var destination: texture_2d<f32>;
@group(0) @binding(2) var blend_sampler: sampler;
@vertex fn vs_main(@location(0) quad: vec2<f32>, @location(1) rect: vec4<f32>, @location(2) uv: vec4<f32>, @location(3) alpha: vec4<f32>, @location(4) color: vec4<f32>, @location(5) options: vec4<f32>) -> Out {
  var out: Out;
  out.position = vec4<f32>(rect.xy + quad * rect.zw, 0., 1.);
  out.uv = uv.xy + quad * (uv.zw - uv.xy);
  out.alpha = alpha.x;
  out.color = color;
  out.options = options;
  return out;
}
fn straight_rgb(value: vec4<f32>) -> vec3<f32> {
  let alpha = clamp(value.a, 0., 1.);
  let rgb = clamp(value.rgb / max(alpha, .000001), vec3<f32>(0.), vec3<f32>(1.));
  return select(vec3<f32>(0.), rgb, alpha > .000001);
}
fn artistic(mode: u32, s: vec3<f32>, d: vec3<f32>) -> vec3<f32> {
  var out = s;
  if (mode == 11u) { out = s * d; }
  else if (mode == 12u) { out = s + d - s * d; }
  else if (mode == 13u) { out = select(2. * s * d, 1. - 2. * (1. - s) * (1. - d), d > vec3<f32>(.5)); }
  else if (mode == 14u) { out = min(s, d); }
  else if (mode == 15u) { out = max(s, d); }
  else if (mode == 16u) { out = select(min(d / max(1. - s, vec3<f32>(.000001)), vec3<f32>(1.)), vec3<f32>(1.), s >= vec3<f32>(1.)); }
  else if (mode == 17u) { out = select(max(1. - (1. - d) / max(s, vec3<f32>(.000001)), vec3<f32>(0.)), vec3<f32>(0.), s <= vec3<f32>(0.)); }
  else if (mode == 18u) { out = select(2. * s * d, 1. - 2. * (1. - s) * (1. - d), s > vec3<f32>(.5)); }
  else if (mode == 19u) {
    let low = d - (1. - 2. * s) * d * (1. - d);
    let g = select(sqrt(d), ((16. * d - 12.) * d + 4.) * d, d <= vec3<f32>(.25));
    out = select(low, d + (2. * s - 1.) * (g - d), s > vec3<f32>(.5));
  }
  else if (mode == 20u) { out = abs(d - s); }
  else if (mode == 21u) { out = s + d - 2. * s * d; }
  return clamp(out, vec3<f32>(0.), vec3<f32>(1.));
}
@fragment fn fs_main(input: Out) -> @location(0) vec4<f32> {
  let src_sample = textureSampleLevel(source, blend_sampler, input.uv, 0.) * input.alpha;
  let dst_uv = input.position.xy / max(input.options.yz, vec2<f32>(1.));
  let dst_sample = textureSampleLevel(destination, blend_sampler, dst_uv, 0.);
  let mode = u32(input.options.x + .5);
  let as_ = clamp(src_sample.a, 0., 1.);
  let ad = clamp(dst_sample.a, 0., 1.);
  let ao = clamp(as_ + ad - as_ * ad, 0., 1.);
  var out = vec4<f32>(0.);
  if (mode == 0u) { out = vec4<f32>(src_sample.rgb + dst_sample.rgb * (1. - as_), ao); }
  else if (mode == 1u) { out = src_sample; }
  else if (mode == 2u) { out = vec4<f32>(dst_sample.rgb + src_sample.rgb * (1. - ad), ao); }
  else if (mode == 3u) { out = vec4<f32>(src_sample.rgb * ad, as_ * ad); }
  else if (mode == 4u) { out = vec4<f32>(dst_sample.rgb * as_, ad * as_); }
  else if (mode == 5u) { out = vec4<f32>(src_sample.rgb * (1. - ad), as_ * (1. - ad)); }
  else if (mode == 6u) { out = vec4<f32>(dst_sample.rgb * (1. - as_), ad * (1. - as_)); }
  else if (mode == 7u) { out = vec4<f32>(src_sample.rgb * ad + dst_sample.rgb * (1. - as_), ad); }
  else if (mode == 8u) { out = vec4<f32>(dst_sample.rgb * as_ + src_sample.rgb * (1. - ad), as_); }
  else if (mode == 9u) { out = vec4<f32>(src_sample.rgb * (1. - ad) + dst_sample.rgb * (1. - as_), clamp(as_ + ad - 2. * as_ * ad, 0., 1.)); }
  else if (mode == 10u) { out = vec4<f32>(min(src_sample.rgb + dst_sample.rgb, vec3<f32>(1.)), min(as_ + ad, 1.)); }
  else {
    let blended = artistic(mode, straight_rgb(src_sample), straight_rgb(dst_sample));
    out = vec4<f32>(clamp(src_sample.rgb * (1. - ad) + dst_sample.rgb * (1. - as_) + as_ * ad * blended, vec3<f32>(0.), vec3<f32>(1.)), ao);
  }
  return clamp(out, vec4<f32>(0.), vec4<f32>(1.));
}
