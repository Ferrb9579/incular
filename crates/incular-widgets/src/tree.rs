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
    Arena, ArenaId, Color, DirtyFlags, KeyboardEvent, KeyboardKey, NamedKey, Offset, Rect, Size,
    Transform as CoreTransform,
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
use crate::compositing::ShaderCallback;
use crate::drag_drop::{RetainedDragSource, RetainedDragTarget};
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
use crate::render_object::{
    RenderGeometry, RenderInvalidation, RenderLayers, RenderNode, RenderObjectPayload,
};
use crate::scrolling::{
    SliverChildId, SliverViewportConfig, SliverViewportDelegate, SliverViewportLayout,
};
use crate::selection::{
    SelectableChildPolicy, SelectionAreaController, SelectionContainerDelegate,
    SelectionListenerNotifier,
};

mod focus;
mod interaction;
mod layout;
mod painting;
mod raw_input;
mod reconciliation;
mod rendering;
mod retained;
mod semantics;
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

use semantics::widget_text;
use values::{finite_non_negative, finite_offset};
pub(crate) use widget::WidgetType;
use widget::{
    enforced_constraints, fractional_constraints, physical_scroll_offset, scroll_constraints,
    scroll_delta_for_axis, scroll_size, scroll_translation, scroll_viewport_extent, sliver_anchor,
    sliver_viewport_size, unconstrained_constraints,
};

pub use rendering::{image_fit_rects, image_repeat_destinations, render_kind};
pub use retained::{PERFORMANCE_OVERLAY_KEY, performance_overlay_placeholder};
pub use values::*;
pub use widget::Widget;

/// Shared handles keep algorithm state owned by the retained descriptor while
/// allowing a cheap declarative widget clone. The focused scrolling models
/// themselves remain renderer-neutral and are only driven by the tree adapter.
#[derive(Clone)]
pub struct RetainedWheelScrollView(pub(crate) Rc<RefCell<ListWheelScrollView<Widget>>>);

#[derive(Clone)]
pub struct RetainedWheelViewport(pub(crate) Rc<RefCell<ListWheelViewport<Widget>>>);

#[derive(Clone)]
pub struct RetainedDraggableSheet(pub(crate) Rc<RefCell<DraggableScrollableSheet<Widget>>>);

#[derive(Clone)]
pub struct RetainedActuator(pub(crate) Rc<DraggableScrollableActuator>);

#[derive(Clone)]
pub struct RetainedTwoDimensionalScrollView(
    pub(crate) Rc<RefCell<TwoDimensionalScrollView<Widget>>>,
);

#[derive(Clone)]
pub struct RetainedTwoDimensionalViewport(pub(crate) Rc<RefCell<TwoDimensionalViewport<Widget>>>);

macro_rules! retained_rc_handle_traits {
    ($name:ident, $label:literal) => {
        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_tuple($label).field(&"retained").finish()
            }
        }

        impl PartialEq for $name {
            fn eq(&self, other: &Self) -> bool {
                Rc::ptr_eq(&self.0, &other.0)
            }
        }
    };
}

retained_rc_handle_traits!(RetainedWheelScrollView, "ListWheelScrollView");
retained_rc_handle_traits!(RetainedWheelViewport, "ListWheelViewport");
retained_rc_handle_traits!(RetainedDraggableSheet, "DraggableScrollableSheet");
retained_rc_handle_traits!(RetainedActuator, "DraggableScrollableActuator");
retained_rc_handle_traits!(RetainedTwoDimensionalScrollView, "TwoDimensionalScrollView");
retained_rc_handle_traits!(RetainedTwoDimensionalViewport, "TwoDimensionalViewport");

thread_local! {
    /// Type-erased values made available while a retained layout builder is
    /// materialized. Control libraries use this hook for ambient, typed
    /// scopes without coupling the raw widget crate to a design-system crate.
    static BUILD_ENVIRONMENT: RefCell<Vec<BuildEnvironmentFrame>> = const { RefCell::new(Vec::new()) };
}

enum BuildEnvironmentFrame {
    Values(Option<Rc<dyn Any>>),
    Boundary,
}

fn text_call_label(method: &str, text: &str) -> String {
    let mut preview = text.chars().take(80).collect::<String>();
    if text.chars().count() > 80 {
        preview.push('…');
    }
    format!("{method}({preview:?})")
}

/// A persistent chain of typed values inherited by a retained subtree.
///
/// `Widget::environment_scope` is represented by a layout-builder node so the
/// core widget crate does not need to know the concrete environment types. A
/// single erased value is not enough when independent scopes are nested (for
/// example, Material's theme, input-decoration theme, and control theme), so
/// effective environments retain the complete nearest-first chain here.
struct InheritedEnvironment {
    value: Rc<dyn Any>,
    parent: Option<Rc<InheritedEnvironment>>,
}

fn environment_chain(value: Rc<dyn Any>) -> Rc<InheritedEnvironment> {
    match value.downcast::<InheritedEnvironment>() {
        Ok(chain) => chain,
        Err(value) => Rc::new(InheritedEnvironment {
            value,
            parent: None,
        }),
    }
}

#[doc(hidden)]
pub fn compose_environment(
    local: Option<Rc<dyn Any>>,
    inherited: Option<Rc<dyn Any>>,
) -> Option<Rc<dyn Any>> {
    let Some(local) = local else {
        return inherited;
    };
    let Some(inherited) = inherited else {
        return Some(local);
    };
    Some(Rc::new(InheritedEnvironment {
        value: local,
        parent: Some(environment_chain(inherited)),
    }) as Rc<dyn Any>)
}

fn environment_value<T: Any + Clone>(environment: &Rc<dyn Any>) -> Option<T> {
    if let Some(value) = environment.downcast_ref::<T>() {
        return Some(value.clone());
    }
    let mut chain = environment.downcast_ref::<InheritedEnvironment>();
    while let Some(scope) = chain {
        if let Some(value) = scope.value.downcast_ref::<T>() {
            return Some(value.clone());
        }
        chain = scope.parent.as_deref();
    }
    None
}

/// Runs a retained builder with one inherited, type-erased environment value.
/// This is intentionally small and renderer-neutral; higher-level crates
/// provide typed accessors around it.
pub fn with_build_environment<R>(
    environment: Option<Rc<dyn Any>>,
    callback: impl FnOnce() -> R,
) -> R {
    BUILD_ENVIRONMENT.with(|stack| {
        stack
            .borrow_mut()
            .push(BuildEnvironmentFrame::Values(environment));
    });
    struct EnvironmentGuard;
    impl Drop for EnvironmentGuard {
        fn drop(&mut self) {
            BUILD_ENVIRONMENT.with(|stack| {
                let _ = stack.borrow_mut().pop();
            });
        }
    }
    let _guard = EnvironmentGuard;
    callback()
}

/// Runs a retained builder behind an explicit lookup boundary. The boundary
/// hides all outer typed environments while allowing scopes installed by the
/// builder itself to remain visible to its descendants.
pub fn with_build_environment_boundary<R>(callback: impl FnOnce() -> R) -> R {
    BUILD_ENVIRONMENT.with(|stack| {
        stack.borrow_mut().push(BuildEnvironmentFrame::Boundary);
    });
    struct EnvironmentGuard;
    impl Drop for EnvironmentGuard {
        fn drop(&mut self) {
            BUILD_ENVIRONMENT.with(|stack| {
                let _ = stack.borrow_mut().pop();
            });
        }
    }
    let _guard = EnvironmentGuard;
    callback()
}

