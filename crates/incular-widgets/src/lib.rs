//! Declarative widgets backed by persistent element and render-object arenas.
//!
//! A [`Widget`] is a cheap value. [`WidgetTree`] owns mounted identity and all
//! mutable layout/paint state. Reconciliation only examines direct children of
//! the element being updated.

use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
    time::{Duration, Instant},
};

use incular_animation::AnimationController;
use incular_core::{Arena, ArenaId, Color, DirtyFlags, Offset, Rect, Size, Transform};
use incular_layout::{Alignment, Axis, Constraints, EdgeInsets};
use incular_painting::{DisplayList, LayerId, LayerTree, PaintCommand};
use incular_text::{TextAlign, TextDiagnostics, TextEngine, TextLayout, TextStyle};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ElementId(ArenaId);
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RenderObjectId(ArenaId);
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ActionId(pub u64);
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonState {
    #[default]
    Normal,
    Hovered,
    Pressed,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    Value(u64),
    String(String),
}
impl From<u64> for Key {
    fn from(value: u64) -> Self {
        Self::Value(value)
    }
}
impl From<&str> for Key {
    fn from(value: &str) -> Self {
        Self::String(value.into())
    }
}

#[derive(Clone, Default)]
pub struct ScrollController {
    state: Rc<RefCell<ScrollState>>,
}
impl std::fmt::Debug for ScrollController {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScrollController")
            .field("offset", &self.offset())
            .field("max_offset", &self.max_offset())
            .finish()
    }
}
impl PartialEq for ScrollController {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.state, &other.state)
    }
}
#[derive(Clone, Copy, Debug, Default)]
struct ScrollState {
    offset: f32,
    max_offset: f32,
    revision: u64,
}
impl ScrollController {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    #[must_use]
    pub fn offset(&self) -> f32 {
        self.state.borrow().offset
    }
    #[must_use]
    pub fn max_offset(&self) -> f32 {
        self.state.borrow().max_offset
    }
    pub fn jump_to(&self, offset: f32) -> bool {
        let mut s = self.state.borrow_mut();
        let value = offset.clamp(0., s.max_offset);
        if value == s.offset {
            return false;
        }
        s.offset = value;
        s.revision += 1;
        true
    }
    pub fn scroll_by(&self, delta: f32) -> bool {
        self.jump_to(self.offset() + delta)
    }
    fn set_extents(&self, content: f32, viewport: f32) {
        let mut s = self.state.borrow_mut();
        s.max_offset = (content - viewport).max(0.);
        let next = s.offset.min(s.max_offset);
        if next != s.offset {
            s.offset = next;
            s.revision += 1;
        }
    }
}

#[derive(Clone)]
pub struct TranslationController {
    offset: Rc<Cell<Offset>>,
    revision: Rc<Cell<u64>>,
    animation: Rc<RefCell<AnimationController>>,
    from: Rc<Cell<Offset>>,
    to: Rc<Cell<Offset>>,
}
impl std::fmt::Debug for TranslationController {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TranslationController")
            .field("offset", &self.offset())
            .finish()
    }
}
impl PartialEq for TranslationController {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.offset, &other.offset)
    }
}
impl Default for TranslationController {
    fn default() -> Self {
        Self {
            offset: Rc::new(Cell::new(Offset::ZERO)),
            revision: Rc::new(Cell::new(0)),
            animation: Rc::new(RefCell::new(AnimationController::new(
                Duration::from_millis(300),
            ))),
            from: Rc::new(Cell::new(Offset::ZERO)),
            to: Rc::new(Cell::new(Offset::ZERO)),
        }
    }
}
impl TranslationController {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    #[must_use]
    pub fn offset(&self) -> Offset {
        self.offset.get()
    }
    pub fn set_offset(&self, offset: Offset) -> bool {
        if self.offset.get() == offset {
            return false;
        }
        self.offset.set(offset);
        self.revision.set(self.revision.get() + 1);
        true
    }
    pub fn animate_to(&self, target: Offset, duration: Duration, now: Instant) {
        self.from.set(self.offset());
        self.to.set(target);
        let animation = AnimationController::new(duration);
        animation.forward(now);
        *self.animation.borrow_mut() = animation;
    }
    fn tick(&self, now: Instant) -> bool {
        let animation = self.animation.borrow();
        if !animation.tick(now) {
            return false;
        }
        let t = animation.value();
        self.set_offset(Offset::new(
            self.from.get().x + (self.to.get().x - self.from.get().x) * t,
            self.from.get().y + (self.to.get().y - self.from.get().y) * t,
        ))
    }
    fn is_active(&self) -> bool {
        self.animation.borrow().is_active()
    }
}

