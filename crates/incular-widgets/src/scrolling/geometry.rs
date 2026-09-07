use super::*;
use std::any::Any;

/// Stable identity for a child materialized by a sliver.
///
/// The low 32 bits are owned by the leaf sliver. The high 32 bits contain a
/// compact stack of eight-bit group scopes, with the innermost scope in the
/// least-significant byte. Keeping the scope path in the ID means nested main
/// and cross-axis groups cannot accidentally reuse each other's child state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SliverChildId(pub u64);

impl SliverChildId {
    pub(super) const fn list_item(index: usize) -> Self {
        Self(index as u64 + 1)
    }

    pub(crate) fn item_index(self) -> Option<usize> {
        self.0
            .checked_sub(1)
            .and_then(|index| usize::try_from(index & u32::MAX as u64).ok())
    }

    pub(super) fn scoped(sliver: usize, child: Self) -> Self {
        let path = child.0 >> 32;
        let scope = (sliver as u64).saturating_add(1).min(u8::MAX as u64);
        let path = (((path & 0x00ff_ffff) << 8) | scope) & u32::MAX as u64;
        Self((path << 32) | (child.0 & u32::MAX as u64))
    }

    pub(super) fn scope(self) -> usize {
        let scope = (self.0 >> 32) & u8::MAX as u64;
        scope
            .checked_sub(1)
            .map_or(usize::MAX, |value| value as usize)
    }

    pub(super) fn local(self) -> Self {
        Self((((self.0 >> 32) >> 8) << 32) | (self.0 & u32::MAX as u64))
    }
}

/// One child placement returned by a render sliver.
#[derive(Clone, Debug)]
pub struct SliverChildLayout {
    pub id: SliverChildId,
    pub widget: Widget,
    /// Logical accessibility position. This is independent from `id` because
    /// rows, reorderable keys, and animated entries may use a different
    /// retained identity than their collection position.
    pub semantic_index: Option<usize>,
    /// Main-axis content offset before the viewport scroll transform.
    pub offset: f32,
    /// Cross-axis content offset.
    pub cross_offset: f32,
    /// Box constraints used when the retained child is laid out.
    pub constraints: Constraints,
    /// Measured/estimated main-axis extent used for anchor and pinning math.
    pub extent: f32,
    /// Placement and stacking policy within the viewport.
    pub placement: SliverChildPlacement,
}

/// Retained placement of a sliver child relative to ordinary content.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SliverChildPlacement {
    /// Translate with the content and use normal paint order.
    Flow,
    /// Pin to the viewport edge and paint above flowing content.
    Pinned,
    /// Use sliver-computed placement and paint above flowing content.
    Floating,
}

/// Result of laying out one render sliver.
#[derive(Clone, Debug)]
pub struct SliverLayout {
    pub geometry: SliverGeometry,
    pub children: Vec<SliverChildLayout>,
    /// Overlap absorbed for a following nested viewport.
    pub absorbed_overlap: f32,
}

impl SliverLayout {
    pub(super) fn empty(geometry: SliverGeometry) -> Self {
        Self {
            geometry,
            children: Vec::new(),
            absorbed_overlap: 0.,
        }
    }
}

/// Retained sliver protocol. Implementations receive viewport constraints and
/// return geometry plus only the children needed for the current paint/cache
/// interval. This is the analogue of Flutter's `RenderSliver` contract.
pub trait RenderSliver {
    fn perform_layout(&mut self, constraints: SliverConstraints) -> SliverLayout;

    /// Identifies when scrolling invalidates retained child geometry.
    /// Override for headers or custom slivers whose placement changes inside
    /// an already materialized cache window.
    fn scroll_layout_dependency(&self) -> SliverScrollDependency {
        SliverScrollDependency::CacheWindow
    }

    /// Returns the logical child count when this sliver is backed by an
    /// indexed child delegate. Non-indexed slivers leave it unknown.
    fn child_count(&self) -> Option<usize> {
        None
    }

    /// Records the exact extent measured by the retained child tree.
    fn set_child_extent(&mut self, _child: SliverChildId, _extent: f32) -> bool {
        false
    }

