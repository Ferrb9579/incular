use crate::display_list::{DisplayList, PaintCommand, SurfacePartitionId};
use crate::effects::{
    BlendMode, ColorFilter, DropShadowEffect, GaussianBlur, blur_bounds, drop_shadow_bounds,
};
use crate::geometry::union_rect;
use crate::gradients::Brush;
use incular_core::{Arena, ArenaId, DirtyFlags, Offset, Rect, Size, Transform};
use std::{
    any::{Any, TypeId},
    cell::RefCell,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

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

/// A type-erased application annotation carried by an annotated compositing
/// region. The value remains strongly typed at the API boundary: consumers
/// query it with [`LayerTree::find_annotation`] and receive only values of the
/// requested concrete type. Native platform adapters can therefore project
/// their own typed metadata without making the renderer depend on a platform
/// crate.
#[derive(Clone)]
pub struct Annotation {
    value: Rc<dyn Any>,
    type_id: TypeId,
}
impl Annotation {
    #[must_use]
    pub fn new<T: Any>(value: T) -> Self {
        Self {
            value: Rc::new(value),
            type_id: TypeId::of::<T>(),
        }
    }
    #[must_use]
    pub const fn value_type_id(&self) -> TypeId {
        self.type_id
    }
    #[must_use]
    pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
        self.value.downcast_ref::<T>()
    }
    #[must_use]
    pub fn cloned<T: Any + Clone>(&self) -> Option<T> {
        self.downcast_ref::<T>().cloned()
    }
}
impl std::fmt::Debug for Annotation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Annotation")
            .field("type_id", &self.type_id)
            .finish_non_exhaustive()
    }
}
impl PartialEq for Annotation {
    fn eq(&self, other: &Self) -> bool {
        self.type_id == other.type_id && Rc::ptr_eq(&self.value, &other.value)
    }
}

/// Normalized anchor used by leader/follower layers. `(-1, -1)` is the
/// top-left corner, `(0, 0)` the center, and `(1, 1)` the bottom-right.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LayerAnchor {
    pub x: f32,
    pub y: f32,
}
impl LayerAnchor {
    pub const TOP_LEFT: Self = Self { x: -1., y: -1. };

    #[must_use]
    pub fn new(x: f32, y: f32) -> Self {
        Self {
            x: if x.is_finite() { x } else { 0. },
            y: if y.is_finite() { y } else { 0. },
        }
    }
    #[must_use]
    pub const fn along_size(self, size: Size) -> Offset {
        Offset::new(
            size.width * (self.x + 1.) * 0.5,
            size.height * (self.y + 1.) * 0.5,
        )
    }
}

/// World-space point where a follower's anchor must land, resolved entirely
/// in leader space. The compositor and the widget tree share this half of
/// the projection; each side then inverts it into its own retained frame
/// (layer world vs. render parent), which is why the callers still differ.
#[must_use]
pub fn resolve_follower_target(
    leader_transform: Transform,
    leader_size: Size,
    target_anchor: LayerAnchor,
    offset: Offset,
) -> Offset {
    let target_point = leader_transform.transform_point(target_anchor.along_size(leader_size));
    let leader_origin = leader_transform.transform_point(Offset::ZERO);
    target_point + (leader_transform.transform_point(offset) - leader_origin)
}

#[derive(Clone, Copy, Debug)]
struct LeaderData {
    owner: u64,
    transform: Transform,
    size: Size,
    generation: u64,
}

/// Process-wide publication identity minted per leader layer. A link may be
/// shared across layer trees whose arena indices overlap, so a tree-local
/// `LayerId` cannot name the publisher; the token travels inside the leader
/// layer itself. Zero is never minted and means "no owner".
static NEXT_LINK_PUBLISHER: AtomicU64 = AtomicU64::new(1);

fn mint_link_publisher() -> u64 {
    let mut id = NEXT_LINK_PUBLISHER.fetch_add(1, Ordering::Relaxed);
    if id == 0 {
        id = NEXT_LINK_PUBLISHER.fetch_add(1, Ordering::Relaxed);
    }
    id
}

