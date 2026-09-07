//! Declarative widgets backed by persistent element and render-object arenas.
//!
//! A [`Widget`] is a cheap value. [`WidgetTree`] owns mounted identity and all
//! mutable layout/paint state. Reconciliation only examines direct children of
//! the element being updated.

use std::{
    any::{Any, TypeId},
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
    time::{Duration, Instant},
};

use icu_segmenter::GraphemeClusterSegmenter;
use incular_animation::AnimationController;
use incular_config::{
    Alignment, Axis, Clip, Constraints, CrossAxisAlignment, EdgeInsets, FlexFit, MainAxisAlignment,
    MainAxisSize, RuntimeEnvironment, StackFit, TextDirection, VerticalDirection, WrapAlignment,
    WrapCrossAlignment,
};
use incular_core::{
    Arena, ArenaId, BuildContext as DependencyContext, Color, ConsumerId, DirtyFlags, Invalidation,
    KeyboardEvent, KeyboardKey, NamedKey, Offset, Rect, Size, TrackpadGesture,
    TrackpadGesturePhase, Transform as CoreTransform,
};
use incular_image::ImageHandle;
use incular_rendering as incular_painting;
use incular_rendering::{
    Annotation, BlendMode, Border, Brush, ColorFilter, CornerRadii, DisplayList, DropShadowEffect,
    FillRule, FilterQuality, GaussianBlur, ImageSampling, LayerAnchor, LayerLink, LayerTree,
    PaintCommand, Path, RRect, Stroke, normalize_opacity, normalize_sigma,
};
use incular_scroll::{
    ScrollController, ScrollNotification, ScrollNotificationSubscription, ScrollPhysics,
    ScrollbarGeometry, SliverConstraints, scrollbar_geometry,
};
use incular_semantics::{
    Role as SemanticRole, SemanticActionKind, SemanticNode, SemanticNodeId, SemanticState,
    SemanticsDiagnostics, SemanticsTree, TextSelection as SemanticTextSelection,
};
use incular_text::{
    RichText, TextAlign, TextDiagnostics, TextEditingController, TextEngine, TextLayout,
    TextLayoutOptions, TextOverflow, TextRange, TextSelection, TextStyle,
};
use std::sync::Arc;

#[cfg(feature = "devtools")]
use incular_devtools_protocol::{DebugValue, DevWidgetId, TraceEvent, TracePhase};

use crate::advanced_scrolling::{
    ChildVicinity, DraggableScrollableActuator, DraggableScrollableSheet, ListWheelScrollView,
    ListWheelViewport, RawScrollbar, RawScrollbarStyle, TwoDimensionalScrollView,
    TwoDimensionalViewport,
};
use crate::advanced_scrolling::{
    draggable::DraggableSheetConfig, two_dimensional::TwoDimensionalViewportConfig,
    wheel::WheelViewportConfig,
};
use crate::compositing::ShaderCallback;
use crate::drag_drop::{RetainedDragSource, RetainedDragTarget};
use crate::external_drop::{ExternalDropTargetBinding, ExternalDropTargetMarker};
use crate::focus_keyboard::FocusTraversalPolicyKind;
use crate::gestures::{
    GestureAction, GestureArena, GestureArenaEntry, GestureArenaKey, GestureArenaMember,
    GestureCallbacks, GestureDecision, GestureDisposition, PointerEvent, PointerGestureRecognizer,
    ScaleGestureDetector,
};
use crate::painting_effects::BoxShadow;
use crate::raw_input::{GestureRecognizer, RawInputKind};
use crate::recursion::{DiagnosticNode, DiagnosticNodeId, RecursionDiagnostics};
pub use crate::recursion::{FramePhase, RecursionReport};
use crate::render_object::{RenderGeometry, RenderLayers, RenderNode, RenderObjectPayload};
use crate::scrolling::{
    SliverChildId, SliverViewportConfig, SliverViewportDelegate, SliverViewportLayout,
};
use crate::selection::{
    SelectableChildPolicy, SelectionAreaController, SelectionContainerDelegate,
    SelectionListenerNotifier,
};

