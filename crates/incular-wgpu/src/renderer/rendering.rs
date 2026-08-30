use super::*;

impl WgpuRenderer {
    pub(super) fn ensure_rectangle_capacity(&mut self, required: usize) -> bool {
        if required <= self.instance_capacity {
            return false;
        }
        self.instance_capacity = required.next_power_of_two();
        self.instances = create_instance_buffer(&self.device, self.instance_capacity);
        self.counters.buffer_reallocations += 1;
        true
    }
    pub(super) fn ensure_glyph_capacity(&mut self, required: usize) -> bool {
        if required <= self.glyph_instance_capacity {
            return false;
        }
        self.glyph_instance_capacity = required.next_power_of_two();
        self.glyph_instances = create_glyph_buffer(&self.device, self.glyph_instance_capacity);
        self.counters.glyph_instance_buffer_reallocations += 1;
        true
    }
    pub(super) fn ensure_image_capacity(&mut self, required: usize) -> bool {
        if required <= self.image_instance_capacity {
            return false;
        }
        self.image_instance_capacity = required.next_power_of_two();
        self.image_instances = create_image_buffer(&self.device, self.image_instance_capacity);
        true
    }
    pub(super) fn ensure_composite_capacity(&mut self, required: usize) -> bool {
        if required <= self.composite_instance_capacity {
            return false;
        }
        self.composite_instance_capacity = required.next_power_of_two();
        self.composite_instances =
            create_composite_buffer(&self.device, self.composite_instance_capacity);
        true
    }
    /// One counted queue write for per-frame instance data.
    pub(super) fn write_counted(&mut self, buffer: wgpu::Buffer, offset: u64, data: &[u8]) {
        self.counters.buffer_uploads += 1;
        self.counters.buffer_upload_bytes += data.len() as u64;
        self.queue.write_buffer(&buffer, offset, data);
    }
    pub(super) fn upload_instance_data(
        &mut self,
        batches: &[DrawBatch],
        scale: f32,
        width: u32,
        height: u32,
    ) {
        let mut rectangle_offset = 0_u64;
        let mut glyph_offset = 0_u64;
        let mut image_offset = 0_u64;
        let mut rounded_offset = 0_u64;
        let mut path_offset = 0_u64;
        let mut composite_offset = 0_u64;
        for batch in batches {
            match batch {
                DrawBatch::Rectangles { clip, instances }
                    if *clip != ClipState::Empty && !instances.is_empty() =>
                {
                    let gpu: Vec<_> = instances
                        .iter()
                        .map(|instance| {
                            logical_instance(*instance, width as f32, height as f32, scale)
                        })
                        .collect();
                    self.write_counted(
                        self.instances.clone(),
                        rectangle_offset,
                        bytemuck::cast_slice(&gpu),
                    );
                    rectangle_offset += (gpu.len() * std::mem::size_of::<GpuInstance>()) as u64;
                }
                DrawBatch::Glyphs {
                    clip, instances, ..
                } if *clip != ClipState::Empty && !instances.is_empty() => {
                    self.write_counted(
                        self.glyph_instances.clone(),
                        glyph_offset,
                        bytemuck::cast_slice(instances),
                    );
                    glyph_offset +=
                        (instances.len() * std::mem::size_of::<GpuGlyphInstance>()) as u64;
                }
                DrawBatch::Images {
                    clip, instances, ..
                } if *clip != ClipState::Empty && !instances.is_empty() => {
                    self.write_counted(
                        self.image_instances.clone(),
                        image_offset,
                        bytemuck::cast_slice(instances),
                    );
                    image_offset +=
                        (instances.len() * std::mem::size_of::<GpuImageInstance>()) as u64;
                }
                DrawBatch::RoundedRects {
                    clip, instances, ..
                } if *clip != ClipState::Empty && !instances.is_empty() => {
                    self.write_counted(
                        self.rounded_rect_instances.clone(),
                        rounded_offset,
                        bytemuck::cast_slice(instances),
                    );
                    rounded_offset +=
                        (instances.len() * std::mem::size_of::<GpuRRectInstance>()) as u64;
                }
                DrawBatch::Path { clip, instance, .. } if *clip != ClipState::Empty => {
                    self.write_counted(
                        self.path_instances.clone(),
                        path_offset,
                        bytemuck::bytes_of(instance),
                    );
                    path_offset += std::mem::size_of::<GpuPathInstance>() as u64;
                }
                DrawBatch::StencilRRect { clip, instance, .. } if *clip != ClipState::Empty => {
                    self.write_counted(
                        self.rounded_rect_instances.clone(),
                        rounded_offset,
                        bytemuck::bytes_of(instance),
                    );
                    rounded_offset += std::mem::size_of::<GpuRRectInstance>() as u64;
                }
                DrawBatch::StencilPath { clip, instance, .. } if *clip != ClipState::Empty => {
                    self.write_counted(
                        self.path_instances.clone(),
                        path_offset,
                        bytemuck::bytes_of(instance),
                    );
                    path_offset += std::mem::size_of::<GpuPathInstance>() as u64;
                }
                DrawBatch::Offscreen { clip, instance, .. } if *clip != ClipState::Empty => {
                    self.write_counted(
                        self.composite_instances.clone(),
                        composite_offset,
                        bytemuck::bytes_of(instance),
                    );
                    composite_offset += std::mem::size_of::<GpuCompositeInstance>() as u64;
                }
                DrawBatch::Filtered { clip, instance, .. }
                | DrawBatch::Shadow { clip, instance, .. }
                | DrawBatch::Blend { clip, instance, .. }
                    if *clip != ClipState::Empty =>
                {
                    self.write_counted(
                        self.composite_instances.clone(),
                        composite_offset,
                        bytemuck::bytes_of(instance),
                    );
                    composite_offset += std::mem::size_of::<GpuCompositeInstance>() as u64;
                }
                _ => {}
            }
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn encode_batches(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        color_view: &wgpu::TextureView,
        stencil_view: &wgpu::TextureView,
        batches: &[DrawBatch],
        width: u32,
        height: u32,
        scale: f32,
        clear: wgpu::Color,
        destination_view: Option<&wgpu::TextureView>,
        load_existing: bool,
    ) -> (u32, u32) {
        let mut draw_calls = 0_u32;
        let mut text_draw_calls = 0_u32;
        let mut rectangle_offset = 0_u64;
        let mut glyph_offset = 0_u64;
        let mut image_offset = 0_u64;
        let mut rounded_offset = 0_u64;
        let mut path_offset = 0_u64;
        let mut composite_offset = 0_u64;
        // wgpu-profiler samples only the top-level compositor pass. Offscreen
        // effect passes remain attributable through their renderer counters
        // instead of consuming profiler scopes for every intermediate target.
        let profiler_query = if self.profiler_next_pass {
            self.profiler_next_pass = false;
            Some(
                self.gpu_profiler
                    .begin_pass_query("incular retained compositor pass", encoder),
            )
        } else {
            None
        };
        let timestamp_writes = profiler_query
            .as_ref()
            .and_then(wgpu_profiler::GpuProfilerQuery::render_pass_timestamp_writes);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("incular retained compositor pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: color_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: if load_existing {
                        wgpu::LoadOp::Load
                    } else {
                        wgpu::LoadOp::Clear(clear)
                    },
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: stencil_view,
                depth_ops: None,
                stencil_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0),
                    store: wgpu::StoreOp::Store,
                }),
            }),
            timestamp_writes,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        for batch in batches {
            let clip = match batch {
                DrawBatch::Rectangles { clip, .. }
                | DrawBatch::Glyphs { clip, .. }
                | DrawBatch::Images { clip, .. }
                | DrawBatch::RoundedRects { clip, .. }
                | DrawBatch::Path { clip, .. }
                | DrawBatch::StencilRRect { clip, .. }
                | DrawBatch::StencilPath { clip, .. }
                | DrawBatch::Offscreen { clip, .. }
                | DrawBatch::Filtered { clip, .. }
                | DrawBatch::Shadow { clip, .. }
                | DrawBatch::Blend { clip, .. } => *clip,
            };
            if clip == ClipState::Empty {
                continue;
            }
            if !set_scissor(&mut pass, clip, width, height, scale) {
                match batch {
                    DrawBatch::Rectangles { instances, .. } => {
                        rectangle_offset +=
                            (instances.len() * std::mem::size_of::<GpuInstance>()) as u64;
                    }
                    DrawBatch::Glyphs { instances, .. } => {
                        glyph_offset +=
                            (instances.len() * std::mem::size_of::<GpuGlyphInstance>()) as u64;
                    }
                    DrawBatch::Images { instances, .. } => {
                        image_offset +=
                            (instances.len() * std::mem::size_of::<GpuImageInstance>()) as u64;
                    }
                    DrawBatch::RoundedRects { instances, .. } => {
                        rounded_offset +=
                            (instances.len() * std::mem::size_of::<GpuRRectInstance>()) as u64;
                    }
                    DrawBatch::Path { .. } | DrawBatch::StencilPath { .. } => {
                        path_offset += std::mem::size_of::<GpuPathInstance>() as u64;
                    }
                    DrawBatch::StencilRRect { .. } => {
                        rounded_offset += std::mem::size_of::<GpuRRectInstance>() as u64;
                    }
                    DrawBatch::Offscreen { .. } => {
                        composite_offset += std::mem::size_of::<GpuCompositeInstance>() as u64;
                    }
                    DrawBatch::Filtered { .. } | DrawBatch::Shadow { .. } => {
                        composite_offset += std::mem::size_of::<GpuCompositeInstance>() as u64;
                    }
                    DrawBatch::Blend { .. } => {
                        composite_offset += std::mem::size_of::<GpuCompositeInstance>() as u64;
                    }
                }
                self.counters.clip_culled_draws += 1;
                continue;
            }
            pass.set_stencil_reference(u32::from(stencil_depth(clip)));
            match batch {
                DrawBatch::Rectangles { instances, .. } if !instances.is_empty() => {
                    let start = rectangle_offset;
                    rectangle_offset +=
                        (instances.len() * std::mem::size_of::<GpuInstance>()) as u64;
                    pass.set_pipeline(&self.rectangle_pipeline);
                    pass.set_vertex_buffer(0, self.mesh.slice(..));
                    pass.set_vertex_buffer(1, self.instances.slice(start..rectangle_offset));
                    pass.draw(0..6, 0..instances.len() as u32);
                    draw_calls += 1;
                }
                DrawBatch::Glyphs {
                    page, instances, ..
                } if !instances.is_empty() => {
                    let start = glyph_offset;
                    glyph_offset +=
                        (instances.len() * std::mem::size_of::<GpuGlyphInstance>()) as u64;
                    let Some(atlas_page) = self.atlas_pages.get(usize::from(*page)) else {
                        continue;
                    };
                    pass.set_pipeline(&self.text_pipeline);
                    pass.set_bind_group(0, &atlas_page.bind_group, &[]);
                    pass.set_vertex_buffer(0, self.mesh.slice(..));
                    pass.set_vertex_buffer(1, self.glyph_instances.slice(start..glyph_offset));
                    pass.draw(0..6, 0..instances.len() as u32);
                    draw_calls += 1;
                    text_draw_calls += 1;
                }
                DrawBatch::Images {
                    image,
                    sampling,
                    instances,
                    ..
                } if !instances.is_empty() => {
                    let start = image_offset;
                    image_offset +=
                        (instances.len() * std::mem::size_of::<GpuImageInstance>()) as u64;
                    let Some(bind_group) = self.image_bind_group(*image, *sampling) else {
                        continue;
                    };
                    pass.set_pipeline(&self.image_pipeline);
                    pass.set_bind_group(0, bind_group, &[]);
                    pass.set_vertex_buffer(0, self.mesh.slice(..));
                    pass.set_vertex_buffer(1, self.image_instances.slice(start..image_offset));
                    pass.draw(0..6, 0..instances.len() as u32);
                    draw_calls += 1;
                    self.counters.image_draw_calls += 1;
                }
                DrawBatch::RoundedRects {
                    gradient,
                    instances,
                    ..
                } if !instances.is_empty() => {
                    let start = rounded_offset;
                    rounded_offset +=
                        (instances.len() * std::mem::size_of::<GpuRRectInstance>()) as u64;
                    pass.set_pipeline(&self.rounded_rect_pipeline);
                    pass.set_bind_group(0, self.gradient_bind_group(*gradient), &[]);
                    pass.set_vertex_buffer(0, self.mesh.slice(..));
                    pass.set_vertex_buffer(
                        1,
                        self.rounded_rect_instances.slice(start..rounded_offset),
                    );
                    pass.draw(0..6, 0..instances.len() as u32);
                    draw_calls += 1;
                }
                DrawBatch::Path { key, gradient, .. } => {
                    let start = path_offset;
                    path_offset += std::mem::size_of::<GpuPathInstance>() as u64;
                    if let Some(mesh) = self.gpu_path_cache.get_mut(key) {
                        mesh.last_used_frame = self.counters.frames;
                    } else {
                        continue;
                    }
                    let gradient_bind_group = self.gradient_bind_group(*gradient);
                    let Some(mesh) = self.gpu_path_cache.get(key) else {
                        continue;
                    };
                    pass.set_pipeline(&self.path_pipeline);
                    pass.set_bind_group(0, gradient_bind_group, &[]);
                    pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                    pass.set_vertex_buffer(1, self.path_instances.slice(start..path_offset));
                    pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..mesh.index_count, 0, 0..1);
                    draw_calls += 1;
                    self.counters.path_draw_calls += 1;
                    self.counters.path_triangles += u64::from(mesh.index_count / 3);
                }
                DrawBatch::StencilRRect { increment, .. } => {
                    let start = rounded_offset;
                    rounded_offset += std::mem::size_of::<GpuRRectInstance>() as u64;
                    pass.set_pipeline(if *increment {
                        &self.stencil_rrect_increment_pipeline
                    } else {
                        &self.stencil_rrect_decrement_pipeline
                    });
                    pass.set_bind_group(0, self.gradient_bind_group(None), &[]);
                    pass.set_vertex_buffer(0, self.mesh.slice(..));
                    pass.set_vertex_buffer(
                        1,
                        self.rounded_rect_instances.slice(start..rounded_offset),
                    );
                    pass.draw(0..6, 0..1);
                    draw_calls += 1;
                    self.counters.stencil_mask_draws += 1;
                }
                DrawBatch::StencilPath { key, increment, .. } => {
                    let start = path_offset;
                    path_offset += std::mem::size_of::<GpuPathInstance>() as u64;
                    let Some(mesh) = self.gpu_path_cache.get(key) else {
                        continue;
                    };
                    pass.set_pipeline(if *increment {
                        &self.stencil_path_increment_pipeline
                    } else {
                        &self.stencil_path_decrement_pipeline
                    });
                    pass.set_bind_group(0, self.gradient_bind_group(None), &[]);
                    pass.set_vertex_buffer(0, self.mesh.slice(..));
                    pass.set_vertex_buffer(1, self.path_instances.slice(start..path_offset));
                    pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..mesh.index_count, 0, 0..1);
                    draw_calls += 1;
                    self.counters.stencil_mask_draws += 1;
                }
                DrawBatch::Offscreen {
                    layer, instance, ..
                } => {
                    let start = composite_offset;
                    composite_offset += std::mem::size_of::<GpuCompositeInstance>() as u64;
                    if let Some(entry) = self.offscreen_cache.get_mut(layer) {
                        entry.last_used_frame = self.counters.frames;
                    } else {
                        continue;
                    }
                    let Some(entry) = self.offscreen_cache.get(layer) else {
                        continue;
                    };
                    pass.set_pipeline(&self.composite_pipeline);
                    pass.set_bind_group(0, &entry.bind_group, &[]);
                    pass.set_vertex_buffer(0, self.mesh.slice(..));
                    pass.set_vertex_buffer(
                        1,
                        self.composite_instances.slice(
                            start..start + std::mem::size_of::<GpuCompositeInstance>() as u64,
                        ),
                    );
                    pass.draw(0..6, 0..1);
                    let _ = instance;
                    draw_calls += 1;
                    self.counters.offscreen_composite_draws += 1;
                }
                DrawBatch::Filtered {
                    layer, instance, ..
                } => {
                    let start = composite_offset;
                    composite_offset += std::mem::size_of::<GpuCompositeInstance>() as u64;
                    if let Some(entry) = self.effect_cache.get_mut(layer) {
                        entry.last_used_frame = self.counters.frames;
                    } else {
                        continue;
                    }
                    let Some(entry) = self.effect_cache.get(layer) else {
                        continue;
                    };
                    pass.set_pipeline(&self.composite_pipeline);
                    pass.set_bind_group(0, &entry.bind_group, &[]);
                    pass.set_vertex_buffer(0, self.mesh.slice(..));
                    pass.set_vertex_buffer(
                        1,
                        self.composite_instances.slice(
                            start..start + std::mem::size_of::<GpuCompositeInstance>() as u64,
                        ),
                    );
                    pass.draw(0..6, 0..1);
                    let _ = instance;
                    draw_calls += 1;
                    self.counters.offscreen_composite_draws += 1;
                }
                DrawBatch::Shadow {
                    layer, instance, ..
                } => {
                    let start = composite_offset;
                    composite_offset += std::mem::size_of::<GpuCompositeInstance>() as u64;
                    if let Some(entry) = self.effect_cache.get_mut(layer) {
                        entry.last_used_frame = self.counters.frames;
                    } else if let Some(entry) = self.offscreen_cache.get_mut(layer) {
                        entry.last_used_frame = self.counters.frames;
                    } else {
                        continue;
                    }
                    pass.set_pipeline(&self.composite_pipeline);
                    if let Some(entry) = self.effect_cache.get(layer) {
                        pass.set_bind_group(0, &entry.bind_group, &[]);
                    } else if let Some(entry) = self.offscreen_cache.get(layer) {
                        pass.set_bind_group(0, &entry.bind_group, &[]);
                    } else {
                        continue;
                    }
                    pass.set_vertex_buffer(0, self.mesh.slice(..));
                    pass.set_vertex_buffer(
                        1,
                        self.composite_instances.slice(
                            start..start + std::mem::size_of::<GpuCompositeInstance>() as u64,
                        ),
                    );
                    pass.draw(0..6, 0..1);
                    let _ = instance;
                    draw_calls += 1;
                    self.counters.drop_shadow_composites += 1;
                }
                DrawBatch::Blend {
                    layer,
                    instance,
                    mode,
                    ..
                } => {
                    let start = composite_offset;
                    composite_offset += std::mem::size_of::<GpuCompositeInstance>() as u64;
                    let Some(source) = self.offscreen_cache.get_mut(layer) else {
                        continue;
                    };
                    source.last_used_frame = self.counters.frames;
                    if !mode.requires_destination_read() {
                        let Some(pipeline) = self.fixed_blend_pipelines.get(mode.code() as usize)
                        else {
                            continue;
                        };
                        pass.set_pipeline(pipeline);
                        pass.set_bind_group(0, &source.bind_group, &[]);
                        pass.set_vertex_buffer(0, self.mesh.slice(..));
                        pass.set_vertex_buffer(
                            1,
                            self.composite_instances.slice(
                                start..start + std::mem::size_of::<GpuCompositeInstance>() as u64,
                            ),
                        );
                        pass.draw(0..6, 0..1);
                        self.counters.blend_fixed_function_draws += 1;
                        draw_calls += 1;
                        let _ = instance;
                        continue;
                    }
                    let Some(destination_view) = destination_view else {
                        continue;
                    };
                    let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("incular destination blend bind group"),
                        layout: &self.blend_bind_group_layout,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: wgpu::BindingResource::TextureView(
                                    &source.target.color_view,
                                ),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: wgpu::BindingResource::TextureView(destination_view),
                            },
                            wgpu::BindGroupEntry {
                                binding: 2,
                                resource: wgpu::BindingResource::Sampler(&self.blend_sampler),
                            },
                        ],
                    });
                    pass.set_pipeline(&self.blend_pipeline);
                    pass.set_bind_group(0, &bind_group, &[]);
                    pass.set_vertex_buffer(0, self.mesh.slice(..));
                    pass.set_vertex_buffer(
                        1,
                        self.composite_instances.slice(
                            start..start + std::mem::size_of::<GpuCompositeInstance>() as u64,
                        ),
                    );
                    pass.draw(0..6, 0..1);
                    self.counters.blend_destination_read_draws += 1;
                    if !mode.requires_destination_read() {
                        self.counters.blend_fixed_function_draws += 1;
                    }
                    draw_calls += 1;
                    let _ = instance;
                }
                _ => {}
            }
        }
        drop(pass);
        if let Some(query) = profiler_query {
            self.gpu_profiler.end_query(encoder, query);
            self.gpu_profiler.resolve_queries(encoder);
        }
        (draw_calls, text_draw_calls)
    }
}
