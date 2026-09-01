use super::*;

impl WgpuRenderer {
    /// Drives wgpu-profiler's asynchronous query mappings without blocking.
    /// The profiler owns the query/readback buffers and returns the oldest
    /// completed scope tree when the GPU has finished it.
    pub(super) fn pump_gpu_profiler(&mut self) {
        let _ = self.device.poll(wgpu::PollType::Poll);
        let Some(results) = self
            .gpu_profiler
            .process_finished_frame(self.queue.get_timestamp_period())
        else {
            return;
        };
        let frame = self
            .profiler_frames
            .pop_front()
            .unwrap_or(self.counters.frames);
        if let Some(main_pass_us) = query_duration_us(&results, "incular retained compositor pass")
        {
            self.latest_gpu_timing = Some(GpuFrameTimings {
                frame,
                main_pass_us,
            });
        }
    }

    /// Schedules a readback of the next successfully rendered surface frame.
    /// The request is consumed by [`Self::render`], so ordinary frames do not
    /// pay for a GPU-to-CPU copy.
    pub fn request_capture(&mut self) {
        self.capture_requested = true;
    }

    /// Takes the most recent completed capture, if one was requested.
    pub fn take_capture(&mut self) -> Option<Result<CapturedFrame, String>> {
        self.last_capture.take()
    }

    pub(super) fn capture_surface_texture(
        &self,
        texture: &wgpu::Texture,
    ) -> Result<CapturedFrame, String> {
        if !self.capture_supported {
            return Err(
                "the surface does not support COPY_SRC or is not an RGBA8-compatible format".into(),
            );
        }
        let width = self.config.width;
        let height = self.config.height;
        let row_bytes = width
            .checked_mul(4)
            .ok_or_else(|| "capture row size overflowed".to_owned())?;
        let padded_row_bytes = row_bytes
            .checked_add(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT - 1)
            .ok_or_else(|| "capture padded row size overflowed".to_owned())?
            / wgpu::COPY_BYTES_PER_ROW_ALIGNMENT
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer_size = u64::from(padded_row_bytes)
            .checked_mul(u64::from(height))
            .ok_or_else(|| "capture buffer size overflowed".to_owned())?;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("incular simulator screenshot readback"),
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("incular simulator screenshot copy"),
            });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_row_bytes),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        let submission = self.queue.submit(Some(encoder.finish()));
        let (sender, receiver) = std::sync::mpsc::channel();
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = sender.send(result);
            });
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: Some(Duration::from_secs(5)),
            })
            .map_err(|error| format!("GPU capture readback timed out: {error}"))?;
        receiver
            .recv_timeout(Duration::from_secs(5))
            .map_err(|error| format!("GPU capture mapping did not complete: {error}"))?
            .map_err(|error| format!("GPU capture mapping failed: {error:?}"))?;
        let mapped = buffer
            .slice(..)
            .get_mapped_range()
            .map_err(|error| format!("GPU capture mapping could not be read: {error:?}"))?;
        let is_bgra = matches!(
            self.config.format,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        );
        let alpha_representation = self.alpha_plan.representation();
        let mut rgba8 = Vec::with_capacity(
            usize::try_from(row_bytes)
                .ok()
                .and_then(|row| {
                    usize::try_from(height)
                        .ok()
                        .and_then(|height| row.checked_mul(height))
                })
                .ok_or_else(|| "capture output size overflowed".to_owned())?,
        );
        let padded_row_bytes = usize::try_from(padded_row_bytes)
            .map_err(|_| "capture row size does not fit in usize".to_owned())?;
        let row_bytes = usize::try_from(row_bytes)
            .map_err(|_| "capture row size does not fit in usize".to_owned())?;
        for row in mapped.chunks(padded_row_bytes).take(height as usize) {
            for pixel in row[..row_bytes].as_chunks::<4>().0 {
                let rgba = if is_bgra {
                    [pixel[2], pixel[1], pixel[0], pixel[3]]
                } else {
                    *pixel
                };
                rgba8.extend_from_slice(
                    &alpha_representation.to_straight_rgba8(self.config.format, rgba),
                );
            }
        }
        drop(mapped);
        buffer.unmap();
        Ok(CapturedFrame {
            width,
            height,
            rgba8,
        })
    }

    #[must_use]
    pub fn gpu_timing_supported(&self) -> bool {
        self.device
            .features()
            .contains(wgpu::Features::TIMESTAMP_QUERY)
    }

    /// Latest asynchronously-resolved GPU timing, when supported.
    #[must_use]
    pub const fn gpu_frame_timings(&self) -> Option<GpuFrameTimings> {
        self.latest_gpu_timing
    }
}
