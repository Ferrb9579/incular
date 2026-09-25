
            @vertex fn vs_main(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
                let p = array<vec2f, 3>(vec2f(-1., -1.), vec2f(3., -1.), vec2f(-1., 3.));
                return vec4f(p[i], 0., 1.);
            }
            @fragment fn fs_main() -> @location(0) vec4f { return vec4f(0.); }
