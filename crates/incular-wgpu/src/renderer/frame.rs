use super::*;

impl WgpuRenderer {
    pub(super) fn acquire_surface_texture(
        &mut self,
    ) -> Result<Option<(wgpu::SurfaceTexture, bool)>, RendererError> {
        match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => Ok(Some((frame, false))),
            // A suboptimal texture is still valid for this frame. WGPU
            // explicitly forbids Surface::configure while a SurfaceTexture is
            // outstanding, so defer reconfiguration until after presentation
            // consumes this texture below.
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => Ok(Some((frame, true))),
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                Ok(None)
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                self.surface = self.shared.create_surface(self.target.clone())?;
                self.refresh_surface_alpha_plan()?;
                self.surface.configure(&self.device, &self.config);
                self.window_gpu.presentation.surface_lost();
                Ok(None)
            }
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => Ok(None),
        }
    }

    pub(super) fn ensure_presentation_target(&mut self) {
        if self.presentation_target.as_ref().is_some_and(|target| {
            target.width == self.config.width
                && target.height == self.config.height
                && target.format == self.config.format
        }) {
            return;
        }
        self.presentation_target = Some(create_scene_target(
            &self.device,
            self.config.format,
            self.config.width,
            self.config.height,
            "incular native presentation scene target",
        ));
        self.counters.surface_present_target_creations += 1;
        self.counters.surface_present_cached_bytes = self
            .presentation_target
            .as_ref()
            .map_or(0, OffscreenTarget::bytes);
    }

    pub(super) fn present_straight_alpha_target(
        &mut self,
        target: &OffscreenTarget,
        frame_view: &wgpu::TextureView,
    ) -> u32 {
        self.ensure_composite_capacity(1);
        let instance = composite_instance(
            Offset::ZERO,
            Offset::ZERO,
            self.config.width,
            self.config.height,
            self.config.width,
            self.config.height,
            1.0,
            1.0,
        );
        self.queue
            .write_buffer(&self.composite_instances, 0, bytemuck::bytes_of(&instance));
        let bind_group = self
            .create_composite_bind_group(target, "incular straight-alpha presentation bind group");
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("incular straight-alpha presentation encoder"),
            });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("incular straight-alpha presentation pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: frame_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.straight_alpha_present_pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.set_vertex_buffer(0, self.mesh.slice(..));
        pass.set_vertex_buffer(
            1,
            self.composite_instances
                .slice(..std::mem::size_of::<GpuCompositeInstance>() as u64),
        );
        pass.draw(0..6, 0..1);
        drop(pass);
        self.queue.submit(Some(encoder.finish()));
        self.counters.surface_present_conversion_passes += 1;
        1
    }

    pub(super) fn ensure_destination_targets(&mut self, width: u32, height: u32) {
        if self
            .destination_targets
            .as_ref()
            .is_some_and(|targets| targets.width == width && targets.height == height)
        {
            self.counters.blend_target_reuses += 1;
            return;
        }
        self.destination_targets = Some(DestinationTargets {
            first: self.create_offscreen_target(
                width,
                height,
                "incular destination composition target A",
            ),
            second: self.create_offscreen_target(
                width,
                height,
                "incular destination composition target B",
            ),
            width,
            height,
        });
        let bytes = self
            .destination_targets
            .as_ref()
            .map_or(0, |targets| targets.first.bytes() + targets.second.bytes());
        self.counters.blend_intermediate_cached_bytes = bytes;
        self.counters.blend_intermediate_peak_bytes =
            self.counters.blend_intermediate_peak_bytes.max(bytes);
        self.counters.blend_intermediate_target_creations += 2;
    }

    pub(super) fn copy_color_texture(
        &self,
        source: &OffscreenTarget,
        destination: &OffscreenTarget,
        width: u32,
        height: u32,
        label: &'static str,
    ) {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some(label) });
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &source._color,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &destination._color,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit(Some(encoder.finish()));
    }

    pub(super) fn prepare_batch_capacity(&mut self, batches: &[DrawBatch]) {
        self.ensure_rectangle_capacity(count_rectangles(batches));
        self.ensure_glyph_capacity(count_glyphs(batches));
        self.ensure_image_capacity(count_images(batches));
        self.ensure_composite_capacity(count_composites(batches));
        let rounded = count_rounded(batches);
        if rounded > self.rounded_rect_instance_capacity {
            self.rounded_rect_instance_capacity = rounded.next_power_of_two();
            self.rounded_rect_instances =
                create_rrect_buffer(&self.device, self.rounded_rect_instance_capacity);
        }
        let paths = count_paths(batches);
        if paths > self.path_instance_capacity {
            self.path_instance_capacity = paths.next_power_of_two();
            self.path_instances =
                create_path_instance_buffer(&self.device, self.path_instance_capacity);
        }
    }

    /// Rebuilds the stencil state that is active at a destination-promotion
    /// boundary.  Each ping-pong pass starts with a cleared stencil attachment,
    /// so masks pushed before the boundary have to be replayed before the next
    /// painter segment.  The replay list contains only increment operations;
    /// balanced decrements in the segment itself still update the new pass's
    /// state normally.
    pub(super) fn destination_segment(
        batches: &[DrawBatch],
        start: usize,
        end: usize,
        active_start: &[DrawBatch],
    ) -> (Vec<DrawBatch>, Vec<DrawBatch>) {
        let mut active = active_start.to_vec();
        let mut segment = active_start.to_vec();
        for batch in &batches[start..end] {
            segment.push(batch.clone());
            match batch {
                DrawBatch::StencilRRect {
                    increment: true, ..
                }
                | DrawBatch::StencilPath {
                    increment: true, ..
                } => active.push(batch.clone()),
                DrawBatch::StencilRRect {
                    increment: false, ..
                }
                | DrawBatch::StencilPath {
                    increment: false, ..
                } => {
                    let _ = active.pop();
                }
                _ => {}
            }
        }
        (segment, active)
    }

    /// Renders an ordered stream that contains destination-reading blend
    /// batches using two sampleable composition targets.  Each segment is
    /// painted into the current target, copied to the alternate target before
    /// a blend draw, and the blend shader samples the untouched current target.
    /// This keeps the read and write textures distinct while preserving
    /// painter order and clip/stencil state.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn render_destination_batches(
        &mut self,
        batches: &[DrawBatch],
        scale: f32,
        width: u32,
        height: u32,
        targets: &mut DestinationTargets,
        label: &'static str,
        clear: wgpu::Color,
    ) -> (bool, u32, u32, u32) {
        let mut current_first = true;
        let mut first_pass = true;
        let mut segment_start = 0_usize;
        let mut draw_calls = 0_u32;
        let mut text_draw_calls = 0_u32;
        let mut passes = 0_u32;
        let mut active_stencils = Vec::new();

        for (index, batch) in batches.iter().enumerate() {
            if !matches!(
                batch,
                DrawBatch::Blend {
                    mode,
                    ..
                } if mode.requires_destination_read()
            ) {
                continue;
            }
            let (segment, next_active_stencils) =
                Self::destination_segment(batches, segment_start, index, &active_stencils);
            let (current, _) = if current_first {
                (&targets.first, &targets.second)
            } else {
                (&targets.second, &targets.first)
            };
            self.prepare_batch_capacity(&segment);
            self.upload_instance_data(&segment, scale, width, height);
            self.prepare_image_bind_groups(&segment);
            let current_view = current.color_view.clone();
            let current_stencil = current.stencil_view.clone();
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some(label) });
            let (draws, text) = self.encode_batches(
                &mut encoder,
                &current_view,
                &current_stencil,
                &segment,
                width,
                height,
                scale,
                clear,
                None,
                !first_pass,
            );
            self.queue.submit(Some(encoder.finish()));
            draw_calls += draws;
            text_draw_calls += text;
            passes += 1;
            first_pass = false;
            active_stencils = next_active_stencils;

            let (current, alternate) = if current_first {
                (&targets.first, &targets.second)
            } else {
                (&targets.second, &targets.first)
            };
            self.copy_color_texture(
                current,
                alternate,
                width,
                height,
                "incular destination composition copy",
            );
            let mut blend_segment = active_stencils.clone();
            blend_segment.push(batch.clone());
            self.prepare_batch_capacity(&blend_segment);
            self.upload_instance_data(&blend_segment, scale, width, height);
            self.prepare_image_bind_groups(&blend_segment);
            let current_view = current.color_view.clone();
            let alternate_view = alternate.color_view.clone();
            let alternate_stencil = alternate.stencil_view.clone();
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("incular destination blend pass"),
                });
            let (draws, text) = self.encode_batches(
                &mut encoder,
                &alternate_view,
                &alternate_stencil,
                &blend_segment,
                width,
                height,
                scale,
                wgpu::Color::TRANSPARENT,
                Some(&current_view),
                true,
            );
            self.queue.submit(Some(encoder.finish()));
            draw_calls += draws;
            text_draw_calls += text;
            passes += 1;
            current_first = !current_first;
            segment_start = index + 1;
        }

        let (trailing, _) =
            Self::destination_segment(batches, segment_start, batches.len(), &active_stencils);
        let (current, _) = if current_first {
            (&targets.first, &targets.second)
        } else {
            (&targets.second, &targets.first)
        };
        self.prepare_batch_capacity(&trailing);
        self.upload_instance_data(&trailing, scale, width, height);
        self.prepare_image_bind_groups(&trailing);
        let current_view = current.color_view.clone();
        let current_stencil = current.stencil_view.clone();
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some(label) });
        let (draws, text) = self.encode_batches(
            &mut encoder,
            &current_view,
            &current_stencil,
            &trailing,
            width,
            height,
            scale,
            clear,
            None,
            !first_pass,
        );
        self.queue.submit(Some(encoder.finish()));
        draw_calls += draws;
        text_draw_calls += text;
        passes += 1;
        (current_first, draw_calls, text_draw_calls, passes)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn present_composition_target(
        &mut self,
        target: &OffscreenTarget,
        frame_view: &wgpu::TextureView,
        frame_stencil_view: &wgpu::TextureView,
        target_origin: Offset,
        target_width: u32,
        target_height: u32,
        parent_width: u32,
        parent_height: u32,
        scale: f32,
    ) -> u32 {
        self.ensure_composite_capacity(1);
        let instance = composite_instance(
            target_origin,
            Offset::ZERO,
            parent_width,
            parent_height,
            target_width,
            target_height,
            scale,
            1.,
        );
        self.queue
            .write_buffer(&self.composite_instances, 0, bytemuck::bytes_of(&instance));
        let bind_group = self.create_composite_bind_group(
            target,
            "incular destination composition presentation bind group",
        );
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("incular destination composition presentation"),
            });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("incular destination composition presentation pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: frame_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: frame_stencil_view,
                depth_ops: None,
                stencil_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0),
                    store: wgpu::StoreOp::Store,
                }),
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_stencil_reference(0);
        pass.set_pipeline(&self.composite_pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.set_vertex_buffer(0, self.mesh.slice(..));
        pass.set_vertex_buffer(
            1,
            self.composite_instances
                .slice(..std::mem::size_of::<GpuCompositeInstance>() as u64),
        );
        pass.draw(0..6, 0..1);
        drop(pass);
        self.queue.submit(Some(encoder.finish()));
        1
    }
    pub fn render(
        &mut self,
        list: &DisplayList,
        scale_factor: f64,
    ) -> Result<RenderStats, RendererError> {
        let stats = self.render_composited(list, scale_factor)?;
        if stats.presented {
            self.window_gpu.presentation.record_present();
        } else {
            self.window_gpu.presentation.record_skipped();
        }
        Ok(stats)
    }
    pub(super) fn render_composited(
        &mut self,
        list: &DisplayList,
        scale_factor: f64,
    ) -> Result<RenderStats, RendererError> {
        if !self.window_gpu.presentation.configured {
            // Still driven but presenting nothing (zero-size surface): no
            // frame advances, so age eviction cannot run — but stale shared
            // generations can still be released safely here.
            self.reclaim_stale_textures();
            return Ok(RenderStats::default());
        }
        // Profiling deltas: every counter touched between these snapshots is
        // attributable to this frame without touching individual call sites.
        let before = self.counters;
        let scale = normalized_scale(scale_factor);
        let promote_destination = commands_have_destination_blend(list.commands());
        let (target_origin, target_width, target_height) =
            if promote_destination && self.background_color.alpha == 0 {
                destination_composition_scope(list, scale, self.config.width, self.config.height)
            } else {
                (Offset::ZERO, self.config.width, self.config.height)
            };
        self.target_width = target_width;
        self.target_height = target_height;
        self.target_origin = target_origin;
        let prepare_started = std::time::Instant::now();
        let batches = self.lower_commands(
            list.commands(),
            scale,
            Transform::translation(Offset::new(-target_origin.x, -target_origin.y)),
            ClipState::Unbounded,
        )?;
        let rectangles = batches
            .iter()
            .filter_map(|batch| match batch {
                DrawBatch::Rectangles { clip, instances } if *clip != ClipState::Empty => {
                    Some(instances.len())
                }
                _ => None,
            })
            .sum::<usize>();
        let glyphs = batches
            .iter()
            .filter_map(|batch| match batch {
                DrawBatch::Glyphs {
                    clip, instances, ..
                } if *clip != ClipState::Empty => Some(instances.len()),
                _ => None,
            })
            .sum::<usize>();
        let images = batches
            .iter()
            .filter_map(|batch| match batch {
                DrawBatch::Images {
                    clip, instances, ..
                } if *clip != ClipState::Empty => Some(instances.len()),
                _ => None,
            })
            .sum::<usize>();
        let rounded = batches
            .iter()
            .map(|batch| match batch {
                DrawBatch::RoundedRects {
                    clip, instances, ..
                } if *clip != ClipState::Empty => instances.len(),
                DrawBatch::StencilRRect { clip, .. } if *clip != ClipState::Empty => 1,
                _ => 0,
            })
            .sum::<usize>();
        let paths = batches
            .iter()
            .map(|batch| match batch {
                DrawBatch::Path { clip, .. } | DrawBatch::StencilPath { clip, .. }
                    if *clip != ClipState::Empty =>
                {
                    1
                }
                _ => 0,
            })
            .sum::<usize>();
        let stencil_masks = batches
            .iter()
            .filter(|batch| {
                matches!(
                    batch,
                    DrawBatch::StencilPath { clip, .. } | DrawBatch::StencilRRect { clip, .. }
                        if *clip != ClipState::Empty
                )
            })
            .count();
        let path_draws_this_frame = paths;
        let composites = batches
            .iter()
            .filter(|batch| {
                matches!(
                    batch,
                    DrawBatch::Offscreen { clip, .. }
                        | DrawBatch::Filtered { clip, .. }
                        | DrawBatch::Shadow { clip, .. }
                        | DrawBatch::Blend { clip, .. }
                        if *clip != ClipState::Empty
                )
            })
            .count();
        let rectangle_reallocated = self.ensure_rectangle_capacity(rectangles);
        let glyph_reallocated = self.ensure_glyph_capacity(glyphs);
        let image_reallocated = self.ensure_image_capacity(images);
        let _composite_reallocated = self.ensure_composite_capacity(composites);
        if rounded > self.rounded_rect_instance_capacity {
            self.rounded_rect_instance_capacity = rounded.next_power_of_two();
            self.rounded_rect_instances =
                create_rrect_buffer(&self.device, self.rounded_rect_instance_capacity);
        }
        if paths > self.path_instance_capacity {
            self.path_instance_capacity = paths.next_power_of_two();
            self.path_instances =
                create_path_instance_buffer(&self.device, self.path_instance_capacity);
        }
        self.upload_instance_data(&batches, scale, self.target_width, self.target_height);
        self.prepare_image_bind_groups(&batches);
        let prepare_us = us_since(prepare_started);
        let encode_started = std::time::Instant::now();
        self.profiler_next_pass = self.gpu_timing_supported();
        let Some((frame, reconfigure_after_present)) = self.acquire_surface_texture()? else {
            return Ok(RenderStats::default());
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let presentation_target = if self.alpha_plan.requires_straight_alpha_conversion() {
            self.ensure_presentation_target();
            self.presentation_target.clone()
        } else {
            None
        };
        let scene_view = presentation_target
            .as_ref()
            .map_or_else(|| view.clone(), |target| target.color_view.clone());
        let scene_stencil = presentation_target.as_ref().map_or_else(
            || self.stencil_view.clone(),
            |target| target.stencil_view.clone(),
        );
        let has_destination_blend = batches.iter().any(|batch| {
            matches!(
                batch,
                DrawBatch::Blend {
                    mode,
                    ..
                } if mode.requires_destination_read()
            )
        });
        let (mut draw_calls, _text_draw_calls) = if has_destination_blend {
            // A destination-read blend promotes only this composition scope;
            // ordinary SrcOver frames continue through the direct surface
            // path above.
            self.ensure_destination_targets(target_width, target_height);
            let mut targets = self
                .destination_targets
                .take()
                .expect("destination targets after ensure");
            let (current_first, draws, text, passes) = self.render_destination_batches(
                &batches,
                scale,
                target_width,
                target_height,
                &mut targets,
                "incular destination composition segment",
                self.scene_background_clear(),
            );
            self.counters.full_frame_intermediate_passes += 1;
            self.counters.offscreen_render_passes += u64::from(passes);
            let final_target = if current_first {
                targets.first.clone()
            } else {
                targets.second.clone()
            };
            let present_draw = self.present_composition_target(
                &final_target,
                &scene_view,
                &scene_stencil,
                target_origin,
                target_width,
                target_height,
                self.config.width,
                self.config.height,
                scale,
            );
            self.destination_targets = Some(targets);
            (draws + present_draw, text)
        } else {
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("incular frame encoder"),
                });
            let result = self.encode_batches(
                &mut encoder,
                &scene_view,
                &scene_stencil,
                &batches,
                target_width,
                target_height,
                scale,
                self.scene_background_clear(),
                None,
                false,
            );
            self.queue.submit(Some(encoder.finish()));
            result
        };
        if let Some(target) = presentation_target.as_ref() {
            draw_calls += self.present_straight_alpha_target(target, &view);
        }
        if self.capture_requested {
            self.last_capture = Some(self.capture_surface_texture(&frame.texture));
            self.capture_requested = false;
        }
        self.queue.present(frame);
        if reconfigure_after_present {
            self.surface.configure(&self.device, &self.config);
        }
        if self.gpu_timing_supported() {
            // wgpu-profiler drops the newest pending frame when its bounded
            // queue is full; mirror that bookkeeping for the frame ids kept
            // alongside the profiler results.
            if self.profiler_frames.len() >= 3 {
                self.profiler_frames.pop_back();
            }
            self.gpu_profiler
                .end_frame()
                .map_err(|error| RendererError::GpuProfiler(error.to_string()))?;
            self.profiler_frames
                .push_back(self.counters.frames.saturating_add(1));
            self.pump_gpu_profiler();
        }
        let submit_us = us_since(encode_started);
        let encode_us = submit_us;
        self.counters.frames += 1;
        // Report this frame's image use to the shared owner (batched,
        // generation-checked) and drop locally stale entries before the
        // age-based eviction below. Submitted work stays valid through the
        // wgpu lifetime contract.
        self.sync_shared_textures();
        self.evict_unused_images();
        self.evict_unused_path_meshes();
        self.evict_unused_gradients();
        self.evict_offscreen_cache();
        let after = self.counters;
        Ok(RenderStats {
            draw_calls,
            rectangle_instances: rectangles as u32,
            glyph_instances: glyphs as u32,
            image_instances: images as u32,
            buffer_reallocated: rectangle_reallocated,
            glyph_buffer_reallocated: glyph_reallocated,
            image_buffer_reallocated: image_reallocated,
            presented: true,
            rounded_rect_instances: rounded as u32 + stencil_masks as u32,
            path_draws: path_draws_this_frame as u32,
            render_passes: 1
                + (after.offscreen_render_passes - before.offscreen_render_passes) as u32
                + (after.surface_present_conversion_passes
                    - before.surface_present_conversion_passes) as u32,
            path_triangles: (after.path_triangles - before.path_triangles) as u32,
            upload_bytes: after.buffer_upload_bytes - before.buffer_upload_bytes,
            texture_upload_bytes: after.texture_upload_bytes - before.texture_upload_bytes,
            queue_submissions: (after.queue_submissions - before.queue_submissions) as u32,
            prepare_us,
            encode_us,
            submit_us,
            pipelines_created: (after.total_pipeline_creations()
                - before.total_pipeline_creations()) as u32,
        })
    }
}
