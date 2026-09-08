use super::*;

impl WgpuRenderer {
    pub(super) fn append_path(
        &mut self,
        batches: &mut Vec<DrawBatch>,
        clip: ClipState,
        path: &std::sync::Arc<Path>,
        kind: PathMeshKind,
        brush: &Brush,
        placement: PathPlacement,
    ) {
        if clip == ClipState::Empty || !path_visible(path, placement.transform, clip) {
            return;
        }
        let key = PathMeshKey {
            path: path.id(),
            kind,
        };
        if !self.ensure_path_mesh(key, path) {
            return;
        }
        let gradient_id = self.ensure_gradient(brush);
        let (color, gradient, options) = path_paint(brush);
        batches.push(DrawBatch::Path {
            clip,
            key,
            gradient: gradient_id,
            instance: GpuPathInstance {
                affine: physical_affine(placement.transform, placement.scale).0,
                translation: physical_affine(placement.transform, placement.scale).1,
                surface: [self.target_width as f32, self.target_height as f32, 0., 0.],
                color: color.to_linear_rgba(),
                gradient,
                options,
            },
        });
    }
    pub(super) fn ensure_path_mesh(&mut self, key: PathMeshKey, path: &Path) -> bool {
        self.counters.path_tessellation_requests += 1;
        let mesh = match self.cpu_path_cache.entry(key) {
            Entry::Occupied(entry) => {
                self.counters.path_tessellation_cache_hits += 1;
                entry.into_mut()
            }
            Entry::Vacant(entry) => {
                self.counters.path_tessellation_cache_misses += 1;
                let (rule, stroke) = match key.kind {
                    PathMeshKind::Fill(rule) => {
                        self.counters.path_fill_tessellations += 1;
                        (rule, None)
                    }
                    PathMeshKind::Stroke {
                        width,
                        cap,
                        join,
                        miter_limit,
                    } => {
                        self.counters.path_stroke_tessellations += 1;
                        (
                            FillRule::NonZero,
                            Some(Stroke {
                                width: f32::from_bits(width),
                                cap,
                                join,
                                miter_limit: f32::from_bits(miter_limit),
                            }),
                        )
                    }
                };
                let Some(mesh) = tessellate_path(path, rule, stroke) else {
                    return false;
                };
                self.counters.path_cpu_vertices += mesh.vertices.len() as u64;
                self.counters.path_cpu_indices += mesh.indices.len() as u64;
                if mesh.vertices.is_empty() || mesh.indices.is_empty() {
                    return false;
                }
                entry.insert(mesh)
            }
        };
        if let Some(gpu) = self.gpu_path_cache.get_mut(&key) {
            gpu.last_used_frame = self.counters.frames;
            self.counters.path_gpu_cache_hits += 1;
            return true;
        }
        let vertices = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("incular retained path vertices"),
                contents: bytemuck::cast_slice(&mesh.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let indices = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("incular retained path indices"),
                contents: bytemuck::cast_slice(&mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });
        self.gpu_path_cache.insert(
            key,
            GpuPathMesh {
                vertices,
                indices,
                index_count: mesh.indices.len() as u32,
                last_used_frame: self.counters.frames,
            },
        );
        self.counters.path_gpu_cache_misses += 1;
        self.counters.path_gpu_uploads += 1;
        true
    }
    /// Makes the paint lookup retained independently from all geometry.
    /// The local key spans the stops identity and this window's surface
    /// format, matching the shared key exactly; a surface reconfiguration
    /// therefore retires old-format entries as stale instead of reusing
    /// them.
    pub(super) fn ensure_gradient(&mut self, brush: &Brush) -> Option<GradientId> {
        let id = gradient_id(brush)?;
        let key = GradientResourceKey {
            gradient: id,
            format: self.window_gpu.config.format,
        };
        self.counters.gradient_instances += 1;
        match brush {
            Brush::LinearGradient(_) => self.counters.linear_gradient_instances += 1,
            Brush::RadialGradient(_) => self.counters.radial_gradient_instances += 1,
            Brush::SweepGradient(_) => self.counters.radial_gradient_instances += 1,
            Brush::Solid(_) => {}
        }
        if self.gradient_cache.get(&key).is_some() {
            self.gradient_cache.record_use(key, self.counters.frames);
            self.counters.gradient_cache_hits += 1;
            return Some(id);
        }
        let stops = match brush {
            Brush::LinearGradient(g) => &g.stops,
            Brush::RadialGradient(g) => &g.stops,
            Brush::SweepGradient(g) => &g.stops,
            Brush::Solid(_) => unreachable!("solid has no gradient id"),
        };
        // LUT texels are premultiplied linear RGB. The shader unpremultiplies
        // before the renderer's straight-alpha source-over blend, avoiding
        // dark fringes across transparent stops.
        let pixels = gradient_lut_pixels(stops);
        let acquisition = self.shared.gradient_resource(
            id,
            self.window_gpu.config.format,
            &pixels,
            &self.gradient_bind_group_layout,
            &self.gradient_sampler,
        );
        let admitted = acquisition.admitted();
        let uploaded = acquisition.uploaded;
        if admitted {
            debug_assert!(
                self.shared.gradient_textures_contains(&key),
                "window gradient cache must reference a retained shared entry"
            );
        }
        // The shared `Arc` moves into local retention unchanged — never a
        // re-wrapped inner handle — so liveness checks observe renderer
        // ownership (see `into_local_parts`).
        let (resource, retention) = acquisition.into_local_parts();
        self.gradient_cache
            .insert(key, resource, retention, self.counters.frames);
        self.counters.gradient_cache_misses += 1;
        if uploaded {
            self.counters.gradient_resource_creations += 1;
            self.counters.gradient_resource_uploads += 1;
        }
        Some(id)
    }
    pub(super) fn gradient_bind_group(&self, id: Option<GradientId>) -> &wgpu::BindGroup {
        id.and_then(|id| {
            self.gradient_cache
                .get(&GradientResourceKey {
                    gradient: id,
                    format: self.window_gpu.config.format,
                })
                .map(|entry| &entry.resource.resource.bind_group)
        })
        .unwrap_or(&self.solid_gradient.bind_group)
    }
    pub(super) fn ensure_gpu_image(&mut self, image: &ImageHandle) -> Result<(), RendererError> {
        let id = image.id();
        if let Some(entry) = self.image_cache.get(&id) {
            let gpu = &entry.resource;
            debug_assert_eq!(gpu.resource.width, image.decoded().width());
            debug_assert_eq!(gpu.resource.height, image.decoded().height());
            self.image_cache.record_use(id, self.counters.frames);
            self.counters.image_cache_hits += 1;
            return Ok(());
        }
        let acquisition = self.shared.image_resource(image)?;
        // Only admitted textures carry a context-wide identity: bypassed
        // (oversized-for-budget) uploads hold a fresh per-upload identity
        // with no registry entry, so the identity check below covers
        // admitted acquisitions only.
        if acquisition.admitted() {
            debug_assert_eq!(
                self.shared.image_resource_identity(id),
                Some(acquisition.resource.identity),
                "window image cache must reference the context-wide texture identity"
            );
        }
        let uploaded = acquisition.uploaded;
        let (resource, retention) = acquisition.into_local_parts();
        self.image_cache.insert(
            id,
            GpuImage {
                resource,
                bind_groups: HashMap::new(),
            },
            retention,
            self.counters.frames,
        );
        self.counters.image_cache_misses += 1;
        if uploaded {
            self.counters.image_texture_creations += 1;
            self.counters.image_texture_uploads += 1;
        }
        Ok(())
    }
    /// Flushes this frame's texture use into the shared owner in one batch
    /// and drops locally stale entries, for images and gradients together
    /// under a single shared lock. Runs once per frame after submit, so
    /// per-draw references cost no shared lock: use is recorded locally
    /// during encoding and reported here, deduplicated by id. Stale entries
    /// (shared-evicted, or superseded by a re-upload under a new
    /// generation) are dropped with their bind groups; submitted work stays
    /// valid through the wgpu lifetime contract, and the next use
    /// re-resolves to the current shared generation.
    pub(super) fn sync_shared_textures(&mut self) {
        let mut shared = self
            .shared
            .inner
            .resources
            .lock()
            .expect("shared resource lock");
        self.image_cache
            .sync_with_shared(&mut shared.image_textures);
        self.gradient_cache
            .sync_with_shared(&mut shared.gradient_textures);
    }
    /// Releases locally retained textures the shared owner no longer
    /// retains, with their bind groups, for images, gradients, and glyph
    /// atlas pages together. Safe on a renderer presenting no frames:
    /// staleness is pure generation comparison, and anything still
    /// referenced by submitted work stays valid through the wgpu lifetime
    /// contract.
    ///
    /// Reclamation events and executors: the per-frame sync above covers
    /// active renderers; the unconfigured early-return path in `frame.rs`
    /// covers renderers still driven while presenting nothing; otherwise
    /// the window owner calls this during idle maintenance (event-loop idle
    /// work, memory-pressure handling, pre-resume) or simply drops the
    /// renderer, which releases everything. No background reaper keeps idle
    /// renderers alive, and shared eviction never reaches into a renderer
    /// it does not own — all mutation here runs on the owning thread under
    /// one shared lock, the same order the frame path uses.
    pub fn reclaim_stale_textures(&mut self) -> usize {
        let shared = self
            .shared
            .inner
            .resources
            .lock()
            .expect("shared resource lock");
        self.image_cache.reclaim_stale(&shared.image_textures).len()
            + self
                .gradient_cache
                .reclaim_stale(&shared.gradient_textures)
                .len()
            + self
                .atlas_pages
                .reclaim(&|page| shared.glyph_atlas.page_generation(page))
    }

