//! Opaque `Widget` transport operations and built-in lowering constructors.
//!
//! Public authoring should normally enter through concrete descriptors; crate-private constructors here are the lowering boundary into `WidgetKind`.

use super::super::*;
use super::{Widget, WidgetChildren, WidgetNode};

impl Widget {
    fn from_node(node: WidgetNode) -> Self {
        Self {
            node: Some(Rc::new(node)),
        }
    }

    /// Returns the immutable descriptor node backing this handle.
    #[must_use]
    pub(crate) fn node(&self) -> &WidgetNode {
        self.node.as_deref().expect("live widget descriptor handle")
    }

    #[must_use]
    pub(crate) fn node_mut(&mut self) -> &mut WidgetNode {
        Rc::make_mut(self.node.as_mut().expect("live widget descriptor handle"))
    }

    /// Returns whether two handles refer to the exact same declarative
    /// descriptor allocation.
    ///
    /// Descriptor identity is only an optimization hint; retained identity
    /// still belongs to `Element` and reconciliation continues to honor keys
    /// and widget type compatibility.
    #[must_use]
    pub(crate) fn ptr_eq(&self, other: &Self) -> bool {
        match (&self.node, &other.node) {
            (Some(left), Some(right)) => Rc::ptr_eq(left, right),
            (None, None) => true,
            _ => false,
        }
    }

    /// Returns the framework-internal descriptor kind by borrow.
    ///
    /// This is intentionally hidden from the Flutter-facing prelude. Plan 16
    /// removes the remaining workspace consumers that inspect built-in kinds.
    #[must_use]
    pub(crate) fn kind(&self) -> &WidgetKind {
        &self.node().kind
    }

    #[must_use]
    pub(crate) fn kind_mut(&mut self) -> &mut WidgetKind {
        &mut self.node_mut().kind
    }

    #[must_use]
    pub(crate) fn semantic_properties(&self) -> &SemanticProperties {
        &self.node().semantics
    }

    #[must_use]
    pub(crate) fn semantic_properties_mut(&mut self) -> &mut SemanticProperties {
        &mut self.node_mut().semantics
    }

    /// Returns typed metadata stored directly on an inherited-scope widget.
    /// This does not walk descendants; callers use it only when a descriptor
    /// contract intentionally wraps its child in a metadata scope.
    pub(crate) fn environment_value<T: Any>(&self) -> Option<&T> {
        match self.kind() {
            WidgetKind::LayoutBuilder {
                environment: Some(scope),
                ..
            } => scope.value.downcast_ref::<T>(),
            _ => None,
        }
    }

    pub(crate) fn set_key(&mut self, key: Option<Key>) {
        self.node_mut().key = key;
    }

