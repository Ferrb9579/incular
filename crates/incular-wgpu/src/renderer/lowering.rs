use super::*;

impl WgpuRenderer {
    pub(super) fn lower_commands(
        &mut self,
        commands: &[PaintCommand],
        scale: f32,
        initial_transform: Transform,
        initial_clip: ClipState,
    ) -> Result<Vec<DrawBatch>, RendererError> {
        let mut batches = Vec::new();
        let mut transforms = vec![initial_transform];
        let mut clips = vec![initial_clip];
        let mut clip_masks: Vec<Option<ClipMask>> = vec![None];
        let mut command_index = 0_usize;
        while command_index < commands.len() {
            let command = &commands[command_index];
            let transform = *transforms.last().expect("transform stack");
            let translation = transform.translation_offset();
            match command {
                PaintCommand::Rect { rect, color } => {
                    let clip = *clips.last().expect("clip stack");
                    if transform.is_translation() {
                        append_rectangle(
                            &mut batches,
                            clip,
                            RectangleInstance {
                                rect: translated_rect(*rect, translation),
                                color: *color,
                            },
                        );
                    } else {
                        // The fast rectangle instance shader is axis-aligned.
                        // Preserve arbitrary retained affine geometry by
                        // routing it through the already-cached Lyon path
                        // pipeline instead of expanding to a bounding box.
                        self.append_path(
                            &mut batches,
                            clip,
                            &Arc::new(rect_path(*rect)),
                            PathMeshKind::Fill(FillRule::NonZero),
                            &Brush::Solid(*color),
                            PathPlacement { transform, scale },
                        );
                    }
                }
                // Common solid rounded primitives retain painter order even on
                // renderers that do not yet select the analytic pipeline.
                PaintCommand::RRect { rrect, brush } => append_rrect(
                    &mut batches,
                    *clips.last().expect("clip stack"),
                    self.ensure_gradient(brush),
                    rrect_instance(
                        *rrect,
                        brush,
                        transform,
                        scale,
                        self.target_width as f32,
                        self.target_height as f32,
                    ),
                ),
                PaintCommand::Border { rrect, border } => {
                    // Inside-aligned border: analytical shader discards the inset.
                    append_rrect(
                        &mut batches,
                        *clips.last().expect("clip stack"),
                        None,
                        border_instance(
                            *rrect,
                            *border,
                            transform,
                            scale,
                            self.target_width as f32,
                            self.target_height as f32,
                        ),
                    );
                }
                PaintCommand::FillPath {
                    path,
                    brush,
                    fill_rule,
                } => self.append_path(
                    &mut batches,
                    *clips.last().expect("clip stack"),
                    path,
                    PathMeshKind::Fill(*fill_rule),
                    brush,
                    PathPlacement { transform, scale },
                ),
                PaintCommand::StrokePath {
                    path,
                    brush,
                    stroke,
                } => self.append_path(
                    &mut batches,
                    *clips.last().expect("clip stack"),
                    path,
                    stroke_mesh_kind(*stroke),
                    brush,
                    PathPlacement { transform, scale },
                ),
                PaintCommand::GlyphRun { run, color } => {
                    if *clips.last().expect("clip stack") == ClipState::Empty {
                        command_index += 1;
                        continue;
                    }
                    for glyph in run.glyphs.iter() {
                        let Some(raster) =
                            self.shared.rasterize_glyph(run, glyph.id, f64::from(scale))
                        else {
                            self.counters.glyphs_skipped += 1;
                            continue;
                        };
                        if let Some(bitmap) = raster.bitmap.as_deref() {
                            self.upload_glyph(raster.entry, bitmap);
                        } else if raster.entry.width > 0 && raster.entry.height > 0 {
                            // The texture is device-shared, while this window
                            // still needs its own pipeline-compatible bind
                            // group for that atlas page.
                            self.ensure_atlas_page(raster.entry.page);
                        }
                        if raster.entry.width > 0 && raster.entry.height > 0 {
                            append_glyph(
                                &mut batches,
                                *clips.last().expect("clip stack"),
                                raster.entry.page,
                                glyph_instance(
                                    run,
                                    glyph.offset,
                                    raster.entry,
                                    *color,
                                    GlyphSurface {
                                        transform,
                                        width: self.target_width as f32,
                                        height: self.target_height as f32,
                                        scale,
                                    },
                                ),
                            );
                        }
                    }
                }
                PaintCommand::Image {
                    image,
                    source,
                    destination,
                    sampling,
                } => {
                    if *clips.last().expect("clip stack") == ClipState::Empty {
                        command_index += 1;
                        continue;
                    }
                    self.ensure_gpu_image(image)?;
                    append_image(
                        &mut batches,
                        *clips.last().expect("clip stack"),
                        image.id(),
                        *sampling,
                        image_instance(
                            *destination,
                            *source,
                            image,
                            self.target_width as f32,
                            self.target_height as f32,
                            scale,
                            transform,
                        ),
                    );
                }
                PaintCommand::PushOpacity {
                    layer,
                    alpha,
                    generation,
                    bounds,
                } => {
                    let end = find_opacity_end(commands, command_index)?;
                    let start = command_index + 1;
                    let parent_clip = *clips.last().expect("clip stack");
                    let normalized = normalize_opacity(*alpha);
                    command_index = end.saturating_add(1);
                    if normalized <= 0. {
                        self.counters.opacity_zero_fast_paths += 1;
                        continue;
                    }
                    if normalized >= 1. {
                        self.counters.opacity_one_fast_paths += 1;
                        let child = self.lower_commands(
                            &commands[start..end],
                            scale,
                            transform,
                            parent_clip,
                        )?;
                        batches.extend(child);
                        continue;
                    }
                    if let Some(batch) = self.lower_opacity_group(
                        &commands[start..end],
                        scale,
                        *layer,
                        normalized,
                        *generation,
                        *bounds,
                        parent_clip,
                        translation,
                    )? {
                        batches.push(batch);
                    }
                    continue;
                }
                PaintCommand::PopOpacity => {
                    return Err(RendererError::UnbalancedClipStack);
                }
                PaintCommand::PushBlur {
                    layer,
                    blur,
                    generation,
                    bounds,
                } => {
                    let end = find_effect_end(commands, command_index)?;
                    let start = command_index + 1;
                    let parent_clip = *clips.last().expect("clip stack");
                    command_index = end.saturating_add(1);
                    let blur = GaussianBlur::new(blur.sigma_x, blur.sigma_y);
                    if blur.sigma_x <= f32::EPSILON && blur.sigma_y <= f32::EPSILON {
                        let child = self.lower_commands(
                            &commands[start..end],
                            scale,
                            transform,
                            parent_clip,
                        )?;
                        batches.extend(child);
                    } else if let Some(batch) = self.lower_blur_group(
                        &commands[start..end],
                        scale,
                        *layer,
                        blur,
                        *generation,
                        *bounds,
                        parent_clip,
                        translation,
                    )? {
                        batches.push(batch);
                    }
                    continue;
                }
                PaintCommand::PushDropShadow {
                    layer,
                    shadow,
                    generation,
                    bounds,
                } => {
                    let end = find_effect_end(commands, command_index)?;
                    let start = command_index + 1;
                    let parent_clip = *clips.last().expect("clip stack");
                    command_index = end.saturating_add(1);
                    let shadow = DropShadowEffect::asymmetric(
                        shadow.offset,
                        shadow.sigma_x,
                        shadow.sigma_y,
                        shadow.color,
                    );
                    let child = self.lower_drop_shadow_group(
                        &commands[start..end],
                        scale,
                        *layer,
                        shadow,
                        *generation,
                        *bounds,
                        parent_clip,
                        translation,
                    )?;
                    batches.extend(child);
                    continue;
                }
                PaintCommand::PushColorFilter {
                    layer,
                    filter,
                    generation,
                    bounds,
                } => {
                    let end = find_effect_end(commands, command_index)?;
                    let start = command_index + 1;
                    let parent_clip = *clips.last().expect("clip stack");
                    command_index = end.saturating_add(1);
                    if filter.is_identity() {
                        let child = self.lower_commands(
                            &commands[start..end],
                            scale,
                            transform,
                            parent_clip,
                        )?;
                        batches.extend(child);
                    } else if let Some(inner_commands) =
                        commands.get(start..end).filter(|child| !child.is_empty())
                        && let Some(PaintCommand::PushColorFilter {
                            filter: inner_filter,
                            generation: inner_generation,
                            ..
                        }) = inner_commands.first()
                        && let Ok(inner_end) = find_effect_end(inner_commands, 0)
                        && inner_end + 1 == inner_commands.len()
                    {
                        // Nested color matrices are adjacent single-input
                        // stages.  Remove the inner boundary and compose
                        // `inner` followed by `outer`; no blur or shadow is
                        // crossed, so painter order remains exact.
                        let combined = inner_filter.then(*filter);
                        if let Some(batch) = self.lower_color_filter_group(
                            &inner_commands[1..inner_end],
                            scale,
                            *layer,
                            combined,
                            // The fused pass's source is the inner
                            // filter's input. Its generation deliberately
                            // excludes the inner matrix itself, so changing
                            // either adjacent matrix rerenders only this
                            // fused stage rather than its source texture.
                            *inner_generation,
                            *bounds,
                            parent_clip,
                            translation,
                        )? {
                            batches.push(batch);
                        }
                        self.counters.effect_stage_fusions += 1;
                    } else if let Some(batch) = self.lower_color_filter_group(
                        &commands[start..end],
                        scale,
                        *layer,
                        *filter,
                        *generation,
                        *bounds,
                        parent_clip,
                        translation,
                    )? {
                        batches.push(batch);
                    }
                    continue;
                }
                PaintCommand::PushBlend {
                    layer,
                    mode,
                    generation,
                    bounds,
                } => {
                    let end = find_effect_end(commands, command_index)?;
                    let start = command_index + 1;
                    let parent_clip = *clips.last().expect("clip stack");
                    command_index = end.saturating_add(1);
                    if *mode == BlendMode::SrcOver {
                        // SrcOver is the ordinary painter operation. Keep it
                        // on the direct path instead of isolating a source
                        // texture solely to apply the default blend mode.
                        let child = self.lower_commands(
                            &commands[start..end],
                            scale,
                            transform,
                            parent_clip,
                        )?;
                        batches.extend(child);
                    } else if let Some(batch) = self.lower_blend_group(
                        &commands[start..end],
                        scale,
                        *layer,
                        *mode,
                        *generation,
                        *bounds,
                        parent_clip,
                        translation,
                    )? {
                        batches.push(batch);
                    }
                    continue;
                }
                PaintCommand::PushShaderMask { .. } | PaintCommand::PushBackdropFilter { .. } => {
                    let end = find_effect_end(commands, command_index)?;
                    let start = command_index + 1;
                    let parent_clip = *clips.last().expect("clip stack");
                    command_index = end.saturating_add(1);
                    let child =
                        self.lower_commands(&commands[start..end], scale, transform, parent_clip)?;
                    batches.extend(child);
                    continue;
                }
                PaintCommand::PopEffect => {
                    return Err(RendererError::UnbalancedClipStack);
                }
                PaintCommand::PushTransform {
                    transform: local_transform,
                } => transforms.push(transform.then(*local_transform)),
                PaintCommand::PopTransform => {
                    if transforms.len() > 1 {
                        transforms.pop();
                    }
                }
                PaintCommand::PushClip { rect } => {
                    let next = ClipRect {
                        rect: transform.transform_rect_bbox(*rect),
                    };
                    let combined = match *clips.last().expect("clip stack") {
                        ClipState::Unbounded => ClipState::Rect(next),
                        ClipState::Rect(current) => intersect_rect(current.rect, next.rect)
                            .map(|rect| ClipState::Rect(ClipRect { rect }))
                            .unwrap_or(ClipState::Empty),
                        ClipState::Stencil { rect, depth } => match rect {
                            Some(current) => intersect_rect(current.rect, next.rect)
                                .map(|rect| ClipState::Stencil {
                                    rect: Some(ClipRect { rect }),
                                    depth,
                                })
                                .unwrap_or(ClipState::Empty),
                            None => ClipState::Stencil {
                                rect: Some(next),
                                depth,
                            },
                        },
                        ClipState::Empty => ClipState::Empty,
                    };
                    clips.push(combined);
                    clip_masks.push(None);
                    self.counters.clip_rect_pushes += 1;
                }
                PaintCommand::PushClipRRect { rrect } => {
                    let next = ClipRect {
                        rect: translated_rect(rrect.rect, translation),
                    };
                    let parent = *clips.last().expect("clip stack");
                    let scissor = intersect_clip_with_rect(parent, next);
                    let depth = stencil_depth(parent);
                    if depth == MAX_STENCIL_CLIP_DEPTH {
                        return Err(RendererError::StencilDepthOverflow);
                    }
                    let instance = rrect_instance(
                        *rrect,
                        &Brush::Solid(Color::TRANSPARENT),
                        transform,
                        scale,
                        self.target_width as f32,
                        self.target_height as f32,
                    );
                    if scissor == ClipState::Empty {
                        clips.push(ClipState::Empty);
                        clip_masks.push(None);
                    } else {
                        batches.push(DrawBatch::StencilRRect {
                            clip: parent,
                            instance,
                            increment: true,
                        });
                        clips.push(with_stencil_depth(scissor, depth + 1));
                        clip_masks.push(Some(ClipMask::RRect(instance)));
                        self.counters.stencil_depth_max =
                            self.counters.stencil_depth_max.max(u64::from(depth + 1));
                    }
                    self.counters.clip_rrect_pushes += 1;
                }
                PaintCommand::PushClipOval { rect } => {
                    let parent = *clips.last().expect("clip stack");
                    let depth = stencil_depth(parent);
                    let path = Arc::new(oval_path(*rect));
                    let key = PathMeshKey {
                        path: path.id(),
                        kind: PathMeshKind::Fill(FillRule::NonZero),
                    };
                    if self.ensure_path_mesh(key, &path) {
                        let instance = clip_path_instance(
                            transform,
                            scale,
                            self.target_width as f32,
                            self.target_height as f32,
                        );
                        batches.push(DrawBatch::StencilPath {
                            clip: parent,
                            key,
                            instance,
                            increment: true,
                        });
                        clips.push(with_stencil_depth(parent, depth + 1));
                        clip_masks.push(Some(ClipMask::Path { key, instance }));
                    } else {
                        clips.push(ClipState::Empty);
                        clip_masks.push(None);
                    }
                }
                PaintCommand::PushClipPath { path, fill_rule } => {
                    let parent = *clips.last().expect("clip stack");
                    let depth = stencil_depth(parent);
                    if depth == MAX_STENCIL_CLIP_DEPTH {
                        return Err(RendererError::StencilDepthOverflow);
                    }
                    let Some(bounds) = path.bounds() else {
                        clips.push(ClipState::Empty);
                        clip_masks.push(None);
                        self.counters.clip_path_pushes += 1;
                        command_index += 1;
                        continue;
                    };
                    let scissor = intersect_clip_with_rect(
                        parent,
                        ClipRect {
                            rect: translated_rect(bounds, translation),
                        },
                    );
                    let key = PathMeshKey {
                        path: path.id(),
                        kind: PathMeshKind::Fill(*fill_rule),
                    };
                    if scissor == ClipState::Empty || !self.ensure_path_mesh(key, path) {
                        clips.push(ClipState::Empty);
                        clip_masks.push(None);
                    } else {
                        let instance = clip_path_instance(
                            transform,
                            scale,
                            self.target_width as f32,
                            self.target_height as f32,
                        );
                        batches.push(DrawBatch::StencilPath {
                            clip: parent,
                            key,
                            instance,
                            increment: true,
                        });
                        clips.push(with_stencil_depth(scissor, depth + 1));
                        clip_masks.push(Some(ClipMask::Path { key, instance }));
                        self.counters.stencil_depth_max =
                            self.counters.stencil_depth_max.max(u64::from(depth + 1));
                    }
                    self.counters.clip_path_pushes += 1;
                }
                PaintCommand::PopClip => {
                    if clips.len() > 1 {
                        let child = clips.pop().expect("checked clip stack");
                        let parent = *clips.last().expect("parent clip stack");
                        if let Some(mask) = clip_masks.pop().expect("matching mask stack") {
                            debug_assert_eq!(stencil_depth(child), stencil_depth(parent) + 1);
                            match mask {
                                ClipMask::RRect(instance) => {
                                    batches.push(DrawBatch::StencilRRect {
                                        clip: child,
                                        instance,
                                        increment: false,
                                    })
                                }
                                ClipMask::Path { key, instance } => {
                                    batches.push(DrawBatch::StencilPath {
                                        clip: child,
                                        key,
                                        instance,
                                        increment: false,
                                    })
                                }
                            }
                        }
                    }
                    self.counters.clip_pops += 1;
                }
            }
            command_index += 1;
        }
        if clips.len() != 1 || clip_masks.len() != 1 {
            return Err(RendererError::UnbalancedClipStack);
        }
        Ok(batches)
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn lower_opacity_group(
        &mut self,
        commands: &[PaintCommand],
        scale: f32,
        layer: incular_painting::LayerId,
        alpha: f32,
        generation: u64,
        bounds: Rect,
        parent_clip: ClipState,
        translation: Offset,
    ) -> Result<Option<DrawBatch>, RendererError> {
        let active_width = self.target_width;
        let active_height = self.target_height;
        let active_origin = self.target_origin;
        if parent_clip == ClipState::Empty {
            return Ok(None);
        }
        if !bounds.origin.x.is_finite()
            || !bounds.origin.y.is_finite()
            || !bounds.size.width.is_finite()
            || !bounds.size.height.is_finite()
            || bounds.size.width <= 0.
            || bounds.size.height <= 0.
        {
            return Ok(None);
        }
        let left = ((bounds.origin.x - active_origin.x) * scale).floor();
        let top = ((bounds.origin.y - active_origin.y) * scale).floor();
        let right = ((bounds.origin.x + bounds.size.width - active_origin.x) * scale).ceil();
        let bottom = ((bounds.origin.y + bounds.size.height - active_origin.y) * scale).ceil();
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
        if width == 0 || height == 0 {
            return Ok(None);
        }
        let target_origin = Offset::new(
            active_origin.x + left / scale,
            active_origin.y + top / scale,
        );
        let frame = self.counters.frames;
        if let Some(entry) = self.offscreen_cache.get_mut(&layer)
            && entry.generation == generation
            && entry.width == width
            && entry.height == height
            && entry.scale_factor_bits == scale.to_bits()
            && entry.target.format == self.window_gpu.config.format
            && entry.device_generation == self.device_generation
        {
            entry.last_used_frame = frame;
            self.counters.offscreen_group_cache_hits += 1;
            self.counters.offscreen_texture_reuses += 1;
            return Ok(Some(DrawBatch::Offscreen {
                clip: parent_clip,
                layer,
                instance: composite_instance(
                    target_origin,
                    active_origin,
                    active_width,
                    active_height,
                    width,
                    height,
                    scale,
                    alpha,
                ),
            }));
        }
        if let Some(previous) = self.offscreen_cache.remove(&layer) {
            self.counters.offscreen_cached_bytes = self
                .counters
                .offscreen_cached_bytes
                .saturating_sub(previous.bytes);
            self.offscreen_target_pool.recycle(previous.target);
        }
        let saved_target = (self.target_width, self.target_height, self.target_origin);
        self.target_width = width;
        self.target_height = height;
        self.target_origin = target_origin;
        self.offscreen_nesting_depth += 1;
        self.counters.max_offscreen_nesting_depth = self
            .counters
            .max_offscreen_nesting_depth
            .max(self.offscreen_nesting_depth);
        let child_translation = translation - (target_origin - active_origin);
        let child_result = self.lower_commands(
            commands,
            scale,
            Transform::translation(child_translation),
            ClipState::Unbounded,
        );
        let child_batches = match child_result {
            Ok(batches) => batches,
            Err(error) => {
                self.target_width = saved_target.0;
                self.target_height = saved_target.1;
                self.target_origin = saved_target.2;
                self.offscreen_nesting_depth = self.offscreen_nesting_depth.saturating_sub(1);
                if let Some(previous) = self.offscreen_cache.remove(&layer) {
                    self.counters.offscreen_cached_bytes = self
                        .counters
                        .offscreen_cached_bytes
                        .saturating_sub(previous.bytes);
                    self.offscreen_target_pool.recycle(previous.target);
                }
                return Err(error);
            }
        };
        // Clip-only command streams (or an opacity wrapper around no child)
        // have no pixels to isolate.  Avoid allocating a color/stencil target,
        // submitting an empty pass, and emitting a composite draw for them.
        let child_has_content = child_batches.iter().any(|batch| match batch {
            DrawBatch::Rectangles { clip, instances } if *clip != ClipState::Empty => {
                !instances.is_empty()
            }
            DrawBatch::RoundedRects {
                clip, instances, ..
            } if *clip != ClipState::Empty => !instances.is_empty(),
            DrawBatch::Glyphs {
                clip, instances, ..
            } if *clip != ClipState::Empty => !instances.is_empty(),
            DrawBatch::Images {
                clip, instances, ..
            } if *clip != ClipState::Empty => !instances.is_empty(),
            DrawBatch::Path { clip, .. }
            | DrawBatch::Offscreen { clip, .. }
            | DrawBatch::Filtered { clip, .. }
            | DrawBatch::Shadow { clip, .. }
            | DrawBatch::Blend { clip, .. }
                if *clip != ClipState::Empty =>
            {
                true
            }
            // Stencil increment/decrement batches are only clip setup; they
            // become useful when paired with actual content above.
            DrawBatch::StencilRRect { .. } | DrawBatch::StencilPath { .. } => false,
            _ => false,
        });
        if !child_has_content {
            self.target_width = saved_target.0;
            self.target_height = saved_target.1;
            self.target_origin = saved_target.2;
            self.offscreen_nesting_depth = self.offscreen_nesting_depth.saturating_sub(1);
            return Ok(None);
        }
        let target = if let Some(target) =
            self.offscreen_target_pool
                .take(width, height, self.config.format, true)
        {
            self.counters.offscreen_texture_reuses += 1;
            target
        } else {
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("incular retained opacity color target"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.config.format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let color_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let (stencil, stencil_view) = create_stencil_attachment(&self.device, width, height);
            self.counters.offscreen_color_texture_creations += 1;
            self.counters.offscreen_stencil_texture_creations += 1;
            OffscreenTarget {
                _color: texture,
                color_view,
                _stencil: stencil,
                stencil_view,
                width,
                height,
                format: self.config.format,
                has_stencil: true,
            }
        };
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("incular retained opacity target bind group"),
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
        });
        let bytes = target.bytes();
        self.offscreen_cache.insert(
            layer,
            OffscreenCacheEntry {
                target,
                bind_group,
                width,
                height,
                scale_factor_bits: scale.to_bits(),
                generation,
                device_generation: self.device_generation,
                last_used_frame: frame,
                bytes,
            },
        );
        self.counters.offscreen_cached_bytes =
            self.counters.offscreen_cached_bytes.saturating_add(bytes);
        self.counters.offscreen_peak_cached_bytes = self
            .counters
            .offscreen_peak_cached_bytes
            .max(self.counters.offscreen_cached_bytes);
        self.counters.offscreen_group_cache_misses += 1;
        self.counters.offscreen_group_rerenders += 1;
        let child_rectangles = child_batches
            .iter()
            .map(|batch| match batch {
                DrawBatch::Rectangles { clip, instances } if *clip != ClipState::Empty => {
                    instances.len()
                }
                _ => 0,
            })
            .sum::<usize>();
        let child_glyphs = child_batches
            .iter()
            .map(|batch| match batch {
                DrawBatch::Glyphs {
                    clip, instances, ..
                } if *clip != ClipState::Empty => instances.len(),
                _ => 0,
            })
            .sum::<usize>();
        let child_images = child_batches
            .iter()
            .map(|batch| match batch {
                DrawBatch::Images {
                    clip, instances, ..
                } if *clip != ClipState::Empty => instances.len(),
                _ => 0,
            })
            .sum::<usize>();
        let child_rounded = child_batches
            .iter()
            .map(|batch| match batch {
                DrawBatch::RoundedRects {
                    clip, instances, ..
                } if *clip != ClipState::Empty => instances.len(),
                DrawBatch::StencilRRect { clip, .. } if *clip != ClipState::Empty => 1,
                _ => 0,
            })
            .sum::<usize>();
        let child_paths = child_batches
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
        let child_composites = child_batches
            .iter()
            .filter(|batch| {
                matches!(
                    batch,
                    DrawBatch::Offscreen { clip, .. }
                        | DrawBatch::Filtered { clip, .. }
                        | DrawBatch::Shadow { clip, .. }
                        if *clip != ClipState::Empty
                )
            })
            .count();
        self.ensure_rectangle_capacity(child_rectangles);
        self.ensure_glyph_capacity(child_glyphs);
        self.ensure_image_capacity(child_images);
        self.ensure_composite_capacity(child_composites);
        if child_rounded > self.rounded_rect_instance_capacity {
            self.rounded_rect_instance_capacity = child_rounded.next_power_of_two();
            self.rounded_rect_instances =
                create_rrect_buffer(&self.device, self.rounded_rect_instance_capacity);
        }
        if child_paths > self.path_instance_capacity {
            self.path_instance_capacity = child_paths.next_power_of_two();
            self.path_instances =
                create_path_instance_buffer(&self.device, self.path_instance_capacity);
        }
        self.upload_instance_data(&child_batches, scale, width, height);
        self.prepare_image_bind_groups(&child_batches);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("incular opacity group encoder"),
            });
        let (target_view, target_stencil) = {
            let entry = self
                .offscreen_cache
                .get(&layer)
                .expect("new opacity target in cache");
            (
                entry.target.color_view.clone(),
                entry.target.stencil_view.clone(),
            )
        };
        self.encode_batches(
            &mut encoder,
            &target_view,
            &target_stencil,
            &child_batches,
            width,
            height,
            scale,
            wgpu::Color::TRANSPARENT,
            None,
            false,
        );
        self.queue.submit(Some(encoder.finish()));
        self.counters.offscreen_render_passes += 1;
        self.target_width = saved_target.0;
        self.target_height = saved_target.1;
        self.target_origin = saved_target.2;
        self.offscreen_nesting_depth = self.offscreen_nesting_depth.saturating_sub(1);
        Ok(Some(DrawBatch::Offscreen {
            clip: parent_clip,
            layer,
            instance: composite_instance(
                target_origin,
                active_origin,
                active_width,
                active_height,
                width,
                height,
                scale,
                alpha,
            ),
        }))
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn lower_source_group(
        &mut self,
        commands: &[PaintCommand],
        scale: f32,
        layer: incular_painting::LayerId,
        generation: u64,
        bounds: Rect,
        parent_clip: ClipState,
        translation: Offset,
    ) -> Result<Option<CachedSource>, RendererError> {
        let active_origin = self.target_origin;
        let base_source = !commands_have_effects(commands);
        if parent_clip == ClipState::Empty
            || !bounds.origin.x.is_finite()
            || !bounds.origin.y.is_finite()
            || !bounds.size.width.is_finite()
            || !bounds.size.height.is_finite()
            || bounds.size.width <= 0.
            || bounds.size.height <= 0.
        {
            return Ok(None);
        }
        let left = ((bounds.origin.x - active_origin.x) * scale).floor();
        let top = ((bounds.origin.y - active_origin.y) * scale).floor();
        let right = ((bounds.origin.x + bounds.size.width - active_origin.x) * scale).ceil();
        let bottom = ((bounds.origin.y + bounds.size.height - active_origin.y) * scale).ceil();
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
        if width == 0 || height == 0 {
            return Ok(None);
        }
        let target_origin = Offset::new(
            active_origin.x + left / scale,
            active_origin.y + top / scale,
        );
        let frame = self.counters.frames;
        if let Some(entry) = self.offscreen_cache.get_mut(&layer)
            && entry.generation == generation
            && entry.width == width
            && entry.height == height
            && entry.scale_factor_bits == scale.to_bits()
            && entry.target.format == self.window_gpu.config.format
            && entry.device_generation == self.device_generation
        {
            entry.last_used_frame = frame;
            if base_source {
                self.counters.effect_source_cache_hits += 1;
            } else {
                self.counters.effect_stage_cache_hits += 1;
            }
            self.counters.offscreen_texture_reuses += 1;
            return Ok(Some(CachedSource {
                origin: target_origin,
                width,
                height,
            }));
        }
        if let Some(previous) = self.offscreen_cache.remove(&layer) {
            self.counters.offscreen_cached_bytes = self
                .counters
                .offscreen_cached_bytes
                .saturating_sub(previous.bytes);
            self.offscreen_target_pool.recycle(previous.target);
        }
        let saved_target = (self.target_width, self.target_height, self.target_origin);
        self.target_width = width;
        self.target_height = height;
        self.target_origin = target_origin;
        self.offscreen_nesting_depth += 1;
        self.counters.max_offscreen_nesting_depth = self
            .counters
            .max_offscreen_nesting_depth
            .max(self.offscreen_nesting_depth);
        let child_translation = translation - (target_origin - active_origin);
        let child_batches = match self.lower_commands(
            commands,
            scale,
            Transform::translation(child_translation),
            ClipState::Unbounded,
        ) {
            Ok(batches) => batches,
            Err(error) => {
                self.target_width = saved_target.0;
                self.target_height = saved_target.1;
                self.target_origin = saved_target.2;
                self.offscreen_nesting_depth = self.offscreen_nesting_depth.saturating_sub(1);
                return Err(error);
            }
        };
        if !batches_have_content(&child_batches) {
            self.target_width = saved_target.0;
            self.target_height = saved_target.1;
            self.target_origin = saved_target.2;
            self.offscreen_nesting_depth = self.offscreen_nesting_depth.saturating_sub(1);
            return Ok(None);
        }
        let target = if let Some(target) =
            self.offscreen_target_pool
                .take(width, height, self.config.format, true)
        {
            self.counters.offscreen_texture_reuses += 1;
            target
        } else {
            self.create_offscreen_target(width, height, "incular retained effect source")
        };
        let bind_group =
            self.create_composite_bind_group(&target, "incular retained effect source bind group");
        let bytes = target.bytes();
        self.offscreen_cache.insert(
            layer,
            OffscreenCacheEntry {
                target,
                bind_group,
                width,
                height,
                scale_factor_bits: scale.to_bits(),
                generation,
                device_generation: self.device_generation,
                last_used_frame: frame,
                bytes,
            },
        );
        self.counters.offscreen_cached_bytes =
            self.counters.offscreen_cached_bytes.saturating_add(bytes);
        self.counters.offscreen_peak_cached_bytes = self
            .counters
            .offscreen_peak_cached_bytes
            .max(self.counters.offscreen_cached_bytes);
        if base_source {
            self.counters.effect_source_cache_misses += 1;
            self.counters.effect_source_rerenders += 1;
        } else {
            self.counters.effect_stage_cache_misses += 1;
            self.counters.effect_stage_rerenders += 1;
        }
        self.render_cached_batches(
            &child_batches,
            scale,
            width,
            height,
            layer,
            "incular effect source encoder",
        );
        self.target_width = saved_target.0;
        self.target_height = saved_target.1;
        self.target_origin = saved_target.2;
        self.offscreen_nesting_depth = self.offscreen_nesting_depth.saturating_sub(1);
        Ok(Some(CachedSource {
            origin: target_origin,
            width,
            height,
        }))
    }
}
