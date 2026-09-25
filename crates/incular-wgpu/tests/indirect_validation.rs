//! Exercise WGPU's internal validation shaders after build-time parsing.
use std::time::Duration;
use wgpu::util::DeviceExt;

#[test]
fn indirect_dispatch_clamps_invalid_workgroups_and_preserves_valid_dispatches() {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default()));
    let Ok(adapter) = adapter else {
        assert_ne!(
            std::env::var("INCULAR_WGPU_REQUIRE_GPU").as_deref(),
            Ok("1")
        );
        return;
    };
    let timestamps = adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY);
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_features: if timestamps {
            wgpu::Features::TIMESTAMP_QUERY
        } else {
            wgpu::Features::empty()
        },
        ..Default::default()
    }))
    .unwrap();
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("indirect validation regression"),
        source: wgpu::ShaderSource::Wgsl("@group(0) @binding(0) var<storage, read_write> count: atomic<u32>; @compute @workgroup_size(1) fn main() { atomicAdd(&count, 1u); }".into()),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: None,
        module: &module,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });
    let count = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 4,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let arguments = [
        1_u32,
        1,
        1,
        device.limits().max_compute_workgroups_per_dimension + 1,
        1,
        1,
        2,
        1,
        1,
    ];
    let indirect = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: None,
        contents: bytemuck::cast_slice(&arguments),
        usage: wgpu::BufferUsages::INDIRECT,
    });
    let bindings = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: count.as_entire_binding(),
        }],
    });
    let query = timestamps.then(|| {
        device.create_query_set(&wgpu::QuerySetDescriptor {
            label: None,
            ty: wgpu::QueryType::Timestamp,
            count: 2,
        })
    });
    let resolved = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 16,
        usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 24,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bindings, &[]);
        for offset in [0, 12, 24] {
            pass.dispatch_workgroups_indirect(&indirect, offset);
        }
    }
    // Keep query readback separate from the indirect-validation assertion.
    if timestamps {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: None,
            timestamp_writes: query
                .as_ref()
                .map(|query_set| wgpu::ComputePassTimestampWrites {
                    query_set,
                    beginning_of_pass_write_index: Some(0),
                    end_of_pass_write_index: Some(1),
                }),
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bindings, &[]);
        pass.dispatch_workgroups(1, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&count, 0, &readback, 0, 4);
    if let Some(query) = &query {
        encoder.resolve_query_set(query, 0..2, &resolved, 0);
        encoder.copy_buffer_to_buffer(&resolved, 0, &readback, 8, 16);
    }
    let submission = queue.submit([encoder.finish()]);
    let (send, receive) = std::sync::mpsc::channel();
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            send.send(result).unwrap();
        });
    device
        .poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: Some(Duration::from_secs(10)),
        })
        .unwrap();
    receive
        .recv_timeout(Duration::from_secs(10))
        .unwrap()
        .unwrap();
    let data = readback.slice(..).get_mapped_range().unwrap();
    assert_eq!(
        u32::from_le_bytes(data[0..4].try_into().unwrap()),
        if timestamps { 4 } else { 3 }
    );
    if timestamps {
        let start = u64::from_le_bytes(data[8..16].try_into().unwrap());
        let end = u64::from_le_bytes(data[16..24].try_into().unwrap());
        assert!(
            end >= start,
            "timestamps out of order: start={start}, end={end}"
        );
        // This host also returns zero on unpatched WGPU's Vulkan path. Keep
        // that limitation visible: zero is not evidence of useful GPU timing.
        eprintln!(
            "timestamp readback: start={start}, end={end}; nonzero={}",
            end != 0
        );
    }
}