    /// Structural/measurement revision used to invalidate a viewport's
    /// materialized child range without rebuilding the application.
    fn revision(&self) -> u64 {
        0
    }

    /// Advances retained sliver-local animation state without rebuilding the
    /// application widget description. The viewport calls this from the
    /// compositor phase, and schedules layout only when geometry changed.
    fn tick(&mut self, _now: Instant) -> bool {
        false
    }

    /// Whether another frame is required for sliver-local animation.
    fn is_animating(&self) -> bool {
        false
    }

    /// Downcast hook for compatible-state transfer across descriptor updates.
    /// Only slivers with retained measurement override this; the default
    /// keeps every other sliver replaceable without coupling them together.
    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        None
    }

    /// Invalidates retained measurement for one materialized child after its
    /// subtree rebuilt with new content. Routing mirrors
    /// [`RenderSliver::set_child_extent`]: sequences descend by scope and
    /// transparent wrappers forward; measured slivers demote back to an
    /// unverified estimate for unbounded revalidation. The default is a
    /// no-op so stateless slivers ignore content rebuilds.
    fn invalidate_child_measurement(&mut self, _child: SliverChildId) {}
}

/// Scroll input that invalidates a sliver's retained layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SliverScrollDependency {
    /// Existing children can be translated until the cache window is exhausted.
    CacheWindow,
    /// Every scroll offset change can alter child size or placement.
    ScrollOffset,
}

/// Private bridge for widget materialization and viewport-scoped child IDs.
pub(crate) trait SliverViewportDelegate {
    fn scroll_layout_dependency(&self) -> SliverScrollDependency;
    fn perform_layout(&self, constraints: SliverConstraints) -> SliverViewportLayout;
    fn set_child_extent(&self, child: SliverChildId, extent: f32) -> bool;
    fn revision(&self) -> u64;
    fn sliver_count(&self) -> usize;
    fn child_count(&self) -> Option<usize>;
    fn tick(&self, now: Instant) -> bool;
    fn is_animating(&self) -> bool;
    /// Downcast hook for compatible-state transfer. Only sequence delegates
    /// override this; every other delegate keeps the replaceable default.
    fn as_any(&self) -> Option<&dyn Any> {
        None
    }
    /// Inherits compatible retained sliver state (validated measurements,
    /// reversal tracking) from the delegate this one replaces. The default is
    /// a no-op so custom delegates keep existing replacement semantics.
    fn adopt_compatible_state(&self, _previous: &dyn SliverViewportDelegate) {}
    /// Invalidates one viewport-scoped child's retained measurement after a
    /// content rebuild below it. Returns whether a sliver was found to
    /// notify; the default reports none.
    fn invalidate_sliver_child(&self, _child: SliverChildId) -> bool {
        false
    }
}

pub struct SliverViewportConfig {
    pub(crate) controller: ScrollController,
    pub(crate) axis: Axis,
    pub(crate) reverse: bool,
    pub(crate) physics: ScrollPhysics,
    pub(crate) cache_extent: f32,
    pub(crate) shrink_wrap: bool,
    pub(crate) clip_behavior: Clip,
    pub(crate) delegate: Rc<dyn SliverViewportDelegate>,
}

impl std::fmt::Debug for SliverViewportConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SliverViewportConfig")
            .field("axis", &self.axis)
            .field("reverse", &self.reverse)
            .field("physics", &self.physics)
            .field("cache_extent", &self.cache_extent)
            .field("shrink_wrap", &self.shrink_wrap)
            .field("clip_behavior", &self.clip_behavior)
            .field("sliver_count", &self.delegate.sliver_count())
            .finish()
    }
}

impl PartialEq for SliverViewportConfig {
    fn eq(&self, other: &Self) -> bool {
        self.controller == other.controller
            && self.axis == other.axis
            && self.reverse == other.reverse
            && self.physics == other.physics
            && self.cache_extent == other.cache_extent
            && self.shrink_wrap == other.shrink_wrap
            && self.clip_behavior == other.clip_behavior
            && Rc::ptr_eq(&self.delegate, &other.delegate)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SliverViewportLayout {
    pub geometry: SliverGeometry,
    pub children: Vec<SliverChildLayout>,
}