mod context;
mod descriptors;
mod external_drop;
mod focus;
mod interaction;
mod invariants;
mod layout;
mod painting;
mod raw_input;
mod reconciliation;
mod rendering;
mod retained;
mod semantics;
mod specs;
mod text;
mod values;
mod widget;

/// Central stack policy for the small set of tree algorithms whose shape is
/// naturally recursive (layout, paint, semantic grouping, and compatible
/// subtree reconciliation). Bookkeeping traversals must use explicit work
/// stacks instead of calling this helper.
#[inline]
pub(super) fn with_recursive_tree_stack<R>(f: impl FnOnce() -> R) -> R {
    const RED_ZONE_BYTES: usize = 128 * 1024;
    const STACK_SEGMENT_BYTES: usize = 2 * 1024 * 1024;
    stacker::maybe_grow(RED_ZONE_BYTES, STACK_SEGMENT_BYTES, f)
}

pub(crate) use context::LayoutBuilderCallback;
pub use context::{BuildContext, InheritedScopeValue};
pub use descriptors::*;
pub(crate) use specs::WidgetKind;
pub use specs::{ButtonSpec, ExplicitSemantics, TextFieldSpec};
use specs::{SemanticCallbacks, SemanticProperties};

use semantics::widget_text;
use values::finite_offset;
pub(crate) use widget::WidgetType;
use widget::{
    enforced_constraints, fractional_constraints, physical_scroll_offset, scroll_constraints,
    scroll_delta_for_axis, scroll_size, scroll_translation, scroll_viewport_extent, sliver_anchor,
    sliver_viewport_size, unconstrained_constraints,
};

#[doc(hidden)]
pub use invariants::{InvariantCategory, InvariantViolation};
pub use rendering::{image_fit_rects, image_repeat_destinations, render_kind};
pub use retained::{PERFORMANCE_OVERLAY_KEY, performance_overlay_placeholder};
pub use values::*;
pub use widget::Widget;

fn text_call_label(method: &str, text: &str) -> String {
    let mut preview = text.chars().take(80).collect::<String>();
    if text.chars().count() > 80 {
        preview.push('…');
    }
    format!("{method}({preview:?})")
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Diagnostics {
    pub mounts: u64,
    pub unmounts: u64,
    pub rebuilds: u64,
    pub layouts: u64,
    pub paints: u64,
    pub composites: u64,
    pub scroll_events: u64,
    pub scroll_offset_updates: u64,
    pub animation_ticks: u64,
    pub lazy_layouts: u64,
    pub items_built: u64,
    pub items_mounted: u64,
    pub items_unmounted: u64,
    pub items_reused: u64,
    /// Reconciliation prefix/suffix fast-path hits (children matched in place
    /// without entering the keyed middle-diff).
    pub reconciliation_fast_paths: u64,
    // Task 15 structural breakdown. All cheap monotonic counters.
    /// `reconcile_children` invocations that reached scanning (parent changed).
    pub child_list_scans: u64,
    /// Child updates skipped because the retained element already equaled the
    /// desired widget (the unchanged-subtree bailout).
    pub identical_child_bailouts: u64,
    /// Widget type comparisons performed during compatibility checks.
    pub widget_type_comparisons: u64,
    /// Key comparisons performed during compatibility checks and lookups.
    pub key_comparisons: u64,
    /// Full widget configuration equality checks (deep ==).
    pub config_comparisons: u64,
    /// Old-key maps built for keyed middle ranges.
    pub key_maps_built: u64,
    /// Entries inserted into those key maps.
    pub key_map_entries: u64,
    /// Keyed lookups against those maps.
    pub key_lookups: u64,
    /// New elements mounted during reconciliation.
    pub elements_created: u64,
    /// Existing elements reused (compatible match) during reconciliation.
    pub elements_reused: u64,
    /// Elements unmounted during reconciliation.
    pub elements_removed: u64,
    /// Reused elements whose position changed relative to the previous list.
    pub elements_moved: u64,
    /// Every invalidation request (before deduplication).
    pub dirty_requests: u64,
    /// Requests that transitioned a retained object into a dirty state.
    pub dirty_queue_insertions: u64,
    /// Requests coalesced because the target was already dirty.
    pub dirty_queue_deduplicated: u64,
    /// LAYOUT passes skipped because constraints were unchanged.
    pub layout_cache_hits: u64,
    /// Retained per-render-object display lists replayed without recording.
    pub display_lists_reused: u64,
    /// Compositor-only mutations (transform/opacity/effect) that skipped
    /// BUILD/LAYOUT/PAINT entirely.
    pub compositor_only_updates: u64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeneratedChildIdentity {
    Sliver(String),
    Advanced(String),
    LayoutBuilder,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TreeError {
    MissingElement(ElementId),
    DuplicateKey {
        key: Key,
        parent: Option<ElementId>,
    },
    InvalidGeneratedChild {
        owner: ElementId,
        child: GeneratedChildIdentity,
        source: Box<TreeError>,
    },
    InvalidWidgetConfiguration {
        widget: &'static str,
        reason: String,
    },
    /// No live window record matched the requested window identity.
    WindowUnknown,
}

impl std::fmt::Display for TreeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingElement(id) => write!(formatter, "element {id:?} is not mounted"),
            Self::DuplicateKey { key, parent } => {
                write!(formatter, "duplicate sibling key {key:?}")?;
                if let Some(parent) = parent {
                    write!(formatter, " under parent {parent:?}")?;
                }
                Ok(())
            }
            Self::InvalidGeneratedChild {
                owner,
                child,
                source,
            } => write!(
                formatter,
                "generated child {child:?} for owner {owner:?} is invalid: {source}"
            ),
            Self::InvalidWidgetConfiguration { widget, reason } => {
                write!(formatter, "invalid {widget} configuration: {reason}")
            }
            Self::WindowUnknown => formatter.write_str("window is not mounted"),
        }
    }
}