/// Reads the nearest typed value from the active retained builder scope.
#[must_use]
pub fn current_build_environment<T: Any + Clone>() -> Option<T> {
    BUILD_ENVIRONMENT.with(|stack| {
        let stack = stack.borrow();
        for frame in stack.iter().rev() {
            match frame {
                BuildEnvironmentFrame::Boundary => break,
                BuildEnvironmentFrame::Values(value) => {
                    let Some(environment) = value.as_ref() else {
                        continue;
                    };
                    if let Some(value) = environment_value::<T>(environment) {
                        return Some(value);
                    }
                }
            }
        }
        None
    })
}

/// Reads a typed retained environment without placing the value-sized result
/// on the caller's stack. This matters for large theme descriptors on native
/// entry threads, whose stack is smaller than a test harness thread's stack.
#[must_use]
pub fn current_build_environment_boxed<T: Any + Clone>() -> Option<Box<T>> {
    BUILD_ENVIRONMENT.with(|stack| {
        let stack = stack.borrow();
        for frame in stack.iter().rev() {
            match frame {
                BuildEnvironmentFrame::Boundary => break,
                BuildEnvironmentFrame::Values(value) => {
                    let Some(environment) = value.as_ref() else {
                        continue;
                    };
                    if let Some(value) = environment_value::<T>(environment) {
                        return Some(Box::new(value));
                    }
                }
            }
        }
        None
    })
}

/// Application-authored semantic metadata for a visual widget that does not
/// have a more specific built-in semantic role. Incular keeps this data in its
/// retained `SemanticsTree`; native adapters only project that tree.
#[derive(Clone, Debug, PartialEq)]
pub struct ExplicitSemantics {
    pub role: SemanticRole,
    pub label: Option<String>,
    pub value: Option<String>,
    pub description: Option<String>,
    pub state: SemanticState,
    pub actions: Vec<SemanticActionKind>,
}

impl ExplicitSemantics {
    #[must_use]
    pub fn new(role: SemanticRole) -> Self {
        Self {
            role,
            label: None,
            value: None,
            description: None,
            state: SemanticState::default(),
            actions: Vec::new(),
        }
    }

    #[must_use]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    #[must_use]
    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    #[must_use]
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    #[must_use]
    pub fn state(mut self, state: SemanticState) -> Self {
        self.state = state;
        self
    }

    #[must_use]
    pub fn actions(mut self, actions: impl IntoIterator<Item = SemanticActionKind>) -> Self {
        self.actions = actions.into_iter().collect();
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct SemanticProperties {
    pub(super) label: Option<String>,
    pub(super) description: Option<String>,
    pub(super) hidden: bool,
    pub(super) explicit: Option<ExplicitSemantics>,
    pub(super) merge_descendants: bool,
    pub(super) block_previous_siblings: bool,
    pub(super) focus_traversal_policy: Option<FocusTraversalPolicyKind>,
    pub(super) focus_traversal_order: Option<f64>,
    pub(super) exclude_focus: bool,
    pub(super) exclude_focus_traversal: bool,
    pub(super) undo_history_max_entries: Option<usize>,
    pub(super) focus_scope_autofocus: bool,
    pub(super) text_input_type: Option<TextInputTypeHint>,
    pub(super) text_input_action: Option<TextInputActionHint>,
    pub(super) callbacks: SemanticCallbacks,
}

#[derive(Clone, Default)]
pub(super) struct SemanticCallbacks {
    activate: Option<Rc<dyn Fn() + 'static>>,
    increment: Option<Rc<dyn Fn() + 'static>>,
    decrement: Option<Rc<dyn Fn() + 'static>>,
    scroll_forward: Option<Rc<dyn Fn() + 'static>>,
    scroll_backward: Option<Rc<dyn Fn() + 'static>>,
}

impl std::fmt::Debug for SemanticCallbacks {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SemanticCallbacks")
            .field("activate", &self.activate.is_some())
            .field("increment", &self.increment.is_some())
            .field("decrement", &self.decrement.is_some())
            .field("scroll_forward", &self.scroll_forward.is_some())
            .field("scroll_backward", &self.scroll_backward.is_some())
            .finish()
    }
}

impl PartialEq for SemanticCallbacks {
    fn eq(&self, other: &Self) -> bool {
        fn same<T: ?Sized>(left: &Option<Rc<T>>, right: &Option<Rc<T>>) -> bool {
            match (left, right) {
                (Some(left), Some(right)) => Rc::ptr_eq(left, right),
                (None, None) => true,
                _ => false,
            }
        }

        same(&self.activate, &other.activate)
            && same(&self.increment, &other.increment)
            && same(&self.decrement, &other.decrement)
            && same(&self.scroll_forward, &other.scroll_forward)
            && same(&self.scroll_backward, &other.scroll_backward)
    }
}

impl SemanticCallbacks {
    pub(super) fn callback(&self, action: SemanticActionKind) -> Option<Rc<dyn Fn() + 'static>> {
        match action {
            SemanticActionKind::Activate => self.activate.clone(),
            SemanticActionKind::Increment => self.increment.clone(),
            SemanticActionKind::Decrement => self.decrement.clone(),
            SemanticActionKind::ScrollForward => self.scroll_forward.clone(),
            SemanticActionKind::ScrollBackward => self.scroll_backward.clone(),
            SemanticActionKind::Focus
            | SemanticActionKind::SetText
            | SemanticActionKind::SetSelection => None,
        }
    }

    pub(super) fn supports(&self, action: SemanticActionKind) -> bool {
        self.callback(action).is_some()
    }
}
#[doc(hidden)]
#[derive(Clone)]
pub struct ButtonSpec {
    pub(crate) size: Size,
    pub(crate) color: Color,
    pub(crate) hover_color: Option<Color>,
    pub(crate) pressed_color: Option<Color>,
    pub(crate) focused_color: Option<Color>,
    pub(crate) disabled_color: Option<Color>,
    pub(crate) enabled: bool,
    pub(crate) focusable_when_disabled: bool,
    pub(crate) action: ActionId,
    pub(crate) callback: Option<Rc<dyn Fn()>>,
    pub(crate) hover_action: ActionId,
    pub(crate) hover_callback: Option<Rc<dyn Fn()>>,
    pub(crate) exit_action: ActionId,
    pub(crate) exit_callback: Option<Rc<dyn Fn()>>,
    pub(crate) has_callback: bool,
    pub(crate) child: Option<Widget>,
}

#[doc(hidden)]
#[derive(Clone)]
pub struct TextFieldSpec {
    pub(crate) controller: TextEditingController,
    pub(crate) size: Size,
    pub(crate) style: TextStyle,
    pub(crate) placeholder: String,
    pub(crate) on_submit: Option<Rc<dyn Fn(String)>>,
    pub(crate) multiline: bool,
    pub(crate) min_lines: Option<usize>,
    pub(crate) max_lines: Option<usize>,
    pub(crate) expands: bool,
    pub(crate) text_align: TextAlign,
    pub(crate) enabled: bool,
    pub(crate) read_only: bool,
    pub(crate) obscure_text: bool,
    pub(crate) cursor_width: f32,
    pub(crate) cursor_height: Option<f32>,
    pub(crate) cursor_radius: f32,
    pub(crate) show_cursor: bool,
    pub(crate) cursor_color: Color,
    pub(crate) selection_color: Color,
}