    pub(super) fn evict_unused_images(&mut self) {
        let dropped = self
            .image_cache
            .evict_unused(self.counters.frames, IMAGE_CACHE_MAX_UNUSED_FRAMES);
        self.counters.image_texture_evictions += dropped as u64;
    }
    pub(super) fn evict_unused_path_meshes(&mut self) {
        let frame = self.counters.frames;
        let before = self.gpu_path_cache.len();
        self.gpu_path_cache.retain(|_, mesh| {
            frame.saturating_sub(mesh.last_used_frame) <= PATH_CACHE_MAX_UNUSED_FRAMES
        });
        self.counters.path_gpu_evictions += (before - self.gpu_path_cache.len()) as u64;
    }
    pub(super) fn evict_unused_gradients(&mut self) {
        let dropped = self
            .gradient_cache
            .evict_unused(self.counters.frames, GRADIENT_CACHE_MAX_UNUSED_FRAMES);
        self.counters.gradient_resource_evictions += dropped as u64;
    }
    pub(super) fn evict_offscreen_cache(&mut self) {
        let frame = self.counters.frames;
        loop {
            let source = self
                .offscreen_cache
                .iter()
                .min_by_key(|(_, entry)| entry.last_used_frame)
                .map(|(layer, _)| (*layer, true));
            let effect = self
                .effect_cache
                .iter()
                .min_by_key(|(_, entry)| entry.last_used_frame)
                .map(|(layer, _)| (*layer, false));
            let Some((layer, is_source)) = (match (source, effect) {
                (Some(left), Some(right)) => Some(if left.1 && !right.1 {
                    // Pick the oldest timestamp rather than privileging one
                    // cache. The bool only identifies ownership.
                    let left_frame = self
                        .offscreen_cache
                        .get(&left.0)
                        .map_or(u64::MAX, |entry| entry.last_used_frame);
                    let right_frame = self
                        .effect_cache
                        .get(&right.0)
                        .map_or(u64::MAX, |entry| entry.last_used_frame);
                    if left_frame <= right_frame {
                        left
                    } else {
                        right
                    }
                } else {
                    left
                }),
                (Some(value), None) | (None, Some(value)) => Some(value),
                (None, None) => None,
            }) else {
                break;
            };
            let last_used = if is_source {
                self.offscreen_cache
                    .get(&layer)
                    .map_or(u64::MAX, |entry| entry.last_used_frame)
            } else {
                self.effect_cache
                    .get(&layer)
                    .map_or(u64::MAX, |entry| entry.last_used_frame)
            };
            let stale = frame.saturating_sub(last_used) > OFFSCREEN_CACHE_MAX_UNUSED_FRAMES;
            if self.counters.offscreen_cached_bytes <= self.offscreen_cache_budget && !stale {
                break;
            }
            if is_source {
                let Some(entry) = self.offscreen_cache.remove(&layer) else {
                    continue;
                };
                self.counters.offscreen_cached_bytes = self
                    .counters
                    .offscreen_cached_bytes
                    .saturating_sub(entry.bytes);
                self.offscreen_target_pool.recycle(entry.target);
            } else {
                let Some(entry) = self.effect_cache.remove(&layer) else {
                    continue;
                };
                self.remove_effect_cache_bytes(entry.bytes);
                self.offscreen_target_pool.recycle(entry.target);
            }
            self.counters.offscreen_texture_evictions += 1;
            self.counters.effect_texture_evictions += u64::from(!is_source);
        }
    }
    pub(super) fn prepare_image_bind_groups(&mut self, batches: &[DrawBatch]) {
        for batch in batches {
            if let DrawBatch::Images {
                image, sampling, ..
            } = batch
            {
                let already_bound = self
                    .image_cache
                    .get(image)
                    .is_some_and(|entry| entry.resource.bind_groups.contains_key(sampling));
                if already_bound {
                    continue;
                }
                let Some(view) = self
                    .image_cache
                    .get(image)
                    .map(|entry| &entry.resource.resource.view)
                else {
                    continue;
                };
                let Some(sampler) = self.image_samplers.get(sampling) else {
                    continue;
                };
                let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("incular retained image bind group"),
                    layout: &self.image_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(sampler),
                        },
                    ],
                });
                if let Some(entry) = self.image_cache.get_mut(image) {
                    entry.resource.bind_groups.insert(*sampling, bind_group);
                }
            }
        }
    }
    pub(super) fn image_bind_group(
        &self,
        image: ImageId,
        sampling: ImageSampling,
    ) -> Option<&wgpu::BindGroup> {
        self.image_cache
            .get(&image)?
            .resource
            .bind_groups
            .get(&sampling)
    }
    pub(super) fn upload_glyph(&mut self, entry: AtlasEntry, bitmap: &[u8]) {
        self.ensure_atlas_page(entry.page, entry.generation);
        let page = self
            .atlas_pages
            .get(entry.page)
            .expect("atlas page just ensured");
        let padded_width = usize::from(entry.width + ATLAS_PADDING * 2);
        let padded_height = usize::from(entry.height + ATLAS_PADDING * 2);
        // A freshly allocated texture has undefined contents. Explicitly write
        // the allocation, including its transparent border, before enabling
        // linear filtering so adjacent glyphs can never bleed into this mask.
        let mut padded = vec![0_u8; padded_width * padded_height];
        for row in 0..usize::from(entry.height) {
            let source_start = row * usize::from(entry.width);
            let destination_start =
                (row + usize::from(ATLAS_PADDING)) * padded_width + usize::from(ATLAS_PADDING);
            padded[destination_start..destination_start + usize::from(entry.width)]
                .copy_from_slice(&bitmap[source_start..source_start + usize::from(entry.width)]);
        }
        self.counters.texture_upload_bytes += padded.len() as u64;
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &page.texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: u32::from(entry.x - ATLAS_PADDING),
                    y: u32::from(entry.y - ATLAS_PADDING),
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &padded,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_width as u32),
                rows_per_image: Some(padded_height as u32),
            },
            wgpu::Extent3d {
                width: padded_width as u32,
                height: padded_height as u32,
                depth_or_array_layers: 1,
            },
        );
    }
    pub(super) fn ensure_atlas_page(&mut self, page: u16, generation: u64) {
        // A generation mismatch replaces the slot: the shared page was
        // evicted and reused for different content since this binding was
        // built. Replacement is safe here because callers only ensure
        // freshly resolved generations, and frame-pinned pages can never be
        // selected for eviction mid-frame — so no already-emitted batch of
        // this frame can reference the replaced texture.
        let missing_or_stale = self.atlas_pages.generation(page) != Some(generation);
        if missing_or_stale {
            let texture = self.shared.shared_glyph_texture(page, generation);
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("incular glyph atlas bind group"),
                layout: &self.atlas_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.atlas_sampler),
                    },
                ],
            });
            self.atlas_pages.ensure(page, generation, || GpuAtlasPage {
                texture,
                bind_group,
            });
            self.counters.atlas_texture_recreations += 1;
        }
    }
}

impl crate::ReclaimStaleTextures for WgpuRenderer {
    fn reclaim_stale_textures(&mut self) -> usize {
        WgpuRenderer::reclaim_stale_textures(self)
    }
}
