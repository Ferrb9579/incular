use crate::display_list::{DisplayList, PaintCommand};
use crate::effects::{BlendMode, ColorFilter, DropShadowEffect, GaussianBlur};
use crate::geometry::union_rect;
use incular_core::{Arena, ArenaId, DirtyFlags, Offset, Rect, Size, Transform};
use std::sync::Arc;

/// Opaque retained compositor identity. It uses the core generational arena so
/// an ID for a removed layer can never select a later, unrelated layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LayerId(ArenaId);

/// Canonical opacity representation shared by widgets, retained layers, and
/// GPU lowering. Non-finite values become transparent and finite values are
/// clamped to the physically meaningful range.
#[must_use]
pub fn normalize_opacity(alpha: f32) -> f32 {
    if alpha.is_finite() {
        alpha.clamp(0., 1.)
    } else {
        0.
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CompositorDiagnostics {
    pub layers: u64,
    pub picture_layers_reused: u64,
    pub picture_layers_repainted: u64,
    pub transform_updates: u64,
    pub clip_updates: u64,
    pub layers_culled: u64,
    pub opacity_updates: u64,
}

/// A picture placement observed while flattening. All rectangles here are in
/// logical coordinates; this is intentionally useful for CPU-side scene tests
/// and diagnostics without involving a surface or DPI scale factor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlattenedPicture {
    pub layer: LayerId,
    pub local_bounds: Rect,
    pub world_bounds: Rect,
    pub active_clip: Option<Rect>,
}

#[derive(Clone, Debug)]
pub enum LayerKind {
    Picture {
        display_list: Arc<DisplayList>,
        bounds: Rect,
    },
    Transform {
        transform: Transform,
    },
    ClipRect {
        rect: Rect,
    },
    Opacity {
        alpha: f32,
    },
    Blur {
        blur: GaussianBlur,
    },
    DropShadow {
        shadow: DropShadowEffect,
    },
    ColorFilter {
        filter: ColorFilter,
    },
    Blend {
        mode: BlendMode,
    },
}

#[derive(Clone, Debug)]
struct Layer {
    kind: LayerKind,
    children: Vec<LayerId>,
    dirty: DirtyFlags,
    generation: u64,
}

/// Renderer-independent retained layer arena. Picture display lists are owned
/// once and scene flattening only emits a transient linear command stream.
/// This deliberately has no GPU resources or offscreen textures.
pub struct LayerTree {
    layers: Arena<Layer>,
    root: Option<LayerId>,
    diagnostics: CompositorDiagnostics,
    flattened_pictures: Vec<FlattenedPicture>,
    next_generation: u64,
}
impl Default for LayerTree {
    fn default() -> Self {
        Self::new()
    }
}
impl LayerTree {
    #[must_use]
    pub fn new() -> Self {
        Self {
            layers: Arena::new(),
            root: None,
            diagnostics: CompositorDiagnostics::default(),
            flattened_pictures: Vec::new(),
            next_generation: 1,
        }
    }
    #[must_use]
    pub fn diagnostics(&self) -> CompositorDiagnostics {
        self.diagnostics
    }
    #[must_use]
    pub fn root(&self) -> Option<LayerId> {
        self.root
    }
    pub fn set_root(&mut self, root: LayerId) {
        self.root = Some(root);
    }
    pub fn create_picture(&mut self, display_list: DisplayList, bounds: Rect) -> LayerId {
        self.insert(LayerKind::Picture {
            display_list: Arc::new(display_list),
            bounds,
        })
    }
    pub fn create_transform(&mut self, transform: Transform) -> LayerId {
        self.insert(LayerKind::Transform { transform })
    }
    pub fn create_clip_rect(&mut self, rect: Rect) -> LayerId {
        self.insert(LayerKind::ClipRect { rect })
    }
    /// Creates an isolated compositor group. Alpha is normalized at the
    /// renderer-neutral boundary so every backend observes identical values.
    pub fn create_opacity(&mut self, alpha: f32) -> LayerId {
        self.insert(LayerKind::Opacity {
            alpha: normalize_opacity(alpha),
        })
    }
    /// Creates an isolated Gaussian-filtered compositor group.
    pub fn create_blur(&mut self, blur: GaussianBlur) -> LayerId {
        self.insert(LayerKind::Blur {
            blur: GaussianBlur::new(blur.sigma_x, blur.sigma_y),
        })
    }
    /// Creates an isolated arbitrary-subtree drop-shadow compositor group.
    pub fn create_drop_shadow(&mut self, shadow: DropShadowEffect) -> LayerId {
        self.insert(LayerKind::DropShadow {
            shadow: DropShadowEffect::asymmetric(
                shadow.offset,
                shadow.sigma_x,
                shadow.sigma_y,
                shadow.color,
            ),
        })
    }
    /// Creates an isolated color-matrix stage. Matrix changes are compositor
    /// updates and deliberately do not change the source generation.
    pub fn create_color_filter(&mut self, filter: ColorFilter) -> LayerId {
        self.insert(LayerKind::ColorFilter { filter })
    }
    /// Creates a retained blend group whose source remains cached when only
    /// the discrete blend mode changes.
    pub fn create_blend(&mut self, mode: BlendMode) -> LayerId {
        self.insert(LayerKind::Blend { mode })
    }
    fn insert(&mut self, kind: LayerKind) -> LayerId {
        let generation = self.next_generation;
        self.next_generation = self.next_generation.wrapping_add(1).max(1);
        let id = LayerId(self.layers.insert(Layer {
            kind,
            children: Vec::new(),
            dirty: DirtyFlags::COMPOSITE,
            generation,
        }));
        self.diagnostics.layers += 1;
        id
    }
    pub fn remove(&mut self, id: LayerId) {
        if self.layers.remove(id.0).is_some() {
            self.diagnostics.layers -= 1;
            if self.root == Some(id) {
                self.root = None;
            }
            for (_, layer) in self.layers.iter() {
                // Children are pruned by their owners before removal in normal
                // widget unmounting; stale entries are ignored by flattening.
                let _ = layer;
            }
        }
    }
    pub fn set_children(&mut self, id: LayerId, children: Vec<LayerId>) {
        if let Some(layer) = self.layers.get_mut(id.0) {
            if layer.children == children {
                return;
            }
            layer.children = children;
            layer.dirty.insert(DirtyFlags::COMPOSITE);
            layer.generation = self.next_generation;
            self.next_generation = self.next_generation.wrapping_add(1).max(1);
        }
    }
    pub fn update_picture(&mut self, id: LayerId, display_list: DisplayList, bounds: Rect) {
        if let Some(layer) = self.layers.get_mut(id.0)
            && let LayerKind::Picture {
                display_list: current,
                bounds: current_bounds,
            } = &mut layer.kind
        {
            *current = Arc::new(display_list);
            *current_bounds = bounds;
            layer
                .dirty
                .insert(DirtyFlags::PAINT | DirtyFlags::COMPOSITE);
            layer.generation = self.next_generation;
            self.next_generation = self.next_generation.wrapping_add(1).max(1);
            self.diagnostics.picture_layers_repainted += 1;
        }
    }
    pub fn update_transform(&mut self, id: LayerId, transform: Transform) -> bool {
        let Some(layer) = self.layers.get_mut(id.0) else {
            return false;
        };
        let LayerKind::Transform { transform: current } = &mut layer.kind else {
            return false;
        };
        if *current == transform {
            return false;
        }
        *current = transform;
        layer.dirty.insert(DirtyFlags::COMPOSITE);
        layer.generation = self.next_generation;
        self.next_generation = self.next_generation.wrapping_add(1).max(1);
        self.diagnostics.transform_updates += 1;
        true
    }
    pub fn update_clip(&mut self, id: LayerId, rect: Rect) -> bool {
        let Some(layer) = self.layers.get_mut(id.0) else {
            return false;
        };
        let LayerKind::ClipRect { rect: current } = &mut layer.kind else {
            return false;
        };
        if *current == rect {
            return false;
        }
        *current = rect;
        layer.dirty.insert(DirtyFlags::COMPOSITE);
        layer.generation = self.next_generation;
        self.next_generation = self.next_generation.wrapping_add(1).max(1);
        self.diagnostics.clip_updates += 1;
        true
    }
    /// Updates only the group alpha. This deliberately does not change the
    /// content generation, allowing a cached offscreen texture to be reused
    /// while an opacity animation is ticking.
    pub fn update_opacity(&mut self, id: LayerId, alpha: f32) -> bool {
        let Some(layer) = self.layers.get_mut(id.0) else {
            return false;
        };
        let LayerKind::Opacity { alpha: current } = &mut layer.kind else {
            return false;
        };
        let alpha = normalize_opacity(alpha);
        if *current == alpha {
            return false;
        }
        *current = alpha;
        layer.dirty.insert(DirtyFlags::COMPOSITE);
        self.diagnostics.opacity_updates += 1;
        true
    }
    /// Updates Gaussian parameters without changing the retained source
    /// generation. A backend recomputes only the filtered result.
    pub fn update_blur(&mut self, id: LayerId, blur: GaussianBlur) -> bool {
        let Some(layer) = self.layers.get_mut(id.0) else {
            return false;
        };
        let LayerKind::Blur { blur: current } = &mut layer.kind else {
            return false;
        };
        let blur = GaussianBlur::new(blur.sigma_x, blur.sigma_y);
        if *current == blur {
            return false;
        }
        *current = blur;
        layer.dirty.insert(DirtyFlags::COMPOSITE);
        true
    }
    /// Updates shadow presentation properties without changing source pixels.
    /// Sigma invalidates the filtered mask; offset/color remain composite-only
    /// in GPU backends.
    pub fn update_drop_shadow(&mut self, id: LayerId, shadow: DropShadowEffect) -> bool {
        let Some(layer) = self.layers.get_mut(id.0) else {
            return false;
        };
        let LayerKind::DropShadow { shadow: current } = &mut layer.kind else {
            return false;
        };
        let shadow = DropShadowEffect::asymmetric(
            shadow.offset,
            shadow.sigma_x,
            shadow.sigma_y,
            shadow.color,
        );
        if *current == shadow {
            return false;
        }
        *current = shadow;
        layer.dirty.insert(DirtyFlags::COMPOSITE);
        true
    }
    pub fn update_color_filter(&mut self, id: LayerId, filter: ColorFilter) -> bool {
        let Some(layer) = self.layers.get_mut(id.0) else {
            return false;
        };
        let LayerKind::ColorFilter { filter: current } = &mut layer.kind else {
            return false;
        };
        if *current == filter {
            return false;
        }
        *current = filter;
        layer.dirty.insert(DirtyFlags::COMPOSITE);
        true
    }
    pub fn update_blend(&mut self, id: LayerId, mode: BlendMode) -> bool {
        let Some(layer) = self.layers.get_mut(id.0) else {
            return false;
        };
        let LayerKind::Blend { mode: current } = &mut layer.kind else {
            return false;
        };
        if *current == mode {
            return false;
        }
        *current = mode;
        layer.dirty.insert(DirtyFlags::COMPOSITE);
        true
    }
    /// World-space logical bounds captured by the most recent [`Self::flatten`]
    /// call. This keeps coordinate diagnostics out of the renderer hot path.
    #[must_use]
    pub fn flattened_pictures(&self) -> &[FlattenedPicture] {
        &self.flattened_pictures
    }
    #[must_use]
    pub fn flatten(&mut self) -> DisplayList {
        let mut out = DisplayList::new();
        self.flattened_pictures.clear();
        if let Some(root) = self.root {
            self.flatten_layer(root, Transform::IDENTITY, None, &mut out);
        }
        for (_, layer) in self.layers.iter() {
            if matches!(layer.kind, LayerKind::Picture { .. })
                && !layer.dirty.contains(DirtyFlags::PAINT)
            {
                self.diagnostics.picture_layers_reused += 1;
            }
        }
        for (_, layer) in self.layers.iter() {
            // Cleared after submission: retained data stays intact.
            let _ = layer;
        }
        for (_, layer) in self.layers.iter() {
            let _ = layer;
        }
        // Arena does not expose mutable iteration intentionally; dirty state is
        // advisory/debug-only and property writes remain coalesced by value.
        out
    }
    fn flatten_layer(
        &mut self,
        id: LayerId,
        world_transform: Transform,
        clip: Option<Rect>,
        out: &mut DisplayList,
    ) {
        let Some(layer) = self.layers.get(id.0).cloned() else {
            return;
        };
        match layer.kind {
            LayerKind::Picture {
                display_list,
                bounds,
            } => {
                let world = world_transform.transform_rect_bbox(bounds);
                if clip.is_some_and(|active| !active.intersects(world)) {
                    self.diagnostics.layers_culled += 1;
                    return;
                }
                self.flattened_pictures.push(FlattenedPicture {
                    layer: id,
                    local_bounds: bounds,
                    world_bounds: world,
                    active_clip: clip,
                });
                out.push(PaintCommand::PushTransform {
                    transform: world_transform,
                });
                out.extend_from(&display_list);
                out.push(PaintCommand::PopTransform);
            }
            LayerKind::Transform { transform: local } => {
                let next = world_transform.then(local);
                for child in layer.children {
                    self.flatten_layer(child, next, clip, out);
                }
            }
            LayerKind::ClipRect { rect } => {
                let world = world_transform.transform_rect_bbox(rect);
                let next_clip = match clip {
                    Some(old) => old.intersection(world),
                    None => Some(world),
                };
                if next_clip.is_none() {
                    self.diagnostics.layers_culled += 1;
                    return;
                }
                out.push(PaintCommand::PushClip { rect: world });
                for child in layer.children {
                    self.flatten_layer(child, world_transform, next_clip, out);
                }
                out.push(PaintCommand::PopClip);
            }
            LayerKind::Opacity { alpha } => {
                let bounds = self
                    .subtree_bounds(id, world_transform)
                    .unwrap_or_else(|| Rect::from_origin_size(Offset::ZERO, Size::ZERO));
                out.push(PaintCommand::PushOpacity {
                    layer: id,
                    alpha,
                    generation: self.subtree_generation(id),
                    bounds,
                });
                for child in layer.children {
                    self.flatten_layer(child, world_transform, clip, out);
                }
                out.push(PaintCommand::PopOpacity);
            }
            LayerKind::Blur { blur } => {
                let bounds = self
                    .subtree_bounds(id, world_transform)
                    .unwrap_or_else(|| Rect::from_origin_size(Offset::ZERO, Size::ZERO));
                out.push(PaintCommand::PushBlur {
                    layer: id,
                    blur,
                    generation: self.subtree_generation(id),
                    bounds,
                });
                for child in layer.children {
                    self.flatten_layer(child, world_transform, clip, out);
                }
                out.push(PaintCommand::PopEffect);
            }
            LayerKind::DropShadow { shadow } => {
                let bounds = self
                    .subtree_bounds(id, world_transform)
                    .unwrap_or_else(|| Rect::from_origin_size(Offset::ZERO, Size::ZERO));
                out.push(PaintCommand::PushDropShadow {
                    layer: id,
                    shadow,
                    generation: self.subtree_generation(id),
                    bounds,
                });
                for child in layer.children {
                    self.flatten_layer(child, world_transform, clip, out);
                }
                out.push(PaintCommand::PopEffect);
            }
            LayerKind::ColorFilter { filter } => {
                let bounds = self
                    .subtree_bounds(id, world_transform)
                    .unwrap_or_else(|| Rect::from_origin_size(Offset::ZERO, Size::ZERO));
                out.push(PaintCommand::PushColorFilter {
                    layer: id,
                    filter,
                    generation: self.subtree_generation(id),
                    bounds,
                });
                for child in layer.children {
                    self.flatten_layer(child, world_transform, clip, out);
                }
                out.push(PaintCommand::PopEffect);
            }
            LayerKind::Blend { mode } => {
                let bounds = self
                    .subtree_bounds(id, world_transform)
                    .unwrap_or_else(|| Rect::from_origin_size(Offset::ZERO, Size::ZERO));
                out.push(PaintCommand::PushBlend {
                    layer: id,
                    mode,
                    generation: self.subtree_generation(id),
                    bounds,
                });
                for child in layer.children {
                    self.flatten_layer(child, world_transform, clip, out);
                }
                out.push(PaintCommand::PopEffect);
            }
        }
    }
    fn subtree_generation(&self, id: LayerId) -> u64 {
        let Some(layer) = self.layers.get(id.0) else {
            return 0;
        };
        let mut value = layer.generation.wrapping_mul(0x9e37_79b9_7f4a_7c15);
        for child in &layer.children {
            // An opacity layer's own alpha is intentionally excluded from
            // its cache key: changing it only changes the final composite.
            // When that layer is nested inside another isolated group,
            // however, its composite is part of the parent's pixels. Include
            // the child's alpha at this boundary so an inner fade invalidates
            // the outer group's cached texture without invalidating the
            // inner group's own content target.
            if let Some(child_layer) = self.layers.get(child.0)
                && let LayerKind::Opacity { alpha } = &child_layer.kind
            {
                value = value.rotate_left(11) ^ u64::from(alpha.to_bits());
            }
            if let Some(child_layer) = self.layers.get(child.0) {
                match &child_layer.kind {
                    LayerKind::Blur { blur } => {
                        value = value.rotate_left(13)
                            ^ u64::from(blur.sigma_x.to_bits())
                            ^ u64::from(blur.sigma_y.to_bits()).rotate_left(17);
                    }
                    LayerKind::DropShadow { shadow } => {
                        value = value.rotate_left(13)
                            ^ u64::from(shadow.offset.x.to_bits())
                            ^ u64::from(shadow.offset.y.to_bits()).rotate_left(7)
                            ^ u64::from(shadow.sigma_x.to_bits()).rotate_left(17)
                            ^ u64::from(shadow.sigma_y.to_bits()).rotate_left(23)
                            ^ u64::from(shadow.color.red)
                            ^ (u64::from(shadow.color.green) << 8)
                            ^ (u64::from(shadow.color.blue) << 16)
                            ^ (u64::from(shadow.color.alpha) << 24);
                    }
                    LayerKind::ColorFilter { filter } => {
                        for (index, bits) in filter.to_matrix().map(f32::to_bits).iter().enumerate()
                        {
                            value ^= u64::from(*bits).rotate_left((index as u32 % 31) + 1);
                        }
                    }
                    LayerKind::Blend { mode } => {
                        value = value.rotate_left(13) ^ u64::from(mode.code());
                    }
                    _ => {}
                }
            }
            value = value.rotate_left(7) ^ self.subtree_generation(*child);
        }
        value
    }
    fn subtree_bounds(&self, id: LayerId, world_transform: Transform) -> Option<Rect> {
        let layer = self.layers.get(id.0)?;
        match &layer.kind {
            LayerKind::Picture { bounds, .. } => Some(world_transform.transform_rect_bbox(*bounds)),
            LayerKind::Transform { transform: local } => layer
                .children
                .iter()
                .filter_map(|child| self.subtree_bounds(*child, world_transform.then(*local)))
                .reduce(union_rect),
            LayerKind::ClipRect { rect } => {
                let world = world_transform.transform_rect_bbox(*rect);
                layer
                    .children
                    .iter()
                    .filter_map(|child| self.subtree_bounds(*child, world_transform))
                    .reduce(union_rect)
                    .and_then(|bounds| bounds.intersection(world))
            }
            LayerKind::Opacity { .. } => layer
                .children
                .iter()
                .filter_map(|child| self.subtree_bounds(*child, world_transform))
                .reduce(union_rect),
            LayerKind::Blur { .. } | LayerKind::DropShadow { .. } => layer
                .children
                .iter()
                .filter_map(|child| self.subtree_bounds(*child, world_transform))
                .reduce(union_rect),
            LayerKind::ColorFilter { .. } | LayerKind::Blend { .. } => layer
                .children
                .iter()
                .filter_map(|child| self.subtree_bounds(*child, world_transform))
                .reduce(union_rect),
        }
    }
    #[must_use]
    pub fn debug_tree(&self) -> String {
        let mut out = String::new();
        if let Some(root) = self.root {
            self.write_debug(root, 0, &mut out);
        }
        out
    }
    fn write_debug(&self, id: LayerId, depth: usize, out: &mut String) {
        self.write_debug_at(id, None, depth, Transform::IDENTITY, None, out);
    }
    fn write_debug_at(
        &self,
        id: LayerId,
        parent: Option<LayerId>,
        depth: usize,
        world_transform: Transform,
        clip: Option<Rect>,
        out: &mut String,
    ) {
        let Some(layer) = self.layers.get(id.0) else {
            return;
        };
        let indent = "  ".repeat(depth);
        let kind = match &layer.kind {
            LayerKind::Picture { bounds, .. } => format!(
                "Picture(local_bounds={bounds:?}, world_bounds={:?}, clip={clip:?})",
                world_transform.transform_rect_bbox(*bounds)
            ),
            LayerKind::Transform { transform } => {
                format!(
                    "Transform(local={:?}, world={:?})",
                    transform,
                    world_transform.then(*transform)
                )
            }
            LayerKind::ClipRect { rect } => format!(
                "ClipRect(local={rect:?}, world={:?})",
                world_transform.transform_rect_bbox(*rect)
            ),
            LayerKind::Opacity { alpha } => format!(
                "Opacity(alpha={alpha:.3}, bounds={:?}, generation={})",
                self.subtree_bounds(id, world_transform),
                self.subtree_generation(id)
            ),
            LayerKind::Blur { blur } => format!(
                "Blur(sigma=({:.3},{:.3}), bounds={:?}, generation={})",
                blur.sigma_x,
                blur.sigma_y,
                self.subtree_bounds(id, world_transform),
                self.subtree_generation(id)
            ),
            LayerKind::DropShadow { shadow } => format!(
                "DropShadow(sigma=({:.3},{:.3}), offset={:?}, bounds={:?}, generation={})",
                shadow.sigma_x,
                shadow.sigma_y,
                shadow.offset,
                self.subtree_bounds(id, world_transform),
                self.subtree_generation(id)
            ),
            LayerKind::ColorFilter { filter } => format!(
                "ColorFilter(matrix={:?}, bounds={:?}, generation={})",
                filter.to_matrix(),
                self.subtree_bounds(id, world_transform),
                self.subtree_generation(id)
            ),
            LayerKind::Blend { mode } => format!(
                "Blend(mode={mode:?}, bounds={:?}, generation={})",
                self.subtree_bounds(id, world_transform),
                self.subtree_generation(id)
            ),
        };
        out.push_str(&format!(
            "{indent}{id:?} parent={parent:?} {kind}, dirty={:?}\n",
            layer.dirty
        ));
        let (next_translation, next_clip) = match layer.kind {
            LayerKind::Picture { .. } => (world_transform, clip),
            LayerKind::Transform { transform: local } => (world_transform.then(local), clip),
            LayerKind::ClipRect { rect } => {
                let world = world_transform.transform_rect_bbox(rect);
                (
                    world_transform,
                    Some(clip.map_or(world, |old| old.intersection(world).unwrap_or(world))),
                )
            }
            LayerKind::Opacity { .. } => (world_transform, clip),
            LayerKind::Blur { .. } | LayerKind::DropShadow { .. } => (world_transform, clip),
            LayerKind::ColorFilter { .. } | LayerKind::Blend { .. } => (world_transform, clip),
        };
        for child in &layer.children {
            self.write_debug_at(
                *child,
                Some(id),
                depth + 1,
                next_translation,
                next_clip,
                out,
            );
        }
    }
}
