//! Built-in descriptor payloads and the sealed widget taxonomy.
//!
//! These types describe immutable widget configuration only. They do not own mounted identity, layout state, or rendering resources.

use super::*;

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
pub(crate) struct SemanticProperties {
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
    pub(super) activate: Option<Rc<dyn Fn() + 'static>>,
    pub(super) increment: Option<Rc<dyn Fn() + 'static>>,
    pub(super) decrement: Option<Rc<dyn Fn() + 'static>>,
    pub(super) scroll_forward: Option<Rc<dyn Fn() + 'static>>,
    pub(super) scroll_backward: Option<Rc<dyn Fn() + 'static>>,
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
pub(crate) enum WidgetKind {
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
        builder: Rc<LayoutBuilderCallback>,
        environment: Option<InheritedScopeValue>,
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
        child: Widget,
    },
    TwoDimensionalScrollView {
        config: Rc<TwoDimensionalViewportConfig<Widget>>,
    },
    TwoDimensionalViewport {
        config: Rc<TwoDimensionalViewportConfig<Widget>>,
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