/// The first built-in widgets. Their values contain no mutable runtime state.
#[derive(Clone)]
pub struct Widget {
    key: Option<Key>,
    kind: WidgetKind,
}
#[derive(Clone)]
enum WidgetKind {
    Box {
        size: Size,
        color: Color,
    },
    Button {
        size: Size,
        color: Color,
        action: ActionId,
        callback: Option<Rc<dyn Fn()>>,
        child: Option<Box<Widget>>,
    },
    Text {
        text: String,
        style: TextStyle,
        align: TextAlign,
    },
    Padding {
        padding: EdgeInsets,
        child: Box<Widget>,
    },
    Align {
        alignment: Alignment,
        child: Box<Widget>,
    },
    Flex {
        axis: Axis,
        children: Vec<Widget>,
    },
    Scroll {
        controller: ScrollController,
        child: Box<Widget>,
    },
    Translate {
        controller: TranslationController,
        child: Box<Widget>,
    },
}
impl std::fmt::Debug for Widget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Widget")
            .field("key", &self.key)
            .field("kind", &self.kind)
            .finish()
    }
}
impl PartialEq for Widget {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key && self.kind == other.kind
    }
}
impl std::fmt::Debug for WidgetKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Box { size, color } => f
                .debug_struct("Box")
                .field("size", size)
                .field("color", color)
                .finish(),
            Self::Button {
                size,
                color,
                action,
                ..
            } => f
                .debug_struct("Button")
                .field("size", size)
                .field("color", color)
                .field("action", action)
                .finish(),
            Self::Text { text, style, align } => f
                .debug_struct("Text")
                .field("text", text)
                .field("style", style)
                .field("align", align)
                .finish(),
            Self::Padding { padding, child } => f
                .debug_struct("Padding")
                .field("padding", padding)
                .field("child", child)
                .finish(),
            Self::Align { alignment, child } => f
                .debug_struct("Align")
                .field("alignment", alignment)
                .field("child", child)
                .finish(),
            Self::Flex { axis, children } => f
                .debug_struct("Flex")
                .field("axis", axis)
                .field("children", children)
                .finish(),
            Self::Scroll { .. } => f.debug_struct("ScrollView").finish(),
            Self::Translate { .. } => f.debug_struct("Translate").finish(),
        }
    }
}
impl PartialEq for WidgetKind {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Box { size: a, color: b }, Self::Box { size: c, color: d }) => a == c && b == d,
            (
                Self::Button {
                    size: a,
                    color: b,
                    action: c,
                    callback: d,
                    child: i,
                },
                Self::Button {
                    size: e,
                    color: f,
                    action: g,
                    callback: h,
                    child: j,
                },
            ) => {
                a == e
                    && b == f
                    && c == g
                    && i == j
                    && match (d, h) {
                        (Some(x), Some(y)) => Rc::ptr_eq(x, y),
                        (None, None) => true,
                        _ => false,
                    }
            }
            (
                Self::Text {
                    text: a,
                    style: b,
                    align: c,
                },
                Self::Text {
                    text: d,
                    style: e,
                    align: f,
                },
            ) => a == d && b == e && c == f,
            (
                Self::Padding {
                    padding: a,
                    child: b,
                },
                Self::Padding {
                    padding: c,
                    child: d,
                },
            ) => a == c && b == d,
            (
                Self::Scroll {
                    controller: a,
                    child: b,
                },
                Self::Scroll {
                    controller: c,
                    child: d,
                },
            ) => Rc::ptr_eq(&a.state, &c.state) && b == d,
            (
                Self::Translate {
                    controller: a,
                    child: b,
                },
                Self::Translate {
                    controller: c,
                    child: d,
                },
            ) => Rc::ptr_eq(&a.offset, &c.offset) && b == d,
            (
                Self::Align {
                    alignment: a,
                    child: b,
                },
                Self::Align {
                    alignment: c,
                    child: d,
                },
            ) => a == c && b == d,
            (
                Self::Flex {
                    axis: a,
                    children: b,
                },
                Self::Flex {
                    axis: c,
                    children: d,
                },
            ) => a == c && b == d,
            _ => false,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WidgetType {
    Box,
    Button,
    Text,
    Padding,
    Align,
    Flex,
    Scroll,
    Translate,
}
impl Widget {
    #[must_use]
    pub fn box_(size: Size, color: Color) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Box { size, color },
        }
    }
    #[must_use]
    pub fn fixed_box(size: Size, color: Color) -> Self {
        Self::box_(size, color)
    }
    #[must_use]
    pub fn button(size: Size, color: Color, action: ActionId) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Button {
                size,
                color,
                action,
                callback: None,
                child: None,
            },
        }
    }
    pub fn bind_callbacks(&mut self, allocate: &mut impl FnMut(Rc<dyn Fn()>) -> ActionId) {
        match &mut self.kind {
            WidgetKind::Button {
                action,
                callback,
                child,
                ..
            } => {
                if let Some(callback) = callback.take() {
                    *action = allocate(callback);
                }
                if let Some(child) = child {
                    child.bind_callbacks(allocate);
                }
            }
            WidgetKind::Padding { child, .. }
            | WidgetKind::Align { child, .. }
            | WidgetKind::Scroll { child, .. }
            | WidgetKind::Translate { child, .. } => child.bind_callbacks(allocate),
            WidgetKind::Flex { children, .. } => {
                for child in children {
                    child.bind_callbacks(allocate);
                }
            }
            WidgetKind::Box { .. } | WidgetKind::Text { .. } => {}
        }
    }
    #[must_use]
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Text {
                text: text.into(),
                style: TextStyle::default(),
                align: TextAlign::Start,
            },
        }
    }
    #[must_use]
    pub fn text_styled(text: impl Into<String>, style: TextStyle, align: TextAlign) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Text {
                text: text.into(),
                style,
                align,
            },
        }
    }
    #[must_use]
    pub fn padding(padding: EdgeInsets, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Padding {
                padding,
                child: Box::new(child),
            },
        }
    }
    #[must_use]
    pub fn align(alignment: Alignment, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Align {
                alignment,
                child: Box::new(child),
            },
        }
    }
    #[must_use]
    pub fn row(children: impl Into<Vec<Self>>) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Flex {
                axis: Axis::Horizontal,
                children: children.into(),
            },
        }
    }
    #[must_use]
    pub fn column(children: impl Into<Vec<Self>>) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Flex {
                axis: Axis::Vertical,
                children: children.into(),
            },
        }
    }
    #[must_use]
    pub fn scroll_view(controller: ScrollController, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Scroll {
                controller,
                child: Box::new(child),
            },
        }
    }
    #[must_use]
    pub fn translate(controller: TranslationController, child: Self) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Translate {
                controller,
                child: Box::new(child),
            },
        }
    }
    #[must_use]
    pub fn with_key(mut self, key: impl Into<Key>) -> Self {
        self.key = Some(key.into());
        self
    }
    #[must_use]
    pub fn key(&self) -> Option<&Key> {
        self.key.as_ref()
    }
    fn type_(&self) -> WidgetType {
        match self.kind {
            WidgetKind::Box { .. } => WidgetType::Box,
            WidgetKind::Button { .. } => WidgetType::Button,
            WidgetKind::Text { .. } => WidgetType::Text,
            WidgetKind::Padding { .. } => WidgetType::Padding,
            WidgetKind::Align { .. } => WidgetType::Align,
            WidgetKind::Flex { .. } => WidgetType::Flex,
            WidgetKind::Scroll { .. } => WidgetType::Scroll,
            WidgetKind::Translate { .. } => WidgetType::Translate,
        }
    }
    fn children(&self) -> Vec<Widget> {
        match &self.kind {
            WidgetKind::Box { .. } | WidgetKind::Text { .. } => Vec::new(),
            WidgetKind::Button { child, .. } => {
                child.iter().map(|child| child.as_ref().clone()).collect()
            }
            WidgetKind::Padding { child, .. }
            | WidgetKind::Align { child, .. }
            | WidgetKind::Scroll { child, .. }
            | WidgetKind::Translate { child, .. } => {
                vec![child.as_ref().clone()]
            }
            WidgetKind::Flex { children, .. } => children.clone(),
        }
    }
}