#[derive(Clone)]
pub enum WidgetKind {
    Box {
        size: Size,
        color: Color,
    },
    Shape {
        path: Arc<Path>,
        fill: Option<Brush>,
        stroke: Option<(Brush, Stroke)>,
        size: Option<Size>,
    },
    CustomPaint {
        size: Size,
        display_list: DisplayList,
    },
    Decorated {
        size: Option<Size>,
        background: Option<Brush>,
        border: Option<Border>,
        radius: CornerRadii,
        child: Widget,
    },
    Banner {
        message: String,
        text_direction: Option<TextDirection>,
        location: crate::utilities::BannerLocation,
        layout_direction: Option<TextDirection>,
        color: Color,
        text_style: TextStyle,
        shadow: BoxShadow,
        child: Option<Widget>,
    },
    Button(ButtonSpec),
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
    SelectionArea {
        controller: SelectionAreaController,
        child: Widget,
    },
    SelectionContainer {
        delegate: SelectionContainerDelegate,
        child: Widget,
    },
    SelectionListener {
        notifier: SelectionListenerNotifier,
        delegate: SelectionContainerDelegate,
        child: Widget,
    },
    IndexedSemantics {
        index: usize,
        child: Widget,
    },
    SemanticsDebugger {
        label_style: TextStyle,
        max_nodes: usize,
        child: Widget,
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
    TextField(TextFieldSpec),
    Padding {
        padding: EdgeInsets,
        child: Widget,
    },
    Constrained {
        constraints: Constraints,
        child: Widget,
    },
    Limited {
        max_width: f32,
        max_height: f32,
        child: Widget,
    },
    Overflow {
        min_width: Option<f32>,
        max_width: Option<f32>,
        min_height: Option<f32>,
        max_height: Option<f32>,
        child: Widget,
    },
    Unconstrained {
        constrained_axis: Option<Axis>,
        child: Widget,
    },
    Fractional {
        width_factor: Option<f32>,
        height_factor: Option<f32>,
        child: Widget,
    },
    Baseline {
        baseline: f32,
        child: Widget,
    },
    RepaintBoundary {
        child: Widget,
    },
    Gesture {
        behavior: crate::gestures::HitTestBehavior,
        callbacks: Box<GestureCallbacks>,
        child: Widget,
    },
    RawInput {
        kind: RawInputKind,
        child: Option<Widget>,
    },
    Draggable {
        source: Rc<dyn RetainedDragSource>,
        child: Widget,
    },
    DragTarget {
        target: Rc<dyn RetainedDragTarget>,
        child: Widget,
    },
    IgnorePointer {
        ignoring: bool,
        child: Widget,
    },
    AbsorbPointer {
        absorbing: bool,
        child: Widget,
    },
    Align {
        alignment: Alignment,
        width_factor: Option<f32>,
        height_factor: Option<f32>,
        child: Widget,
    },
    Flex {
        axis: Axis,
        main_axis_alignment: MainAxisAlignment,
        main_axis_size: MainAxisSize,
        cross_axis_alignment: CrossAxisAlignment,
        text_direction: TextDirection,
        vertical_direction: VerticalDirection,
        spacing: f32,
        children: Vec<Widget>,
    },
    Flexible {
        flex: u32,
        fit: incular_config::FlexFit,
        child: Widget,
    },
    Wrap {
        axis: Axis,
        alignment: WrapAlignment,
        spacing: f32,
        run_alignment: WrapAlignment,
        run_spacing: f32,
        cross_axis_alignment: WrapCrossAlignment,
        text_direction: TextDirection,
        vertical_direction: VerticalDirection,
        children: Vec<Widget>,
    },
    Table {
        columns: usize,
        column_spacing: f32,
        row_spacing: f32,
        children: Vec<Widget>,
    },
    Stack {
        alignment: Alignment,
        text_direction: TextDirection,
        fit: StackFit,
        clip_behavior: Clip,
        children: Vec<Widget>,
    },
    Positioned {
        left: Option<f32>,
        top: Option<f32>,
        right: Option<f32>,
        bottom: Option<f32>,
        width: Option<f32>,
        height: Option<f32>,
        child: Widget,
    },
    IndexedStack {
        alignment: Alignment,
        index: usize,
        children: Vec<Widget>,
    },
    SafeArea {
        minimum: EdgeInsets,
        left: bool,
        top: bool,
        right: bool,
        bottom: bool,
        maintain_bottom_view_padding: bool,
        child: Widget,
    },
    ClipRect {
        clip_behavior: Clip,
        child: Widget,
    },
    ClipRRect {
        radius: CornerRadii,
        clip_behavior: Clip,
        child: Widget,
    },
    ClipOval {
        clip_behavior: Clip,
        child: Widget,
    },
    ClipPath {
        path: Arc<Path>,
        clip_behavior: Clip,
        child: Widget,
    },
    LayoutBuilder {
        builder: Rc<dyn Fn(Constraints) -> Widget>,
        environment: Option<Rc<dyn Any>>,
        environment_boundary: bool,
        /// Optional mutable revision for builders whose callback updates
        /// retained local state without replacing the parent widget.  The
        /// runtime samples this value during layout and rematerializes the
        /// builder child when it changes.
        revision: Option<Rc<Cell<u64>>>,
    },
    Visibility {
        visible: bool,
        child: Widget,
    },
    AspectRatio {
        ratio: f32,
        child: Widget,
    },
    Scroll {
        controller: ScrollController,
        axis: Axis,
        reverse: bool,
        physics: ScrollPhysics,
        child: Widget,
    },
    RawScrollbar {
        controller: ScrollController,
        style: RawScrollbarStyle,
        child: Widget,
    },
    ListWheelScrollView {
        view: RetainedWheelScrollView,
    },
    ListWheelViewport {
        viewport: RetainedWheelViewport,
    },
    DraggableScrollableSheet {
        sheet: RetainedDraggableSheet,
    },
    DraggableScrollableActuator {
        actuator: RetainedActuator,
        child: Widget,
    },
    TwoDimensionalScrollView {
        view: RetainedTwoDimensionalScrollView,
    },
    TwoDimensionalViewport {
        viewport: RetainedTwoDimensionalViewport,
    },
    /// A flow child which remains in the scrolling layout while an inner
    /// compositor transform pins it at the viewport's leading edge.
    PersistentHeader {
        controller: ScrollController,
        axis: Axis,
        reverse: bool,
        pinned: bool,
        child: Widget,
    },
    NotificationListener {
        callback: Option<Rc<dyn Fn(ScrollNotification) -> bool>>,
        child: Widget,
    },
    SliverViewport {
        config: Rc<SliverViewportConfig>,
    },
    Translate {
        controller: TranslationController,
        child: Widget,
    },
    Transform {
        transform: CoreTransform,
        origin: Option<Offset>,
        child: Widget,
    },
    Scale {
        controller: ScaleController,
        origin: Option<Offset>,
        child: Widget,
    },
    Rotation {
        controller: RotationController,
        origin: Option<Offset>,
        alignment: Option<Alignment>,
        child: Widget,
    },
    FittedBox {
        fit: ImageFit,
        alignment: Alignment,
        child: Widget,
    },
    Opacity {
        alpha: f32,
        controller: Option<OpacityController>,
        child: Widget,
    },
    Blur {
        sigma_x: f32,
        sigma_y: f32,
        controller: Option<BlurController>,
        child: Widget,
    },
    DropShadow {
        offset: Offset,
        sigma_x: f32,
        sigma_y: f32,
        color: Color,
        controller: Option<DropShadowController>,
        child: Widget,
    },
    ColorFiltered {
        filter: ColorFilter,
        controller: Option<ColorFilterController>,
        child: Widget,
    },
    Blend {
        mode: BlendMode,
        child: Widget,
    },
    ShaderMask {
        shader: ShaderCallback,
        blend_mode: BlendMode,
        child: Widget,
    },
    BackdropFilter {
        blur: GaussianBlur,
        blend_mode: BlendMode,
        enabled: bool,
        child: Widget,
    },
    AnnotatedRegion {
        annotation: Annotation,
        sized: bool,
        child: Widget,
    },
    CompositedTransformTarget {
        link: LayerLink,
        child: Widget,
    },
    CompositedTransformFollower {
        link: LayerLink,
        show_when_unlinked: bool,
        offset: Offset,
        target_anchor: LayerAnchor,
        follower_anchor: LayerAnchor,
        child: Widget,
    },
}

/// Renderer-neutral image sizing policy (maps to Flutter's `BoxFit`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ImageFit {
    Fill,
    #[default]
    Contain,
    Cover,
    FitWidth,
    FitHeight,
    None,
    ScaleDown,
}

