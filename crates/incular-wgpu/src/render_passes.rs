use super::create_stencil_attachment;
use super::prelude::*;

#[derive(Clone)]
pub(crate) struct OffscreenTarget {
    pub(crate) _color: wgpu::Texture,
    pub(crate) color_view: wgpu::TextureView,
    pub(crate) _stencil: wgpu::Texture,
    pub(crate) stencil_view: wgpu::TextureView,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) format: wgpu::TextureFormat,
    pub(crate) has_stencil: bool,
}
pub(crate) struct DestinationTargets {
    pub(crate) first: OffscreenTarget,
    pub(crate) second: OffscreenTarget,
    pub(crate) width: u32,
    pub(crate) height: u32,
}
impl OffscreenTarget {
    pub(crate) fn bytes(&self) -> usize {
        let color_bytes = self.width as usize * self.height as usize * 4;
        color_bytes + if self.has_stencil { color_bytes } else { 0 }
    }
}

/// Creates one sampleable premultiplied scene target using the renderer's
/// target format. Ownership/counter policy stays with the caller so retained
/// effects and native presentation can share the allocation primitive without
/// conflating their diagnostics.
pub(crate) fn create_scene_target(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
    label: &'static str,
) -> OffscreenTarget {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let color_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let (stencil, stencil_view) = create_stencil_attachment(device, width, height);
    OffscreenTarget {
        _color: texture,
        color_view,
        _stencil: stencil,
        stencil_view,
        width,
        height,
        format,
        has_stencil: true,
    }
}
#[derive(Default)]
pub(crate) struct OffscreenTargetPool {
    pub(crate) free: Vec<OffscreenTarget>,
    pub(crate) bytes: usize,
}
impl OffscreenTargetPool {
    const MAX_BYTES: usize = 16 * 1024 * 1024;

    pub(crate) fn take(
        &mut self,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        has_stencil: bool,
    ) -> Option<OffscreenTarget> {
        let index = self.free.iter().position(|target| {
            target.width == width
                && target.height == height
                && target.format == format
                && target.has_stencil == has_stencil
        })?;
        let target = self.free.swap_remove(index);
        self.bytes = self.bytes.saturating_sub(target.bytes());
        Some(target)
    }

    pub(crate) fn recycle(&mut self, target: OffscreenTarget) {
        let bytes = target.bytes();
        if self.bytes.saturating_add(bytes) > Self::MAX_BYTES {
            return;
        }
        self.bytes = self.bytes.saturating_add(bytes);
        self.free.push(target);
    }
}
pub(crate) struct OffscreenCacheEntry {
    pub(crate) target: OffscreenTarget,
    pub(crate) bind_group: wgpu::BindGroup,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) scale_factor_bits: u32,
    pub(crate) generation: u64,
    pub(crate) device_generation: u64,
    pub(crate) last_used_frame: u64,
    pub(crate) bytes: usize,
}
pub(crate) struct EffectCacheEntry {
    pub(crate) target: OffscreenTarget,
    pub(crate) bind_group: wgpu::BindGroup,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) scale_factor_bits: u32,
    pub(crate) source_generation: u64,
    pub(crate) sigma_x_bits: u32,
    pub(crate) sigma_y_bits: u32,
    pub(crate) matrix_bits: [u32; 20],
    pub(crate) device_generation: u64,
    pub(crate) last_used_frame: u64,
    pub(crate) bytes: usize,
    pub(crate) downsample_factor: u32,
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct CachedSource {
    pub(crate) origin: Offset,
    pub(crate) width: u32,
    pub(crate) height: u32,
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct CachedEffect {
    pub(crate) origin: Offset,
    pub(crate) width: u32,
    pub(crate) height: u32,
}