impl std::error::Error for TreeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidGeneratedChild { source, .. } => Some(source.as_ref()),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum AdvancedChildKey {
    Wheel(usize),
    TwoDimensional(ChildVicinity),
    Sheet,
}

pub struct Element {
    pub parent: Option<ElementId>,
    pub children: Vec<ElementId>,
    pub widget: Widget,
    pub render: RenderObjectId,
    pub dirty: DirtyFlags,
    /// Parallel to `children` for a sliver viewport. IDs are viewport-scoped
    /// and remain stable while the cache window moves.
    sliver_child_ids: Vec<SliverChildId>,
    /// Parallel to `children` for a sliver viewport. This is separate from
    /// the retained identity because reorderable and grid slivers may use a
    /// stable row/item ID while their accessibility position is different.
    sliver_child_semantic_indices: Vec<Option<usize>>,
    /// Pinned children are painted above normal flow children while logical
    /// accessibility order remains unchanged.
    sliver_overlay_ids: HashSet<SliverChildId>,
    /// Stable identities for lazily materialized advanced-scrolling children.
    advanced_child_keys: Vec<AdvancedChildKey>,
    /// RAII subscriptions for a NotificationListener. They are rebuilt after
    /// layout so lazily materialized sliver viewports are included without
    /// keeping dead controller listeners alive.
    notification_subscriptions: Vec<ScrollNotificationSubscription>,
    sliver_delegate_revision: u64,
    sliver_scroll_revision: u64,
    layout_builder_constraints: Option<Constraints>,
    layout_builder_revision: u64,
    /// Dependency owner used while this element materializes a builder.
    build_context: DependencyContext,
    /// Separate dependency owner used by render lowering so rebuilding a
    /// LayoutBuilder cannot erase dependencies observed by a retained render
    /// object (for example DefaultTextStyle).
    render_context: DependencyContext,
    /// Environment supplied directly by this element, if any. The value lives
    /// in the retained BuildContext environment rather than an ambient stack.
    environment_override: Option<InheritedScopeValue>,
    /// Whether this element cuts off all environments installed above it.
    environment_boundary: bool,
    /// DevTools-only instrumentation. Zero cost in production builds.
    #[cfg(feature = "devtools")]
    pub dev: ElementDevData,
}

