
struct Out { @builtin(position) position: vec4<f32>, @location(0) local: vec2<f32>, @location(1) color: vec4<f32>, @location(2) gradient: vec4<f32>, @location(3) options: vec4<f32> };
@group(0) @binding(0) var gradient_lut: texture_2d<f32>;
@group(0) @binding(1) var gradient_sampler: sampler;
@vertex fn vs_main(@location(0) local: vec2<f32>, @location(1) affine: vec4<f32>, @location(2) translation: vec4<f32>, @location(3) surface: vec4<f32>, @location(4) color: vec4<f32>, @location(5) gradient: vec4<f32>, @location(6) options: vec4<f32>) -> Out {
  var out: Out;
  let physical = vec2<f32>(
    affine.x * local.x + affine.z * local.y + translation.x,
    affine.y * local.x + affine.w * local.y + translation.y,
  );
  out.position = vec4<f32>(physical.x / surface.x * 2. - 1., 1. - physical.y / surface.y * 2., 0., 1.);
  out.color = color;
  out.local = local;
  out.gradient = gradient;
  out.options = options;
  return out;
}
fn lookup(t: f32) -> vec4<f32> { let p=textureSampleLevel(gradient_lut,gradient_sampler,vec2(clamp(t,0.,1.),.5),0.); return select(vec4(0.),vec4(p.rgb/max(p.a,.00001),p.a),p.a>0.); }
@fragment fn fs_main(i: Out) -> @location(0) vec4<f32> { var t=0.; if(i.options.x==1.) { let d=i.gradient.zw-i.gradient.xy; t=clamp(dot(i.local-i.gradient.xy,d)/max(dot(d,d),.0001),0.,1.); } else if(i.options.x==2.) { t=clamp(length(i.local-i.gradient.xy)/max(i.gradient.z,.0001),0.,1.); } else if(i.options.x==3.) { t=fract((atan2(i.local.y-i.gradient.y,i.local.x-i.gradient.x)-i.gradient.z)/6.2831853); } return select(i.color,lookup(t),i.options.x>0.); }
