
            @group(0) @binding(0)
            var<storage, read_write> dst: array<u32, 6>;
            @group(1) @binding(0)
            var<storage, read> src: array<u32>;
            struct OffsetPc {
                inner: u32,
            }
            var<immediate> offset: OffsetPc;

            @compute @workgroup_size(1)
            fn main() {
                let src = vec3(src[offset.inner], src[offset.inner + 1], src[offset.inner + 2]);
                let max_compute_workgroups_per_dimension = 4294967295u;
                if (
                    src.x > max_compute_workgroups_per_dimension ||
                    src.y > max_compute_workgroups_per_dimension ||
                    src.z > max_compute_workgroups_per_dimension
                ) {
                    dst = array(0u, 0u, 0u, 0u, 0u, 0u);
                } else {
                    dst = array(src.x, src.y, src.z, src.x, src.y, src.z);
                }
            }
        