/// Per-element counters and the latest invalidation cause, captured only
/// while the `devtools` feature is enabled.
#[cfg(feature = "devtools")]
#[derive(Default, Clone, Debug)]
pub struct ElementDevData {
    pub builds: u64,
    pub layouts: u64,
    pub paints: u64,
    pub composites: u64,
    pub semantic_updates: u64,
    /// Monotonic per-node content revision for incremental tree deltas.
    pub revision: u64,
    pub last_cause: Option<InvalidationCause>,
    /// Small ordered cause set coalesced until the next observed build. This
    /// avoids inventing a single winner when several real invalidations land
    /// before a frame.
    pub invalidation_causes: Vec<InvalidationCause>,
    /// Structured, bounded configuration change set for Why Did This Rebuild.
    pub property_changes: Vec<incular_devtools_protocol::PropertyChange>,
    pub layout_history: Vec<LayoutHistoryRecord>,
    pub layout_reason: Option<String>,
    pub paint_reason: Option<String>,
    pub composite_reason: Option<String>,
}

#[cfg(feature = "devtools")]
#[derive(Clone, Debug)]
pub struct LayoutHistoryRecord {
    pub sequence: u64,
    pub old_constraints: Option<Constraints>,
    pub new_constraints: Constraints,
    pub old_size: Size,
    pub new_size: Size,
    pub cause: Option<String>,
}

/// Why an element last entered BUILD. Recorded by the runtime at the exact
/// invalidation sites; retained bounded summaries only.
#[cfg(feature = "devtools")]
#[derive(Clone, Debug)]
pub enum InvalidationCause {
    Signal {
        id: u64,
        name: Option<String>,
        old: Option<String>,
        new: Option<String>,
    },
    ParentReconciliation,
    WidgetConfigurationChanged,
    EnvironmentChanged,
    LocaleChanged,
    WindowMetricsChanged,
    ConstraintsChanged,
    Animation,
    TaskCompletion,
    Navigation,
    Restoration,
    Manual,
    Mounted,
}

