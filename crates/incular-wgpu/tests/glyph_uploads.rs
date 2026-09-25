//! Real-GPU readback of the production upload batch, including sparse writes,
//! page replacement, padding, automatic budget flush and warm no-op behavior.
use incular_wgpu::{AtlasEntry, GLYPH_ATLAS_PADDING, GlyphAtlasClass};
use std::time::Duration;

#[path = "../src/glyph_uploads.rs"]
mod glyph_uploads;

fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()));
    let Ok(adapter) = adapter else {
        assert_ne!(
            std::env::var("INCULAR_WGPU_REQUIRE_GPU").as_deref(),
            Ok("1"),
            "GPU required"
        );
        eprintln!("glyph upload readback skipped: no GPU adapter");
        return None;
    };
    eprintln!("glyph uploads: {:?}", adapter.get_info());
    Some(pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap())
}

fn texture(device: &wgpu::Device) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("glyph upload readback page"),
        size: wgpu::Extent3d {
            width: 1024,
            height: 1024,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::R8Unorm,
        usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}

fn entry(x: u16, y: u16, width: u16, height: u16) -> AtlasEntry {
    AtlasEntry {
        page: 0,
        x,
        y,
        width,
        height,
        atlas_class: GlyphAtlasClass::Normal,
        bearing_x: 0,
        bearing_y: 0,
        generation: 1,
    }
}

fn pixel(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    x: u32,
    y: u32,
) -> u8 {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("glyph pixel readback"),
        size: 256,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d { x, y, z: 0 },
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(256),
                rows_per_image: Some(1),
            },
        },
        wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
    );
    let submission = queue.submit(Some(encoder.finish()));
    let (send, receive) = std::sync::mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            send.send(result).unwrap();
        });
    device
        .poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: Some(Duration::from_secs(5)),
        })
        .unwrap();
    receive
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
        .unwrap();
    let value = buffer.slice(..).get_mapped_range().unwrap()[0];
    buffer.unmap();
    value
}

#[test]
fn sparse_uploads_preserve_neighbors_and_zero_padding_across_page_generations() {
    let Some((device, queue)) = device() else {
        return;
    };
    let old = texture(&device);
    let replacement = texture(&device);
    let mut uploads = glyph_uploads::GlyphUploads::default();
    assert!(!uploads.flush(&device, &queue));
    uploads.push(
        &device,
        &queue,
        old.clone(),
        entry(1, 1, 3, 2),
        &[10, 20, 30, 40, 50, 60],
    );
    uploads.push(
        &device,
        &queue,
        replacement.clone(),
        entry(1, 1, 3, 2),
        &[101, 102, 103, 104, 105, 106],
    );
    assert!(uploads.flush(&device, &queue));
    uploads.push(&device, &queue, old.clone(), entry(20, 15, 2, 1), &[7, 8]);
    assert!(uploads.flush(&device, &queue));
    assert!(!uploads.flush(&device, &queue));
    assert_eq!(pixel(&device, &queue, &old, 3, 2), 60);
    assert_eq!(pixel(&device, &queue, &replacement, 3, 2), 106);
    assert_eq!(pixel(&device, &queue, &old, 21, 15), 8);
    for (x, y) in [(0, 1), (1, 0), (4, 1), (1, 3), (19, 15), (22, 15)] {
        assert_eq!(pixel(&device, &queue, &old, x, y), 0, "padding at {x},{y}");
    }
}

#[test]
fn upload_budget_flushes_automatically_and_subsequent_copies_remain_ordered() {
    let Some((device, queue)) = device() else {
        return;
    };
    let target = texture(&device);
    let mut uploads = glyph_uploads::GlyphUploads::default();
    // Four padded 1024-square R8 copies reach the four-MiB pending limit.
    for value in 1..=4 {
        uploads.push(
            &device,
            &queue,
            target.clone(),
            entry(1, 1, 1022, 1022),
            &vec![value; 1022 * 1022],
        );
    }
    assert!(
        !uploads.flush(&device, &queue),
        "budget must already have flushed"
    );
    assert_eq!(pixel(&device, &queue, &target, 512, 512), 4);
    uploads.push(&device, &queue, target.clone(), entry(1, 1, 1, 1), &[99]);
    assert!(uploads.flush(&device, &queue));
    assert_eq!(pixel(&device, &queue, &target, 1, 1), 99);
    assert_eq!(pixel(&device, &queue, &target, 512, 512), 4);
}