/// Alias for [`ImageFit`], matching Flutter naming.
pub type BoxFit = ImageFit;

/// Repetition policy for raster image painting inside its allocated bounds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ImageRepeat {
    #[default]
    NoRepeat,
    RepeatX,
    RepeatY,
    Repeat,
}
/// A declarative raster image. It retains only a shared asset handle.
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    image: ImageHandle,
    width: Option<f32>,
    height: Option<f32>,
    fit: ImageFit,
    repeat: ImageRepeat,
    alignment: Alignment,
    sampling: ImageSampling,
}
impl Image {
    #[must_use]
    pub fn new(image: ImageHandle) -> Self {
        Self {
            image,
            width: None,
            height: None,
            fit: ImageFit::Contain,
            repeat: ImageRepeat::NoRepeat,
            alignment: Alignment::CENTER,
            sampling: ImageSampling::Linear,
        }
    }
    #[must_use]
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }
    #[must_use]
    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }
    #[must_use]
    pub fn fit(mut self, fit: ImageFit) -> Self {
        self.fit = fit;
        self
    }
    #[must_use]
    pub fn repeat(mut self, repeat: ImageRepeat) -> Self {
        self.repeat = repeat;
        self
    }
    #[must_use]
    pub fn alignment(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }
    #[must_use]
    pub fn sampling(mut self, sampling: ImageSampling) -> Self {
        self.sampling = sampling;
        self
    }

    #[must_use]
    pub fn filter_quality(self, quality: FilterQuality) -> Self {
        self.sampling(quality.into())
    }
}
impl From<Image> for Widget {
    fn from(value: Image) -> Self {
        Widget::image(
            value.image,
            value.width,
            value.height,
            value.fit,
            value.repeat,
            value.alignment,
            value.sampling,
        )
    }
}

/// Immutable declarative vector shape. Its coordinates remain local; normal
/// layout and compositor translation position it without changing PathId.
#[derive(Clone, Debug, PartialEq)]
pub struct PathView {
    path: Arc<Path>,
    fill: Option<Brush>,
    stroke: Option<(Brush, Stroke)>,
    size: Option<Size>,
}
impl PathView {
    #[must_use]
    pub fn new(path: impl Into<Arc<Path>>) -> Self {
        Self {
            path: path.into(),
            fill: None,
            stroke: None,
            size: None,
        }
    }
    #[must_use]
    pub fn fill(mut self, brush: impl Into<Brush>) -> Self {
        self.fill = Some(brush.into());
        self
    }
    #[must_use]
    pub fn stroke(mut self, brush: impl Into<Brush>, stroke: Stroke) -> Self {
        self.stroke = Some((brush.into(), stroke));
        self
    }
    #[must_use]
    pub fn size(mut self, size: Size) -> Self {
        self.size = Some(size);
        self
    }
}
impl From<PathView> for Widget {
    fn from(value: PathView) -> Self {
        Widget::shape(value.path, value.fill, value.stroke, value.size)
    }
}

/// Reusable vector icon backed by the same retained Path renderer as PathView.
#[derive(Clone, Debug, PartialEq)]
pub struct Icon {
    path: Arc<Path>,
    size: f32,
    brush: Brush,
}
impl Icon {
    #[must_use]
    pub fn new(path: impl Into<Arc<Path>>) -> Self {
        Self {
            path: path.into(),
            size: 24.,
            brush: Color::WHITE.into(),
        }
    }
    #[must_use]
    pub fn size(mut self, size: f32) -> Self {
        self.size = size.max(0.);
        self
    }
    #[must_use]
    pub fn brush(mut self, brush: impl Into<Brush>) -> Self {
        self.brush = brush.into();
        self
    }
}
impl From<Icon> for Widget {
    fn from(value: Icon) -> Self {
        // Icon paths use a canonical coordinate system (the built-in paths
        // are authored around a 24px viewport).  A PathView's `size` controls
        // layout only, so fit the geometry itself as well; otherwise a 12px
        // check would still paint at coordinates 3..21 and appear offset or
        // clipped inside its indicator.
        let path = value
            .path
            .bounds()
            .filter(|bounds| bounds.size.width > 0. && bounds.size.height > 0.)
            .map_or_else(
                || value.path.clone(),
                |bounds| {
                    let scale =
                        (value.size / bounds.size.width).min(value.size / bounds.size.height);
                    let fitted = Size::new(bounds.size.width * scale, bounds.size.height * scale);
                    let offset = Offset::new(
                        (value.size - fitted.width) * 0.5 - bounds.origin.x * scale,
                        (value.size - fitted.height) * 0.5 - bounds.origin.y * scale,
                    );
                    Arc::new(
                        value.path.transformed(
                            CoreTransform::translation(offset)
                                .then(CoreTransform::scale_non_uniform(scale, scale)),
                        ),
                    )
                },
            );
        PathView::new(path)
            .fill(value.brush)
            .size(Size::new(value.size, value.size))
            .into()
    }
}

/// Small shared demo icons. Each function returns the same immutable `Path`,
/// so repeated icons share PathId and retained tessellation/GPU meshes.
pub mod icons {
    use super::*;
    use std::sync::OnceLock;
    fn path(build: impl FnOnce(&mut incular_painting::PathBuilder)) -> Arc<Path> {
        let mut builder = Path::builder();
        build(&mut builder);
        Arc::new(builder.build())
    }
    #[must_use]
    pub fn check() -> Arc<Path> {
        static PATH: OnceLock<Arc<Path>> = OnceLock::new();
        PATH.get_or_init(|| {
            path(|p| {
                p.move_to(Offset::new(3., 12.))
                    .line_to(Offset::new(9., 18.))
                    .line_to(Offset::new(21., 4.))
                    .line_to(Offset::new(18., 2.))
                    .line_to(Offset::new(9., 14.))
                    .line_to(Offset::new(5., 10.))
                    .close();
            })
        })
        .clone()
    }
    #[must_use]
    pub fn close() -> Arc<Path> {
        static PATH: OnceLock<Arc<Path>> = OnceLock::new();
        PATH.get_or_init(|| {
            path(|p| {
                p.move_to(Offset::new(3., 5.))
                    .line_to(Offset::new(5., 3.))
                    .line_to(Offset::new(12., 10.))
                    .line_to(Offset::new(19., 3.))
                    .line_to(Offset::new(21., 5.))
                    .line_to(Offset::new(14., 12.))
                    .line_to(Offset::new(21., 19.))
                    .line_to(Offset::new(19., 21.))
                    .line_to(Offset::new(12., 14.))
                    .line_to(Offset::new(5., 21.))
                    .line_to(Offset::new(3., 19.))
                    .line_to(Offset::new(10., 12.))
                    .close();
            })
        })
        .clone()
    }
    #[must_use]
    pub fn plus() -> Arc<Path> {
        static PATH: OnceLock<Arc<Path>> = OnceLock::new();
        PATH.get_or_init(|| {
            path(|p| {
                p.move_to(Offset::new(10., 3.))
                    .line_to(Offset::new(14., 3.))
                    .line_to(Offset::new(14., 10.))
                    .line_to(Offset::new(21., 10.))
                    .line_to(Offset::new(21., 14.))
                    .line_to(Offset::new(14., 14.))
                    .line_to(Offset::new(14., 21.))
                    .line_to(Offset::new(10., 21.))
                    .line_to(Offset::new(10., 14.))
                    .line_to(Offset::new(3., 14.))
                    .line_to(Offset::new(3., 10.))
                    .line_to(Offset::new(10., 10.))
                    .close();
            })
        })
        .clone()
    }
    #[must_use]
    pub fn minus() -> Arc<Path> {
        static PATH: OnceLock<Arc<Path>> = OnceLock::new();
        PATH.get_or_init(|| {
            path(|p| {
                p.move_to(Offset::new(3., 10.))
                    .line_to(Offset::new(21., 10.))
                    .line_to(Offset::new(21., 14.))
                    .line_to(Offset::new(3., 14.))
                    .close();
            })
        })
        .clone()
    }
    #[must_use]
    pub fn chevron_right() -> Arc<Path> {
        static PATH: OnceLock<Arc<Path>> = OnceLock::new();
        PATH.get_or_init(|| {
            path(|p| {
                p.move_to(Offset::new(7., 3.))
                    .line_to(Offset::new(10., 0.))
                    .line_to(Offset::new(22., 12.))
                    .line_to(Offset::new(10., 24.))
                    .line_to(Offset::new(7., 21.))
                    .line_to(Offset::new(16., 12.))
                    .close();
            })
        })
        .clone()
    }