/// Public text description. Its font metrics are resolved during layout, not paint.
#[derive(Clone, Debug, PartialEq)]
pub struct Text {
    text: String,
    style: TextStyle,
    align: TextAlign,
}
impl Text {
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style: TextStyle::default(),
            align: TextAlign::Start,
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
}
impl From<Text> for Widget {
    fn from(value: Text) -> Self {
        Widget::text_styled(value.text, value.style, value.align)
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

/// A compositional button description. The runtime converts its callback into
/// an opaque handler ID while it is mounted; applications never allocate IDs.
pub struct Button {
    label: String,
    callback: Option<Rc<dyn Fn()>>,
    color: Color,
}
impl Button {
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            callback: None,
            color: Color::rgba(70, 120, 220, 255),
        }
    }
    #[must_use]
    pub fn on_press(mut self, callback: impl Fn() + 'static) -> Self {
        self.callback = Some(Rc::new(callback));
        self
    }
    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }
}
impl From<Button> for Widget {
    fn from(value: Button) -> Self {
        let label = Widget::padding(
            EdgeInsets::all(10.),
            Widget::text_styled(
                value.label,
                TextStyle {
                    color: Color::WHITE,
                    ..TextStyle::default()
                },
                TextAlign::Start,
            ),
        );
        Self {
            key: None,
            kind: WidgetKind::Button {
                size: Size::new(96., 40.),
                color: value.color,
                action: ActionId(0),
                callback: value.callback,
                child: Some(Box::new(label)),
            },
        }
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
}
#[derive(Debug, PartialEq, Eq)]
pub enum TreeError {
    MissingElement(ElementId),
    DuplicateKey(Key),
}

struct Element {
    parent: Option<ElementId>,
    children: Vec<ElementId>,
    widget: Widget,
    render: RenderObjectId,
    dirty: DirtyFlags,
}
#[derive(Clone, Debug, PartialEq)]
enum RenderKind {
    Box {
        desired: Size,
        color: Color,
    },
    Button {
        desired: Size,
        color: Color,
    },
    Padding {
        padding: EdgeInsets,
    },
    Align {
        alignment: Alignment,
    },
    Flex {
        axis: Axis,
    },
    Text {
        text: String,
        style: TextStyle,
        align: TextAlign,
    },
    Scroll {
        controller: ScrollController,
    },
    Translate {
        controller: TranslationController,
    },
}
struct RenderObject {
    parent: Option<RenderObjectId>,
    children: Vec<RenderObjectId>,
    kind: RenderKind,
    size: Size,
    offset: Offset,
    constraints: Option<Constraints>,
    dirty: DirtyFlags,
    cache: DisplayList,
    text_layout: Option<Arc<TextLayout>>,
    baseline: Option<f32>,
    button_state: ButtonState,
    /// Static parent-relative layout placement. This is never used to store a
    /// scroll or animation displacement.
    layer: LayerId,
    picture: Option<LayerId>,
    clip_layer: Option<LayerId>,
    content_layer: Option<LayerId>,
}

/// Persistent UI state. IDs become invalid immediately after unmount.
pub struct WidgetTree {
    elements: Arena<Element>,
    renders: Arena<RenderObject>,
    root: Option<ElementId>,
    diagnostics: Diagnostics,
    unmounted: Vec<ElementId>,
    text_engine: TextEngine,
    compositor: LayerTree,
    compositor_initialized: bool,
}
impl Default for WidgetTree {
    fn default() -> Self {
        Self::new()
    }
}
impl WidgetTree {
    #[must_use]
    pub fn new() -> Self {
        Self {
            elements: Arena::new(),
            renders: Arena::new(),
            root: None,
            diagnostics: Diagnostics::default(),
            unmounted: Vec::new(),
            text_engine: TextEngine::new(),
            compositor: LayerTree::new(),
            compositor_initialized: false,
        }
    }
    pub fn mount(&mut self, widget: Widget) -> Result<ElementId, TreeError> {
        if let Some(root) = self.root {
            self.unmount(root)?;
        }
        let root = self.mount_element(None, widget)?;
        self.root = Some(root);
        Ok(root)
    }
    #[must_use]
    pub fn root(&self) -> Option<ElementId> {
        self.root
    }
    #[must_use]
    pub fn diagnostics(&self) -> Diagnostics {
        self.diagnostics
    }
    pub fn take_unmounted(&mut self) -> Vec<ElementId> {
        std::mem::take(&mut self.unmounted)
    }
    #[must_use]
    pub fn element_exists(&self, id: ElementId) -> bool {
        self.elements.contains(id.0)
    }
    #[must_use]
    pub fn children(&self, id: ElementId) -> Option<&[ElementId]> {
        self.elements.get(id.0).map(|e| e.children.as_slice())
    }
    #[must_use]
    pub fn parent(&self, id: ElementId) -> Option<ElementId> {
        self.elements.get(id.0).and_then(|e| e.parent)
    }
    pub fn mark_build(&mut self, id: ElementId) -> Result<(), TreeError> {
        let element = self
            .elements
            .get_mut(id.0)
            .ok_or(TreeError::MissingElement(id))?;
        element.dirty.insert(DirtyFlags::BUILD);
        Ok(())
    }
    #[must_use]
    pub fn is_build_dirty(&self, id: ElementId) -> bool {
        self.elements
            .get(id.0)
            .is_some_and(|element| element.dirty.contains(DirtyFlags::BUILD))
    }
    #[must_use]
    pub fn render_id(&self, id: ElementId) -> Option<RenderObjectId> {
        self.elements.get(id.0).map(|e| e.render)
    }
    #[must_use]
    pub fn element_for_render(&self, render: RenderObjectId) -> Option<ElementId> {
        // Render IDs are opaque; a linear reverse lookup is only on input paths,
        // never layout/paint hot paths. A reverse arena index can be added when
        // profiling demonstrates it matters.
        self.elements
            .iter()
            .find_map(|(raw, element)| (element.render == render).then_some(ElementId(raw)))
    }
    #[must_use]
    pub fn action_for_element(&self, id: ElementId) -> Option<ActionId> {
        match &self.elements.get(id.0)?.widget.kind {
            WidgetKind::Button { action, .. } => Some(*action),
            _ => None,
        }
    }
    #[must_use]
    pub fn action_ids(&self) -> HashSet<ActionId> {
        self.elements
            .iter()
            .filter_map(|(_, element)| match element.widget.kind {
                WidgetKind::Button { action, .. } if action.0 != 0 => Some(action),
                _ => None,
            })
            .collect()
    }
    #[must_use]
    pub fn action_ancestor(&self, mut id: ElementId) -> Option<(ElementId, ActionId)> {
        loop {
            if let Some(action) = self.action_for_element(id) {
                return (action.0 != 0).then_some((id, action));
            }
            id = self.parent(id)?;
        }
    }
    #[must_use]
    pub fn render_size(&self, id: RenderObjectId) -> Option<Size> {
        self.renders.get(id.0).map(|r| r.size)
    }
    /// Baseline in this render object's local logical coordinate system.
    #[must_use]
    pub fn baseline(&self, id: RenderObjectId) -> Option<f32> {
        self.renders.get(id.0).and_then(|render| render.baseline)
    }
    #[must_use]
    pub fn text_diagnostics(&self) -> TextDiagnostics {
        self.text_engine.diagnostics()
    }
    #[must_use]
    pub fn compositor_debug_tree(&self) -> String {
        self.compositor.debug_tree()
    }
    #[must_use]
    pub fn compositor_diagnostics(&self) -> incular_painting::CompositorDiagnostics {
        self.compositor.diagnostics()
    }
    #[must_use]
    pub fn button_state(&self, id: ElementId) -> Option<ButtonState> {
        self.render_id(id)
            .and_then(|render| self.renders.get(render.0))
            .map(|render| render.button_state)
    }
    pub fn set_button_state(&mut self, id: ElementId, state: ButtonState) -> Result<(), TreeError> {
        let render = self.render_id(id).ok_or(TreeError::MissingElement(id))?;
        let node = self.renders.get_mut(render.0).expect("live render");
        if matches!(node.kind, RenderKind::Button { .. }) && node.button_state != state {
            node.button_state = state;
            node.dirty.insert(DirtyFlags::PAINT);
        }
        Ok(())
    }
    /// Applies only retained compositor properties. It never marks a render
    /// object for build, layout, or paint.
    pub fn update_compositor(&mut self, now: Instant) -> (bool, bool) {
        let nodes = self
            .renders
            .iter()
            .map(|(id, node)| (RenderObjectId(id), node.kind.clone(), node.content_layer))
            .collect::<Vec<_>>();
        let mut changed = false;
        let mut active = false;
        for (_, kind, content_layer) in nodes {
            match kind {
                RenderKind::Scroll { controller } => {
                    if let Some(content) = content_layer
                        && self.compositor.update_transform(
                            content,
                            Transform::translation(Offset::new(0., -controller.offset())),
                        )
                    {
                        changed = true;
                        self.diagnostics.scroll_offset_updates += 1;
                    }
                }
                RenderKind::Translate { controller } => {
                    if controller.tick(now) {
                        self.diagnostics.animation_ticks += 1;
                    }
                    active |= controller.is_active();
                    // Layout placement lives on `layer`; the inner retained
                    // transform carries only the compositor-only movement.
                    if let Some(content) = content_layer
                        && self
                            .compositor
                            .update_transform(content, Transform::translation(controller.offset()))
                    {
                        changed = true;
                    }
                }
                _ => {}
            }
        }
        if !self.compositor_initialized {
            changed = true;
            self.compositor_initialized = true;
        }
        if changed {
            self.diagnostics.composites += 1;
        }
        (changed, active)
    }
    pub fn scroll_at(&mut self, point: Offset, delta: Offset) -> bool {
        let Some(root) = self.root.and_then(|id| self.render_id(id)) else {
            return false;
        };
        let Some(scroll) = self.scroll_target(root, point, Offset::ZERO) else {
            return false;
        };
        let RenderKind::Scroll { controller } = self
            .renders
            .get(scroll.0)
            .expect("live scroll")
            .kind
            .clone()
        else {
            return false;
        };
        if controller.scroll_by(delta.y) {
            self.diagnostics.scroll_events += 1;
            true
        } else {
            false
        }
    }
    pub fn update(&mut self, id: ElementId, widget: Widget) -> Result<(), TreeError> {
        if !self.elements.contains(id.0) {
            return Err(TreeError::MissingElement(id));
        }
        self.update_existing(id, widget)
    }
    pub fn mark_paint(&mut self, id: ElementId) -> Result<(), TreeError> {
        let render = self.render_id(id).ok_or(TreeError::MissingElement(id))?;
        self.mark_render_dirty(render, DirtyFlags::PAINT, false);
        Ok(())
    }
    pub fn layout(&mut self, constraints: Constraints) {
        if let Some(root) = self.root.and_then(|id| self.render_id(id)) {
            self.layout_render(root, constraints);
        }
    }
    #[must_use]
    pub fn paint(&mut self) -> DisplayList {
        let mut ignored = DisplayList::new();
        if let Some(root) = self.root.and_then(|id| self.render_id(id)) {
            self.paint_render(root, &mut ignored);
        }
        self.compositor.flatten()
    }
    #[must_use]
    pub fn hit_test(&self, point: Offset) -> Option<RenderObjectId> {
        self.root
            .and_then(|id| self.render_id(id))
            .and_then(|id| self.hit_test_render(id, point, Offset::ZERO))
    }

