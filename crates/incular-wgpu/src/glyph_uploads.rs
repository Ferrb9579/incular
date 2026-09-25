//! Batch sparse atlas writes into one mapped upload buffer, rather than one
//! driver allocation per glyph. Shared-context ownership lets either window
//! flush masks before drawing, including masks resolved by a failed frame.
use crate::{AtlasEntry, GLYPH_ATLAS_PADDING};

const MAX_PENDING_BYTES: usize = 4 * 1024 * 1024;

struct Region {
    texture: wgpu::Texture,
    origin: wgpu::Origin3d,
    size: wgpu::Extent3d,
    offset: u64,
    stride: u32,
}

#[derive(Default)]
pub(crate) struct GlyphUploads {
    bytes: Vec<u8>,
    regions: Vec<Region>,
}

impl GlyphUploads {
    pub(crate) fn push(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture: wgpu::Texture,
        entry: AtlasEntry,
        bitmap: &[u8],
    ) {
        if entry.width == 0 || entry.height == 0 {
            return;
        }
        let padding = usize::from(GLYPH_ATLAS_PADDING);
        let width = usize::from(entry.width) + padding * 2;
        let height = usize::from(entry.height) + padding * 2;
        let alignment = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize;
        let stride = width.div_ceil(alignment) * alignment;
        let len = stride * height;
        if self.bytes.len() + len > MAX_PENDING_BYTES {
            self.flush(device, queue);
        }
        let start = self.bytes.len();
        // Explicitly zero the allocation border and row alignment padding.
        // Copies are disjoint rectangles, so existing atlas neighbors survive.
        self.bytes.resize(start + len, 0);
        for row in 0..usize::from(entry.height) {
            let source = row * usize::from(entry.width);
            let destination = start + (row + padding) * stride + padding;
            self.bytes[destination..destination + usize::from(entry.width)]
                .copy_from_slice(&bitmap[source..source + usize::from(entry.width)]);
        }
        self.regions.push(Region {
            texture,
            origin: wgpu::Origin3d {
                x: u32::from(entry.x - GLYPH_ATLAS_PADDING),
                y: u32::from(entry.y - GLYPH_ATLAS_PADDING),
                z: 0,
            },
            size: wgpu::Extent3d {
                width: width as u32,
                height: height as u32,
                depth_or_array_layers: 1,
            },
            offset: start as u64,
            stride: stride as u32,
        });
        if self.bytes.len() >= MAX_PENDING_BYTES {
            self.flush(device, queue);
        }
    }

    pub(crate) fn flush(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) -> bool {
        if self.regions.is_empty() {
            return false;
        }
        let bytes = std::mem::take(&mut self.bytes);
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("incular batched glyph upload"),
            size: bytes.len() as u64,
            usage: wgpu::BufferUsages::MAP_WRITE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: true,
        });
        buffer
            .slice(..)
            .get_mapped_range_mut()
            .expect("new upload buffer is mapped")
            .copy_from_slice(&bytes);
        buffer.unmap();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("incular batched glyph copies"),
        });
        for region in std::mem::take(&mut self.regions) {
            encoder.copy_buffer_to_texture(
                wgpu::TexelCopyBufferInfo {
                    buffer: &buffer,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: region.offset,
                        bytes_per_row: Some(region.stride),
                        rows_per_image: Some(region.size.height),
                    },
                },
                wgpu::TexelCopyTextureInfo {
                    texture: &region.texture,
                    mip_level: 0,
                    origin: region.origin,
                    aspect: wgpu::TextureAspect::All,
                },
                region.size,
            );
        }
        queue.submit(Some(encoder.finish()));
        // No CPU staging capacity or mapped buffer remains retained at idle.
        // Submitted copies retain their GPU resources until completion.
        true
    }
}