    /// A compact downward chevron used by select, disclosure, and menu
    /// controls.  Keeping each direction as a shared immutable path avoids
    /// per-control path construction and makes the icon independent of font
    /// fallback or glyph metrics.
    #[must_use]
    pub fn chevron_down() -> Arc<Path> {
        static PATH: OnceLock<Arc<Path>> = OnceLock::new();
        PATH.get_or_init(|| {
            path(|p| {
                p.move_to(Offset::new(3., 7.))
                    .line_to(Offset::new(6., 4.))
                    .line_to(Offset::new(12., 10.))
                    .line_to(Offset::new(18., 4.))
                    .line_to(Offset::new(21., 7.))
                    .line_to(Offset::new(12., 16.))
                    .close();
            })
        })
        .clone()
    }

    /// A compact upward chevron.
    #[must_use]
    pub fn chevron_up() -> Arc<Path> {
        static PATH: OnceLock<Arc<Path>> = OnceLock::new();
        PATH.get_or_init(|| {
            path(|p| {
                p.move_to(Offset::new(3., 17.))
                    .line_to(Offset::new(6., 20.))
                    .line_to(Offset::new(12., 14.))
                    .line_to(Offset::new(18., 20.))
                    .line_to(Offset::new(21., 17.))
                    .line_to(Offset::new(12., 8.))
                    .close();
            })
        })
        .clone()
    }

    /// A compact left-pointing chevron.
    #[must_use]
    pub fn chevron_left() -> Arc<Path> {
        static PATH: OnceLock<Arc<Path>> = OnceLock::new();
        PATH.get_or_init(|| {
            path(|p| {
                p.move_to(Offset::new(17., 3.))
                    .line_to(Offset::new(20., 6.))
                    .line_to(Offset::new(14., 12.))
                    .line_to(Offset::new(20., 18.))
                    .line_to(Offset::new(17., 21.))
                    .line_to(Offset::new(8., 12.))
                    .close();
            })
        })
        .clone()
    }
}

/// A small background/border wrapper. It intentionally does not clip children;
/// true rounded clipping belongs to Phase 9.1C.
#[derive(Clone, Debug, PartialEq)]
pub struct DecoratedBox {
    child: Widget,
    size: Option<Size>,
    background: Option<Brush>,
    border: Option<Border>,
    radius: CornerRadii,
}
impl DecoratedBox {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            size: None,
            background: None,
            border: None,
            radius: CornerRadii::default(),
        }
    }
    #[must_use]
    pub fn size(mut self, size: Size) -> Self {
        self.size = Some(size);
        self
    }
    #[must_use]
    pub fn background(mut self, brush: impl Into<Brush>) -> Self {
        self.background = Some(brush.into());
        self
    }
    #[must_use]
    pub fn border(mut self, border: impl Into<Border>) -> Self {
        self.border = Some(border.into());
        self
    }
    #[must_use]
    pub fn radius(mut self, radius: f32) -> Self {
        self.radius = CornerRadii::uniform(radius);
        self
    }
    #[must_use]
    pub fn decoration(mut self, decoration: crate::BoxDecoration) -> Self {
        if let Some(color) = decoration.color {
            self.background = Some(Brush::Solid(color));
        }
        if let Some(border) = decoration.border {
            self.border = Some(incular_rendering::Border::new(
                border.top.width,
                border.top.color,
            ));
        }
        if let Some(radius) = decoration.border_radius {
            self.radius = radius.to_corner_radii();
        }
        self
    }
}
impl From<DecoratedBox> for Widget {
    fn from(value: DecoratedBox) -> Self {
        Widget::decorated(
            value.size,
            value.background,
            value.border,
            value.radius,
            value.child,
        )
    }
}

/// Public text description. Its font metrics are resolved during layout, not
/// paint.  A plain [`Text::new`] uses [`TextStyle::default`] (16 logical px,
/// system UI family, normal weight) and reports its natural shaped size; it
/// does not expand to the parent width unless a parent layout allocates that
/// width explicitly.
#[derive(Clone, Debug, PartialEq)]
pub struct Text {
    text: String,
    style: TextStyle,
    align: TextAlign,
    soft_wrap: bool,
    max_lines: Option<usize>,
    overflow: TextOverflow,
}
impl Text {
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style: TextStyle::default(),
            align: TextAlign::Start,
            soft_wrap: true,
            max_lines: None,
            overflow: TextOverflow::Clip,
        }
    }
    #[must_use]
    pub fn style(mut self, style: TextStyle) -> Self {
        self.style = style;
        self
    }
    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.style.color = color;
        self
    }
    #[must_use]
    pub fn align(mut self, align: TextAlign) -> Self {
        self.align = align;
        self
    }
    /// Enables or disables Parley's soft line breaking at the allocated width.
    #[must_use]
    pub fn soft_wrap(mut self, soft_wrap: bool) -> Self {
        self.soft_wrap = soft_wrap;
        self
    }
    #[must_use]
    pub fn max_lines(mut self, max_lines: Option<usize>) -> Self {
        self.max_lines = max_lines;
        self
    }
    #[must_use]
    pub fn overflow(mut self, overflow: TextOverflow) -> Self {
        self.overflow = overflow;
        self
    }
}
impl From<Text> for Widget {
    fn from(value: Text) -> Self {
        Widget::text_configured(
            value.text,
            value.style,
            value.align,
            value.soft_wrap,
            value.max_lines,
            value.overflow,
        )
    }
}

/// Makes the renderer-independent rich paragraph description available in a
/// retained widget tree. Paragraph policy is preserved; inline style flattening
/// follows the existing `incular-text::RichText` contract.
impl From<RichText> for Widget {
    fn from(value: RichText) -> Self {
        let style = value
            .flatten()
            .first()
            .map_or_else(TextStyle::default, |run| run.style.clone());
        Widget::text_configured(
            value.plain_text(),
            style,
            value.text_align,
            value.soft_wrap,
            value.max_lines,
            value.overflow,
        )
    }
}