    fn mount_element(
        &mut self,
        parent: Option<ElementId>,
        widget: Widget,
    ) -> Result<ElementId, TreeError> {
        self.check_keys(&widget.children())?;
        let layer = self
            .compositor
            .create_transform(Transform::translation(Offset::ZERO));
        let picture = matches!(
            widget.kind,
            WidgetKind::Box { .. } | WidgetKind::Button { .. } | WidgetKind::Text { .. }
        )
        .then(|| {
            self.compositor.create_picture(
                DisplayList::new(),
                Rect::from_origin_size(Offset::ZERO, Size::ZERO),
            )
        });
        let (clip_layer, content_layer) = match &widget.kind {
            WidgetKind::Scroll { .. } => {
                let clip = self
                    .compositor
                    .create_clip_rect(Rect::from_origin_size(Offset::ZERO, Size::ZERO));
                let content = self
                    .compositor
                    .create_transform(Transform::translation(Offset::ZERO));
                self.compositor.set_children(layer, vec![clip]);
                self.compositor.set_children(clip, vec![content]);
                (Some(clip), Some(content))
            }
            WidgetKind::Translate { .. } => {
                // Keep dynamic movement structurally below static layout
                // placement: parent -> layout transform -> animated transform
                // -> normal child tree. This is also what lets animation avoid
                // repainting local picture commands.
                let content = self
                    .compositor
                    .create_transform(Transform::translation(Offset::ZERO));
                self.compositor.set_children(layer, vec![content]);
                (None, Some(content))
            }
            _ => {
                self.compositor
                    .set_children(layer, picture.into_iter().collect());
                (None, None)
            }
        };
        let render = self.renders.insert(RenderObject {
            parent: None,
            children: Vec::new(),
            kind: render_kind(&widget),
            size: Size::ZERO,
            offset: Offset::ZERO,
            constraints: None,
            dirty: DirtyFlags::LAYOUT | DirtyFlags::PAINT,
            cache: DisplayList::new(),
            text_layout: None,
            baseline: None,
            button_state: ButtonState::Normal,
            layer,
            picture,
            clip_layer,
            content_layer,
        });
        let id = ElementId(self.elements.insert(Element {
            parent,
            children: Vec::new(),
            widget: widget.clone(),
            render: RenderObjectId(render),
            dirty: DirtyFlags::NONE,
        }));
        let mut children = Vec::with_capacity(widget.children().len());
        for child in widget.children() {
            children.push(self.mount_element(Some(id), child)?);
        }
        self.elements.get_mut(id.0).expect("fresh element").children = children;
        self.sync_render_children(id);
        if parent.is_none() {
            self.compositor.set_root(layer);
        }
        self.diagnostics.mounts += 1;
        Ok(id)
    }
    fn update_existing(&mut self, id: ElementId, widget: Widget) -> Result<(), TreeError> {
        let old = self
            .elements
            .get(id.0)
            .ok_or(TreeError::MissingElement(id))?
            .widget
            .clone();
        if old == widget {
            return Ok(());
        }
        debug_assert_eq!(
            old.type_(),
            widget.type_(),
            "only compatible elements may update"
        );
        self.check_keys(&widget.children())?;
        let render = self.elements.get(id.0).expect("present").render;
        let old_kind = render_kind(&old);
        let new_kind = render_kind(&widget);
        if old_kind != new_kind {
            self.renders.get_mut(render.0).expect("present").kind = new_kind;
            if text_paint_only_change(&old_kind, &render_kind(&widget)) {
                self.mark_render_dirty(render, DirtyFlags::PAINT, false);
            } else {
                self.mark_render_dirty(render, DirtyFlags::LAYOUT | DirtyFlags::PAINT, true);
            }
        }
        self.elements.get_mut(id.0).expect("present").widget = widget.clone();
        self.elements
            .get_mut(id.0)
            .expect("present")
            .dirty
            .remove(DirtyFlags::BUILD);
        self.diagnostics.rebuilds += 1;
        let previous = self.elements.get(id.0).expect("present").children.clone();
        let desired = widget.children();
        let reconciled = self.reconcile_children(id, previous, desired)?;
        self.elements.get_mut(id.0).expect("present").children = reconciled;
        self.sync_render_children(id);
        Ok(())
    }
    fn compatible(&self, id: ElementId, widget: &Widget) -> bool {
        self.elements
            .get(id.0)
            .is_some_and(|e| e.widget.type_() == widget.type_() && e.widget.key == widget.key)
    }
    fn reconcile_children(
        &mut self,
        parent: ElementId,
        previous: Vec<ElementId>,
        desired: Vec<Widget>,
    ) -> Result<Vec<ElementId>, TreeError> {
        self.check_keys(&desired)?;
        let mut start = 0;
        let mut old_end = previous.len();
        let mut new_end = desired.len();
        let mut next = Vec::with_capacity(desired.len());
        while start < old_end
            && start < new_end
            && self.compatible(previous[start], &desired[start])
        {
            let id = previous[start];
            self.update_existing(id, desired[start].clone())?;
            next.push(id);
            start += 1;
        }
        while start < old_end
            && start < new_end
            && self.compatible(previous[old_end - 1], &desired[new_end - 1])
        {
            old_end -= 1;
            new_end -= 1;
        }
        let old_middle = &previous[start..old_end];
        let use_keys = old_middle.iter().any(|id| {
            self.elements
                .get(id.0)
                .is_some_and(|e| e.widget.key.is_some())
        }) || desired[start..new_end].iter().any(|w| w.key.is_some());
        let mut keyed = HashMap::new();
        if use_keys {
            for &id in old_middle {
                if let Some(key) = self.elements.get(id.0).and_then(|e| e.widget.key.clone()) {
                    keyed.insert(key, id);
                }
            }
        }
        let mut used = HashSet::new();
        let unkeyed_ids: Vec<_> = old_middle
            .iter()
            .copied()
            .filter(|id| {
                self.elements
                    .get(id.0)
                    .is_some_and(|e| e.widget.key.is_none())
            })
            .collect();
        let mut unkeyed = unkeyed_ids.into_iter();
        for widget in &desired[start..new_end] {
            let candidate = if let Some(key) = widget.key() {
                keyed.get(key).copied()
            } else {
                unkeyed.next()
            };
            if let Some(id) = candidate.filter(|id| self.compatible(*id, widget)) {
                self.update_existing(id, widget.clone())?;
                used.insert(id);
                next.push(id);
            } else {
                next.push(self.mount_element(Some(parent), widget.clone())?);
            }
        }
        for id in old_middle {
            if !used.contains(id) {
                self.unmount_element(*id);
            }
        }
        let mut suffix = Vec::new();
        for index in new_end..desired.len() {
            let id = previous[old_end + (index - new_end)];
            self.update_existing(id, desired[index].clone())?;
            suffix.push(id);
        }
        next.extend(suffix);
        Ok(next)
    }
    fn unmount(&mut self, id: ElementId) -> Result<(), TreeError> {
        if !self.elements.contains(id.0) {
            return Err(TreeError::MissingElement(id));
        }
        self.unmount_element(id);
        if self.root == Some(id) {
            self.root = None;
        }
        Ok(())
    }
    fn unmount_element(&mut self, id: ElementId) {
        let Some(element) = self.elements.remove(id.0) else {
            return;
        };
        for child in element.children {
            self.unmount_element(child);
        }
        if let Some(render) = self.renders.remove(element.render.0) {
            if let Some(picture) = render.picture {
                self.compositor.remove(picture);
            }
            if let Some(layer) = render.clip_layer {
                self.compositor.remove(layer);
            }
            if let Some(layer) = render.content_layer {
                self.compositor.remove(layer);
            }
            self.compositor.remove(render.layer);
        }
        self.unmounted.push(id);
        self.diagnostics.unmounts += 1;
    }
    fn check_keys(&self, widgets: &[Widget]) -> Result<(), TreeError> {
        let mut keys = HashSet::new();
        for key in widgets.iter().filter_map(Widget::key) {
            if !keys.insert(key.clone()) {
                return Err(TreeError::DuplicateKey(key.clone()));
            }
        }
        Ok(())
    }
    fn sync_render_children(&mut self, id: ElementId) {
        let (render, children) = {
            let e = self.elements.get(id.0).expect("mounted");
            (e.render, e.children.clone())
        };
        let render_children: Vec<_> = children
            .into_iter()
            .filter_map(|child| self.render_id(child))
            .collect();
        {
            let node = self.renders.get_mut(render.0).expect("mounted");
            node.children = render_children.clone();
            node.dirty.insert(DirtyFlags::LAYOUT | DirtyFlags::PAINT);
        }
        let (layer, picture, content_layer) = {
            let node = self.renders.get(render.0).expect("mounted");
            (node.layer, node.picture, node.content_layer)
        };
        let child_layers = render_children
            .iter()
            .filter_map(|child| self.renders.get(child.0).map(|render| render.layer))
            .collect::<Vec<_>>();
        if let Some(content) = content_layer {
            self.compositor.set_children(content, child_layers);
        } else {
            let mut layers = Vec::with_capacity(child_layers.len() + 1);
            layers.extend(picture);
            layers.extend(child_layers);
            self.compositor.set_children(layer, layers);
        }
        for child in render_children {
            self.renders.get_mut(child.0).expect("mounted").parent = Some(render);
        }
        self.mark_render_dirty(render, DirtyFlags::LAYOUT | DirtyFlags::PAINT, true);
    }
    fn mark_render_dirty(&mut self, id: RenderObjectId, flags: DirtyFlags, propagate_layout: bool) {
        let mut current = Some(id);
        while let Some(render) = current {
            let node = self.renders.get_mut(render.0).expect("live render");
            node.dirty.insert(flags);
            current = if propagate_layout && flags.contains(DirtyFlags::LAYOUT) {
                node.parent
            } else {
                None
            };
        }
    }
    fn layout_render(&mut self, id: RenderObjectId, constraints: Constraints) {
        let needs = self
            .renders
            .get(id.0)
            .expect("live")
            .dirty
            .contains(DirtyFlags::LAYOUT)
            || self.renders.get(id.0).expect("live").constraints != Some(constraints);
        if !needs {
            return;
        }
        let (kind, children) = {
            let n = self.renders.get(id.0).expect("live");
            (n.kind.clone(), n.children.clone())
        };
        let (size, offsets) = match kind {
            RenderKind::Box { desired, .. } => (constraints.constrain(desired), Vec::new()),
            RenderKind::Button { desired, .. } => {
                if let Some(&child) = children.first() {
                    self.layout_render(child, constraints.loosen());
                    let child_size = self.renders.get(child.0).expect("live").size;
                    let size = constraints.constrain(Size::new(
                        desired.width.max(child_size.width),
                        desired.height.max(child_size.height),
                    ));
                    (
                        size,
                        vec![Offset::new(
                            (size.width - child_size.width) / 2.0,
                            (size.height - child_size.height) / 2.0,
                        )],
                    )
                } else {
                    (constraints.constrain(desired), Vec::new())
                }
            }
            RenderKind::Padding { padding } => {
                let child_constraints =
                    constraints.deflate(padding.horizontal(), padding.vertical());
                if let Some(&child) = children.first() {
                    self.layout_render(child, child_constraints);
                    let s = self.renders.get(child.0).expect("live").size;
                    (
                        constraints.constrain(Size::new(
                            s.width + padding.horizontal(),
                            s.height + padding.vertical(),
                        )),
                        vec![Offset::new(padding.left, padding.top)],
                    )
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            RenderKind::Align { alignment } => {
                if let Some(&child) = children.first() {
                    self.layout_render(child, constraints.loosen());
                    let child_size = self.renders.get(child.0).expect("live").size;
                    let size = constraints.constrain(child_size);
                    let x = (size.width - child_size.width) * (alignment.x + 1.0) / 2.0;
                    let y = (size.height - child_size.height) * (alignment.y + 1.0) / 2.0;
                    (size, vec![Offset::new(x, y)])
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            RenderKind::Flex { axis } => {
                let child_constraints = match axis {
                    Axis::Horizontal => {
                        Constraints::new(0.0, f32::INFINITY, 0.0, constraints.max_height)
                    }
                    Axis::Vertical => {
                        Constraints::new(0.0, constraints.max_width, 0.0, f32::INFINITY)
                    }
                };
                let mut main = 0.0;
                let mut cross: f32 = 0.0;
                let mut offsets = Vec::with_capacity(children.len());
                for child in &children {
                    self.layout_render(*child, child_constraints);
                    let s = self.renders.get(child.0).expect("live").size;
                    offsets.push(match axis {
                        Axis::Horizontal => Offset::new(main, 0.0),
                        Axis::Vertical => Offset::new(0.0, main),
                    });
                    main += match axis {
                        Axis::Horizontal => s.width,
                        Axis::Vertical => s.height,
                    };
                    cross = cross.max(match axis {
                        Axis::Horizontal => s.height,
                        Axis::Vertical => s.width,
                    });
                }
                let natural = match axis {
                    Axis::Horizontal => Size::new(main, cross),
                    Axis::Vertical => Size::new(cross, main),
                };
                (constraints.constrain(natural), offsets)
            }
            RenderKind::Text { text, style, align } => {
                let width = constraints
                    .is_width_bounded()
                    .then_some(constraints.max_width);
                let layout = self.text_engine.layout(&text, &style, width, align);
                let size = constraints.constrain(layout.metrics.size);
                let node = self.renders.get_mut(id.0).expect("live");
                node.text_layout = Some(layout.clone());
                node.baseline = Some(layout.metrics.baseline);
                (size, Vec::new())
            }
            RenderKind::Scroll { controller } => {
                if let Some(&child) = children.first() {
                    self.layout_render(
                        child,
                        Constraints::new(0.0, constraints.max_width, 0.0, f32::INFINITY),
                    );
                    let content = self.renders.get(child.0).expect("live").size;
                    let size = constraints.constrain(Size::new(
                        if constraints.is_width_bounded() {
                            constraints.max_width
                        } else {
                            content.width
                        },
                        if constraints.is_height_bounded() {
                            constraints.max_height
                        } else {
                            content.height
                        },
                    ));
                    controller.set_extents(content.height, size.height);
                    (size, vec![Offset::ZERO])
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
            RenderKind::Translate { .. } => {
                if let Some(&child) = children.first() {
                    self.layout_render(child, constraints.loosen());
                    let size = constraints.constrain(self.renders.get(child.0).expect("live").size);
                    (size, vec![Offset::ZERO])
                } else {
                    (constraints.constrain(Size::ZERO), Vec::new())
                }
            }
        };
        for (child, offset) in children.into_iter().zip(offsets) {
            // The parent computes this placement after the child has completed
            // its own layout. Update the retained placement layer here rather
            // than waiting for a later child layout pass (which may not occur).
            let child = self.renders.get_mut(child.0).expect("live");
            child.offset = offset;
            self.compositor
                .update_transform(child.layer, Transform::translation(offset));
        }
        let node = self.renders.get_mut(id.0).expect("live");
        node.size = size;
        node.constraints = Some(constraints);
        node.dirty.remove(DirtyFlags::LAYOUT);
        node.dirty.insert(DirtyFlags::PAINT);
        self.compositor
            .update_transform(node.layer, Transform::translation(node.offset));
        if let Some(clip) = node.clip_layer {
            self.compositor
                .update_clip(clip, Rect::from_origin_size(Offset::ZERO, node.size));
        }
        self.diagnostics.layouts += 1;
    }
    fn paint_render(&mut self, id: RenderObjectId, output: &mut DisplayList) {
        let (offset, cache, children) = {
            let node = self.renders.get(id.0).expect("live");
            (node.offset, node.cache.clone(), node.children.clone())
        };
        let dirty = self
            .renders
            .get(id.0)
            .expect("live")
            .dirty
            .contains(DirtyFlags::PAINT);
        if dirty {
            let kind = self.renders.get(id.0).expect("live").kind.clone();
            let size = self.renders.get(id.0).expect("live").size;
            let mut cache = DisplayList::new();
            match kind {
                RenderKind::Box { color, .. } => cache.push(PaintCommand::Rect {
                    rect: Rect::from_origin_size(Offset::ZERO, size),
                    color,
                }),
                RenderKind::Button { color, .. } => {
                    let state = self.renders.get(id.0).expect("live").button_state;
                    let adjust = match state {
                        ButtonState::Normal => 0,
                        ButtonState::Hovered => 18,
                        ButtonState::Pressed => -24,
                    };
                    let shift = |value: u8| (value as i16 + adjust).clamp(0, 255) as u8;
                    cache.push(PaintCommand::Rect {
                        rect: Rect::from_origin_size(Offset::ZERO, size),
                        color: Color::rgba(
                            shift(color.red),
                            shift(color.green),
                            shift(color.blue),
                            color.alpha,
                        ),
                    });
                }
                RenderKind::Text { style, .. } => {
                    if let Some(layout) = self.renders.get(id.0).expect("live").text_layout.clone()
                    {
                        for line in layout.lines.iter() {
                            cache.push(PaintCommand::GlyphRun {
                                run: line.run.clone(),
                                color: style.color,
                            });
                        }
                    }
                }
                _ => {}
            }
            let node = self.renders.get_mut(id.0).expect("live");
            node.cache = cache;
            node.dirty.remove(DirtyFlags::PAINT);
            if let Some(picture) = node.picture {
                self.compositor.update_picture(
                    picture,
                    node.cache.clone(),
                    Rect::from_origin_size(Offset::ZERO, node.size),
                );
            }
            self.diagnostics.paints += 1;
        }
        output.push(PaintCommand::PushTransform {
            transform: Transform::translation(offset),
        });
        output.extend_from(if dirty {
            &self.renders.get(id.0).expect("live").cache
        } else {
            &cache
        });
        for child in children {
            self.paint_render(child, output);
        }
        output.push(PaintCommand::PopTransform);
    }
    fn hit_test_render(
        &self,
        id: RenderObjectId,
        point: Offset,
        origin: Offset,
    ) -> Option<RenderObjectId> {
        let node = self.renders.get(id.0)?;
        let current = match &node.kind {
            RenderKind::Translate { controller } => origin + node.offset + controller.offset(),
            _ => origin + node.offset,
        };
        if !Rect::from_origin_size(current, node.size).contains(point) {
            return None;
        }
        let child_origin = match &node.kind {
            RenderKind::Scroll { controller } => current - Offset::new(0., controller.offset()),
            RenderKind::Translate { .. } => current,
            _ => current,
        };
        for child in node.children.iter().rev() {
            if let Some(hit) = self.hit_test_render(*child, point, child_origin) {
                return Some(hit);
            }
        }
        Some(id)
    }
    fn scroll_target(
        &self,
        id: RenderObjectId,
        point: Offset,
        origin: Offset,
    ) -> Option<RenderObjectId> {
        let node = self.renders.get(id.0)?;
        let current = match &node.kind {
            RenderKind::Translate { controller } => origin + node.offset + controller.offset(),
            _ => origin + node.offset,
        };
        if !Rect::from_origin_size(current, node.size).contains(point) {
            return None;
        }
        let child_origin = match &node.kind {
            RenderKind::Scroll { controller } => current - Offset::new(0., controller.offset()),
            RenderKind::Translate { .. } => current,
            _ => current,
        };
        for child in node.children.iter().rev() {
            if let Some(found) = self.scroll_target(*child, point, child_origin) {
                return Some(found);
            }
        }
        matches!(node.kind, RenderKind::Scroll { .. }).then_some(id)
    }
}
fn render_kind(widget: &Widget) -> RenderKind {
    match &widget.kind {
        WidgetKind::Box { size, color } => RenderKind::Box {
            desired: *size,
            color: *color,
        },
        WidgetKind::Button { size, color, .. } => RenderKind::Button {
            desired: *size,
            color: *color,
        },
        WidgetKind::Text { text, style, align } => RenderKind::Text {
            text: text.clone(),
            style: style.clone(),
            align: *align,
        },
        WidgetKind::Padding { padding, .. } => RenderKind::Padding { padding: *padding },
        WidgetKind::Align { alignment, .. } => RenderKind::Align {
            alignment: *alignment,
        },
        WidgetKind::Flex { axis, .. } => RenderKind::Flex { axis: *axis },
        WidgetKind::Scroll { controller, .. } => RenderKind::Scroll {
            controller: controller.clone(),
        },
        WidgetKind::Translate { controller, .. } => RenderKind::Translate {
            controller: controller.clone(),
        },
    }
}
fn text_paint_only_change(old: &RenderKind, new: &RenderKind) -> bool {
    let (
        RenderKind::Text {
            text: old_text,
            style: old_style,
            align: old_align,
        },
        RenderKind::Text {
            text: new_text,
            style: new_style,
            align: new_align,
        },
    ) = (old, new)
    else {
        return false;
    };
    old_text == new_text
        && old_align == new_align
        && old_style.family == new_style.family
        && old_style.size == new_style.size
        && old_style.weight == new_style.weight
        && old_style.style == new_style.style
        && old_style.line_height == new_style.line_height
        && old_style.letter_spacing == new_style.letter_spacing
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect_origins(list: &DisplayList) -> Vec<Offset> {
        let mut transforms = vec![Offset::ZERO];
        let mut origins = Vec::new();
        for command in list.commands() {
            match command {
                PaintCommand::PushTransform { transform } => {
                    transforms.push(*transforms.last().unwrap() + transform.translation);
                }
                PaintCommand::PopTransform => {
                    transforms.pop();
                }
                PaintCommand::Rect { rect, .. } => {
                    origins.push(rect.origin + *transforms.last().unwrap());
                }
                PaintCommand::GlyphRun { .. }
                | PaintCommand::PushClip { .. }
                | PaintCommand::PopClip => {}
            }
        }
        origins
    }
    fn glyph_origins(list: &DisplayList) -> Vec<Offset> {
        let mut transforms = vec![Offset::ZERO];
        let mut origins = Vec::new();
        for command in list.commands() {
            match command {
                PaintCommand::PushTransform { transform } => {
                    transforms.push(*transforms.last().unwrap() + transform.translation);
                }
                PaintCommand::PopTransform => {
                    transforms.pop();
                }
                PaintCommand::GlyphRun { run, .. } => {
                    origins.push(run.origin + *transforms.last().unwrap());
                }
                PaintCommand::Rect { .. }
                | PaintCommand::PushClip { .. }
                | PaintCommand::PopClip => {}
            }
        }
        origins
    }
    fn box_(key: u64) -> Widget {
        Widget::box_(Size::new(10., 10.), Color::WHITE).with_key(key)
    }
    #[test]
    fn keyed_reorder_reuses_elements() {
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(Widget::row(vec![box_(1), box_(2), box_(3)]))
            .unwrap();
        let before = tree.children(root).unwrap().to_vec();
        tree.update(root, Widget::row(vec![box_(3), box_(1), box_(2)]))
            .unwrap();
        let after = tree.children(root).unwrap();
        assert_eq!(after, &[before[2], before[0], before[1]]);
    }
    #[test]
    fn removed_ids_are_stale_and_unmounted_once() {
        let mut tree = WidgetTree::new();
        let root = tree.mount(Widget::row(vec![box_(1), box_(2)])).unwrap();
        let removed = tree.children(root).unwrap()[1];
        tree.update(root, Widget::row(vec![box_(1)])).unwrap();
        assert!(!tree.element_exists(removed));
        assert_eq!(tree.diagnostics().unmounts, 1);
    }
    #[test]
    fn nested_layout_and_paint_cache_are_incremental() {
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(Widget::padding(
                EdgeInsets::all(2.),
                Widget::box_(Size::new(10., 5.), Color::WHITE),
            ))
            .unwrap();
        tree.layout(Constraints::tight(Size::new(20., 20.)));
        assert_eq!(
            tree.render_size(tree.render_id(root).unwrap()),
            Some(Size::new(20., 20.))
        );
        let first = tree.paint();
        let paints = tree.diagnostics().paints;
        let _ = tree.paint();
        assert_eq!(tree.diagnostics().paints, paints);
        assert!(!first.is_empty());
    }
    #[test]
    fn duplicate_local_keys_are_rejected() {
        let mut tree = WidgetTree::new();
        assert_eq!(
            tree.mount(Widget::row(vec![box_(1), box_(1)])).unwrap_err(),
            TreeError::DuplicateKey(Key::Value(1))
        );
    }
    #[test]
    fn hit_test_uses_reverse_paint_order_and_nested_offsets() {
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(Widget::padding(
                EdgeInsets::all(2.),
                Widget::row(vec![
                    Widget::box_(Size::new(10., 10.), Color::WHITE),
                    Widget::box_(Size::new(10., 10.), Color::BLACK),
                ]),
            ))
            .unwrap();
        tree.layout(Constraints::tight(Size::new(30., 20.)));
        let hit = tree.hit_test(Offset::new(13., 5.)).unwrap();
        let row = tree.children(root).unwrap()[0];
        let second = tree.children(row).unwrap()[1];
        assert_eq!(tree.element_for_render(hit), Some(second));
    }
    #[test]
    fn retained_pictures_apply_column_row_and_padding_offsets_once() {
        let mut tree = WidgetTree::new();
        tree.mount(Widget::padding(
            EdgeInsets {
                left: 20.,
                top: 10.,
                right: 0.,
                bottom: 0.,
            },
            Widget::column(vec![
                Widget::box_(Size::new(100., 20.), Color::WHITE),
                Widget::row(vec![
                    Widget::box_(Size::new(30., 30.), Color::WHITE),
                    Widget::box_(Size::new(40., 30.), Color::WHITE),
                ]),
                Widget::box_(Size::new(100., 40.), Color::WHITE),
            ]),
        ))
        .unwrap();
        tree.layout(Constraints::tight(Size::new(200., 200.)));
        assert_eq!(
            rect_origins(&tree.paint()),
            vec![
                Offset::new(20., 10.),
                Offset::new(20., 30.),
                Offset::new(50., 30.),
                Offset::new(20., 60.),
            ]
        );
    }
    #[test]
    fn retained_text_pictures_keep_independent_column_origins() {
        let style = TextStyle {
            size: 20.,
            line_height: Some(30.),
            ..TextStyle::default()
        };
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(Widget::column(vec![
                Widget::text_styled("A", style.clone(), TextAlign::Start),
                Widget::text_styled("B", style.clone(), TextAlign::Start),
                Widget::text_styled("C", style, TextAlign::Start),
            ]))
            .unwrap();
        tree.layout(Constraints::tight(Size::new(200., 200.)));
        let origins = glyph_origins(&tree.paint());
        assert_eq!(origins.len(), 3);
        let children = tree.children(root).unwrap();
        let a_height = tree
            .render_size(tree.render_id(children[0]).unwrap())
            .unwrap()
            .height;
        let b_height = tree
            .render_size(tree.render_id(children[1]).unwrap())
            .unwrap()
            .height;
        assert_eq!(origins[1].y - origins[0].y, a_height);
        assert_eq!(origins[2].y - origins[1].y, b_height);
        assert!(origins[0].y < origins[1].y && origins[1].y < origins[2].y);
    }
    #[test]
    fn button_label_receives_the_button_parent_placement() {
        let mut tree = WidgetTree::new();
        tree.mount(Widget::column(vec![
            Widget::box_(Size::new(100., 30.), Color::BLACK),
            Button::new("Placed label").into(),
        ]))
        .unwrap();
        tree.layout(Constraints::tight(Size::new(200., 120.)));
        let list = tree.paint();
        let button_origin = rect_origins(&list)[1];
        let label_origin = glyph_origins(&list)[0];
        assert_eq!(button_origin.y, 30.);
        assert!(label_origin.y >= button_origin.y);
    }
    #[test]
    fn scroll_and_animation_compose_with_static_layout_placement() {
        let scroll = ScrollController::new();
        let translation = TranslationController::new();
        translation.set_offset(Offset::new(15., 0.));
        let mut tree = WidgetTree::new();
        tree.mount(Widget::padding(
            EdgeInsets {
                left: 0.,
                top: 10.,
                right: 0.,
                bottom: 0.,
            },
            Widget::scroll_view(
                scroll.clone(),
                Widget::translate(
                    translation.clone(),
                    Widget::column(vec![
                        Widget::box_(Size::new(40., 20.), Color::WHITE),
                        Widget::box_(Size::new(40., 30.), Color::WHITE),
                    ]),
                ),
            ),
        ))
        .unwrap();
        let constraints = Constraints::tight(Size::new(100., 40.));
        tree.layout(constraints);
        let _ = tree.update_compositor(Instant::now());
        assert_eq!(
            rect_origins(&tree.paint()),
            vec![Offset::new(15., 10.), Offset::new(15., 30.)]
        );
        assert!(scroll.jump_to(10.));
        let _ = tree.update_compositor(Instant::now());
        assert_eq!(
            rect_origins(&tree.paint()),
            vec![Offset::new(15., 0.), Offset::new(15., 20.)]
        );
    }
    #[test]
    fn text_picture_replacement_and_root_unmount_do_not_leak_layers() {
        let mut tree = WidgetTree::new();
        let root = tree.mount(Widget::text("Count: 0")).unwrap();
        tree.layout(Constraints::tight(Size::new(100., 40.)));
        let _ = tree.paint();
        let before = tree.compositor_diagnostics().layers;
        tree.update(root, Widget::text("Count: 1")).unwrap();
        tree.layout(Constraints::tight(Size::new(100., 40.)));
        let _ = tree.paint();
        assert_eq!(tree.compositor_diagnostics().layers, before);
        tree.update(root, Widget::text("Count: 2")).unwrap();
        tree.layout(Constraints::tight(Size::new(100., 40.)));
        let _ = tree.paint();
        assert_eq!(tree.compositor_diagnostics().layers, before);
        tree.mount(Widget::box_(Size::new(1., 1.), Color::WHITE))
            .unwrap();
        assert_eq!(tree.compositor_diagnostics().layers, 2);
    }
    #[test]
    fn scroll_positions_are_logical_clamped_and_clip_the_viewport() {
        let controller = ScrollController::new();
        let mut tree = WidgetTree::new();
        tree.mount(Widget::scroll_view(
            controller.clone(),
            Widget::column(
                (0..5)
                    .map(|_| Widget::box_(Size::new(80., 40.), Color::WHITE))
                    .collect::<Vec<_>>(),
            ),
        ))
        .unwrap();
        tree.layout(Constraints::tight(Size::new(100., 100.)));
        let _ = tree.update_compositor(Instant::now());
        assert_eq!(controller.max_offset(), 100.);
        assert_eq!(
            rect_origins(&tree.paint()),
            vec![
                Offset::new(0., 0.),
                Offset::new(0., 40.),
                Offset::new(0., 80.),
            ]
        );
        assert!(controller.jump_to(50.));
        let _ = tree.update_compositor(Instant::now());
        assert_eq!(
            rect_origins(&tree.paint()),
            vec![
                Offset::new(0., -10.),
                Offset::new(0., 30.),
                Offset::new(0., 70.),
            ]
        );
        assert!(controller.jump_to(10_000.));
        assert_eq!(controller.offset(), 100.);
        let _ = tree.update_compositor(Instant::now());
        assert_eq!(
            rect_origins(&tree.paint()),
            vec![
                Offset::new(0., -20.),
                Offset::new(0., 20.),
                Offset::new(0., 60.),
            ]
        );
        assert!(controller.jump_to(-1.));
        assert_eq!(controller.offset(), 0.);
    }
}