/// Shared identity used to resolve a composited target and any number of
/// followers across otherwise unrelated retained subtrees.
#[derive(Clone)]
pub struct LayerLink(Rc<RefCell<Option<LeaderData>>>);
impl Default for LayerLink {
    fn default() -> Self {
        Self::new()
    }
}
impl std::fmt::Debug for LayerLink {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LayerLink")
            .field("linked", &self.is_linked())
            .finish()
    }
}
impl PartialEq for LayerLink {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}
impl LayerLink {
    #[must_use]
    pub fn new() -> Self {
        Self(Rc::new(RefCell::new(None)))
    }
    #[must_use]
    pub fn is_linked(&self) -> bool {
        self.0.borrow().is_some()
    }
    #[must_use]
    pub fn leader_size(&self) -> Option<Size> {
        self.0.borrow().as_ref().map(|leader| leader.size)
    }
    #[must_use]
    pub fn leader_transform(&self) -> Option<Transform> {
        self.0.borrow().as_ref().map(|leader| leader.transform)
    }
    #[must_use]
    pub fn leader_generation(&self) -> u64 {
        self.0
            .borrow()
            .as_ref()
            .map_or(0, |leader| leader.generation)
    }
    /// Releases the publication only when `owner` made it. Removing or
    /// rebinding any other leader layer must not disturb the winner.
    fn clear_if_owned_by(&self, owner: u64) {
        let mut state = self.0.borrow_mut();
        if state.is_some_and(|leader| leader.owner == owner) {
            *state = None;
        }
    }
    /// Unconditional frame reset. The resolution pass republishes every live
    /// leader right after, so no reader observes the gap.
    fn clear_publication(&self) {
        *self.0.borrow_mut() = None;
    }
    fn publish(&self, owner: u64, transform: Transform, size: Size, generation: u64) {
        // One target per link and the first target in paint order wins.
        // Keeping that rule deterministic is preferable to allowing a later
        // subtree to move an already published follower.
        let mut state = self.0.borrow_mut();
        if state.is_none() {
            *state = Some(LeaderData {
                owner,
                transform,
                size,
                generation,
            });
        }
    }
}

/// Annotation placement captured by the most recent layer flatten. A `None`
/// bounds value represents `sized: false`, which participates at every point
/// in the scene subject to ancestor clipping.
#[derive(Clone, Debug, PartialEq)]
pub struct FlattenedAnnotation {
    pub annotation: Annotation,
    pub world_bounds: Option<Rect>,
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
    ShaderMask {
        shader: Brush,
        blend_mode: BlendMode,
        mask_size: Size,
        mask_transform: Transform,
    },
    BackdropFilter {
        blur: GaussianBlur,
        blend_mode: BlendMode,
        enabled: bool,
    },
    AnnotatedRegion {
        annotation: Annotation,
        sized: bool,
        size: Size,
    },
    Leader {
        link: LayerLink,
        size: Size,
        publisher: u64,
    },
    Follower {
        link: LayerLink,
        show_when_unlinked: bool,
        offset: Offset,
        target_anchor: LayerAnchor,
        follower_anchor: LayerAnchor,
        size: Size,
    },
}

#[derive(Clone, Debug)]
struct Layer {
    kind: LayerKind,
    children: Vec<LayerId>,
    surface_partition: Option<SurfacePartitionId>,
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
    flattened_annotations: Vec<FlattenedAnnotation>,
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
            flattened_annotations: Vec::new(),
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
    #[doc(hidden)]
    #[must_use]
    pub fn contains(&self, id: LayerId) -> bool {
        self.layers.contains(id.0)
    }
    pub fn set_root(&mut self, root: LayerId) {
        self.root = Some(root);
    }