/// Core editable text widget.
///
/// This is Incular's renderer-neutral counterpart to Flutter's
/// `widgets/EditableText`.  Material `TextField` and multiline field chrome
/// belong to a higher-level component library. Keep the controller outside a
/// rebuilt widget description so text, selection, and active composition
/// survive rebuilds.
pub struct EditableText {
    controller: TextEditingController,
    size: Size,
    style: TextStyle,
    placeholder: String,
    on_submit: Option<Rc<dyn Fn(String)>>,
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
    input_type: TextInputTypeHint,
    input_action: TextInputActionHint,
}
impl EditableText {
    #[must_use]
    pub fn new(controller: TextEditingController) -> Self {
        Self {
            controller,
            size: Size::ZERO,
            style: TextStyle::default(),
            placeholder: String::new(),
            on_submit: None,
            multiline: false,
            min_lines: None,
            max_lines: Some(1),
            expands: false,
            text_align: TextAlign::Start,
            enabled: true,
            read_only: false,
            obscure_text: false,
            cursor_width: 1.0,
            cursor_height: None,
            cursor_radius: 0.0,
            show_cursor: true,
            cursor_color: Color::WHITE,
            selection_color: Color::rgba(72, 120, 220, 150),
            input_type: TextInputTypeHint::Text,
            input_action: TextInputActionHint::Unspecified,
        }
    }
    #[must_use]
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
    #[must_use]
    pub fn style(mut self, style: TextStyle) -> Self {
        self.style = style;
        self
    }
    #[must_use]
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }
    #[must_use]
    pub fn on_submit(mut self, callback: impl Fn(String) + 'static) -> Self {
        self.on_submit = Some(Rc::new(callback));
        self
    }
    /// Enables multiline editing. Material text-field wrappers should use
    /// this core primitive rather than introducing a separate `TextArea`
    /// widget name.
    #[must_use]
    pub fn multiline(mut self, multiline: bool) -> Self {
        self.multiline = multiline;
        if multiline && self.max_lines == Some(1) {
            self.max_lines = None;
        } else if !multiline {
            self.max_lines = Some(1);
        }
        self
    }
    /// Sets an explicit multiline height while preserving the current width.
    #[must_use]
    pub fn height(mut self, height: f32) -> Self {
        self.size = Size::new(self.size.width, height);
        self
    }

    #[must_use]
    pub fn min_lines(mut self, lines: Option<usize>) -> Self {
        self.min_lines = lines.map(|value| value.max(1));
        self
    }

    #[must_use]
    pub fn max_lines(mut self, lines: Option<usize>) -> Self {
        self.max_lines = lines.map(|value| value.max(1));
        self.multiline = !matches!(self.max_lines, Some(1));
        self
    }

    #[must_use]
    pub fn expands(mut self, expands: bool) -> Self {
        self.expands = expands;
        self
    }

    #[must_use]
    pub fn text_align(mut self, align: TextAlign) -> Self {
        self.text_align = align;
        self
    }

    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    #[must_use]
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    #[must_use]
    pub fn obscure_text(mut self, obscure_text: bool) -> Self {
        self.obscure_text = obscure_text;
        self
    }

    /// Sets the caret width in logical pixels.
    #[must_use]
    pub fn cursor_width(mut self, width: f32) -> Self {
        self.cursor_width = width.max(0.0);
        self
    }

    /// Sets an optional caret height. `None` uses the shaped line height.
    #[must_use]
    pub fn cursor_height(mut self, height: Option<f32>) -> Self {
        self.cursor_height = height.map(|value| value.max(0.0));
        self
    }

    /// Sets the caret corner radius. A zero radius keeps the caret rectangular.
    #[must_use]
    pub fn cursor_radius(mut self, radius: f32) -> Self {
        self.cursor_radius = radius.max(0.0);
        self
    }

    /// Controls whether the caret is painted while this editor is focused.
    #[must_use]
    pub fn show_cursor(mut self, show: bool) -> Self {
        self.show_cursor = show;
        self
    }

    /// Sets the caret paint color.
    #[must_use]
    pub fn cursor_color(mut self, color: Color) -> Self {
        self.cursor_color = color;
        self
    }

    /// Sets the selection highlight paint color.
    #[must_use]
    pub fn selection_color(mut self, color: Color) -> Self {
        self.selection_color = color;
        self
    }

    /// Selects the native keyboard/input method hint for this editor. The
    /// runtime still validates and owns the committed text value.
    #[must_use]
    pub fn input_type(mut self, input_type: TextInputTypeHint) -> Self {
        self.input_type = input_type;
        self
    }

    /// Selects the action shown by a native software keyboard.
    #[must_use]
    pub fn input_action(mut self, input_action: TextInputActionHint) -> Self {
        self.input_action = input_action;
        self
    }
}
impl From<EditableText> for Widget {
    fn from(value: EditableText) -> Self {
        Widget::editable_text_configured_with_cursor(
            value.controller,
            value.size,
            value.style,
            value.placeholder,
            value.on_submit,
            value.multiline,
            value.min_lines,
            value.max_lines,
            value.expands,
            value.text_align,
            value.enabled,
            value.read_only,
            value.obscure_text,
            value.cursor_width,
            value.cursor_height,
            value.cursor_radius,
            value.show_cursor,
            value.cursor_color,
            value.selection_color,
        )
        .with_text_input_hints(value.input_type, value.input_action)
    }
}

/// Declarative retained group opacity. The child is painted into an isolated
/// compositor target when alpha is between zero and one, so overlapping
/// descendants are attenuated exactly once.
#[derive(Clone, Debug, PartialEq)]
pub struct Opacity {
    alpha: f32,
    controller: Option<OpacityController>,
    child: Widget,
}
impl Opacity {
    #[must_use]
    pub fn new(alpha: f32, child: impl Into<Widget>) -> Self {
        Self {
            alpha: normalize_opacity(alpha),
            controller: None,
            child: child.into(),
        }
    }
    #[must_use]
    pub fn controlled(controller: OpacityController, child: impl Into<Widget>) -> Self {
        Self {
            alpha: controller.opacity(),
            controller: Some(controller),
            child: child.into(),
        }
    }
    #[must_use]
    pub fn controller(mut self, controller: OpacityController) -> Self {
        self.alpha = controller.opacity();
        self.controller = Some(controller);
        self
    }
    #[must_use]
    pub fn alpha(mut self, alpha: f32) -> Self {
        self.alpha = normalize_opacity(alpha);
        self
    }
}
impl From<Opacity> for Widget {
    fn from(value: Opacity) -> Self {
        match value.controller {
            Some(controller) => Widget::controlled_opacity(controller, value.child),
            None => Widget::opacity(value.alpha, value.child),
        }
    }
}

/// A retained opacity transition driven directly by an [`OpacityController`].
#[derive(Clone, Debug, PartialEq)]
pub struct FadeTransition {
    controller: OpacityController,
    child: Widget,
}
impl FadeTransition {
    #[must_use]
    pub fn new(controller: OpacityController, child: impl Into<Widget>) -> Self {
        Self {
            controller,
            child: child.into(),
        }
    }
}
impl From<FadeTransition> for Widget {
    fn from(value: FadeTransition) -> Self {
        Widget::controlled_opacity(value.controller, value.child)
    }
}

