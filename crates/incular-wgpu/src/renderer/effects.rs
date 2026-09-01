use super::*;

impl WgpuRenderer {
    pub(super) fn create_offscreen_target(
        &mut self,
        width: u32,
        height: u32,
        label: &'static str,
    ) -> OffscreenTarget {
        let target = create_scene_target(&self.device, self.config.format, width, height, label);
        self.counters.offscreen_color_texture_creations += 1;
        self.counters.offscreen_stencil_texture_creations += 1;
        target
    }

    pub(super) fn create_composite_bind_group(
        &self,
        target: &OffscreenTarget,
        label: &'static str,
    ) -> wgpu::BindGroup {
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(label),
            layout: &self.composite_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&target.color_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.composite_sampler),
                },
            ],
        })
    }

    pub(super) fn render_cached_batches(
        &mut self,
        batches: &[DrawBatch],
        scale: f32,
        width: u32,
        height: u32,
        layer: incular_painting::LayerId,
        label: &'static str,
    ) {
        if batches.iter().any(|batch| {
            matches!(
                batch,
                DrawBatch::Blend {
                    mode,
                    ..
                } if mode.requires_destination_read()
            )
        }) {
            // Nested destination reads use the same ping-pong graph as the
            // presentation path, then copy the completed result into the
            // retained source target.  The copy is outside the blend pass,
            // so no texture is sampled while it is being rendered.
            let Some(output) = self
                .offscreen_cache
                .get(&layer)
                .map(|entry| entry.target.clone())
            else {
                return;
            };
            self.ensure_destination_targets(width, height);
            let mut targets = self
                .destination_targets
                .take()
                .expect("destination targets after ensure");
            let (current_first, _, _, passes) = self.render_destination_batches(
                batches,
                scale,
                width,
                height,
                &mut targets,
                label,
                wgpu::Color::TRANSPARENT,
            );
            let final_target = if current_first {
                &targets.first
            } else {
                &targets.second
            };
            self.copy_color_texture(
                final_target,
                &output,
                width,
                height,
                "incular nested destination composition copy",
            );
            self.destination_targets = Some(targets);
            self.counters.offscreen_render_passes += u64::from(passes);
            return;
        }
        let rectangles = count_rectangles(batches);
        let glyphs = count_glyphs(batches);
        let images = count_images(batches);
        let rounded = count_rounded(batches);
        let paths = count_paths(batches);
        let composites = count_composites(batches);
        self.ensure_rectangle_capacity(rectangles);
        self.ensure_glyph_capacity(glyphs);
        self.ensure_image_capacity(images);
        self.ensure_composite_capacity(composites);
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
        self.upload_instance_data(batches, scale, width, height);
        self.prepare_image_bind_groups(batches);
        let Some(entry) = self.offscreen_cache.get(&layer) else {
            return;
        };
        let color_view = entry.target.color_view.clone();
        let stencil_view = entry.target.stencil_view.clone();
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some(label) });
        self.encode_batches(
            &mut encoder,
            &color_view,
            &stencil_view,
            batches,
            width,
            height,
            scale,
            wgpu::Color::TRANSPARENT,
            None,
            false,
        );
        self.queue.submit(Some(encoder.finish()));
        self.counters.offscreen_render_passes += 1;
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn lower_blur_group(
        &mut self,
        commands: &[PaintCommand],
        scale: f32,
        layer: incular_painting::LayerId,
        blur: GaussianBlur,
        generation: u64,
        bounds: Rect,
        parent_clip: ClipState,
        translation: Offset,
    ) -> Result<Option<DrawBatch>, RendererError> {
        let Some(source) = self.lower_source_group(
            commands,
            scale,
            layer,
            generation,
            bounds,
            parent_clip,
            translation,
        )?
        else {
            return Ok(None);
        };
        let Some(effect) = self.render_blur_from_source(
            scale,
            layer,
            generation,
            bounds,
            GaussianBlur::new(blur.sigma_x, blur.sigma_y),
            source,
            false,
        )?
        else {
            return Ok(None);
        };
        self.counters.effect_chain_compilations += 1;
        let active_origin = self.target_origin;
        Ok(Some(DrawBatch::Filtered {
            clip: parent_clip,
            layer,
            instance: composite_instance(
                effect.origin,
                active_origin,
                self.target_width,
                self.target_height,
                effect.width,
                effect.height,
                scale,
                1.,
            ),
        }))
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn lower_color_filter_group(
        &mut self,
        commands: &[PaintCommand],
        scale: f32,
        layer: incular_painting::LayerId,
        filter: ColorFilter,
        generation: u64,
        bounds: Rect,
        parent_clip: ClipState,
        translation: Offset,
    ) -> Result<Option<DrawBatch>, RendererError> {
        let Some(source) = self.lower_source_group(
            commands,
            scale,
            layer,
            generation,
            bounds,
            parent_clip,
            translation,
        )?
        else {
            return Ok(None);
        };
        let Some(effect) =
            self.render_color_filter_from_source(scale, layer, generation, filter, source)?
        else {
            return Ok(None);
        };
        self.counters.effect_chain_compilations += 1;
        Ok(Some(DrawBatch::Filtered {
            clip: parent_clip,
            layer,
            instance: composite_instance(
                effect.origin,
                self.target_origin,
                self.target_width,
                self.target_height,
                effect.width,
                effect.height,
                scale,
                1.,
            ),
        }))
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn lower_blend_group(
        &mut self,
        commands: &[PaintCommand],
        scale: f32,
        layer: incular_painting::LayerId,
        mode: BlendMode,
        generation: u64,
        bounds: Rect,
        parent_clip: ClipState,
        translation: Offset,
    ) -> Result<Option<DrawBatch>, RendererError> {
        if parent_clip == ClipState::Empty {
            return Ok(None);
        }
        let Some(source) = self.lower_source_group(
            commands,
            scale,
            layer,
            generation,
            bounds,
            parent_clip,
            translation,
        )?
        else {
            return Ok(None);
        };
        self.counters.effect_chain_compilations += 1;
        Ok(Some(DrawBatch::Blend {
            clip: parent_clip,
            layer,
            mode,
            instance: blend_instance(
                source.origin,
                self.target_origin,
                self.target_width,
                self.target_height,
                source.width,
                source.height,
                scale,
                mode,
            ),
        }))
    }

    pub(super) fn render_color_filter_from_source(
        &mut self,
        scale: f32,
        layer: incular_painting::LayerId,
        generation: u64,
        filter: ColorFilter,
        source: CachedSource,
    ) -> Result<Option<CachedEffect>, RendererError> {
        let frame = self.counters.frames;
        let matrix_bits = filter.to_matrix().map(f32::to_bits);
        if let Some(entry) = self.effect_cache.get_mut(&layer)
            && entry.source_generation == generation
            && entry.width == source.width
            && entry.height == source.height
            && entry.scale_factor_bits == scale.to_bits()
            && entry.matrix_bits == matrix_bits
            && entry.sigma_x_bits == 0
            && entry.sigma_y_bits == 0
            && entry.target.format == self.window_gpu.config.format
            && entry.device_generation == self.device_generation
        {
            entry.last_used_frame = frame;
            self.counters.effect_stage_cache_hits += 1;
            return Ok(Some(CachedEffect {
                origin: source.origin,
                width: source.width,
                height: source.height,
            }));
        }
        if let Some(previous) = self.effect_cache.remove(&layer) {
            self.remove_effect_cache_bytes(previous.bytes);
            self.offscreen_target_pool.recycle(previous.target);
        }
        let target = if let Some(target) =
            self.offscreen_target_pool
                .take(source.width, source.height, self.config.format, true)
        {
            self.counters.effect_texture_pool_hits += 1;
            target
        } else {
            self.counters.effect_texture_pool_misses += 1;
            self.counters.effect_texture_creations += 1;
            self.create_offscreen_target(source.width, source.height, "incular color matrix result")
        };
        let source_view = self
            .offscreen_cache
            .get(&layer)
            .expect("source cache for color filter")
            .target
            .color_view
            .clone();
        self.run_color_matrix_pass(&source_view, &target.color_view, filter);
        let bind_group =
            self.create_composite_bind_group(&target, "incular color matrix bind group");
        let bytes = target.bytes();
        self.effect_cache.insert(
            layer,
            EffectCacheEntry {
                target,
                bind_group,
                width: source.width,
                height: source.height,
                scale_factor_bits: scale.to_bits(),
                source_generation: generation,
                sigma_x_bits: 0,
                sigma_y_bits: 0,
                matrix_bits,
                device_generation: self.device_generation,
                last_used_frame: frame,
                bytes,
                downsample_factor: 1,
            },
        );
        self.add_effect_cache_bytes(bytes);
        self.counters.effect_stage_cache_misses += 1;
        self.counters.effect_stage_rerenders += 1;
        self.counters.color_matrix_passes += 1;
        Ok(Some(CachedEffect {
            origin: source.origin,
            width: source.width,
            height: source.height,
        }))
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn lower_drop_shadow_group(
        &mut self,
        commands: &[PaintCommand],
        scale: f32,
        layer: incular_painting::LayerId,
        shadow: DropShadowEffect,
        generation: u64,
        bounds: Rect,
        parent_clip: ClipState,
        translation: Offset,
    ) -> Result<Vec<DrawBatch>, RendererError> {
        let Some(source) = self.lower_source_group(
            commands,
            scale,
            layer,
            generation,
            bounds,
            parent_clip,
            translation,
        )?
        else {
            return Ok(Vec::new());
        };
        self.counters.effect_chain_compilations += 1;
        let active_origin = self.target_origin;
        let blur = GaussianBlur::new(shadow.sigma_x, shadow.sigma_y);
        let shadow_batch = if blur.sigma_x <= f32::EPSILON && blur.sigma_y <= f32::EPSILON {
            DrawBatch::Shadow {
                clip: parent_clip,
                layer,
                instance: composite_instance_with_color(
                    source.origin + shadow.offset,
                    active_origin,
                    self.target_width,
                    self.target_height,
                    source.width,
                    source.height,
                    scale,
                    1.,
                    shadow.color,
                    true,
                ),
            }
        } else {
            let Some(effect) =
                self.render_blur_from_source(scale, layer, generation, bounds, blur, source, true)?
            else {
                return Ok(Vec::new());
            };
            DrawBatch::Shadow {
                clip: parent_clip,
                layer,
                instance: composite_instance_with_color(
                    effect.origin + shadow.offset,
                    active_origin,
                    self.target_width,
                    self.target_height,
                    effect.width,
                    effect.height,
                    scale,
                    1.,
                    shadow.color,
                    true,
                ),
            }
        };
        Ok(vec![
            shadow_batch,
            DrawBatch::Offscreen {
                clip: parent_clip,
                layer,
                instance: composite_instance(
                    source.origin,
                    active_origin,
                    self.target_width,
                    self.target_height,
                    source.width,
                    source.height,
                    scale,
                    1.,
                ),
            },
        ])
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn render_blur_from_source(
        &mut self,
        scale: f32,
        layer: incular_painting::LayerId,
        generation: u64,
        source_bounds: Rect,
        blur: GaussianBlur,
        source: CachedSource,
        for_shadow: bool,
    ) -> Result<Option<CachedEffect>, RendererError> {
        let active_origin = self.target_origin;
        let effect_bounds = blur_bounds(source_bounds, blur.sigma_x, blur.sigma_y);
        let left = ((effect_bounds.origin.x - active_origin.x) * scale).floor();
        let top = ((effect_bounds.origin.y - active_origin.y) * scale).floor();
        let right =
            ((effect_bounds.origin.x + effect_bounds.size.width - active_origin.x) * scale).ceil();
        let bottom =
            ((effect_bounds.origin.y + effect_bounds.size.height - active_origin.y) * scale).ceil();
        let width_f = right - left;
        let height_f = bottom - top;
        let limit = self.device.limits().max_texture_dimension_2d;
        if width_f <= 0. || height_f <= 0. {
            return Ok(None);
        }
        if width_f > limit as f32 || height_f > limit as f32 {
            return Err(RendererError::OffscreenTargetTooLarge {
                width: width_f.min(u32::MAX as f32) as u32,
                height: height_f.min(u32::MAX as f32) as u32,
                limit,
            });
        }
        let width = width_f as u32;
        let height = height_f as u32;
        let effect_origin = Offset::new(
            active_origin.x + left / scale,
            active_origin.y + top / scale,
        );
        let sigma_x_physical = normalize_sigma(blur.sigma_x * scale);
        let sigma_y_physical = normalize_sigma(blur.sigma_y * scale);
        let downsample_factor =
            choose_blur_downsample_factor(sigma_x_physical.max(sigma_y_physical));
        let frame = self.counters.frames;
        if let Some(entry) = self.effect_cache.get_mut(&layer)
            && entry.source_generation == generation
            && entry.width == width
            && entry.height == height
            && entry.scale_factor_bits == scale.to_bits()
            && entry.sigma_x_bits == sigma_x_physical.to_bits()
            && entry.sigma_y_bits == sigma_y_physical.to_bits()
            && entry.downsample_factor == downsample_factor
            && entry.target.format == self.window_gpu.config.format
            && entry.device_generation == self.device_generation
        {
            entry.last_used_frame = frame;
            self.counters.blur_cache_hits += 1;
            self.counters.effect_stage_cache_hits += 1;
            if for_shadow {
                self.counters.drop_shadow_blur_reuses += 1;
            }
            return Ok(Some(CachedEffect {
                origin: effect_origin,
                width,
                height,
            }));
        }
        if let Some(previous) = self.effect_cache.remove(&layer) {
            self.remove_effect_cache_bytes(previous.bytes);
            self.offscreen_target_pool.recycle(previous.target);
        }
        let final_target = if let Some(target) =
            self.offscreen_target_pool
                .take(width, height, self.config.format, true)
        {
            self.counters.effect_texture_pool_hits += 1;
            target
        } else {
            self.counters.effect_texture_pool_misses += 1;
            self.counters.effect_texture_creations += 1;
            self.create_offscreen_target(width, height, "incular retained blur result")
        };
        let padding = [
            ((source.origin.x - effect_origin.x) * scale).max(0.),
            ((source.origin.y - effect_origin.y) * scale).max(0.),
        ];
        let source_size = [source.width as f32, source.height as f32];
        let output_size = [width as f32, height as f32];
        let mut temps = Vec::new();
        let factor = downsample_factor as f32;
        let low_width = ((width as f32) / factor).ceil().max(1.) as u32;
        let low_height = ((height as f32) / factor).ceil().max(1.) as u32;
        let (horizontal, vertical) = if downsample_factor == 1 {
            let horizontal = self.take_effect_temp(width, height);
            (horizontal, None)
        } else {
            let downsample = self.take_effect_temp(low_width, low_height);
            let horizontal = self.take_effect_temp(low_width, low_height);
            let vertical = self.take_effect_temp(low_width, low_height);
            temps.push(downsample);
            (horizontal, Some(vertical))
        };
        let (horizontal, vertical_for_large) = (horizontal, vertical);
        let source_view = self
            .offscreen_cache
            .get(&layer)
            .expect("source cache for effect")
            .target
            .color_view
            .clone();
        if downsample_factor > 1 {
            let downsample = &temps[0];
            self.run_effect_pass(
                &source_view,
                &downsample.color_view,
                [padding[0], padding[1]],
                source_size,
                [low_width as f32, low_height as f32],
                [factor, factor],
                None,
            );
            self.counters.blur_downsample_passes += 1;
        }
        let blur_source_view = if downsample_factor > 1 {
            temps[0].color_view.clone()
        } else {
            source_view.clone()
        };
        let blur_source_size = if downsample_factor > 1 {
            [low_width as f32, low_height as f32]
        } else {
            source_size
        };
        let blur_output_size = if downsample_factor > 1 {
            [low_width as f32, low_height as f32]
        } else {
            output_size
        };
        let kernel_x = self.blur_kernel(sigma_x_physical / factor, downsample_factor);
        let kernel_y = self.blur_kernel(sigma_y_physical / factor, downsample_factor);
        self.run_effect_pass(
            &blur_source_view,
            &horizontal.color_view,
            if downsample_factor > 1 {
                [0., 0.]
            } else {
                padding
            },
            blur_source_size,
            blur_output_size,
            [1., 0.],
            Some(&kernel_x),
        );
        self.counters.blur_horizontal_passes += 1;
        let vertical_target = vertical_for_large.as_ref().unwrap_or(&final_target);
        self.run_effect_pass(
            &horizontal.color_view,
            &vertical_target.color_view,
            [0., 0.],
            blur_output_size,
            blur_output_size,
            [0., 1.],
            Some(&kernel_y),
        );
        self.counters.blur_vertical_passes += 1;
        if downsample_factor > 1 {
            self.run_effect_pass(
                &vertical_target.color_view,
                &final_target.color_view,
                [0., 0.],
                [low_width as f32, low_height as f32],
                output_size,
                [1. / factor, 1. / factor],
                None,
            );
            self.counters.blur_upsample_passes += 1;
        }
        self.recycle_effect_temp(horizontal);
        if let Some(vertical) = vertical_for_large {
            self.recycle_effect_temp(vertical);
        }
        for temp in temps {
            self.recycle_effect_temp(temp);
        }
        self.counters.blur_cache_misses += 1;
        self.counters.effect_stage_cache_misses += 1;
        self.counters.effect_stage_rerenders += 1;
        let bind_group =
            self.create_composite_bind_group(&final_target, "incular retained blur bind group");
        let bytes = final_target.bytes();
        self.effect_cache.insert(
            layer,
            EffectCacheEntry {
                target: final_target,
                bind_group,
                width,
                height,
                scale_factor_bits: scale.to_bits(),
                source_generation: generation,
                sigma_x_bits: sigma_x_physical.to_bits(),
                sigma_y_bits: sigma_y_physical.to_bits(),
                matrix_bits: [0; 20],
                device_generation: self.device_generation,
                last_used_frame: frame,
                bytes,
                downsample_factor,
            },
        );
        self.add_effect_cache_bytes(bytes);
        Ok(Some(CachedEffect {
            origin: effect_origin,
            width,
            height,
        }))
    }

    pub(super) fn take_effect_temp(&mut self, width: u32, height: u32) -> OffscreenTarget {
        if let Some(target) =
            self.offscreen_target_pool
                .take(width, height, self.config.format, true)
        {
            self.counters.effect_texture_pool_hits += 1;
            target
        } else {
            self.counters.effect_texture_pool_misses += 1;
            self.counters.effect_texture_creations += 1;
            self.create_offscreen_target(width, height, "incular transient blur target")
        }
    }
    pub(super) fn recycle_effect_temp(&mut self, target: OffscreenTarget) {
        self.offscreen_target_pool.recycle(target);
    }
    pub(super) fn run_color_matrix_pass(
        &mut self,
        source: &wgpu::TextureView,
        destination: &wgpu::TextureView,
        filter: ColorFilter,
    ) {
        let params = GpuColorMatrixParams {
            matrix: filter.to_matrix(),
        };
        self.queue
            .write_buffer(&self.color_matrix_params, 0, bytemuck::bytes_of(&params));
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("incular color matrix pass bind group"),
            layout: &self.color_matrix_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(source),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.color_matrix_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.color_matrix_params.as_entire_binding(),
                },
            ],
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("incular color matrix pass encoder"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("incular color matrix pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: destination,
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
            pass.set_pipeline(&self.color_matrix_pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.set_vertex_buffer(0, self.mesh.slice(..));
            pass.draw(0..6, 0..1);
        }
        self.queue.submit(Some(encoder.finish()));
    }
    pub(super) fn add_effect_cache_bytes(&mut self, bytes: usize) {
        self.counters.offscreen_cached_bytes =
            self.counters.offscreen_cached_bytes.saturating_add(bytes);
        self.counters.effect_cached_bytes = self.counters.effect_cached_bytes.saturating_add(bytes);
        self.counters.effect_chain_cached_bytes = self.counters.effect_cached_bytes;
        self.counters.effect_peak_bytes = self
            .counters
            .effect_peak_bytes
            .max(self.counters.effect_cached_bytes);
        self.counters.offscreen_peak_cached_bytes = self
            .counters
            .offscreen_peak_cached_bytes
            .max(self.counters.offscreen_cached_bytes);
    }
    pub(super) fn remove_effect_cache_bytes(&mut self, bytes: usize) {
        self.counters.offscreen_cached_bytes =
            self.counters.offscreen_cached_bytes.saturating_sub(bytes);
        self.counters.effect_cached_bytes = self.counters.effect_cached_bytes.saturating_sub(bytes);
        self.counters.effect_chain_cached_bytes = self.counters.effect_cached_bytes;
    }
    pub(super) fn blur_kernel(&mut self, sigma: f32, downsample_factor: u32) -> BlurKernel {
        let sigma = quantize_blur_sigma(sigma);
        let key = BlurKernelKey {
            sigma_bits: sigma.to_bits(),
            downsample_factor,
        };
        if let Some(kernel) = self.blur_kernel_cache.get(&key) {
            self.counters.blur_kernel_cache_hits += 1;
            return kernel.clone();
        }
        let full = gaussian_kernel_weights(sigma);
        let radius = ((full.len().saturating_sub(1)) / 2).min(MAX_BLUR_RADIUS) as u32;
        let center = full.len() / 2;
        let mut weights = [0.; BLUR_WEIGHT_SLOTS];
        let radius = radius as usize;
        weights[..=radius].copy_from_slice(&full[center..=center + radius]);
        let kernel = BlurKernel {
            radius: radius as u32,
            weights,
        };
        self.blur_kernel_cache.insert(key, kernel.clone());
        self.counters.blur_kernel_cache_misses += 1;
        self.counters.blur_kernel_uploads += 1;
        kernel
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn run_effect_pass(
        &mut self,
        source: &wgpu::TextureView,
        destination: &wgpu::TextureView,
        source_origin: [f32; 2],
        source_size: [f32; 2],
        output_size: [f32; 2],
        direction: [f32; 2],
        kernel: Option<&BlurKernel>,
    ) {
        let mut weights = [0.; BLUR_WEIGHT_SLOTS];
        let radius = kernel.map_or(0, |kernel| {
            weights = kernel.weights;
            kernel.radius
        });
        let params = GpuBlurParams {
            source_origin,
            source_size,
            output_size,
            direction,
            radius,
            mode: u32::from(kernel.is_none()),
            _padding: [0; 2],
            weights,
        };
        self.queue
            .write_buffer(&self.blur_params, 0, bytemuck::bytes_of(&params));
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("incular transient effect bind group"),
            layout: &self.blur_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(source),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.blur_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.blur_params.as_entire_binding(),
                },
            ],
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("incular transient effect pass"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("incular gaussian pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: destination,
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
            pass.set_pipeline(if kernel.is_some() {
                &self.blur_pipeline
            } else {
                &self.resample_pipeline
            });
            pass.set_bind_group(0, &bind_group, &[]);
            pass.set_vertex_buffer(0, self.mesh.slice(..));
            pass.draw(0..6, 0..1);
        }
        self.queue.submit(Some(encoder.finish()));
        self.counters.offscreen_render_passes += 1;
    }
}