    /// Returns the retained scene's conservative world-space visual bounds
    /// after compositor transforms, rectangular clipping, blur support, and
    /// drop-shadow support have been applied.
    ///
    /// This is renderer-independent geometry. It is useful to embedders that
    /// need to size a native host around retained content without inspecting
    /// widget types or GPU commands.
    #[must_use]
    pub fn scene_bounds(&self) -> Option<Rect> {
        self.root
            .and_then(|root| self.visual_subtree_bounds(root, Transform::IDENTITY))
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
    /// Creates an isolated shader-mask stage. The callback is evaluated by
    /// the retained widget during paint; this layer only owns the resolved,
    /// renderer-neutral shader description and its local coordinate space.
    pub fn create_shader_mask(
        &mut self,
        shader: Brush,
        blend_mode: BlendMode,
        mask_size: Size,
        mask_transform: Transform,
    ) -> LayerId {
        self.insert(LayerKind::ShaderMask {
            shader,
            blend_mode,
            mask_size,
            mask_transform,
        })
    }
    /// Creates a backdrop-filter stage. Unlike [`Self::create_blur`], this
    /// stage samples the already painted destination before its child is
    /// composited.
    pub fn create_backdrop_filter(
        &mut self,
        blur: GaussianBlur,
        blend_mode: BlendMode,
        enabled: bool,
    ) -> LayerId {
        self.insert(LayerKind::BackdropFilter {
            blur: GaussianBlur::new(blur.sigma_x, blur.sigma_y),
            blend_mode,
            enabled,
        })
    }
    pub fn create_annotated_region(
        &mut self,
        annotation: Annotation,
        sized: bool,
        size: Size,
    ) -> LayerId {
        self.insert(LayerKind::AnnotatedRegion {
            annotation,
            sized,
            size,
        })
    }
    pub fn create_leader(&mut self, link: LayerLink, size: Size) -> LayerId {
        self.insert(LayerKind::Leader {
            link,
            size,
            publisher: mint_link_publisher(),
        })
    }
    pub fn create_follower(
        &mut self,
        link: LayerLink,
        show_when_unlinked: bool,
        offset: Offset,
        target_anchor: LayerAnchor,
        follower_anchor: LayerAnchor,
        size: Size,
    ) -> LayerId {
        self.insert(LayerKind::Follower {
            link,
            show_when_unlinked,
            offset,
            target_anchor,
            follower_anchor,
            size,
        })
    }
    fn insert(&mut self, kind: LayerKind) -> LayerId {
        let generation = self.next_generation;
        self.next_generation = self.next_generation.wrapping_add(1).max(1);
        let id = LayerId(self.layers.insert(Layer {
            kind,
            children: Vec::new(),
            surface_partition: None,
            dirty: DirtyFlags::COMPOSITE,
            generation,
        }));
        self.diagnostics.layers += 1;
        id
    }
    pub fn remove(&mut self, id: LayerId) {
        if let Some(layer) = self.layers.remove(id.0) {
            // A removed leader stops resolving only when it owned the
            // publication: its shared link state would otherwise outlive it
            // and followers would track a ghost. A non-owner's removal must
            // not disturb the winner.
            if let LayerKind::Leader {
                link, publisher, ..
            } = &layer.kind
            {
                link.clear_if_owned_by(*publisher);
            }
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

    /// Marks one retained layer subtree as detachable presentation content.
    /// The marker is renderer-neutral and does not alter layout, painting, or
    /// the retained layer topology.
    pub fn set_surface_partition(&mut self, id: LayerId, partition: Option<SurfacePartitionId>) {
        let Some(layer) = self.layers.get_mut(id.0) else {
            return;
        };
        if layer.surface_partition == partition {
            return;
        }
        layer.surface_partition = partition;
        layer.dirty.insert(DirtyFlags::COMPOSITE);
    }

    /// Clears every detachable-surface marker. WidgetTree reapplies markers for
    /// the currently visible transient portals before flattening each frame.
    pub fn clear_surface_partitions(&mut self) {
        let ids = self
            .layers
            .iter()
            .filter_map(|(raw, layer)| layer.surface_partition.is_some().then_some(LayerId(raw)))
            .collect::<Vec<_>>();
        for id in ids {
            self.set_surface_partition(id, None);
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
    pub fn update_shader_mask(
        &mut self,
        id: LayerId,
        shader: Brush,
        blend_mode: BlendMode,
        mask_size: Size,
        mask_transform: Transform,
    ) -> bool {
        let Some(layer) = self.layers.get_mut(id.0) else {
            return false;
        };
        let LayerKind::ShaderMask {
            shader: current_shader,
            blend_mode: current_mode,
            mask_size: current_size,
            mask_transform: current_transform,
        } = &mut layer.kind
        else {
            return false;
        };
        if *current_shader == shader
            && *current_mode == blend_mode
            && *current_size == mask_size
            && *current_transform == mask_transform
        {
            return false;
        }
        *current_shader = shader;
        *current_mode = blend_mode;
        *current_size = mask_size;
        *current_transform = mask_transform;
        layer.dirty.insert(DirtyFlags::COMPOSITE);
        layer.generation = self.next_generation;
        self.next_generation = self.next_generation.wrapping_add(1).max(1);
        true
    }
    pub fn update_shader_mask_blend(&mut self, id: LayerId, blend_mode: BlendMode) -> bool {
        let Some(layer) = self.layers.get_mut(id.0) else {
            return false;
        };
        let LayerKind::ShaderMask {
            blend_mode: current,
            ..
        } = &mut layer.kind
        else {
            return false;
        };
        if *current == blend_mode {
            return false;
        }
        *current = blend_mode;
        layer.dirty.insert(DirtyFlags::COMPOSITE);
        layer.generation = self.next_generation;
        self.next_generation = self.next_generation.wrapping_add(1).max(1);
        true
    }
    pub fn update_backdrop_filter(
        &mut self,
        id: LayerId,
        blur: GaussianBlur,
        blend_mode: BlendMode,
        enabled: bool,
    ) -> bool {
        let Some(layer) = self.layers.get_mut(id.0) else {
            return false;
        };
        let LayerKind::BackdropFilter {
            blur: current_blur,
            blend_mode: current_mode,
            enabled: current_enabled,
        } = &mut layer.kind
        else {
            return false;
        };
        let blur = GaussianBlur::new(blur.sigma_x, blur.sigma_y);
        if *current_blur == blur && *current_mode == blend_mode && *current_enabled == enabled {
            return false;
        }
        *current_blur = blur;
        *current_mode = blend_mode;
        *current_enabled = enabled;
        layer.dirty.insert(DirtyFlags::COMPOSITE);
        layer.generation = self.next_generation;
        self.next_generation = self.next_generation.wrapping_add(1).max(1);
        true
    }
    pub fn update_annotated_region(
        &mut self,
        id: LayerId,
        annotation: Annotation,
        sized: bool,
        size: Size,
    ) -> bool {
        let Some(layer) = self.layers.get_mut(id.0) else {
            return false;
        };
        let LayerKind::AnnotatedRegion {
            annotation: current_annotation,
            sized: current_sized,
            size: current_size,
        } = &mut layer.kind
        else {
            return false;
        };
        if *current_annotation == annotation && *current_sized == sized && *current_size == size {
            return false;
        }
        *current_annotation = annotation;
        *current_sized = sized;
        *current_size = size;
        layer.dirty.insert(DirtyFlags::COMPOSITE);
        layer.generation = self.next_generation;
        self.next_generation = self.next_generation.wrapping_add(1).max(1);
        true
    }
    pub fn update_leader(&mut self, id: LayerId, link: LayerLink, size: Size) -> bool {
        let Some(layer) = self.layers.get_mut(id.0) else {
            return false;
        };
        let LayerKind::Leader {
            link: current_link,
            size: current_size,
            publisher,
        } = &mut layer.kind
        else {
            return false;
        };
        if *current_link == link && *current_size == size {
            return false;
        }
        // The rebound layer releases only its own publication; a non-owner
        // rebind must not clear the winner it never displaced.
        let publisher = *publisher;
        current_link.clear_if_owned_by(publisher);
        *current_link = link;
        *current_size = size;
        layer.dirty.insert(DirtyFlags::COMPOSITE);
        layer.generation = self.next_generation;
        self.next_generation = self.next_generation.wrapping_add(1).max(1);
        true
    }
    pub fn update_leader_size(&mut self, id: LayerId, size: Size) -> bool {
        let Some(layer) = self.layers.get_mut(id.0) else {
            return false;
        };
        let LayerKind::Leader { size: current, .. } = &mut layer.kind else {
            return false;
        };
        if *current == size {
            return false;
        }
        *current = size;
        layer.dirty.insert(DirtyFlags::COMPOSITE);
        layer.generation = self.next_generation;
        self.next_generation = self.next_generation.wrapping_add(1).max(1);
        true
    }
    // Keep the follower fields explicit to preserve the public update API and
    // make each retained follower property visible at the call site.
    #[allow(clippy::too_many_arguments)]
    pub fn update_follower(
        &mut self,
        id: LayerId,
        link: LayerLink,
        show_when_unlinked: bool,
        offset: Offset,
        target_anchor: LayerAnchor,
        follower_anchor: LayerAnchor,
        size: Size,
    ) -> bool {
        let Some(layer) = self.layers.get_mut(id.0) else {
            return false;
        };
        let LayerKind::Follower {
            link: current_link,
            show_when_unlinked: current_show,
            offset: current_offset,
            target_anchor: current_target,
            follower_anchor: current_follower,
            size: current_size,
        } = &mut layer.kind
        else {
            return false;
        };
        if *current_link == link
            && *current_show == show_when_unlinked
            && *current_offset == offset
            && *current_target == target_anchor
            && *current_follower == follower_anchor
            && *current_size == size
        {
            return false;
        }
        *current_link = link;
        *current_show = show_when_unlinked;
        *current_offset = offset;
        *current_target = target_anchor;
        *current_follower = follower_anchor;
        *current_size = size;
        layer.dirty.insert(DirtyFlags::COMPOSITE);
        layer.generation = self.next_generation;
        self.next_generation = self.next_generation.wrapping_add(1).max(1);
        true
    }
    /// World-space logical bounds captured by the most recent [`Self::flatten`]
    /// call. This keeps coordinate diagnostics out of the renderer hot path.
    #[must_use]
    pub fn flattened_pictures(&self) -> &[FlattenedPicture] {
        &self.flattened_pictures
    }
    #[must_use]
    pub fn flattened_annotations(&self) -> &[FlattenedAnnotation] {
        &self.flattened_annotations
    }
    /// Returns the front-most annotation of type `T` at `point`. Annotation
    /// lookup follows layer paint order, honours ancestor clips, and treats
    /// `sized: false` regions as covering the entire clipped subtree.
    #[must_use]
    pub fn find_annotation<T: Any + Clone>(&self, point: Offset) -> Option<T> {
        self.flattened_annotations.iter().find_map(|entry| {
            let in_bounds = entry
                .world_bounds
                .is_none_or(|bounds| bounds.contains(point));
            in_bounds.then(|| entry.annotation.cloned::<T>()).flatten()
        })
    }
    #[must_use]
    pub fn find_annotations<T: Any + Clone>(&self, point: Offset) -> Vec<T> {
        self.flattened_annotations
            .iter()
            .filter_map(|entry| {
                let in_bounds = entry
                    .world_bounds
                    .is_none_or(|bounds| bounds.contains(point));
                in_bounds.then(|| entry.annotation.cloned::<T>()).flatten()
            })
            .collect()
    }
    #[must_use]
    pub fn flatten(&mut self) -> DisplayList {
        let mut out = DisplayList::new();
        self.flattened_pictures.clear();
        self.flattened_annotations.clear();
        if let Some(root) = self.root {
            self.clear_link_states(root);
            // Leader publication runs as its own pass before any follower
            // resolves, so flatten order cannot strand a follower behind
            // its leader: paint, hit testing, and semantics all read the
            // same post-publication state.
            self.publish_leaders(root, Transform::IDENTITY);
            self.flatten_layer(root, Transform::IDENTITY, None, &mut out);
            self.collect_annotations(root, Transform::IDENTITY, None);
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
        let partition = self
            .layers
            .get(id.0)
            .and_then(|layer| layer.surface_partition);
        if let Some(id) = partition {
            out.push(PaintCommand::PushSurfacePartition { id });
        }
        self.flatten_layer_contents(id, world_transform, clip, out);
        if partition.is_some() {
            out.push(PaintCommand::PopSurfacePartition);
        }
    }

    fn flatten_layer_contents(
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
            LayerKind::ShaderMask {
                shader,
                blend_mode,
                mask_size,
                mask_transform,
            } => {
                let bounds = self
                    .subtree_bounds(id, world_transform)
                    .unwrap_or_else(|| Rect::from_origin_size(Offset::ZERO, Size::ZERO));
                if !bounds.size.width.is_finite()
                    || !bounds.size.height.is_finite()
                    || bounds.size.width <= 0.
                    || bounds.size.height <= 0.
                {
                    return;
                }
                out.push(PaintCommand::PushShaderMask {
                    layer: id,
                    shader,
                    blend_mode,
                    mask_size,
                    mask_transform,
                    generation: self.subtree_generation(id),
                    bounds,
                });
                for child in layer.children {
                    self.flatten_layer(child, world_transform, clip, out);
                }
                out.push(PaintCommand::PopEffect);
            }
            LayerKind::BackdropFilter {
                blur,
                blend_mode,
                enabled,
            } => {
                if !enabled || (blur.sigma_x <= f32::EPSILON && blur.sigma_y <= f32::EPSILON) {
                    for child in layer.children {
                        self.flatten_layer(child, world_transform, clip, out);
                    }
                    return;
                }
                let bounds = self
                    .subtree_bounds(id, world_transform)
                    .unwrap_or_else(|| Rect::from_origin_size(Offset::ZERO, Size::ZERO));
                out.push(PaintCommand::PushBackdropFilter {
                    layer: id,
                    blur,
                    blend_mode,
                    enabled,
                    generation: self.subtree_generation(id),
                    bounds,
                });
                for child in layer.children {
                    self.flatten_layer(child, world_transform, clip, out);
                }
                out.push(PaintCommand::PopEffect);
            }
            LayerKind::AnnotatedRegion { .. } => {
                for child in layer.children {
                    self.flatten_layer(child, world_transform, clip, out);
                }
            }
            LayerKind::Leader { .. } => {
                // Publication already ran in the dedicated pass above; the
                // flatten walk only positions children here.
                for child in layer.children {
                    self.flatten_layer(child, world_transform, clip, out);
                }
            }
            LayerKind::Follower {
                link,
                show_when_unlinked,
                offset,
                target_anchor,
                follower_anchor,
                size,
            } => {
                let Some(next_transform) = self.follower_transform(
                    world_transform,
                    link,
                    show_when_unlinked,
                    offset,
                    target_anchor,
                    follower_anchor,
                    size,
                ) else {
                    return;
                };
                for child in layer.children {
                    self.flatten_layer(child, next_transform, clip, out);
                }
            }
        }
    }
    fn clear_link_states(&self, id: LayerId) {
        let Some(layer) = self.layers.get(id.0) else {
            return;
        };
        if let LayerKind::Leader { link, .. } = &layer.kind {
            link.clear_publication();
        }
        let children = layer.children.clone();
        for child in children {
            self.clear_link_states(child);
        }
    }
    fn publish_leaders(&self, id: LayerId, world_transform: Transform) {
        let Some(layer) = self.layers.get(id.0) else {
            return;
        };
        if let LayerKind::Leader {
            link,
            size,
            publisher,
        } = &layer.kind
        {
            link.publish(*publisher, world_transform, *size, layer.generation);
        }
        let next = match &layer.kind {
            LayerKind::Transform { transform } => world_transform.then(*transform),
            _ => world_transform,
        };
        let children = layer.children.clone();
        for child in children {
            self.publish_leaders(child, next);
        }
    }
    // Keep the transform inputs explicit so this helper mirrors the retained
    // follower state without changing the existing call-site contract.
    #[allow(clippy::too_many_arguments)]
    fn follower_transform(
        &self,
        world_transform: Transform,
        link: LayerLink,
        show_when_unlinked: bool,
        offset: Offset,
        target_anchor: LayerAnchor,
        follower_anchor: LayerAnchor,
        size: Size,
    ) -> Option<Transform> {
        let Some(leader_transform) = link.leader_transform() else {
            return show_when_unlinked.then_some(world_transform);
        };
        let leader_size = link.leader_size().unwrap_or(Size::ZERO);
        let desired_anchor =
            resolve_follower_target(leader_transform, leader_size, target_anchor, offset);
        let desired_local = world_transform.inverse_transform_point(desired_anchor)?;
        let local_delta = desired_local - follower_anchor.along_size(size);
        Some(world_transform.then(Transform::translation(local_delta)))
    }
    fn collect_annotations(&mut self, id: LayerId, world_transform: Transform, clip: Option<Rect>) {
        let Some(layer) = self.layers.get(id.0).cloned() else {
            return;
        };
        match layer.kind {
            LayerKind::Transform { transform } => {
                let next = world_transform.then(transform);
                for child in layer.children {
                    self.collect_annotations(child, next, clip);
                }
            }
            LayerKind::ClipRect { rect } => {
                let world = world_transform.transform_rect_bbox(rect);
                let next_clip = match clip {
                    Some(current) => current.intersection(world),
                    None => Some(world),
                };
                if next_clip.is_none() {
                    return;
                }
                for child in layer.children {
                    self.collect_annotations(child, world_transform, next_clip);
                }
            }
            LayerKind::Follower {
                link,
                show_when_unlinked,
                offset,
                target_anchor,
                follower_anchor,
                size,
            } => {
                let Some(next) = self.follower_transform(
                    world_transform,
                    link,
                    show_when_unlinked,
                    offset,
                    target_anchor,
                    follower_anchor,
                    size,
                ) else {
                    return;
                };
                for child in layer.children {
                    self.collect_annotations(child, next, clip);
                }
            }
            LayerKind::AnnotatedRegion {
                annotation,
                sized,
                size,
            } => {
                // Container layers are painted in child order, but annotation
                // lookup walks visually front-to-back. Visit children in
                // reverse order, then append this layer's own annotation.
                for child in layer.children.iter().rev() {
                    self.collect_annotations(*child, world_transform, clip);
                }
                let region =
                    world_transform.transform_rect_bbox(Rect::from_origin_size(Offset::ZERO, size));
                let world_bounds = if sized {
                    match clip {
                        Some(active) => active.intersection(region),
                        None => Some(region),
                    }
                } else {
                    clip
                };
                if world_bounds
                    .is_none_or(|bounds| bounds.size.width > 0. && bounds.size.height > 0.)
                {
                    self.flattened_annotations.push(FlattenedAnnotation {
                        annotation,
                        world_bounds,
                    });
                }
            }
            LayerKind::Picture { .. }
            | LayerKind::Opacity { .. }
            | LayerKind::Blur { .. }
            | LayerKind::DropShadow { .. }
            | LayerKind::ColorFilter { .. }
            | LayerKind::Blend { .. }
            | LayerKind::ShaderMask { .. }
            | LayerKind::BackdropFilter { .. }
            | LayerKind::Leader { .. } => {
                for child in layer.children {
                    self.collect_annotations(child, world_transform, clip);
                }
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
                    LayerKind::ShaderMask {
                        shader,
                        blend_mode,
                        mask_size,
                        mask_transform,
                    } => {
                        value = value.rotate_left(13)
                            ^ brush_generation(shader)
                            ^ u64::from(blend_mode.code())
                            ^ u64::from(mask_size.width.to_bits()).rotate_left(9)
                            ^ u64::from(mask_size.height.to_bits()).rotate_left(15)
                            ^ transform_generation(*mask_transform);
                    }
                    LayerKind::BackdropFilter {
                        blur,
                        blend_mode,
                        enabled,
                    } => {
                        value = value.rotate_left(13)
                            ^ u64::from(blur.sigma_x.to_bits())
                            ^ u64::from(blur.sigma_y.to_bits()).rotate_left(7)
                            ^ u64::from(blend_mode.code())
                            ^ u64::from(*enabled as u8);
                    }
                    LayerKind::AnnotatedRegion {
                        annotation: _,
                        sized,
                        size,
                    } => {
                        value = value.rotate_left(13)
                            ^ u64::from(*sized as u8)
                            ^ u64::from(size.width.to_bits()).rotate_left(9)
                            ^ u64::from(size.height.to_bits()).rotate_left(15);
                    }
                    LayerKind::Leader { link, size, .. } => {
                        value = value.rotate_left(13)
                            ^ link.leader_generation()
                            ^ u64::from(size.width.to_bits()).rotate_left(9)
                            ^ u64::from(size.height.to_bits()).rotate_left(15);
                    }
                    LayerKind::Follower {
                        link,
                        show_when_unlinked,
                        offset,
                        target_anchor,
                        follower_anchor,
                        size,
                    } => {
                        value = value.rotate_left(13)
                            ^ link.leader_generation()
                            ^ u64::from(*show_when_unlinked as u8)
                            ^ u64::from(offset.x.to_bits())
                            ^ u64::from(offset.y.to_bits()).rotate_left(7)
                            ^ u64::from(target_anchor.x.to_bits()).rotate_left(13)
                            ^ u64::from(target_anchor.y.to_bits()).rotate_left(17)
                            ^ u64::from(follower_anchor.x.to_bits()).rotate_left(21)
                            ^ u64::from(follower_anchor.y.to_bits()).rotate_left(25)
                            ^ u64::from(size.width.to_bits()).rotate_left(29)
                            ^ u64::from(size.height.to_bits()).rotate_left(31);
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
            LayerKind::ShaderMask { .. }
            | LayerKind::BackdropFilter { .. }
            | LayerKind::AnnotatedRegion { .. }
            | LayerKind::Leader { .. } => layer
                .children
                .iter()
                .filter_map(|child| self.subtree_bounds(*child, world_transform))
                .reduce(union_rect),
            LayerKind::Follower {
                link,
                show_when_unlinked,
                offset,
                target_anchor,
                follower_anchor,
                size,
            } => {
                let next = self.follower_transform(
                    world_transform,
                    link.clone(),
                    *show_when_unlinked,
                    *offset,
                    *target_anchor,
                    *follower_anchor,
                    *size,
                )?;
                layer
                    .children
                    .iter()
                    .filter_map(|child| self.subtree_bounds(*child, next))
                    .reduce(union_rect)
            }
        }
    }

    fn visual_subtree_bounds(&self, id: LayerId, world_transform: Transform) -> Option<Rect> {
        let layer = self.layers.get(id.0)?;
        let child_bounds = |tree: &Self, transform: Transform| {
            layer
                .children
                .iter()
                .filter_map(|child| tree.visual_subtree_bounds(*child, transform))
                .reduce(union_rect)
        };
        match &layer.kind {
            LayerKind::Picture { bounds, .. } => Some(world_transform.transform_rect_bbox(*bounds)),
            LayerKind::Transform { transform: local } => {
                child_bounds(self, world_transform.then(*local))
            }
            LayerKind::ClipRect { rect } => {
                let world = world_transform.transform_rect_bbox(*rect);
                child_bounds(self, world_transform).and_then(|bounds| bounds.intersection(world))
            }
            LayerKind::Opacity { .. }
            | LayerKind::ColorFilter { .. }
            | LayerKind::Blend { .. }
            | LayerKind::ShaderMask { .. }
            | LayerKind::BackdropFilter { .. }
            | LayerKind::AnnotatedRegion { .. }
            | LayerKind::Leader { .. } => child_bounds(self, world_transform),
            LayerKind::Blur { blur } => child_bounds(self, world_transform)
                .map(|bounds| blur_bounds(bounds, blur.sigma_x, blur.sigma_y)),
            LayerKind::DropShadow { shadow } => child_bounds(self, world_transform).map(|bounds| {
                drop_shadow_bounds(bounds, shadow.offset, shadow.sigma_x, shadow.sigma_y)
            }),
            LayerKind::Follower {
                link,
                show_when_unlinked,
                offset,
                target_anchor,
                follower_anchor,
                size,
            } => {
                let next = self.follower_transform(
                    world_transform,
                    link.clone(),
                    *show_when_unlinked,
                    *offset,
                    *target_anchor,
                    *follower_anchor,
                    *size,
                )?;
                child_bounds(self, next)
            }
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
            LayerKind::ShaderMask {
                blend_mode,
                mask_size,
                ..
            } => format!(
                "ShaderMask(blend_mode={blend_mode:?}, mask_size={mask_size:?}, bounds={:?}, generation={})",
                self.subtree_bounds(id, world_transform),
                self.subtree_generation(id)
            ),
            LayerKind::BackdropFilter {
                blur,
                blend_mode,
                enabled,
            } => format!(
                "BackdropFilter(sigma=({:.3},{:.3}), blend_mode={blend_mode:?}, enabled={enabled}, bounds={:?}, generation={})",
                blur.sigma_x,
                blur.sigma_y,
                self.subtree_bounds(id, world_transform),
                self.subtree_generation(id)
            ),
            LayerKind::AnnotatedRegion { sized, size, .. } => format!(
                "AnnotatedRegion(sized={sized}, size={size:?}, generation={})",
                self.subtree_generation(id)
            ),
            LayerKind::Leader { link, size, .. } => format!(
                "CompositedTransformTarget(size={size:?}, linked={}, generation={})",
                link.is_linked(),
                self.subtree_generation(id)
            ),
            LayerKind::Follower {
                show_when_unlinked,
                offset,
                target_anchor,
                follower_anchor,
                ..
            } => format!(
                "CompositedTransformFollower(show_when_unlinked={show_when_unlinked}, offset={offset:?}, target_anchor={target_anchor:?}, follower_anchor={follower_anchor:?}, bounds={:?}, generation={})",
                self.subtree_bounds(id, world_transform),
                self.subtree_generation(id)
            ),
        };
        out.push_str(&format!(
            "{indent}{id:?} parent={parent:?} {kind}, dirty={:?}\n",
            layer.dirty
        ));
        let (next_translation, next_clip) = match &layer.kind {
            LayerKind::Picture { .. } => (world_transform, clip),
            LayerKind::Transform { transform: local } => (world_transform.then(*local), clip),
            LayerKind::ClipRect { rect } => {
                let world = world_transform.transform_rect_bbox(*rect);
                (
                    world_transform,
                    Some(clip.map_or(world, |old| old.intersection(world).unwrap_or(world))),
                )
            }
            LayerKind::Opacity { .. } => (world_transform, clip),
            LayerKind::Blur { .. } | LayerKind::DropShadow { .. } => (world_transform, clip),
            LayerKind::ColorFilter { .. } | LayerKind::Blend { .. } => (world_transform, clip),
            LayerKind::ShaderMask { .. }
            | LayerKind::BackdropFilter { .. }
            | LayerKind::AnnotatedRegion { .. }
            | LayerKind::Leader { .. } => (world_transform, clip),
            LayerKind::Follower {
                link,
                show_when_unlinked,
                offset,
                target_anchor,
                follower_anchor,
                size,
            } => (
                self.follower_transform(
                    world_transform,
                    link.clone(),
                    *show_when_unlinked,
                    *offset,
                    *target_anchor,
                    *follower_anchor,
                    *size,
                )
                .unwrap_or(world_transform),
                clip,
            ),
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

fn brush_generation(brush: &Brush) -> u64 {
    match brush {
        Brush::Solid(color) => {
            u64::from(color.red)
                | (u64::from(color.green) << 8)
                | (u64::from(color.blue) << 16)
                | (u64::from(color.alpha) << 24)
        }
        Brush::LinearGradient(gradient) => {
            gradient.stops.id().get()
                ^ u64::from(gradient.start.x.to_bits()).rotate_left(7)
                ^ u64::from(gradient.start.y.to_bits()).rotate_left(13)
                ^ u64::from(gradient.end.x.to_bits()).rotate_left(19)
                ^ u64::from(gradient.end.y.to_bits()).rotate_left(23)
        }
        Brush::RadialGradient(gradient) => {
            gradient.stops.id().get()
                ^ u64::from(gradient.center.x.to_bits()).rotate_left(7)
                ^ u64::from(gradient.center.y.to_bits()).rotate_left(13)
                ^ u64::from(gradient.radius.to_bits()).rotate_left(19)
        }
        Brush::SweepGradient(gradient) => {
            gradient.stops.id().get()
                ^ u64::from(gradient.center.x.to_bits()).rotate_left(7)
                ^ u64::from(gradient.center.y.to_bits()).rotate_left(13)
                ^ u64::from(gradient.start_angle.to_bits()).rotate_left(19)
        }
    }
}

fn transform_generation(transform: Transform) -> u64 {
    transform
        .to_kurbo()
        .as_coeffs()
        .iter()
        .enumerate()
        .fold(0, |value, (index, coefficient)| {
            value ^ coefficient.to_bits().rotate_left((index as u32 * 9) % 63)
        })
}