/// A retained translation transition driven directly by a
/// [`TranslationController`].
#[derive(Clone, Debug, PartialEq)]
pub struct SlideTransition {
    controller: TranslationController,
    child: Widget,
}
impl SlideTransition {
    #[must_use]
    pub fn new(controller: TranslationController, child: impl Into<Widget>) -> Self {
        Self {
            controller,
            child: child.into(),
        }
    }
}
impl From<SlideTransition> for Widget {
    fn from(value: SlideTransition) -> Self {
        Widget::translate(value.controller, value.child)
    }
}

/// An arbitrary retained affine transform. Layout remains the child's normal
/// layout; only the compositor, pointer coordinate conversion, and semantic
/// bounds observe the affine transform.
#[derive(Clone, Debug, PartialEq)]
pub struct Transform {
    transform: CoreTransform,
    origin: Option<Offset>,
    child: Widget,
}
impl Transform {
    #[must_use]
    pub fn new(transform: CoreTransform, child: impl Into<Widget>) -> Self {
        Self {
            transform,
            origin: None,
            child: child.into(),
        }
    }
    #[must_use]
    pub fn translation(offset: Offset, child: impl Into<Widget>) -> Self {
        Self::new(CoreTransform::translation(offset), child)
    }
    #[must_use]
    pub fn scale(scale: f32, child: impl Into<Widget>) -> Self {
        Self::new(CoreTransform::scale(scale), child)
    }
    #[must_use]
    pub fn rotation(radians: f32, child: impl Into<Widget>) -> Self {
        Self::new(CoreTransform::rotation(radians), child)
    }
    #[must_use]
    pub fn skew(x: f32, y: f32, child: impl Into<Widget>) -> Self {
        Self::new(CoreTransform::skew(x, y), child)
    }
    /// Selects the local pivot. The default is the child's center.
    #[must_use]
    pub fn origin(mut self, origin: Offset) -> Self {
        self.origin = Some(finite_offset(origin));
        self
    }
}
impl From<Transform> for Widget {
    fn from(value: Transform) -> Self {
        match value.origin {
            Some(origin) => Widget::transform_around(value.transform, origin, value.child),
            None => Widget::transform(value.transform, value.child),
        }
    }
}

/// A compositor-only scale transition driven by [`ScaleController`].
#[derive(Clone, Debug, PartialEq)]
pub struct ScaleTransition {
    controller: ScaleController,
    child: Widget,
}
impl ScaleTransition {
    #[must_use]
    pub fn new(controller: ScaleController, child: impl Into<Widget>) -> Self {
        Self {
            controller,
            child: child.into(),
        }
    }
}
impl From<ScaleTransition> for Widget {
    fn from(value: ScaleTransition) -> Self {
        Widget::controlled_scale(value.controller, value.child)
    }
}

/// A compositor-only clockwise rotation transition driven by
/// [`RotationController`].
#[derive(Clone, Debug, PartialEq)]
pub struct RotationTransition {
    controller: RotationController,
    alignment: Alignment,
    child: Widget,
}
impl RotationTransition {
    #[must_use]
    pub fn new(controller: RotationController, child: impl Into<Widget>) -> Self {
        Self {
            controller,
            alignment: Alignment::CENTER,
            child: child.into(),
        }
    }

    /// Creates a rotation from a normalized turns value without exposing the
    /// retained controller implementation. One turn is a full revolution.
    #[must_use]
    pub fn from_turns(turns: f32, child: impl Into<Widget>) -> Self {
        let controller = RotationController::new();
        controller.set_radians(if turns.is_finite() {
            turns * std::f32::consts::TAU
        } else {
            0.
        });
        Self::new(controller, child)
    }

    /// Selects the normalized pivot for the compositor transform. The
    /// default is the child's center, matching Flutter's transition.
    #[must_use]
    pub fn alignment(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }
}
impl From<RotationTransition> for Widget {
    fn from(value: RotationTransition) -> Self {
        // Resolve alignment against the retained child size at compositor
        // update time; a turns change therefore does not relayout or repaint
        // the child subtree.
        Widget::controlled_rotation_with_alignment(
            value.controller,
            Some(value.alignment),
            value.child,
        )
    }
}

/// A compact composition surface for the retained transition primitives.
/// It intentionally merges fade and slide behavior instead of creating a
/// large hierarchy of narrowly different animation widget types.
#[derive(Clone, Debug, PartialEq)]
pub struct Transition {
    opacity: Option<OpacityController>,
    translation: Option<TranslationController>,
    child: Widget,
}
impl Transition {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            opacity: None,
            translation: None,
            child: child.into(),
        }
    }
    #[must_use]
    pub fn fade(mut self, controller: OpacityController) -> Self {
        self.opacity = Some(controller);
        self
    }
    #[must_use]
    pub fn slide(mut self, controller: TranslationController) -> Self {
        self.translation = Some(controller);
        self
    }
}
impl From<Transition> for Widget {
    fn from(value: Transition) -> Self {
        let child = match value.translation {
            Some(controller) => Widget::translate(controller, value.child),
            None => value.child,
        };
        match value.opacity {
            Some(controller) => Widget::controlled_opacity(controller, child),
            None => child,
        }
    }
}

/// Declarative Gaussian blur isolation. Sigma is in logical pixels and is
/// converted to physical pixels by the renderer at the current DPI.
#[derive(Clone, Debug, PartialEq)]
pub struct Blur {
    sigma_x: f32,
    sigma_y: f32,
    controller: Option<BlurController>,
    child: Widget,
}
impl Blur {
    #[must_use]
    pub fn new(sigma: f32, child: impl Into<Widget>) -> Self {
        Self {
            sigma_x: normalize_sigma(sigma),
            sigma_y: normalize_sigma(sigma),
            controller: None,
            child: child.into(),
        }
    }
    #[must_use]
    pub fn asymmetric(sigma_x: f32, sigma_y: f32, child: impl Into<Widget>) -> Self {
        Self {
            sigma_x: normalize_sigma(sigma_x),
            sigma_y: normalize_sigma(sigma_y),
            controller: None,
            child: child.into(),
        }
    }
    #[must_use]
    pub fn controlled(controller: BlurController, child: impl Into<Widget>) -> Self {
        Self {
            sigma_x: controller.sigma(),
            sigma_y: controller.sigma(),
            controller: Some(controller),
            child: child.into(),
        }
    }
    #[must_use]
    pub fn sigma_x(mut self, sigma: f32) -> Self {
        self.sigma_x = normalize_sigma(sigma);
        self.controller = None;
        self
    }
    #[must_use]
    pub fn sigma_y(mut self, sigma: f32) -> Self {
        self.sigma_y = normalize_sigma(sigma);
        self.controller = None;
        self
    }
}
impl From<Blur> for Widget {
    fn from(value: Blur) -> Self {
        match value.controller {
            Some(controller) => Widget::controlled_blur(controller, value.child),
            None => Widget::asymmetric_blur(value.sigma_x, value.sigma_y, value.child),
        }
    }
}