#[cfg(feature = "devtools")]
impl InvalidationCause {
    pub fn summary(&self) -> String {
        match self {
            Self::Signal { name, .. } => {
                format!("Signal {}", name.as_deref().unwrap_or("<unnamed>"))
            }
            Self::ParentReconciliation => "parent reconciliation".into(),
            Self::WidgetConfigurationChanged => "widget changed".into(),
            Self::EnvironmentChanged => "environment".into(),
            Self::LocaleChanged => "locale".into(),
            Self::WindowMetricsChanged => "window metrics".into(),
            Self::ConstraintsChanged => "constraints".into(),
            Self::Animation => "animation".into(),
            Self::TaskCompletion => "task completion".into(),
            Self::Navigation => "navigation".into(),
            Self::Restoration => "restoration".into(),
            Self::Manual => "manual invalidation".into(),
            Self::Mounted => "mounted".into(),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HiddenLayout {
    Offstage,
    PreserveSpace,
}

/// Policy for an already-retained child. Removal is resolved before lowering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct HiddenVisibility {
    pub layout: HiddenLayout,
    pub animation: bool,
    pub semantics: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RenderKind {
    Box {
        desired: Size,
        color: Color,
    },
    Shape {
        path: Arc<Path>,
        fill: Option<Brush>,
        stroke: Option<(Brush, Stroke)>,
        desired: Size,
    },
    CustomPaint {
        desired: Size,
        display_list: DisplayList,
    },
    Decorated {
        desired: Option<Size>,
        background: Option<Brush>,
        border: Option<Border>,
        radius: CornerRadii,
    },
    Banner {
        message: String,
        text_direction: TextDirection,
        location: crate::utilities::BannerLocation,
        layout_direction: TextDirection,
        color: Color,
        text_style: TextStyle,
        shadow: BoxShadow,
    },
    Button {
        desired: Size,
        color: Color,
        hover_color: Option<Color>,
        pressed_color: Option<Color>,
        focused_color: Option<Color>,
        disabled_color: Option<Color>,
        enabled: bool,
        focusable_when_disabled: bool,
    },
    Padding {
        padding: EdgeInsets,
    },
    Constrained {
        constraints: Constraints,
    },
    Limited {
        max_width: f32,
        max_height: f32,
    },
    Overflow {
        min_width: Option<f32>,
        max_width: Option<f32>,
        min_height: Option<f32>,
        max_height: Option<f32>,
    },
    Unconstrained {
        constrained_axis: Option<Axis>,
    },
    Fractional {
        width_factor: Option<f32>,
        height_factor: Option<f32>,
    },
    Baseline {
        baseline: f32,
    },
    RepaintBoundary,
    Gesture,
    Align {
        alignment: Alignment,
        width_factor: Option<f32>,
        height_factor: Option<f32>,
    },
    Flex {
        flex: incular_layout::Flex,
    },
    Flexible {
        flex: u32,
        fit: incular_config::FlexFit,
    },
    Wrap {
        wrap: incular_layout::Wrap,
    },
    Table {
        columns: usize,
        column_spacing: f32,
        row_spacing: f32,
    },
    Stack {
        stack: incular_layout::Stack,
    },
    Positioned {
        left: Option<f32>,
        top: Option<f32>,
        right: Option<f32>,
        bottom: Option<f32>,
        width: Option<f32>,
        height: Option<f32>,
    },
    IndexedStack {
        alignment: Alignment,
        index: usize,
    },
    SafeArea {
        minimum: EdgeInsets,
        left: bool,
        top: bool,
        right: bool,
        bottom: bool,
        maintain_bottom_view_padding: bool,
    },
    ClipRect {
        clip_behavior: Clip,
    },
    ClipRRect {
        radius: CornerRadii,
        clip_behavior: Clip,
    },
    ClipOval {
        clip_behavior: Clip,
    },
    ClipPath {
        path: Arc<Path>,
        clip_behavior: Clip,
    },
    LayoutBuilder,
    Visibility {
        visible: bool,
        maintain_size: bool,
    },
    AspectRatio {
        ratio: f32,
    },
    Text {
        text: String,
        style: TextStyle,
        align: TextAlign,
        soft_wrap: bool,
        max_lines: Option<usize>,
        overflow: TextOverflow,
    },
    SelectableText {
        text: String,
        style: TextStyle,
        align: TextAlign,
    },
    SelectionArea,
    SelectionContainer,
    SelectionListener,
    IndexedSemantics,
    SemanticsDebugger {
        label_style: TextStyle,
        max_nodes: usize,
    },
    Image {
        image: ImageHandle,
        width: Option<f32>,
        height: Option<f32>,
        fit: ImageFit,
        repeat: ImageRepeat,
        alignment: Alignment,
        sampling: ImageSampling,
    },
    TextField {
        controller: TextEditingController,
        desired: Size,
        style: TextStyle,
        placeholder: String,
        multiline: bool,
        min_lines: Option<usize>,
        max_lines: Option<usize>,
        expands: bool,
        text_align: TextAlign,
        enabled: bool,
        read_only: bool,
        obscure_text: bool,
        cursor_width: f32,
        cursor_height: Option<f32>,
        cursor_radius: f32,
        show_cursor: bool,
        cursor_color: Color,
        selection_color: Color,
    },
    Scroll {
        controller: ScrollController,
        axis: Axis,
        reverse: bool,
        physics: ScrollPhysics,
    },
    RawScrollbar {
        controller: ScrollController,
        style: RawScrollbarStyle,
    },
    ListWheelScrollView {
        config: Rc<WheelViewportConfig<Widget>>,
    },
    ListWheelViewport {
        config: Rc<WheelViewportConfig<Widget>>,
    },
    DraggableScrollableSheet {
        config: Rc<DraggableSheetConfig<Widget>>,
    },
    DraggableScrollableActuator {
        actuator: DraggableScrollableActuator,
    },
    TwoDimensionalScrollView {
        config: Rc<TwoDimensionalViewportConfig<Widget>>,
    },
    TwoDimensionalViewport {
        config: Rc<TwoDimensionalViewportConfig<Widget>>,
    },
    PersistentHeader {
        controller: ScrollController,
        axis: Axis,
        reverse: bool,
        pinned: bool,
    },
    SliverViewport {
        config: Rc<SliverViewportConfig>,
    },
    Translate {
        controller: TranslationController,
    },
    Transform {
        transform: CoreTransform,
        origin: Option<Offset>,
    },
    Scale {
        controller: ScaleController,
        origin: Option<Offset>,
    },
    Rotation {
        controller: RotationController,
        origin: Option<Offset>,
        alignment: Option<Alignment>,
    },
    FittedBox {
        fit: ImageFit,
        alignment: Alignment,
    },
    Opacity {
        alpha: f32,
        controller: Option<OpacityController>,
    },
    Blur {
        sigma_x: f32,
        sigma_y: f32,
        controller: Option<BlurController>,
    },
    DropShadow {
        offset: Offset,
        sigma_x: f32,
        sigma_y: f32,
        color: Color,
        controller: Option<DropShadowController>,
    },
    ColorFiltered {
        filter: ColorFilter,
        controller: Option<ColorFilterController>,
    },
    Blend {
        mode: BlendMode,
    },
    ShaderMask {
        shader: ShaderCallback,
        blend_mode: BlendMode,
    },
    BackdropFilter {
        blur: GaussianBlur,
        blend_mode: BlendMode,
        enabled: bool,
    },
    AnnotatedRegion {
        annotation: Annotation,
        sized: bool,
    },
    Leader {
        link: LayerLink,
    },
    Follower {
        link: LayerLink,
        show_when_unlinked: bool,
        offset: Offset,
        target_anchor: LayerAnchor,
        follower_anchor: LayerAnchor,
    },
}

/// Snapshot of one sliver viewport. Semantic integration can expose
/// `logical_item_count` plus these materialized item indices without creating
/// one semantic node per logical item.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SliverViewportDiagnostics {
    pub logical_item_count: usize,
    pub materialized_range: std::ops::Range<usize>,
    pub materialized_item_count: usize,
    pub scroll_offset: f32,
    pub viewport_extent: f32,
    pub cache_extent: f32,
    pub element_count: usize,
    pub render_object_count: usize,
    pub picture_layer_count: usize,
}
#[derive(Clone, Copy, Debug, PartialEq)]
struct ScrollbarDrag {
    render: RenderObjectId,
    /// Pointer position relative to the rendered thumb's top, fixed for this
    /// captured gesture. Keeping this anchor avoids snapping on a thumb press.
    grab_offset: f32,
}

/// Snapshot of the active thumb drag for deterministic diagnostics.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScrollbarDragDiagnostics {
    pub active: bool,
    pub grab_offset: f32,
    pub track_extent: f32,
    pub thumb_extent: f32,
    pub thumb_travel: f32,
    pub thumb_top: f32,
    pub scroll_offset: f32,
}
struct SemanticBuild {
    element: ElementId,
    parent: Option<ElementId>,
    role: SemanticRole,
    label: Option<String>,
    value: Option<String>,
    description: Option<String>,
    bounds: Rect,
    state: SemanticState,
    actions: Vec<SemanticActionKind>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RetainedGestureKind {
    Pointer,
    Scale,
}

struct ActiveGestureMember {
    element: ElementId,
    member: GestureArenaMember,
    kind: RetainedGestureKind,
    recognizer: Option<PointerGestureRecognizer>,
    on_cancel: Option<Rc<dyn Fn()>>,
}

struct ActiveGesture {
    element: ElementId,
    members: Vec<ActiveGestureMember>,
    cancelled_elements: HashSet<ElementId>,
    start: Offset,
}

struct ActiveTrackpadGesture {
    element: ElementId,
}

struct ActiveRawGestureMember {
    element: ElementId,
    member: GestureArenaMember,
    type_id: TypeId,
    cancelled: bool,
}

struct ActiveRawGesture {
    element: ElementId,
    members: Vec<ActiveRawGestureMember>,
}

struct ActiveDrag {
    source: Rc<dyn RetainedDragSource>,
    target: Option<(ElementId, Rc<dyn RetainedDragTarget>)>,
    start: Offset,
}

#[derive(Clone)]
struct ActiveExternalDrop {
    element: ElementId,
    operation: incular_platform::TransferOperation,
    last_event: incular_platform::ExternalDragEvent,
}

fn disposition_for(
    entries: &[GestureArenaEntry],
    member: GestureArenaMember,
) -> GestureDisposition {
    entries
        .iter()
        .find(|entry| entry.member == member)
        .map_or(GestureDisposition::Cancelled, |entry| entry.disposition)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct StaticSelectionPoint {
    element: ElementId,
    byte: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct StaticSelection {
    area: ElementId,
    anchor: StaticSelectionPoint,
    extent: StaticSelectionPoint,
}

#[cfg(feature = "devtools")]
struct DeepTraceCapture {
    started: Instant,
    events: Vec<TraceEvent>,
    event_starts: Vec<Instant>,
    stack: Vec<u32>,
    max_events: usize,
    dropped_events: u32,
}

#[cfg(feature = "devtools")]
impl DeepTraceCapture {
    fn new(max_events: usize) -> Self {
        Self {
            started: Instant::now(),
            events: Vec::with_capacity(max_events.min(4_096)),
            event_starts: Vec::with_capacity(max_events.min(4_096)),
            stack: Vec::new(),
            max_events,
            dropped_events: 0,
        }
    }

    fn elapsed_us(duration: Duration) -> u32 {
        u32::try_from(duration.as_micros()).unwrap_or(u32::MAX)
    }

    fn begin(&mut self, node: DevWidgetId, phase: TracePhase) -> Option<u32> {
        if self.events.len() >= self.max_events {
            self.dropped_events = self.dropped_events.saturating_add(1);
            return None;
        }
        let now = Instant::now();
        let index = u32::try_from(self.events.len()).ok()?;
        self.events.push(TraceEvent {
            node,
            phase,
            parent: self.stack.last().copied(),
            start_us: Self::elapsed_us(now.saturating_duration_since(self.started)),
            duration_us: 0,
        });
        self.event_starts.push(now);
        self.stack.push(index);
        Some(index)
    }

    fn end(&mut self, token: Option<u32>) {
        let Some(token) = token else { return };
        let Some(event) = self.events.get_mut(token as usize) else {
            return;
        };
        if let Some(started) = self.event_starts.get(token as usize) {
            event.duration_us = Self::elapsed_us(started.elapsed());
        }
        if self.stack.last() == Some(&token) {
            self.stack.pop();
        } else if let Some(position) = self.stack.iter().rposition(|entry| *entry == token) {
            self.stack.remove(position);
        }
    }
}

/// Persistent UI state. IDs become invalid immediately after unmount.
pub struct WidgetTree {
    elements: Arena<Element>,
    renders: Arena<RenderNode>,
    root: Option<ElementId>,
    diagnostics: Diagnostics,
    unmounted: Vec<ElementId>,
    text_engine: TextEngine,
    compositor: LayerTree,
    compositor_initialized: bool,
    /// Maps the platform's monotonic frame clock onto the animation-only
    /// clock. Tokio, input, profiler timing and all other runtime clocks keep
    /// using real time.
    animation_time_scale: f32,
    animation_clock: Option<(Instant, Instant)>,
    next_action: u64,
    pending_handlers: Vec<(ActionId, Rc<dyn Fn()>)>,
    gesture_arena: GestureArena,
    active_gestures: HashMap<GestureArenaKey, ActiveGesture>,
    active_trackpad_gestures: HashMap<GestureArenaKey, ActiveTrackpadGesture>,
    raw_recognizers: HashMap<ElementId, HashMap<TypeId, Box<dyn GestureRecognizer>>>,
    raw_gesture_streams: HashMap<GestureArenaKey, ActiveRawGesture>,
    raw_pointer_routes: HashMap<GestureArenaKey, Vec<ElementId>>,
    mouse_hover: HashMap<GestureArenaKey, Vec<ElementId>>,
    consumed_tap_pointers: HashSet<GestureArenaKey>,
    pointer_captures: HashMap<GestureArenaKey, ElementId>,
    active_drags: HashMap<GestureArenaKey, ActiveDrag>,
    active_external_drop: Option<ActiveExternalDrop>,
    scale_gestures: HashMap<ElementId, ScaleGestureDetector>,
    scrollbar_drag: Option<ScrollbarDrag>,
    semantics: SemanticsTree,
    semantic_ids: HashMap<ElementId, SemanticNodeId>,
    static_selections: HashMap<ElementId, StaticSelection>,
    dependency_root: DependencyContext,
    inherited_consumers: HashMap<ConsumerId, (ElementId, InheritedDependencyKind)>,
    environment: RuntimeEnvironment,
    transient_placements:
        HashMap<crate::transient::TransientSurfaceId, crate::transient::RetainedTransientPlacement>,
    native_transient_bounds: Option<Rect>,
    native_transient_presentations: HashSet<crate::transient::TransientSurfaceId>,
    recursion_diagnostics: RecursionDiagnostics,
    #[cfg(feature = "devtools")]
    deep_trace: Option<DeepTraceCapture>,
}

struct FocusCandidate {
    id: ElementId,
    bounds: Rect,
    order: Option<f64>,
    sequence: usize,
}

enum FocusTraversalMember {
    Candidate(FocusCandidate),
    Group(FocusTraversalGroupMembers),
}

struct FocusTraversalGroupMembers {
    policy: FocusTraversalPolicyKind,
    members: Vec<FocusTraversalMember>,
}

impl FocusTraversalMember {
    fn first_candidate(&self) -> Option<&FocusCandidate> {
        let mut work = vec![self];
        while let Some(member) = work.pop() {
            match member {
                Self::Candidate(candidate) => return Some(candidate),
                Self::Group(group) => work.extend(group.members.iter().rev()),
            }
        }
        None
    }

    fn representative_bounds(&self) -> Rect {
        self.first_candidate()
            .map_or(Rect::default(), |candidate| candidate.bounds)
    }

    fn representative_order(&self) -> Option<f64> {
        self.first_candidate().and_then(|candidate| candidate.order)
    }

    fn representative_sequence(&self) -> usize {
        self.first_candidate()
            .map_or(usize::MAX, |candidate| candidate.sequence)
    }
}

fn sort_focus_members(policy: FocusTraversalPolicyKind, members: &mut [FocusTraversalMember]) {
    match policy {
        FocusTraversalPolicyKind::WidgetOrder => {}
        FocusTraversalPolicyKind::ReadingOrder => {
            members.sort_by(|left, right| {
                let left_bounds = left.representative_bounds();
                let right_bounds = right.representative_bounds();
                let row_tolerance =
                    (left_bounds.size.height.max(right_bounds.size.height) * 0.5) + 1.0;
                let vertical = left_bounds.origin.y - right_bounds.origin.y;
                if vertical.abs() > row_tolerance {
                    left_bounds.origin.y.total_cmp(&right_bounds.origin.y)
                } else {
                    left_bounds.origin.x.total_cmp(&right_bounds.origin.x)
                }
                .then_with(|| {
                    left.representative_sequence()
                        .cmp(&right.representative_sequence())
                })
            });
        }
        FocusTraversalPolicyKind::Ordered => {
            members.sort_by(|left, right| {
                left.representative_order()
                    .unwrap_or(f64::INFINITY)
                    .total_cmp(&right.representative_order().unwrap_or(f64::INFINITY))
                    .then_with(|| {
                        left.representative_sequence()
                            .cmp(&right.representative_sequence())
                    })
            });
        }
    }
}

fn flatten_focus_group(group: FocusTraversalGroupMembers, out: &mut Vec<ElementId>) {
    let mut members = group.members;
    sort_focus_members(group.policy, &mut members);
    let mut work = members.into_iter().rev().collect::<Vec<_>>();
    while let Some(member) = work.pop() {
        match member {
            FocusTraversalMember::Candidate(candidate) => out.push(candidate.id),
            FocusTraversalMember::Group(group) => {
                let mut members = group.members;
                sort_focus_members(group.policy, &mut members);
                work.extend(members.into_iter().rev());
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InheritedDependencyKind {
    Build,
    Render,
}