    /// Creates a widget from a internal kind descriptor.
    #[must_use]
    pub(crate) fn from_kind(kind: WidgetKind) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind,
            semantics: SemanticProperties::default(),
        })
    }

    /// Attaches retained focus traversal metadata to a transparent wrapper.
    /// This is crate-internal because public callers use the Flutter-shaped
    /// `FocusTraversal*` widgets.
    #[doc(hidden)]
    pub(crate) fn with_focus_traversal_policy(mut self, policy: FocusTraversalPolicyKind) -> Self {
        self.semantic_properties_mut().focus_traversal_policy = Some(policy);
        self
    }

    #[doc(hidden)]
    pub(crate) fn with_focus_traversal_order(mut self, order: Option<f64>) -> Self {
        self.semantic_properties_mut().focus_traversal_order =
            order.filter(|value| value.is_finite());
        self
    }

    #[doc(hidden)]
    pub(crate) fn with_excluded_focus(mut self, excluding: bool) -> Self {
        self.semantic_properties_mut().exclude_focus = excluding;
        self
    }

    #[doc(hidden)]
    pub(crate) fn with_excluded_focus_traversal(mut self, excluding: bool) -> Self {
        self.semantic_properties_mut().exclude_focus_traversal = excluding;
        self
    }

    /// Attaches the retained undo-history capacity to a text-field subtree.
    /// The runtime consumes this metadata without making Widgets depend on
    /// the runtime crate.
    #[doc(hidden)]
    pub(crate) fn with_undo_history_max_entries(mut self, max_entries: usize) -> Self {
        self.semantic_properties_mut().undo_history_max_entries = Some(max_entries);
        self
    }

    /// Marks this retained subtree as the initial focus scope.
    #[doc(hidden)]
    pub(crate) fn with_focus_scope_autofocus(mut self, autofocus: bool) -> Self {
        self.semantic_properties_mut().focus_scope_autofocus = autofocus;
        self
    }

    /// Attaches native text-input hints to the retained editor without making
    /// the renderer-neutral widget depend on a platform window handle.
    #[doc(hidden)]
    pub(crate) fn with_text_input_hints(
        mut self,
        input_type: TextInputTypeHint,
        input_action: TextInputActionHint,
    ) -> Self {
        let semantics = self.semantic_properties_mut();
        semantics.text_input_type = Some(input_type);
        semantics.text_input_action = Some(input_action);
        self
    }

    #[doc(hidden)]
    pub(crate) fn with_semantic_callback(
        mut self,
        action: SemanticActionKind,
        callback: Rc<dyn Fn() + 'static>,
    ) -> Self {
        let callbacks = &mut self.semantic_properties_mut().callbacks;
        match action {
            SemanticActionKind::Activate => callbacks.activate = Some(callback),
            SemanticActionKind::Increment => callbacks.increment = Some(callback),
            SemanticActionKind::Decrement => callbacks.decrement = Some(callback),
            SemanticActionKind::ScrollForward => callbacks.scroll_forward = Some(callback),
            SemanticActionKind::ScrollBackward => callbacks.scroll_backward = Some(callback),
            SemanticActionKind::Focus
            | SemanticActionKind::SetText
            | SemanticActionKind::SetSelection => {}
        }
        self
    }

    /// Text content when this widget is text-like (DevTools labels only).
    pub fn text_if_any(&self) -> Option<String> {
        match self.kind() {
            WidgetKind::Text { text, .. } | WidgetKind::SelectableText { text, .. } => {
                Some(text.clone())
            }
            _ => None,
        }
    }

    /// Returns the best text label exposed by this widget or a transparent
    /// semantic/layout wrapper around it.
    ///
    /// Control crates use this when a Flutter-shaped control accepts a custom
    /// child instead of a dedicated `label` argument. The visual child and
    /// the accessible name are separate concepts, but text children provide a
    /// safe default for ordinary custom-content controls.
    #[doc(hidden)]
    pub fn semantic_text(&self) -> Option<String> {
        self.semantic_properties()
            .label
            .clone()
            .filter(|label| !label.trim().is_empty())
            .or_else(|| {
                self.semantic_properties()
                    .explicit
                    .as_ref()
                    .and_then(|semantics| semantics.label.clone())
                    .filter(|label| !label.trim().is_empty())
            })
            .or_else(|| widget_text(self))
    }

    #[must_use]
    pub fn box_(size: Size, color: Color) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Box { size, color },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub(in crate::tree) fn shape(
        path: Arc<Path>,
        fill: Option<Brush>,
        stroke: Option<(Brush, Stroke)>,
        size: Option<Size>,
    ) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Shape {
                path,
                fill,
                stroke,
                size,
            },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub(in crate::tree) fn decorated(
        size: Option<Size>,
        background: Option<Brush>,
        border: Option<Border>,
        radius: CornerRadii,
        child: Widget,
    ) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Decorated {
                size,
                background,
                border,
                radius,
                child,
            },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub(crate) fn button(size: Size, color: Color, action: ActionId) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Button(ButtonSpec {
                size,
                color,
                hover_color: None,
                pressed_color: None,
                focused_color: None,
                disabled_color: None,
                enabled: true,
                focusable_when_disabled: false,
                action,
                callback: None,
                hover_action: ActionId(0),
                hover_callback: None,
                exit_action: ActionId(0),
                exit_callback: None,
                has_callback: false,
                child: None,
            }),
            semantics: SemanticProperties::default(),
        })
    }

    /// Builds the retained action surface shared by sibling control crates.
    ///
    /// This is crate-internal plumbing rather than a Flutter-facing widget;
    /// applications should use `GestureDetector` or a Material button.
    #[doc(hidden)]
    pub(crate) fn action_surface(surface: crate::internal::ActionSurface) -> Self {
        let semantic_label = if surface.label.is_empty() {
            surface.content.as_ref().and_then(Widget::semantic_text)
        } else {
            Some(surface.label.clone())
        };
        let label = surface.content.unwrap_or_else(|| {
            Widget::padding(
                surface.padding,
                Widget::text_styled(surface.label, surface.label_style, TextAlign::Start),
            )
        });
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Button(ButtonSpec {
                size: surface.size,
                color: surface.color,
                hover_color: surface.hover_color,
                pressed_color: surface.pressed_color,
                focused_color: surface.focused_color,
                disabled_color: surface.disabled_color,
                enabled: surface.enabled,
                focusable_when_disabled: surface.focusable_when_disabled,
                action: ActionId(0),
                callback: surface.callback,
                hover_action: ActionId(0),
                hover_callback: surface.hover_callback,
                exit_action: ActionId(0),
                exit_callback: surface.exit_callback,
                has_callback: false,
                child: Some(label),
            }),
            semantics: SemanticProperties {
                // Custom content is inspected for a text or explicit semantic
                // label so low-level controls retain a useful accessible name.
                label: semantic_label,
                ..SemanticProperties::default()
            },
        })
    }
    pub fn bind_callbacks(&mut self, allocate: &mut impl FnMut(Rc<dyn Fn()>) -> ActionId) {
        if let WidgetKind::Button(spec) = self.kind_mut() {
            if let Some(callback) = spec.callback.take() {
                spec.action = allocate(callback);
                spec.has_callback = true;
            }
            if let Some(callback) = spec.hover_callback.take() {
                spec.hover_action = allocate(callback);
            }
            if let Some(callback) = spec.exit_callback.take() {
                spec.exit_action = allocate(callback);
            }
        }
    }
    #[must_use]
    pub(crate) fn text_styled(text: impl Into<String>, style: TextStyle, align: TextAlign) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Text {
                text: text.into(),
                style,
                align,
                soft_wrap: true,
                max_lines: None,
                overflow: TextOverflow::Clip,
            },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub(crate) fn text_configured(
        text: impl Into<String>,
        style: TextStyle,
        align: TextAlign,
        soft_wrap: bool,
        max_lines: Option<usize>,
        overflow: TextOverflow,
    ) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Text {
                text: text.into(),
                style,
                align,
                soft_wrap,
                max_lines,
                overflow,
            },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub fn selectable_text_styled(
        text: impl Into<String>,
        style: TextStyle,
        align: TextAlign,
    ) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::SelectableText {
                text: text.into(),
                style,
                align,
            },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub fn selection_area(controller: SelectionAreaController, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::SelectionArea { controller, child },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub fn selection_container(delegate: SelectionContainerDelegate, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::SelectionContainer { delegate, child },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub(crate) fn selection_listener(notifier: SelectionListenerNotifier, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::SelectionListener {
                delegate: SelectionContainerDelegate::with_controller(notifier.controller()),
                notifier,
                child,
            },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub(crate) fn indexed_semantics(index: usize, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::IndexedSemantics { index, child },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub(crate) fn semantics_debugger(
        label_style: TextStyle,
        max_nodes: usize,
        child: Self,
    ) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::SemanticsDebugger {
                label_style,
                max_nodes,
                child,
            },
            semantics: SemanticProperties::default(),
        })
    }
    pub(crate) fn semantic_index(&self) -> Option<usize> {
        match self.kind() {
            WidgetKind::IndexedSemantics { index, .. } => Some(*index),
            _ => None,
        }
    }
    #[must_use]
    pub(in crate::tree) fn image(
        image: ImageHandle,
        width: Option<f32>,
        height: Option<f32>,
        fit: ImageFit,
        repeat: ImageRepeat,
        alignment: Alignment,
        sampling: ImageSampling,
    ) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Image {
                image,
                width,
                height,
                fit,
                repeat,
                alignment,
                sampling,
            },
            semantics: SemanticProperties::default(),
        })
    }
    /// Creates the editable primitive with renderer-neutral caret and
    /// selection paint controls. Material uses this richer boundary for
    /// `TextField` cursor/selection configuration while the legacy constructor
    /// above keeps its source-compatible defaults.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn editable_text_configured_with_cursor(
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
    ) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::TextField(TextFieldSpec {
                controller,
                size,
                style,
                placeholder,
                on_submit,
                multiline,
                min_lines: min_lines.map(|value| value.max(1)),
                max_lines: max_lines.map(|value| value.max(1)),
                expands,
                text_align,
                enabled,
                read_only,
                obscure_text,
                cursor_width: cursor_width.max(0.0),
                cursor_height: cursor_height.map(|value| value.max(0.0)),
                cursor_radius: cursor_radius.max(0.0),
                show_cursor,
                cursor_color,
                selection_color,
            }),
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub(crate) fn padding(padding: EdgeInsets, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Padding { padding, child },
            semantics: SemanticProperties::default(),
        })
    }
    /// Creates an explicit retained picture boundary around `child`.
    ///
    /// Descendant paint changes update their own cached picture without
    /// repainting this boundary's otherwise empty retained picture.
    #[must_use]
    pub(crate) fn repaint_boundary(child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::RepaintBoundary { child },
            semantics: SemanticProperties::default(),
        })
    }
    pub(crate) fn draggable(source: Rc<dyn RetainedDragSource>, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Draggable { source, child },
            semantics: SemanticProperties::default(),
        })
    }
    pub(crate) fn drag_target(target: Rc<dyn RetainedDragTarget>, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::DragTarget { target, child },
            semantics: SemanticProperties::default(),
        })
    }
    /// Removes this subtree from pointer hit testing while leaving painting and
    /// semantics intact. Siblings behind it remain eligible for the event.
    #[must_use]
    pub(crate) fn ignore_pointer(ignoring: bool, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::IgnorePointer { ignoring, child },
            semantics: SemanticProperties::default(),
        })
    }
    /// Intercepts pointer hit testing at this boundary. Descendants do not
    /// receive ordinary retained interaction while painting and semantics are
    /// preserved.
    #[must_use]
    pub(crate) fn absorb_pointer(absorbing: bool, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::AbsorbPointer { absorbing, child },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub(crate) fn layout_builder(
        builder: impl for<'a> Fn(&BuildContext<'a>, Constraints) -> Self + 'static,
    ) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::LayoutBuilder {
                builder: Rc::new(builder),
                environment: None,
                environment_boundary: false,
                revision: None,
            },
            semantics: SemanticProperties::default(),
        })
    }

    /// Wraps a child in a retained typed inherited scope. The wrapper is
    /// transparent to layout and paint; descendant builders read the value
    /// through their explicit [`BuildContext`].
    #[must_use]
    pub fn environment_scope<T: Any>(value: T, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::LayoutBuilder {
                builder: Rc::new(move |_, _| child.clone()),
                environment: Some(InheritedScopeValue::new(value)),
                environment_boundary: false,
                revision: None,
            },
            semantics: SemanticProperties::default(),
        })
    }

    /// Creates a transparent retained node that prevents descendants from
    /// reading typed environments installed above it.
    #[must_use]
    pub fn environment_boundary(child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::LayoutBuilder {
                builder: Rc::new(move |_, _| child.clone()),
                environment: None,
                environment_boundary: true,
                revision: None,
            },
            semantics: SemanticProperties::default(),
        })
    }

    /// Creates a retained layout builder backed by an explicit local state
    /// revision.  A callback can increment `revision` and the next frame will
    /// rebuild only this builder's child, preserving the rest of the tree.
    /// This is the primitive used by uncontrolled controls such as checkbox,
    /// switch, toggle, and slider.
    #[must_use]
    pub fn stateful_layout_builder(
        revision: Rc<Cell<u64>>,
        builder: impl for<'a> Fn(&BuildContext<'a>, Constraints) -> Self + 'static,
    ) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::LayoutBuilder {
                builder: Rc::new(builder),
                environment: None,
                environment_boundary: false,
                revision: Some(revision),
            },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub(crate) fn scroll_view(controller: ScrollController, child: Self) -> Self {
        Self::scroll_view_with_config(
            controller,
            child,
            Axis::Vertical,
            false,
            ScrollPhysics::default(),
        )
    }

    /// Creates a retained viewport with its complete scroll policy.  The
    /// public `scroll_view` helper intentionally keeps its historical
    /// vertical/clamping defaults; scroll descriptors use this crate-local
    /// constructor so axis, reverse, and physics survive lowering.
    #[must_use]
    pub(crate) fn scroll_view_with_config(
        controller: ScrollController,
        child: Self,
        axis: Axis,
        reverse: bool,
        physics: ScrollPhysics,
    ) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Scroll {
                controller,
                axis,
                reverse,
                physics,
                child,
            },
            semantics: SemanticProperties::default(),
        })
    }
    /// Creates a raw scrollbar with explicit renderer-independent styling.
    #[must_use]
    pub(crate) fn raw_scrollbar_with_style(
        controller: ScrollController,
        style: RawScrollbarStyle,
        child: impl Into<Self>,
    ) -> Self {
        let child = child.into();
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::RawScrollbar {
                controller,
                style,
                child,
            },
            semantics: SemanticProperties::default(),
        })
    }

    /// Creates a retained list-wheel viewport from its focused model.
    #[must_use]
    pub(crate) fn list_wheel_viewport(viewport: ListWheelViewport<Self>) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::ListWheelViewport {
                config: Rc::new(viewport.into_retained_config()),
            },
            semantics: SemanticProperties::default(),
        })
    }

    /// Creates a retained list-wheel scroll view from its focused model.
    #[must_use]
    pub(crate) fn list_wheel_scroll_view(view: ListWheelScrollView<Self>) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::ListWheelScrollView {
                config: Rc::new(view.into_retained_config()),
            },
            semantics: SemanticProperties::default(),
        })
    }

    /// Creates a retained draggable sheet from its focused model.
    #[must_use]
    pub(crate) fn draggable_scrollable_sheet(sheet: DraggableScrollableSheet<Self>) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::DraggableScrollableSheet {
                config: Rc::new(sheet.into_retained_config()),
            },
            semantics: SemanticProperties::default(),
        })
    }

    /// Creates an actuator wrapper which resets the nearest descendant sheets.
    #[must_use]
    pub(crate) fn draggable_scrollable_actuator(
        actuator: DraggableScrollableActuator,
        child: impl Into<Self>,
    ) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::DraggableScrollableActuator {
                actuator,
                child: child.into(),
            },
            semantics: SemanticProperties::default(),
        })
    }

    /// Creates a retained two-dimensional viewport from its focused model.
    #[must_use]
    pub(crate) fn two_dimensional_viewport(viewport: TwoDimensionalViewport<Self>) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::TwoDimensionalViewport {
                config: Rc::new(viewport.into_retained_config()),
            },
            semantics: SemanticProperties::default(),
        })
    }

    /// Creates a retained two-dimensional scroll view from its focused model.
    #[must_use]
    pub(crate) fn two_dimensional_scroll_view(view: TwoDimensionalScrollView<Self>) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::TwoDimensionalScrollView {
                config: Rc::new(view.into_retained_config()),
            },
            semantics: SemanticProperties::default(),
        })
    }
    /// Keeps this flow child at the leading edge of `controller`'s viewport
    /// once it reaches that edge. Consecutive persistent headers push their
    /// predecessors away instead of visually overlapping them.
    #[must_use]
    pub fn persistent_header(controller: ScrollController, child: Self) -> Self {
        Self::persistent_header_with_config(controller, child, Axis::Vertical, false, true)
    }
    /// Creates a persistent header with the axis and direction of its sliver
    /// viewport.  The plain helper retains the historical vertical defaults;
    /// sliver descriptors use this configured form during lowering.
    #[must_use]
    pub(crate) fn persistent_header_with_config(
        controller: ScrollController,
        child: Self,
        axis: Axis,
        reverse: bool,
        pinned: bool,
    ) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::PersistentHeader {
                controller,
                axis,
                reverse,
                pinned,
                child,
            },
            semantics: SemanticProperties::default(),
        })
    }

    /// Creates a retained notification-listener wrapper. The wrapper has no
    /// visual effect; the tree installs the callback on descendant scroll
    /// positions after those positions are mounted.
    pub(crate) fn notification_listener(
        callback: Option<Rc<dyn Fn(ScrollNotification) -> bool>>,
        child: Self,
    ) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::NotificationListener { callback, child },
            semantics: SemanticProperties::default(),
        })
    }

    /// Creates a retained sliver viewport. Sliver children are materialized by
    /// the viewport delegate during layout, so they are not represented as a
    /// declarative `Column` child list.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn sliver_viewport_with_delegate_options(
        controller: ScrollController,
        axis: Axis,
        reverse: bool,
        physics: ScrollPhysics,
        cache_extent: f32,
        shrink_wrap: bool,
        clip_behavior: Clip,
        delegate: Rc<dyn SliverViewportDelegate>,
    ) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::SliverViewport {
                config: Rc::new(SliverViewportConfig {
                    controller,
                    axis,
                    reverse,
                    physics,
                    cache_extent: cache_extent.max(0.),
                    shrink_wrap,
                    clip_behavior,
                    delegate,
                }),
            },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub fn translate(controller: TranslationController, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Translate { controller, child },
            semantics: SemanticProperties::default(),
        })
    }
    /// Applies an arbitrary Kurbo-backed affine transform after layout.
    /// The transform is compositor-only and defaults to the child's center.
    #[must_use]
    pub(crate) fn transform(transform: CoreTransform, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Transform {
                transform,
                origin: None,
                child,
            },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub(crate) fn transform_around(transform: CoreTransform, origin: Offset, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Transform {
                transform,
                origin: Some(finite_offset(origin)),
                child,
            },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub(crate) fn fitted_box(fit: ImageFit, alignment: Alignment, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::FittedBox {
                fit,
                alignment,
                child,
            },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub fn controlled_scale(controller: ScaleController, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Scale {
                controller,
                origin: None,
                child,
            },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub fn controlled_rotation(controller: RotationController, child: Self) -> Self {
        Self::controlled_rotation_with_alignment(controller, None, child)
    }
    #[must_use]
    pub(crate) fn controlled_rotation_with_alignment(
        controller: RotationController,
        alignment: Option<Alignment>,
        child: Self,
    ) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Rotation {
                controller,
                origin: None,
                alignment,
                child,
            },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub(crate) fn opacity(alpha: f32, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Opacity {
                alpha: normalize_opacity(alpha),
                controller: None,
                child,
            },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub fn controlled_opacity(controller: OpacityController, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Opacity {
                alpha: controller.opacity(),
                controller: Some(controller),
                child,
            },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub(crate) fn blur(sigma: f32, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Blur {
                sigma_x: normalize_sigma(sigma),
                sigma_y: normalize_sigma(sigma),
                controller: None,
                child,
            },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub fn asymmetric_blur(sigma_x: f32, sigma_y: f32, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Blur {
                sigma_x: normalize_sigma(sigma_x),
                sigma_y: normalize_sigma(sigma_y),
                controller: None,
                child,
            },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub fn controlled_blur(controller: BlurController, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Blur {
                sigma_x: controller.sigma(),
                sigma_y: controller.sigma(),
                controller: Some(controller),
                child,
            },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub(crate) fn drop_shadow(offset: Offset, sigma: f32, color: Color, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::DropShadow {
                offset: finite_offset(offset),
                sigma_x: normalize_sigma(sigma),
                sigma_y: normalize_sigma(sigma),
                color,
                controller: None,
                child,
            },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub fn controlled_drop_shadow(controller: DropShadowController, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::DropShadow {
                offset: controller.offset(),
                sigma_x: controller.sigma(),
                sigma_y: controller.sigma(),
                color: controller.color(),
                controller: Some(controller),
                child,
            },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub(crate) fn color_filtered(filter: ColorFilter, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::ColorFiltered {
                filter,
                controller: None,
                child,
            },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub fn color_matrix(filter: ColorFilter, child: Self) -> Self {
        Self::color_filtered(filter, child)
    }
    #[must_use]
    pub fn controlled_color_filtered(controller: ColorFilterController, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::ColorFiltered {
                filter: controller.filter(),
                controller: Some(controller),
                child,
            },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub(crate) fn blend(mode: BlendMode, child: Self) -> Self {
        Self::from_node(WidgetNode {
            key: None,
            kind: WidgetKind::Blend { mode, child },
            semantics: SemanticProperties::default(),
        })
    }
    #[must_use]
    pub fn with_key(mut self, key: impl Into<Key>) -> Self {
        self.set_key(Some(key.into()));
        self
    }
    /// Overrides the accessible label contributed by this meaningful widget.
    #[must_use]
    pub fn accessibility_label(mut self, label: impl Into<String>) -> Self {
        self.semantic_properties_mut().label = Some(label.into());
        self
    }
    /// Adds a screen-reader description without changing visible text.
    #[must_use]
    pub fn accessibility_description(mut self, description: impl Into<String>) -> Self {
        self.semantic_properties_mut().description = Some(description.into());
        self
    }
    /// Supplies explicit Incular semantic metadata for this visual widget.
    /// This is useful for icon-only controls, meaningful images, headings,
    /// dialogs, and custom-painted controls; it never exposes native adapter
    /// types to application code.
    #[must_use]
    pub fn semantics(mut self, semantics: ExplicitSemantics) -> Self {
        self.semantic_properties_mut().explicit = Some(semantics);
        self
    }
    /// Excludes this widget and its implementation-detail subtree from semantics.
    #[must_use]
    pub fn exclude_semantics(mut self) -> Self {
        self.semantic_properties_mut().hidden = true;
        self
    }
    /// Merges meaningful descendants into one logical accessible node. The
    /// widget itself must have explicit semantics or a meaningful built-in
    /// role; descendants are not exposed separately.
    #[must_use]
    pub fn merge_semantics(mut self) -> Self {
        self.semantic_properties_mut().merge_descendants = true;
        self
    }
    /// Suppresses preceding semantic siblings at this stacking level. Use it
    /// for a modal/dialog region so screen-reader traversal cannot fall through
    /// to visual content behind the active modal.
    #[must_use]
    pub fn block_semantics(mut self) -> Self {
        self.semantic_properties_mut().block_previous_siblings = true;
        self
    }
    #[must_use]
    pub fn key(&self) -> Option<&Key> {
        self.node().key.as_ref()
    }
    /// Stable diagnostic name for the concrete widget represented by this
    /// opaque transport value.
    ///
    /// This intentionally exposes no layout/render taxonomy or payload data.
    #[must_use]
    pub fn debug_type_name(&self) -> &'static str {
        self.type_().name()
    }
    pub(in crate::tree) fn type_(&self) -> WidgetType {
        self.kind().structure().widget_type
    }
    /// Shallow child view for reconciliation. Deep per-child clones were the
    /// measured allocation fire on wide trees (Task 15); reconciliation only
    /// needs references because cloning happens once per *created* element.
    pub(in crate::tree) fn children_refs(&self) -> WidgetChildren<'_> {
        self.kind().structure().children
    }
}

impl RawScrollbar {
    /// Lowers this stateful scrollbar model into a retained widget overlay.
    #[must_use]
    pub fn into_widget(self, child: impl Into<Widget>) -> Widget {
        Widget::raw_scrollbar_with_style(self.controller(), self.style(), child)
    }

    /// Alias for [`Self::into_widget`] with Flutter-style wrapper wording.
    #[must_use]
    pub fn with_child(self, child: impl Into<Widget>) -> Widget {
        self.into_widget(child)
    }
}

impl From<RawScrollbar> for Widget {
    fn from(value: RawScrollbar) -> Self {
        value.into_widget(Widget::box_(Size::ZERO, Color::TRANSPARENT))
    }
}

impl From<ListWheelViewport<Widget>> for Widget {
    fn from(value: ListWheelViewport<Widget>) -> Self {
        Widget::list_wheel_viewport(value)
    }
}

impl From<ListWheelScrollView<Widget>> for Widget {
    fn from(value: ListWheelScrollView<Widget>) -> Self {
        Widget::list_wheel_scroll_view(value)
    }
}

impl From<DraggableScrollableSheet<Widget>> for Widget {
    fn from(value: DraggableScrollableSheet<Widget>) -> Self {
        Widget::draggable_scrollable_sheet(value)
    }
}

impl DraggableScrollableActuator {
    /// Lowers this reset channel into a transparent retained wrapper.
    #[must_use]
    pub fn with_child(self, child: impl Into<Widget>) -> Widget {
        Widget::draggable_scrollable_actuator(self, child)
    }
}

impl From<DraggableScrollableActuator> for Widget {
    fn from(value: DraggableScrollableActuator) -> Self {
        value.with_child(Widget::box_(Size::ZERO, Color::TRANSPARENT))
    }
}

impl From<TwoDimensionalViewport<Widget>> for Widget {
    fn from(value: TwoDimensionalViewport<Widget>) -> Self {
        Widget::two_dimensional_viewport(value)
    }
}

impl From<TwoDimensionalScrollView<Widget>> for Widget {
    fn from(value: TwoDimensionalScrollView<Widget>) -> Self {
        Widget::two_dimensional_scroll_view(value)
    }
}