/// Declarative arbitrary-subtree drop shadow. The source subtree is isolated
/// and its alpha is blurred, so text, images, gradients, and paths all share
/// the same shadow semantics.
#[derive(Clone, Debug, PartialEq)]
pub struct DropShadow {
    offset: Offset,
    sigma_x: f32,
    sigma_y: f32,
    color: Color,
    controller: Option<DropShadowController>,
    child: Widget,
}
impl DropShadow {
    #[must_use]
    pub fn new(offset: Offset, sigma: f32, color: Color, child: impl Into<Widget>) -> Self {
        Self {
            offset: finite_offset(offset),
            sigma_x: normalize_sigma(sigma),
            sigma_y: normalize_sigma(sigma),
            color,
            controller: None,
            child: child.into(),
        }
    }
    #[must_use]
    pub fn asymmetric(
        offset: Offset,
        sigma_x: f32,
        sigma_y: f32,
        color: Color,
        child: impl Into<Widget>,
    ) -> Self {
        Self {
            offset: finite_offset(offset),
            sigma_x: normalize_sigma(sigma_x),
            sigma_y: normalize_sigma(sigma_y),
            color,
            controller: None,
            child: child.into(),
        }
    }
    #[must_use]
    pub fn controlled(controller: DropShadowController, child: impl Into<Widget>) -> Self {
        Self {
            offset: controller.offset(),
            sigma_x: controller.sigma(),
            sigma_y: controller.sigma(),
            color: controller.color(),
            controller: Some(controller),
            child: child.into(),
        }
    }
    #[must_use]
    pub fn offset(mut self, offset: Offset) -> Self {
        self.offset = finite_offset(offset);
        self.controller = None;
        self
    }
    #[must_use]
    pub fn sigma(mut self, sigma: f32) -> Self {
        self.sigma_x = normalize_sigma(sigma);
        self.sigma_y = normalize_sigma(sigma);
        self.controller = None;
        self
    }
    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self.controller = None;
        self
    }
}
impl From<DropShadow> for Widget {
    fn from(value: DropShadow) -> Self {
        match value.controller {
            Some(controller) => Widget::controlled_drop_shadow(controller, value.child),
            None => Widget::drop_shadow(value.offset, value.sigma_x, value.color, value.child),
        }
    }
}

/// Declarative 4x5 color-matrix stage. The matrix is evaluated in straight
/// RGBA and converted back to the retained premultiplied texture format.
#[derive(Clone, Debug, PartialEq)]
pub struct ColorFiltered {
    filter: ColorFilter,
    controller: Option<ColorFilterController>,
    child: Widget,
}
impl ColorFiltered {
    #[must_use]
    pub fn new(filter: ColorFilter, child: impl Into<Widget>) -> Self {
        Self {
            filter,
            controller: None,
            child: child.into(),
        }
    }
    #[must_use]
    pub fn controlled(controller: ColorFilterController, child: impl Into<Widget>) -> Self {
        Self {
            filter: controller.filter(),
            controller: Some(controller),
            child: child.into(),
        }
    }
    #[must_use]
    pub fn matrix(mut self, matrix: [f32; 20]) -> Self {
        self.filter = ColorFilter::matrix(matrix);
        self.controller = None;
        self
    }
    #[must_use]
    pub fn filter(mut self, filter: ColorFilter) -> Self {
        self.filter = filter;
        self.controller = None;
        self
    }
    #[must_use]
    pub fn controller(mut self, controller: ColorFilterController) -> Self {
        self.filter = controller.filter();
        self.controller = Some(controller);
        self
    }
}
impl From<ColorFiltered> for Widget {
    fn from(value: ColorFiltered) -> Self {
        match value.controller {
            Some(controller) => Widget::controlled_color_filtered(controller, value.child),
            None => Widget::color_filtered(value.filter, value.child),
        }
    }
}

/// Declarative retained blend group. Destination-dependent modes promote only
/// the required composition scope to a sampleable intermediate target.
#[derive(Clone, Debug, PartialEq)]
pub struct Blend {
    mode: BlendMode,
    child: Widget,
}
impl Blend {
    #[must_use]
    pub fn new(mode: BlendMode, child: impl Into<Widget>) -> Self {
        Self {
            mode,
            child: child.into(),
        }
    }
    #[must_use]
    pub fn mode(mut self, mode: BlendMode) -> Self {
        self.mode = mode;
        self
    }
}
impl From<Blend> for Widget {
    fn from(value: Blend) -> Self {
        Widget::blend(value.mode, value.child)
    }
}

/// Small composable builder for ordered retained effects. Each method wraps
/// the current child, so calls read in the same order as execution.
#[derive(Clone, Debug, PartialEq)]
pub struct Effects {
    child: Widget,
}
impl Effects {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
    #[must_use]
    pub fn color_filter(mut self, filter: ColorFilter) -> Self {
        self.child = Widget::color_filtered(filter, self.child);
        self
    }
    #[must_use]
    pub fn color_matrix(self, filter: ColorFilter) -> Self {
        self.color_filter(filter)
    }
    #[must_use]
    pub fn blur(mut self, sigma: f32) -> Self {
        self.child = Widget::blur(sigma, self.child);
        self
    }
    #[must_use]
    pub fn asymmetric_blur(mut self, sigma_x: f32, sigma_y: f32) -> Self {
        self.child = Widget::asymmetric_blur(sigma_x, sigma_y, self.child);
        self
    }
    #[must_use]
    pub fn opacity(mut self, alpha: f32) -> Self {
        self.child = Widget::opacity(alpha, self.child);
        self
    }
    #[must_use]
    pub fn drop_shadow(mut self, offset: Offset, sigma: f32, color: Color) -> Self {
        self.child = Widget::drop_shadow(offset, sigma, color, self.child);
        self
    }
    #[must_use]
    pub fn blend(mut self, mode: BlendMode) -> Self {
        self.child = Widget::blend(mode, self.child);
        self
    }
    #[must_use]
    pub fn build(self) -> Widget {
        self.child
    }
}
impl From<Effects> for Widget {
    fn from(value: Effects) -> Self {
        value.build()
    }
}

/// Vertical retained viewport. Keep a [`ScrollController`] outside a rebuild
/// when application code needs the position to survive a recreated description.
pub struct ScrollView;
impl ScrollView {
    #[must_use]
    pub fn vertical(controller: ScrollController, child: impl Into<Widget>) -> Widget {
        Widget::scroll_view(controller, child.into())
    }
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
    RawScrollbar,
    Wheel(usize),
    TwoDimensional(ChildVicinity),
    Sheet,
    Actuator,
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
    sliver_pinned_ids: HashSet<SliverChildId>,
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
    /// Effective inherited retained builder environment for this element.
    environment: Option<Rc<dyn Any>>,
    /// Environment supplied directly by this element, if any. This lets a
    /// nested scope shadow its parent while descendants continue inheriting.
    environment_override: Option<Rc<dyn Any>>,
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
        view: RetainedWheelScrollView,
    },
    ListWheelViewport {
        viewport: RetainedWheelViewport,
    },
    DraggableScrollableSheet {
        sheet: RetainedDraggableSheet,
    },
    DraggableScrollableActuator {
        actuator: RetainedActuator,
    },
    TwoDimensionalScrollView {
        view: RetainedTwoDimensionalScrollView,
    },
    TwoDimensionalViewport {
        viewport: RetainedTwoDimensionalViewport,
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
    raw_recognizers: HashMap<ElementId, HashMap<TypeId, Box<dyn GestureRecognizer>>>,
    raw_gesture_streams: HashMap<GestureArenaKey, ActiveRawGesture>,
    raw_pointer_routes: HashMap<GestureArenaKey, Vec<ElementId>>,
    mouse_hover: HashMap<GestureArenaKey, Vec<ElementId>>,
    consumed_tap_pointers: HashSet<GestureArenaKey>,
    pointer_captures: HashMap<GestureArenaKey, ElementId>,
    active_drags: HashMap<GestureArenaKey, ActiveDrag>,
    scale_gestures: HashMap<ElementId, ScaleGestureDetector>,
    scrollbar_drag: Option<ScrollbarDrag>,
    semantics: SemanticsTree,
    semantic_ids: HashMap<ElementId, SemanticNodeId>,
    static_selections: HashMap<ElementId, StaticSelection>,
    environment: RuntimeEnvironment,
    pending_tree_error: Option<TreeError>,
    last_tree_error: Option<TreeError>,
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
