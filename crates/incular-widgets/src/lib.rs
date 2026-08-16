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

use incular_accessibility::{
    Role as SemanticRole, SemanticActionKind, SemanticNode, SemanticNodeId, SemanticState,
    SemanticsDiagnostics, SemanticsTree, TextSelection as SemanticTextSelection,
};
use incular_animation::AnimationController;
use incular_assets::ImageHandle;
use incular_core::{Arena, ArenaId, Color, DirtyFlags, Offset, Rect, Size, Transform};
use incular_layout::{Alignment, Axis, Constraints, EdgeInsets};
use incular_painting::{
    Border, Brush, CornerRadii, DisplayList, FillRule, ImageSampling, LayerId, LayerTree,
    PaintCommand, Path, RRect, Stroke,
};
use incular_text::{TextAlign, TextDiagnostics, TextEngine, TextLayout, TextStyle};
use std::sync::Arc;
use unicode_segmentation::UnicodeSegmentation;

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
    Focused,
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
    content_extent: f32,
    viewport_extent: f32,
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
    #[must_use]
    pub fn content_extent(&self) -> f32 {
        self.state.borrow().content_extent
    }
    #[must_use]
    pub fn viewport_extent(&self) -> f32 {
        self.state.borrow().viewport_extent
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
        s.content_extent = content.max(0.);
        s.viewport_extent = viewport.max(0.);
        s.max_offset = (s.content_extent - s.viewport_extent).max(0.);
        let next = s.offset.min(s.max_offset);
        if next != s.offset {
            s.offset = next;
            s.revision += 1;
        }
    }
}

/// Visual configuration for the framework-rendered vertical overlay bar.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollbarStyle {
    pub width: f32,
    pub min_thumb_extent: f32,
    pub track_color: Color,
    pub thumb_color: Color,
}
impl Default for ScrollbarStyle {
    fn default() -> Self {
        Self {
            width: 10.,
            min_thumb_extent: 24.,
            track_color: Color::rgba(20, 22, 30, 120),
            thumb_color: Color::rgba(170, 180, 205, 190),
        }
    }
}
/// Logical overlay-scrollbar geometry, exposed for deterministic diagnostics
/// and tests without exposing a RenderObject.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScrollbarGeometry {
    pub visible: bool,
    pub track: Rect,
    pub thumb: Rect,
    pub max_scroll_extent: f32,
    pub thumb_travel: f32,
}
impl ScrollbarGeometry {
    /// Converts a controller offset into the top of the rendered thumb.
    #[must_use]
    pub fn thumb_top_for_offset(self, offset: f32) -> f32 {
        let normalized = if self.max_scroll_extent > 0. && offset.is_finite() {
            (offset / self.max_scroll_extent).clamp(0., 1.)
        } else {
            0.
        };
        self.track.origin.y + normalized * self.thumb_travel
    }
    /// Converts a global thumb top back into the corresponding controller offset.
    #[must_use]
    pub fn offset_for_thumb_top(self, thumb_top: f32) -> f32 {
        if self.thumb_travel <= 0. || self.max_scroll_extent <= 0. || !thumb_top.is_finite() {
            return 0.;
        }
        ((thumb_top - self.track.origin.y).clamp(0., self.thumb_travel) / self.thumb_travel)
            * self.max_scroll_extent
    }
}
fn scrollbar_geometry(
    size: Size,
    controller: &ScrollController,
    style: ScrollbarStyle,
) -> ScrollbarGeometry {
    let viewport = controller.viewport_extent();
    let content = controller.content_extent();
    if !viewport.is_finite()
        || !content.is_finite()
        || content <= viewport
        || viewport <= 0.
        || !size.height.is_finite()
        || size.height <= 0.
    {
        return ScrollbarGeometry::default();
    }
    let width = style.width.min(size.width).max(0.);
    let track = Rect::from_origin_size(
        Offset::new(size.width - width, 0.),
        Size::new(width, size.height),
    );
    let extent = (size.height * (viewport / content))
        .clamp(style.min_thumb_extent.min(size.height), size.height);
    let mut geometry = ScrollbarGeometry {
        visible: true,
        track,
        thumb: Rect::from_origin_size(Offset::new(track.origin.x, 0.), Size::new(width, extent)),
        max_scroll_extent: controller.max_offset().max(0.),
        thumb_travel: (size.height - extent).max(0.),
    };
    geometry.thumb.origin.y = geometry.thumb_top_for_offset(controller.offset());
    geometry
}

/// A valid UTF-8 byte range. Text editing stores byte offsets because they map
/// directly to Rust `String` slicing; all constructors clamp to boundaries.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextRange {
    pub start: usize,
    pub end: usize,
}
impl TextRange {
    #[must_use]
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }
}
/// Base/extent form preserves Shift-selection direction while `range()`
/// produces the ordered replacement/deletion range.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextSelection {
    pub base: usize,
    pub extent: usize,
}
impl TextSelection {
    #[must_use]
    pub const fn collapsed(offset: usize) -> Self {
        Self {
            base: offset,
            extent: offset,
        }
    }
    #[must_use]
    pub fn range(self) -> TextRange {
        TextRange::new(self.base.min(self.extent), self.base.max(self.extent))
    }
    #[must_use]
    pub fn is_collapsed(self) -> bool {
        self.base == self.extent
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TextEditingValue {
    pub text: String,
    pub selection: TextSelection,
    /// Visual-only active IME preedit, not committed buffer content.
    pub preedit: Option<String>,
    pub preedit_selection: Option<TextRange>,
}
#[derive(Clone)]
pub struct TextEditingController {
    state: Rc<RefCell<TextEditingState>>,
}
#[derive(Clone, Debug, Default)]
struct TextEditingState {
    value: TextEditingValue,
    content_revision: u64,
    visual_revision: u64,
    caret_reset: Option<Instant>,
    preferred_caret_x: Option<f32>,
}
impl Default for TextEditingController {
    fn default() -> Self {
        Self::new()
    }
}
impl PartialEq for TextEditingController {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.state, &other.state)
    }
}
impl std::fmt::Debug for TextEditingController {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextEditingController")
            .field("value", &self.value())
            .finish()
    }
}
impl TextEditingController {
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Rc::new(RefCell::new(TextEditingState::default())),
        }
    }
    #[must_use]
    pub fn with_text(text: impl Into<String>) -> Self {
        let controller = Self::new();
        controller.set_text(text);
        controller
    }
    #[must_use]
    pub fn value(&self) -> TextEditingValue {
        self.state.borrow().value.clone()
    }
    #[must_use]
    pub fn text(&self) -> String {
        self.state.borrow().value.text.clone()
    }
    pub fn set_text(&self, text: impl Into<String>) {
        let text = text.into();
        self.replace_all(TextEditingValue {
            selection: TextSelection::collapsed(text.len()),
            text,
            ..TextEditingValue::default()
        });
    }
    pub fn set_selection(&self, selection: TextSelection) {
        let mut state = self.state.borrow_mut();
        let selection = valid_selection(&state.value.text, selection);
        if state.value.selection != selection {
            state.value.selection = selection;
            state.visual_revision += 1;
            state.preferred_caret_x = None;
        }
    }
    pub fn insert(&self, text: &str) {
        self.replace_selection(text);
    }
    pub fn replace_selection(&self, replacement: &str) {
        let mut state = self.state.borrow_mut();
        if state.value.selection.is_collapsed() && replacement.is_empty() {
            return;
        }
        replace_selected(&mut state.value, replacement);
        state.content_revision += 1;
        state.visual_revision += 1;
        state.preferred_caret_x = None;
    }
    pub fn backspace(&self) {
        let mut state = self.state.borrow_mut();
        if !state.value.selection.is_collapsed() {
            replace_selected(&mut state.value, "");
        } else {
            let end = state.value.selection.extent;
            let start = previous_grapheme_boundary(&state.value.text, end);
            if start == end {
                return;
            }
            state.value.selection = TextSelection::new(start, end);
            replace_selected(&mut state.value, "");
        }
        state.content_revision += 1;
        state.visual_revision += 1;
        state.preferred_caret_x = None;
    }
    pub fn delete(&self) {
        let mut state = self.state.borrow_mut();
        if !state.value.selection.is_collapsed() {
            replace_selected(&mut state.value, "");
        } else {
            let start = state.value.selection.extent;
            let end = next_grapheme_boundary(&state.value.text, start);
            if start == end {
                return;
            }
            state.value.selection = TextSelection::new(start, end);
            replace_selected(&mut state.value, "");
        }
        state.content_revision += 1;
        state.visual_revision += 1;
        state.preferred_caret_x = None;
    }
    pub fn move_left(&self, extend: bool) {
        self.move_to(false, extend);
    }
    pub fn move_right(&self, extend: bool) {
        self.move_to(true, extend);
    }
    pub fn move_home(&self, extend: bool) {
        self.move_cursor(0, extend);
    }
    pub fn move_end(&self, extend: bool) {
        self.move_cursor(self.text().len(), extend);
    }
    pub fn select_all(&self) {
        self.set_selection(TextSelection {
            base: 0,
            extent: self.text().len(),
        });
    }
    #[must_use]
    pub fn selected_text(&self) -> String {
        let state = self.state.borrow();
        let range = state.value.selection.range();
        state.value.text[range.start..range.end].to_owned()
    }
    pub fn set_preedit(&self, text: impl Into<String>, selection: Option<TextRange>) {
        let mut state = self.state.borrow_mut();
        let text = text.into();
        state.value.preedit_selection = selection.map(|range| valid_range(&text, range));
        state.value.preedit = (!text.is_empty()).then_some(text);
        state.content_revision += 1;
        state.visual_revision += 1;
    }
    pub fn commit_preedit(&self, text: &str) {
        self.replace_selection(text);
        self.clear_preedit();
    }
    pub fn clear_preedit(&self) {
        let mut state = self.state.borrow_mut();
        if state.value.preedit.take().is_some() {
            state.value.preedit_selection = None;
            state.content_revision += 1;
            state.visual_revision += 1;
        }
    }
    fn revisions(&self) -> (u64, u64) {
        let state = self.state.borrow();
        (state.content_revision, state.visual_revision)
    }
    /// Restarts the focused caret blink after an editing interaction.
    pub fn reset_caret(&self, now: Instant) {
        let mut state = self.state.borrow_mut();
        state.caret_reset = Some(now);
        state.visual_revision += 1;
    }
    fn caret_visible(&self, now: Instant) -> bool {
        self.state.borrow().caret_reset.is_some_and(|start| {
            (now.checked_duration_since(start)
                .unwrap_or_default()
                .as_millis()
                / 500)
                .is_multiple_of(2)
        })
    }
    fn move_to(&self, right: bool, extend: bool) {
        let state = self.state.borrow();
        let pos = state.value.selection.extent;
        let target = if right {
            next_grapheme_boundary(&state.value.text, pos)
        } else {
            previous_grapheme_boundary(&state.value.text, pos)
        };
        drop(state);
        self.move_cursor(target, extend);
    }
    fn move_cursor(&self, target: usize, extend: bool) {
        let mut state = self.state.borrow_mut();
        let target = valid_boundary(&state.value.text, target);
        state.value.selection = if extend {
            TextSelection {
                base: state.value.selection.base,
                extent: target,
            }
        } else {
            TextSelection::collapsed(target)
        };
        state.visual_revision += 1;
        state.preferred_caret_x = None;
    }
    fn move_cursor_with_x(&self, target: usize, extend: bool, x: f32) {
        let mut state = self.state.borrow_mut();
        let target = valid_boundary(&state.value.text, target);
        state.value.selection = if extend {
            TextSelection {
                base: state.value.selection.base,
                extent: target,
            }
        } else {
            TextSelection::collapsed(target)
        };
        state.preferred_caret_x = Some(x);
        state.visual_revision += 1;
    }
    fn preferred_caret_x(&self) -> Option<f32> {
        self.state.borrow().preferred_caret_x
    }
    fn replace_all(&self, mut value: TextEditingValue) {
        value.selection = valid_selection(&value.text, value.selection);
        let mut state = self.state.borrow_mut();
        state.value = value;
        state.content_revision += 1;
        state.visual_revision += 1;
    }
}
impl TextSelection {
    const fn new(base: usize, extent: usize) -> Self {
        Self { base, extent }
    }
}
fn valid_boundary(text: &str, offset: usize) -> usize {
    let offset = offset.min(text.len());
    text.char_indices()
        .map(|(index, _)| index)
        .chain(std::iter::once(text.len()))
        .take_while(|index| *index <= offset)
        .last()
        .unwrap_or(0)
}
fn valid_range(text: &str, range: TextRange) -> TextRange {
    TextRange::new(
        valid_boundary(text, range.start),
        valid_boundary(text, range.end),
    )
}
fn valid_selection(text: &str, selection: TextSelection) -> TextSelection {
    TextSelection::new(
        valid_boundary(text, selection.base),
        valid_boundary(text, selection.extent),
    )
}
fn previous_grapheme_boundary(text: &str, offset: usize) -> usize {
    UnicodeSegmentation::grapheme_indices(text, true)
        .map(|(index, _)| index)
        .take_while(|index| *index < offset)
        .last()
        .unwrap_or(0)
}
fn next_grapheme_boundary(text: &str, offset: usize) -> usize {
    UnicodeSegmentation::grapheme_indices(text, true)
        .map(|(index, grapheme)| index + grapheme.len())
        .find(|end| *end > offset)
        .unwrap_or(text.len())
}
fn replace_selected(value: &mut TextEditingValue, replacement: &str) {
    let range = value.selection.range();
    value
        .text
        .replace_range(range.start..range.end, replacement);
    let caret = range.start + replacement.len();
    value.selection = TextSelection::collapsed(caret);
    value.preedit = None;
    value.preedit_selection = None;
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
    semantics: SemanticProperties,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct SemanticProperties {
    label: Option<String>,
    description: Option<String>,
    hidden: bool,
}
#[derive(Clone)]
enum WidgetKind {
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
    Decorated {
        size: Option<Size>,
        background: Option<Brush>,
        border: Option<Border>,
        radius: CornerRadii,
        child: Box<Widget>,
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
    Image {
        image: ImageHandle,
        width: Option<f32>,
        height: Option<f32>,
        fit: ImageFit,
        alignment: Alignment,
        sampling: ImageSampling,
    },
    TextField {
        controller: TextEditingController,
        size: Size,
        style: TextStyle,
        placeholder: String,
        on_submit: Option<Rc<dyn Fn(String)>>,
        multiline: bool,
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
    VirtualList {
        config: Rc<VirtualListConfig>,
    },
    Translate {
        controller: TranslationController,
        child: Box<Widget>,
    },
}

/// Fixed-extent lazy viewport configuration. The item builder is invoked only
/// as an index enters the bounded materialized range.
#[derive(Clone)]
struct VirtualListConfig {
    item_count: usize,
    item_extent: f32,
    cache_extent: f32,
    controller: ScrollController,
    builder: Rc<dyn Fn(usize) -> Widget>,
}
impl std::fmt::Debug for VirtualListConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VirtualListConfig")
            .field("item_count", &self.item_count)
            .field("item_extent", &self.item_extent)
            .field("cache_extent", &self.cache_extent)
            .finish()
    }
}
impl PartialEq for VirtualListConfig {
    fn eq(&self, other: &Self) -> bool {
        self.item_count == other.item_count
            && self.item_extent == other.item_extent
            && self.cache_extent == other.cache_extent
            && self.controller == other.controller
            && Rc::ptr_eq(&self.builder, &other.builder)
    }
}

/// Computes an exclusive materialized item range. A row touching the cache
/// boundary is not included; intersecting rows are included.
#[must_use]
pub fn fixed_extent_materialized_range(
    item_count: usize,
    item_extent: f32,
    scroll_offset: f32,
    viewport_extent: f32,
    cache_extent: f32,
) -> std::ops::Range<usize> {
    if item_count == 0 || !item_extent.is_finite() || item_extent <= 0. {
        return 0..0;
    }
    let start = ((scroll_offset.max(0.) - cache_extent.max(0.)) / item_extent)
        .floor()
        .max(0.) as usize;
    let end = ((scroll_offset.max(0.) + viewport_extent.max(0.) + cache_extent.max(0.))
        / item_extent)
        .ceil()
        .max(0.) as usize;
    start.min(item_count)..end.min(item_count).max(start.min(item_count))
}

fn fixed_extent_content_extent(item_count: usize, item_extent: f32) -> f32 {
    // Geometry is f32 today. Saturating keeps malformed or enormous logical
    // data from wrapping while preserving normal list arithmetic exactly.
    ((item_count as f64) * f64::from(item_extent)).min(f64::from(f32::MAX)) as f32
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
        self.key == other.key && self.kind == other.kind && self.semantics == other.semantics
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
            Self::Shape {
                path,
                fill,
                stroke,
                size,
            } => f
                .debug_struct("PathView")
                .field("path", &path.id())
                .field("fill", fill)
                .field("stroke", stroke)
                .field("size", size)
                .finish(),
            Self::Decorated {
                size,
                background,
                border,
                radius,
                ..
            } => f
                .debug_struct("DecoratedBox")
                .field("size", size)
                .field("background", background)
                .field("border", border)
                .field("radius", radius)
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
            Self::Image {
                image,
                width,
                height,
                fit,
                alignment,
                sampling: _,
            } => f
                .debug_struct("Image")
                .field("id", &image.id())
                .field("width", width)
                .field("height", height)
                .field("fit", fit)
                .field("alignment", alignment)
                .finish(),
            Self::TextField { placeholder, .. } => f
                .debug_struct("TextField")
                .field("placeholder", placeholder)
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
            Self::VirtualList { config } => f
                .debug_struct("VirtualList")
                .field("item_count", &config.item_count)
                .field("item_extent", &config.item_extent)
                .field("cache_extent", &config.cache_extent)
                .finish(),
            Self::Translate { .. } => f.debug_struct("Translate").finish(),
        }
    }
}
impl PartialEq for WidgetKind {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Box { size: a, color: b }, Self::Box { size: c, color: d }) => a == c && b == d,
            (
                Self::Shape {
                    path: a,
                    fill: b,
                    stroke: c,
                    size: d,
                },
                Self::Shape {
                    path: e,
                    fill: f,
                    stroke: g,
                    size: h,
                },
            ) => a == e && b == f && c == g && d == h,
            (
                Self::Decorated {
                    size: a,
                    background: b,
                    border: c,
                    radius: d,
                    child: e,
                },
                Self::Decorated {
                    size: f,
                    background: g,
                    border: h,
                    radius: i,
                    child: j,
                },
            ) => a == f && b == g && c == h && d == i && e == j,
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
                Self::Image {
                    image: a,
                    width: b,
                    height: c,
                    fit: d,
                    alignment: e,
                    sampling: k,
                },
                Self::Image {
                    image: f,
                    width: g,
                    height: h,
                    fit: i,
                    alignment: j,
                    sampling: l,
                },
            ) => a == f && b == g && c == h && d == i && e == j && k == l,
            (
                Self::TextField {
                    controller: a,
                    size: b,
                    style: c,
                    placeholder: d,
                    on_submit: e,
                    multiline: k,
                },
                Self::TextField {
                    controller: f,
                    size: g,
                    style: h,
                    placeholder: i,
                    on_submit: j,
                    multiline: l,
                },
            ) => {
                a == f
                    && b == g
                    && c == h
                    && d == i
                    && k == l
                    && match (e, j) {
                        (Some(left), Some(right)) => Rc::ptr_eq(left, right),
                        (None, None) => true,
                        _ => false,
                    }
            }
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
            (Self::VirtualList { config: a }, Self::VirtualList { config: b }) => {
                a.item_count == b.item_count
                    && a.item_extent == b.item_extent
                    && a.cache_extent == b.cache_extent
                    && a.controller == b.controller
                    && Rc::ptr_eq(&a.builder, &b.builder)
            }
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
    Shape,
    Decorated,
    Image,
    Button,
    Text,
    TextField,
    Padding,
    Align,
    Flex,
    Scroll,
    VirtualList,
    Translate,
}
impl Widget {
    #[must_use]
    pub fn box_(size: Size, color: Color) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Box { size, color },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn fixed_box(size: Size, color: Color) -> Self {
        Self::box_(size, color)
    }
    #[must_use]
    fn shape(
        path: Arc<Path>,
        fill: Option<Brush>,
        stroke: Option<(Brush, Stroke)>,
        size: Option<Size>,
    ) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Shape {
                path,
                fill,
                stroke,
                size,
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    fn decorated(
        size: Option<Size>,
        background: Option<Brush>,
        border: Option<Border>,
        radius: CornerRadii,
        child: Widget,
    ) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Decorated {
                size,
                background,
                border,
                radius,
                child: Box::new(child),
            },
            semantics: SemanticProperties::default(),
        }
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
            semantics: SemanticProperties::default(),
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
            WidgetKind::VirtualList { .. } => {}
            WidgetKind::Flex { children, .. } => {
                for child in children {
                    child.bind_callbacks(allocate);
                }
            }
            WidgetKind::Box { .. }
            | WidgetKind::Shape { .. }
            | WidgetKind::Decorated { .. }
            | WidgetKind::Text { .. }
            | WidgetKind::TextField { .. }
            | WidgetKind::Image { .. } => {}
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
            semantics: SemanticProperties::default(),
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
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    fn image(
        image: ImageHandle,
        width: Option<f32>,
        height: Option<f32>,
        fit: ImageFit,
        alignment: Alignment,
        sampling: ImageSampling,
    ) -> Self {
        Self {
            key: None,
            kind: WidgetKind::Image {
                image,
                width,
                height,
                fit,
                alignment,
                sampling,
            },
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    fn text_field(
        controller: TextEditingController,
        size: Size,
        style: TextStyle,
        placeholder: String,
        on_submit: Option<Rc<dyn Fn(String)>>,
        multiline: bool,
    ) -> Self {
        Self {
            key: None,
            kind: WidgetKind::TextField {
                controller,
                size,
                style,
                placeholder,
                on_submit,
                multiline,
            },
            semantics: SemanticProperties::default(),
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
            semantics: SemanticProperties::default(),
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
            semantics: SemanticProperties::default(),
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
            semantics: SemanticProperties::default(),
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
            semantics: SemanticProperties::default(),
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
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    fn virtual_list(config: VirtualListConfig) -> Self {
        Self {
            key: None,
            kind: WidgetKind::VirtualList {
                config: Rc::new(config),
            },
            semantics: SemanticProperties::default(),
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
            semantics: SemanticProperties::default(),
        }
    }
    #[must_use]
    pub fn with_key(mut self, key: impl Into<Key>) -> Self {
        self.key = Some(key.into());
        self
    }
    /// Overrides the accessible label contributed by this meaningful widget.
    #[must_use]
    pub fn accessibility_label(mut self, label: impl Into<String>) -> Self {
        self.semantics.label = Some(label.into());
        self
    }
    /// Adds a screen-reader description without changing visible text.
    #[must_use]
    pub fn accessibility_description(mut self, description: impl Into<String>) -> Self {
        self.semantics.description = Some(description.into());
        self
    }
    /// Excludes this widget and its implementation-detail subtree from semantics.
    #[must_use]
    pub fn exclude_semantics(mut self) -> Self {
        self.semantics.hidden = true;
        self
    }
    #[must_use]
    pub fn key(&self) -> Option<&Key> {
        self.key.as_ref()
    }
    fn type_(&self) -> WidgetType {
        match self.kind {
            WidgetKind::Box { .. } => WidgetType::Box,
            WidgetKind::Shape { .. } => WidgetType::Shape,
            WidgetKind::Decorated { .. } => WidgetType::Decorated,
            WidgetKind::Image { .. } => WidgetType::Image,
            WidgetKind::Button { .. } => WidgetType::Button,
            WidgetKind::Text { .. } => WidgetType::Text,
            WidgetKind::TextField { .. } => WidgetType::TextField,
            WidgetKind::Padding { .. } => WidgetType::Padding,
            WidgetKind::Align { .. } => WidgetType::Align,
            WidgetKind::Flex { .. } => WidgetType::Flex,
            WidgetKind::Scroll { .. } => WidgetType::Scroll,
            WidgetKind::VirtualList { .. } => WidgetType::VirtualList,
            WidgetKind::Translate { .. } => WidgetType::Translate,
        }
    }
    fn children(&self) -> Vec<Widget> {
        match &self.kind {
            WidgetKind::Box { .. }
            | WidgetKind::Shape { .. }
            | WidgetKind::Text { .. }
            | WidgetKind::TextField { .. }
            | WidgetKind::Image { .. } => Vec::new(),
            WidgetKind::Button { child, .. } => {
                child.iter().map(|child| child.as_ref().clone()).collect()
            }
            WidgetKind::Padding { child, .. }
            | WidgetKind::Align { child, .. }
            | WidgetKind::Scroll { child, .. }
            | WidgetKind::Translate { child, .. }
            | WidgetKind::Decorated { child, .. } => {
                vec![child.as_ref().clone()]
            }
            WidgetKind::Flex { children, .. } => children.clone(),
            WidgetKind::VirtualList { .. } => Vec::new(),
        }
    }
}

/// Renderer-neutral image sizing policy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ImageFit {
    Fill,
    #[default]
    Contain,
    Cover,
    None,
    ScaleDown,
}
/// A declarative raster image. It retains only a shared asset handle.
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    image: ImageHandle,
    width: Option<f32>,
    height: Option<f32>,
    fit: ImageFit,
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
    pub fn alignment(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }
    #[must_use]
    pub fn sampling(mut self, sampling: ImageSampling) -> Self {
        self.sampling = sampling;
        self
    }
}
impl From<Image> for Widget {
    fn from(value: Image) -> Self {
        Widget::image(
            value.image,
            value.width,
            value.height,
            value.fit,
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
        PathView::new(value.path)
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
    pub fn border(mut self, border: Border) -> Self {
        self.border = Some(border);
        self
    }
    #[must_use]
    pub fn radius(mut self, radius: f32) -> Self {
        self.radius = CornerRadii::uniform(radius);
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

/// Single-line editable text. Keep the controller outside a rebuilt widget
/// description so the text, selection, and active composition survive rebuilds.
pub struct TextField {
    controller: TextEditingController,
    size: Size,
    style: TextStyle,
    placeholder: String,
    on_submit: Option<Rc<dyn Fn(String)>>,
}
impl TextField {
    #[must_use]
    pub fn new(controller: TextEditingController) -> Self {
        Self {
            controller,
            size: Size::new(260., 40.),
            style: TextStyle::default(),
            placeholder: String::new(),
            on_submit: None,
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
}
impl From<TextField> for Widget {
    fn from(value: TextField) -> Self {
        Widget::text_field(
            value.controller,
            value.size,
            value.style,
            value.placeholder,
            value.on_submit,
            false,
        )
    }
}

/// A bounded, soft-wrapping multiline editor. Enter and Shift+Enter insert a
/// hard newline; unlike [`TextField`], it has no implicit submit action.
pub struct TextArea {
    controller: TextEditingController,
    size: Size,
    style: TextStyle,
    placeholder: String,
}
impl TextArea {
    #[must_use]
    pub fn new(controller: TextEditingController) -> Self {
        Self {
            controller,
            size: Size::new(260., 180.),
            style: TextStyle::default(),
            placeholder: String::new(),
        }
    }
    #[must_use]
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
    #[must_use]
    pub fn height(mut self, height: f32) -> Self {
        self.size = Size::new(self.size.width, height);
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
}
impl From<TextArea> for Widget {
    fn from(value: TextArea) -> Self {
        Widget::text_field(
            value.controller,
            value.size,
            value.style,
            value.placeholder,
            None,
            true,
        )
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

/// A vertically scrolling fixed-extent lazy viewport. It is intentionally
/// distinct from [`ScrollView`]: items are created only while they intersect
/// the viewport plus a bounded logical-pixel cache (240px by default).
pub struct VirtualList;
impl VirtualList {
    pub const DEFAULT_ITEM_EXTENT: f32 = 48.;
    pub const DEFAULT_CACHE_EXTENT: f32 = 240.;

    #[must_use]
    pub fn builder<W>(item_count: usize, builder: impl Fn(usize) -> W + 'static) -> Widget
    where
        W: Into<Widget> + 'static,
    {
        Self::fixed_extent(item_count, Self::DEFAULT_ITEM_EXTENT, builder)
    }
    #[must_use]
    pub fn fixed_extent<W>(
        item_count: usize,
        item_extent: f32,
        builder: impl Fn(usize) -> W + 'static,
    ) -> Widget
    where
        W: Into<Widget> + 'static,
    {
        Self::fixed_extent_with_controller(
            item_count,
            item_extent,
            ScrollController::new(),
            builder,
        )
    }
    #[must_use]
    pub fn fixed_extent_with_controller<W>(
        item_count: usize,
        item_extent: f32,
        controller: ScrollController,
        builder: impl Fn(usize) -> W + 'static,
    ) -> Widget
    where
        W: Into<Widget> + 'static,
    {
        Self::fixed_extent_with_controller_and_cache(
            item_count,
            item_extent,
            Self::DEFAULT_CACHE_EXTENT,
            controller,
            builder,
        )
    }
    #[must_use]
    pub fn fixed_extent_with_controller_and_cache<W>(
        item_count: usize,
        item_extent: f32,
        cache_extent: f32,
        controller: ScrollController,
        builder: impl Fn(usize) -> W + 'static,
    ) -> Widget
    where
        W: Into<Widget> + 'static,
    {
        assert!(
            item_extent.is_finite() && item_extent > 0.,
            "item extent must be positive and finite"
        );
        assert!(
            cache_extent.is_finite() && cache_extent >= 0.,
            "cache extent must be finite and non-negative"
        );
        Widget::virtual_list(VirtualListConfig {
            item_count,
            item_extent,
            cache_extent,
            controller,
            builder: Rc::new(move |index| builder(index).into()),
        })
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
            semantics: SemanticProperties::default(),
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
    pub lazy_layouts: u64,
    pub items_built: u64,
    pub items_mounted: u64,
    pub items_unmounted: u64,
    pub items_reused: u64,
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
    /// Parallel to `children` only for a virtual-list element. Item indices
    /// are identity, never reusable visible-slot numbers.
    virtual_indices: Vec<usize>,
}
#[derive(Clone, Debug, PartialEq)]
enum RenderKind {
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
    Decorated {
        desired: Option<Size>,
        background: Option<Brush>,
        border: Option<Border>,
        radius: CornerRadii,
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
    Image {
        image: ImageHandle,
        width: Option<f32>,
        height: Option<f32>,
        fit: ImageFit,
        alignment: Alignment,
        sampling: ImageSampling,
    },
    TextField {
        controller: TextEditingController,
        desired: Size,
        style: TextStyle,
        placeholder: String,
        multiline: bool,
    },
    Scroll {
        controller: ScrollController,
    },
    VirtualList {
        config: Rc<VirtualListConfig>,
    },
    Translate {
        controller: TranslationController,
    },
}

/// Snapshot of one lazy viewport. Semantic integration can expose
/// `logical_item_count` plus these materialized item indices without creating
/// one semantic node per logical item.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VirtualListDiagnostics {
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
    text_revision: u64,
    text_visual_revision: u64,
    text_scroll_x: f32,
    text_scroll_y: f32,
    scrollbar_hovered: bool,
    scrollbar_dragging: bool,
    focused: bool,
    baseline: Option<f32>,
    button_state: ButtonState,
    /// Static parent-relative layout placement. This is never used to store a
    /// scroll or animation displacement.
    layer: LayerId,
    picture: Option<LayerId>,
    clip_layer: Option<LayerId>,
    content_layer: Option<LayerId>,
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
    next_action: u64,
    pending_handlers: Vec<(ActionId, Rc<dyn Fn()>)>,
    scrollbar_drag: Option<ScrollbarDrag>,
    semantics: SemanticsTree,
    semantic_ids: HashMap<ElementId, SemanticNodeId>,
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
            next_action: 1,
            pending_handlers: Vec::new(),
            scrollbar_drag: None,
            semantics: SemanticsTree::new(),
            semantic_ids: HashMap::new(),
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
    pub fn element_count(&self) -> usize {
        self.elements.len()
    }
    #[must_use]
    pub fn render_object_count(&self) -> usize {
        self.renders.len()
    }
    #[must_use]
    pub fn virtual_list_diagnostics(&self) -> Option<VirtualListDiagnostics> {
        self.elements.iter().find_map(|(_raw, element)| {
            let WidgetKind::VirtualList { config } = &element.widget.kind else {
                return None;
            };
            let render = self.renders.get(element.render.0)?;
            let range = element.virtual_indices.first().copied().unwrap_or(0)
                ..element.virtual_indices.last().map_or(0, |index| index + 1);
            Some(VirtualListDiagnostics {
                logical_item_count: config.item_count,
                materialized_item_count: element.virtual_indices.len(),
                materialized_range: range,
                scroll_offset: config.controller.offset(),
                viewport_extent: render.size.height,
                cache_extent: config.cache_extent,
                element_count: self.elements.len(),
                render_object_count: self.renders.len(),
                picture_layer_count: self.compositor.diagnostics().layers as usize,
            })
        })
    }
    #[must_use]
    pub fn diagnostics(&self) -> Diagnostics {
        self.diagnostics
    }
    /// Retained, renderer-independent semantic tree. It reflects meaningful
    /// controls, rather than paint commands or compositor pictures.
    #[must_use]
    pub fn semantics(&self) -> &SemanticsTree {
        &self.semantics
    }
    #[must_use]
    pub fn semantics_diagnostics(&self) -> SemanticsDiagnostics {
        self.semantics.diagnostics()
    }
    #[must_use]
    pub fn semantic_node_for_element(&self, element: ElementId) -> Option<SemanticNodeId> {
        self.semantic_ids.get(&element).copied()
    }
    #[must_use]
    pub fn element_for_semantic_node(&self, node: SemanticNodeId) -> Option<ElementId> {
        self.semantic_ids
            .iter()
            .find_map(|(element, current)| (*current == node).then_some(*element))
    }
    #[must_use]
    pub fn semantics_debug_dump(&self) -> String {
        self.semantics.debug_dump()
    }
    pub fn note_semantic_action(&mut self) {
        self.semantics.note_action();
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
    /// Allocates an opaque callback action. The runtime owns dispatch, while
    /// the tree uses this shared sequence for lazy children mounted during
    /// layout so IDs can never collide with eagerly prepared buttons.
    pub fn allocate_action(&mut self) -> ActionId {
        let action = ActionId(self.next_action);
        self.next_action += 1;
        action
    }
    pub fn take_pending_handlers(&mut self) -> Vec<(ActionId, Rc<dyn Fn()>)> {
        std::mem::take(&mut self.pending_handlers)
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
        for (_render, kind, content_layer) in nodes {
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
                        // Only this viewport's overlay picture changes; the
                        // retained content subtree remains compositor-only.
                        self.renders
                            .get_mut(_render.0)
                            .expect("live")
                            .dirty
                            .insert(DirtyFlags::PAINT);
                    }
                }
                RenderKind::VirtualList { config } => {
                    if let Some(content) = content_layer
                        && self.compositor.update_transform(
                            content,
                            Transform::translation(Offset::new(0., -config.controller.offset())),
                        )
                    {
                        changed = true;
                        self.diagnostics.scroll_offset_updates += 1;
                        self.renders
                            .get_mut(_render.0)
                            .expect("live")
                            .dirty
                            .insert(DirtyFlags::PAINT);
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
        let controller = match self
            .renders
            .get(scroll.0)
            .expect("live scroll")
            .kind
            .clone()
        {
            RenderKind::Scroll { controller } => controller,
            RenderKind::VirtualList { config } => config.controller.clone(),
            _ => return false,
        };
        if controller.scroll_by(delta.y) {
            self.diagnostics.scroll_events += 1;
            true
        } else {
            false
        }
    }
    /// Applies a semantic page action to the controller owned by this viewport.
    pub fn semantic_scroll(&mut self, id: ElementId, forward: bool) -> bool {
        let Some(render) = self.render_id(id) else {
            return false;
        };
        let controller = match &self.renders.get(render.0).expect("live").kind {
            RenderKind::Scroll { controller } => controller.clone(),
            RenderKind::VirtualList { config } => config.controller.clone(),
            _ => return false,
        };
        let delta = if forward {
            controller.viewport_extent()
        } else {
            -controller.viewport_extent()
        };
        let changed = controller.scroll_by(delta);
        if changed {
            self.diagnostics.scroll_events += 1;
        }
        changed
    }
    /// Handles overlay scrollbar hit testing and capture. A thumb drag maps
    /// directly to the same controller used by wheel input; a track click
    /// pages one viewport toward the pointer.
    pub fn scrollbar_pointer(&mut self, phase: incular_core::PointerPhase, point: Offset) -> bool {
        match phase {
            incular_core::PointerPhase::Down => {
                let Some(render) = self.scrollbar_at(point) else {
                    return false;
                };
                let (controller, geometry) = self
                    .scrollbar_controller_and_geometry(render)
                    .expect("scrollbar render");
                if geometry.thumb.contains(point) {
                    let grab = point.y - geometry.thumb.origin.y;
                    self.scrollbar_drag = Some(ScrollbarDrag {
                        render,
                        grab_offset: grab.clamp(0., geometry.thumb.size.height),
                    });
                    self.renders
                        .get_mut(render.0)
                        .expect("live")
                        .scrollbar_dragging = true;
                } else {
                    let delta = if point.y < geometry.thumb.origin.y {
                        -controller.viewport_extent()
                    } else {
                        controller.viewport_extent()
                    };
                    let _ = controller.scroll_by(delta);
                }
                self.renders
                    .get_mut(render.0)
                    .expect("live")
                    .dirty
                    .insert(DirtyFlags::PAINT);
                true
            }
            incular_core::PointerPhase::Move => {
                if let Some(drag) = self.scrollbar_drag {
                    let (controller, geometry) = self
                        .scrollbar_controller_and_geometry(drag.render)
                        .expect("live drag");
                    let thumb_top = point.y - drag.grab_offset;
                    let offset = geometry.offset_for_thumb_top(thumb_top);
                    let changed = controller.jump_to(offset);
                    if changed {
                        self.diagnostics.scroll_events += 1;
                    }
                    self.renders
                        .get_mut(drag.render.0)
                        .expect("live")
                        .dirty
                        .insert(DirtyFlags::PAINT);
                    return true;
                }
                let hovered = self.scrollbar_at(point);
                let mut changed = false;
                let ids = self
                    .renders
                    .iter()
                    .map(|(raw, _)| RenderObjectId(raw))
                    .collect::<Vec<_>>();
                for render in ids {
                    let node = self.renders.get_mut(render.0).expect("live");
                    let is_hovered = hovered == Some(render);
                    if node.scrollbar_hovered != is_hovered {
                        node.scrollbar_hovered = is_hovered;
                        node.dirty.insert(DirtyFlags::PAINT);
                        changed = true;
                    }
                }
                changed || hovered.is_some()
            }
            incular_core::PointerPhase::Up | incular_core::PointerPhase::Cancel => {
                let Some(drag) = self.scrollbar_drag.take() else {
                    return false;
                };
                let node = self.renders.get_mut(drag.render.0).expect("live");
                node.scrollbar_dragging = false;
                node.dirty.insert(DirtyFlags::PAINT);
                true
            }
        }
    }
    #[must_use]
    pub fn scrollbar_diagnostics(&self) -> Vec<ScrollbarGeometry> {
        self.renders
            .iter()
            .filter_map(|(raw, _)| {
                self.scrollbar_controller_and_geometry(RenderObjectId(raw))
                    .map(|(_, geometry)| geometry)
            })
            .collect()
    }
    #[must_use]
    pub fn scrollbar_drag_diagnostics(&self) -> ScrollbarDragDiagnostics {
        let Some(drag) = self.scrollbar_drag else {
            return ScrollbarDragDiagnostics::default();
        };
        let Some((controller, geometry)) = self.scrollbar_controller_and_geometry(drag.render)
        else {
            return ScrollbarDragDiagnostics::default();
        };
        ScrollbarDragDiagnostics {
            active: true,
            grab_offset: drag.grab_offset,
            track_extent: geometry.track.size.height,
            thumb_extent: geometry.thumb.size.height,
            thumb_travel: geometry.thumb_travel,
            thumb_top: geometry.thumb.origin.y,
            scroll_offset: controller.offset(),
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
        self.refresh_text_fields();
        self.refresh_virtual_ranges();
        if let Some(root) = self.root.and_then(|id| self.render_id(id)) {
            self.layout_render(root, constraints);
        }
    }
    /// Synchronizes the retained semantic arena after layout/compositor state
    /// is valid. Non-semantic layout widgets merge their descendants into the
    /// closest meaningful semantic ancestor.
    pub fn update_semantics(&mut self) {
        let mut built = Vec::new();
        if let Some(root) = self.root {
            self.collect_semantics(root, None, &mut built);
        }
        let live: HashSet<_> = built.iter().map(|node| node.element).collect();
        let stale: Vec<_> = self
            .semantic_ids
            .iter()
            .filter_map(|(element, node)| (!live.contains(element)).then_some((*element, *node)))
            .collect();
        for (element, node) in stale {
            self.semantic_ids.remove(&element);
            let _ = self.semantics.remove(node);
        }
        for build in &built {
            let id = *self.semantic_ids.entry(build.element).or_insert_with(|| {
                self.semantics.insert(SemanticNode {
                    id: SemanticNodeId(ArenaId::from_parts(0, 0)),
                    role: build.role,
                    label: build.label.clone(),
                    value: build.value.clone(),
                    description: build.description.clone(),
                    bounds: build.bounds,
                    state: build.state.clone(),
                    actions: build.actions.clone(),
                    children: Vec::new(),
                })
            });
            let children = built
                .iter()
                .filter(|child| child.parent == Some(build.element))
                .filter_map(|child| self.semantic_ids.get(&child.element).copied())
                .collect();
            let _ = self.semantics.update(
                id,
                SemanticNode {
                    id,
                    role: build.role,
                    label: build.label.clone(),
                    value: build.value.clone(),
                    description: build.description.clone(),
                    bounds: build.bounds,
                    state: build.state.clone(),
                    actions: build.actions.clone(),
                    children,
                },
            );
        }
        self.semantics.set_root(
            built
                .iter()
                .find(|node| node.parent.is_none())
                .and_then(|node| self.semantic_ids.get(&node.element).copied()),
        );
    }
    fn collect_semantics(
        &self,
        element: ElementId,
        semantic_parent: Option<ElementId>,
        out: &mut Vec<SemanticBuild>,
    ) {
        let Some(entry) = self.elements.get(element.0) else {
            return;
        };
        if entry.widget.semantics.hidden {
            return;
        }
        let render = match self.renders.get(entry.render.0) {
            Some(render) => render,
            None => return,
        };
        let (role, default_label, value, state, actions) = match &entry.widget.kind {
            WidgetKind::Button { .. } => (
                Some(SemanticRole::Button),
                widget_text(&entry.widget),
                None,
                SemanticState {
                    enabled: true,
                    focused: render.button_state == ButtonState::Focused,
                    focusable: true,
                    ..SemanticState::default()
                },
                vec![SemanticActionKind::Focus, SemanticActionKind::Activate],
            ),
            WidgetKind::Text { text, .. } => (
                Some(SemanticRole::Text),
                Some(text.clone()),
                None,
                SemanticState::default(),
                vec![],
            ),
            WidgetKind::TextField {
                controller,
                multiline,
                ..
            } => {
                let value = controller.value();
                (
                    Some(if *multiline {
                        SemanticRole::TextArea
                    } else {
                        SemanticRole::TextField
                    }),
                    None,
                    Some(value.text),
                    SemanticState {
                        enabled: true,
                        focused: render.focused,
                        focusable: true,
                        editable: true,
                        multiline: *multiline,
                        selection: Some(SemanticTextSelection {
                            base: value.selection.base,
                            extent: value.selection.extent,
                        }),
                        ..SemanticState::default()
                    },
                    vec![
                        SemanticActionKind::Focus,
                        SemanticActionKind::SetText,
                        SemanticActionKind::SetSelection,
                    ],
                )
            }
            WidgetKind::Scroll { controller, .. } => (
                Some(SemanticRole::ScrollView),
                None,
                Some(format!(
                    "{:.0}/{:.0}",
                    controller.offset(),
                    controller.max_offset()
                )),
                SemanticState::default(),
                vec![
                    SemanticActionKind::ScrollForward,
                    SemanticActionKind::ScrollBackward,
                ],
            ),
            WidgetKind::VirtualList { config } => (
                Some(SemanticRole::List),
                None,
                None,
                SemanticState {
                    set_size: Some(config.item_count),
                    ..SemanticState::default()
                },
                vec![
                    SemanticActionKind::ScrollForward,
                    SemanticActionKind::ScrollBackward,
                ],
            ),
            _ => (None, None, None, SemanticState::default(), vec![]),
        };
        let this_parent = if let Some(role) = role {
            let mut state = state;
            if let Some(parent) = entry.parent.and_then(|parent| self.elements.get(parent.0)) {
                if let WidgetKind::VirtualList { config } = &parent.widget.kind {
                    if let Some(slot) = parent.children.iter().position(|child| *child == element) {
                        state.item_index = parent.virtual_indices.get(slot).copied();
                        state.set_size = Some(config.item_count);
                    }
                }
            }
            out.push(SemanticBuild {
                element,
                parent: semantic_parent,
                role,
                label: entry.widget.semantics.label.clone().or(default_label),
                value,
                description: entry.widget.semantics.description.clone(),
                bounds: Rect::from_origin_size(self.render_origin(entry.render), render.size),
                state,
                actions,
            });
            Some(element)
        } else {
            semantic_parent
        };
        // A Button deliberately merges its visual label/icon subtree into the
        // one control node. Other containers preserve logical child order.
        if !matches!(entry.widget.kind, WidgetKind::Button { .. }) {
            for child in &entry.children {
                self.collect_semantics(*child, this_parent, out);
            }
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
    #[must_use]
    pub fn focusable_elements(&self) -> Vec<ElementId> {
        let mut result = Vec::new();
        if let Some(root) = self.root {
            self.collect_focusable(root, &mut result);
        }
        result
    }
    #[must_use]
    pub fn text_field_at(&self, point: Offset) -> Option<ElementId> {
        let hit = self
            .hit_test(point)
            .and_then(|render| self.element_for_render(render))?;
        self.text_field_ancestor(hit)
    }
    pub fn set_focused(
        &mut self,
        id: ElementId,
        focused: bool,
        now: Instant,
    ) -> Result<(), TreeError> {
        let render = self.render_id(id).ok_or(TreeError::MissingElement(id))?;
        let node = self.renders.get_mut(render.0).expect("live");
        if matches!(node.kind, RenderKind::Button { .. }) {
            node.button_state = if focused {
                ButtonState::Focused
            } else {
                ButtonState::Normal
            };
            node.dirty.insert(DirtyFlags::PAINT);
        }
        if matches!(node.kind, RenderKind::TextField { .. }) && node.focused != focused {
            node.focused = focused;
            node.dirty.insert(DirtyFlags::PAINT);
            if focused {
                if let RenderKind::TextField { controller, .. } = &node.kind {
                    controller.reset_caret(now);
                }
            }
        }
        Ok(())
    }
    #[must_use]
    pub fn is_text_field(&self, id: ElementId) -> bool {
        self.elements
            .get(id.0)
            .is_some_and(|element| matches!(element.widget.kind, WidgetKind::TextField { .. }))
    }
    #[must_use]
    pub fn is_multiline_text_field(&self, id: ElementId) -> bool {
        self.elements.get(id.0).is_some_and(|element| {
            matches!(
                element.widget.kind,
                WidgetKind::TextField {
                    multiline: true,
                    ..
                }
            )
        })
    }
    pub fn text_field_move_vertical(&mut self, id: ElementId, down: bool, extend: bool) -> bool {
        let Some(render) = self.render_id(id) else {
            return false;
        };
        let Some(node) = self.renders.get(render.0) else {
            return false;
        };
        let (
            RenderKind::TextField {
                controller,
                multiline,
                ..
            },
            Some(layout),
        ) = (&node.kind, node.text_layout.clone())
        else {
            return false;
        };
        if !multiline {
            return false;
        }
        let current = controller.value().selection.extent;
        let line = line_for_byte(&layout, current);
        let target_line = if down {
            line.saturating_add(1)
        } else {
            line.saturating_sub(1)
        };
        let Some(target) = layout.lines.get(target_line) else {
            return false;
        };
        let x = controller
            .preferred_caret_x()
            .unwrap_or_else(|| line_caret_x(&layout.lines[line], current));
        controller.move_cursor_with_x(caret_for_line_x(target, x), extend, x);
        self.renders
            .get_mut(render.0)
            .expect("live")
            .dirty
            .insert(DirtyFlags::PAINT);
        true
    }
    pub fn text_field_move_line_edge(&mut self, id: ElementId, end: bool, extend: bool) -> bool {
        let Some(render) = self.render_id(id) else {
            return false;
        };
        let Some(node) = self.renders.get(render.0) else {
            return false;
        };
        let (
            RenderKind::TextField {
                controller,
                multiline,
                ..
            },
            Some(layout),
        ) = (&node.kind, node.text_layout.clone())
        else {
            return false;
        };
        if !multiline {
            return false;
        }
        let line = line_for_byte(&layout, controller.value().selection.extent);
        let target = if end {
            layout.lines[line].end
        } else {
            layout.lines[line].start
        };
        controller.move_cursor(target, extend);
        self.renders
            .get_mut(render.0)
            .expect("live")
            .dirty
            .insert(DirtyFlags::PAINT);
        true
    }
    pub fn text_field_set_caret(
        &mut self,
        id: ElementId,
        point: Offset,
        extend: bool,
        now: Instant,
    ) -> bool {
        let Some(render) = self.render_id(id) else {
            return false;
        };
        let (layout, scroll_x, scroll_y, multiline, controller) =
            match self.renders.get(render.0).map(|node| {
                (
                    &node.kind,
                    node.text_layout.clone(),
                    node.text_scroll_x,
                    node.text_scroll_y,
                )
            }) {
                Some((
                    RenderKind::TextField {
                        controller,
                        multiline,
                        ..
                    },
                    layout,
                    scroll_x,
                    scroll_y,
                )) => (layout, scroll_x, scroll_y, *multiline, controller.clone()),
                _ => return false,
            };
        let origin = self.render_origin(render);
        let x = point.x - origin.x - 8. + scroll_x;
        let y = point.y - origin.y - if multiline { 8. } else { 0. } + scroll_y;
        let text_len = controller.text().len();
        let index = layout.as_ref().map_or(0, |layout| {
            let line = layout
                .lines
                .get((y / layout.metrics.line_height).floor().max(0.) as usize)
                .or_else(|| layout.lines.last());
            line.map_or(0, |line| caret_for_line_x(line, x))
                .min(text_len)
        });
        let previous = controller.value().selection;
        controller.set_selection(if extend {
            TextSelection {
                base: previous.base,
                extent: index,
            }
        } else {
            TextSelection::collapsed(index)
        });
        controller.reset_caret(now);
        self.renders
            .get_mut(render.0)
            .expect("live")
            .dirty
            .insert(DirtyFlags::PAINT);
        true
    }
    #[must_use]
    pub fn text_controller(&self, id: ElementId) -> Option<TextEditingController> {
        let render = self.render_id(id)?;
        match &self.renders.get(render.0)?.kind {
            RenderKind::TextField { controller, .. } => Some(controller.clone()),
            _ => None,
        }
    }
    pub fn submit_text_field(&self, id: ElementId) -> bool {
        let Some(element) = self.elements.get(id.0) else {
            return false;
        };
        let WidgetKind::TextField {
            controller,
            on_submit,
            ..
        } = &element.widget.kind
        else {
            return false;
        };
        if let Some(callback) = on_submit {
            callback(controller.text());
        }
        true
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
            WidgetKind::Box { .. }
                | WidgetKind::Shape { .. }
                | WidgetKind::Decorated { .. }
                | WidgetKind::Button { .. }
                | WidgetKind::Text { .. }
                | WidgetKind::TextField { .. }
                | WidgetKind::Image { .. }
                | WidgetKind::Scroll { .. }
                | WidgetKind::VirtualList { .. }
        )
        .then(|| {
            self.compositor.create_picture(
                DisplayList::new(),
                Rect::from_origin_size(Offset::ZERO, Size::ZERO),
            )
        });
        let (clip_layer, content_layer) = match &widget.kind {
            WidgetKind::Scroll { .. } | WidgetKind::VirtualList { .. } => {
                let clip = self
                    .compositor
                    .create_clip_rect(Rect::from_origin_size(Offset::ZERO, Size::ZERO));
                let content = self
                    .compositor
                    .create_transform(Transform::translation(Offset::ZERO));
                self.compositor
                    .set_children(layer, std::iter::once(clip).chain(picture).collect());
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
            text_revision: 0,
            text_visual_revision: 0,
            text_scroll_x: 0.,
            text_scroll_y: 0.,
            scrollbar_hovered: false,
            scrollbar_dragging: false,
            focused: false,
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
            virtual_indices: Vec::new(),
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
        // Lazy children are owned by the viewport's indexed materialization
        // map, not by `Widget::children()`. Recreating a VirtualList
        // description must preserve every still-valid mounted row; the next
        // layout pass will add/drop only indices required by the new config.
        if matches!(widget.kind, WidgetKind::VirtualList { .. }) {
            return Ok(());
        }
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
    /// Reconciles only the requested indexed window. Items outside it are
    /// unmounted instead of being recycled into unrelated logical indices.
    fn materialize_virtual_children(
        &mut self,
        id: RenderObjectId,
        config: &VirtualListConfig,
        viewport: Size,
        _constraints: Constraints,
    ) {
        let element_id = self.element_for_render(id).expect("virtual list element");
        let wanted = fixed_extent_materialized_range(
            config.item_count,
            config.item_extent,
            config.controller.offset(),
            viewport.height,
            config.cache_extent,
        );
        let (old_indices, old_children) = {
            let element = self
                .elements
                .get(element_id.0)
                .expect("virtual list element");
            (element.virtual_indices.clone(), element.children.clone())
        };
        if old_indices.as_slice() == (wanted.clone().collect::<Vec<_>>()).as_slice() {
            return;
        }
        let existing = old_indices
            .into_iter()
            .zip(old_children.iter().copied())
            .collect::<HashMap<_, _>>();
        let mut next_indices = Vec::with_capacity(wanted.len());
        let mut next_children = Vec::with_capacity(wanted.len());
        for index in wanted.clone() {
            if let Some(child) = existing.get(&index).copied() {
                next_indices.push(index);
                next_children.push(child);
                self.diagnostics.items_reused += 1;
                continue;
            }
            let mut widget = (config.builder)(index);
            let handlers = &mut self.pending_handlers;
            let next = &mut self.next_action;
            widget.bind_callbacks(&mut |callback| {
                let action = ActionId(*next);
                *next += 1;
                handlers.push((action, callback));
                action
            });
            self.diagnostics.items_built += 1;
            match self.mount_element(Some(element_id), widget) {
                Ok(child) => {
                    next_indices.push(index);
                    next_children.push(child);
                    self.diagnostics.items_mounted += 1;
                }
                // An item builder is application code; keep the viewport
                // structurally valid if it produces duplicate sibling keys.
                Err(error) => panic!("virtual list item {index} could not mount: {error:?}"),
            }
        }
        let retained = next_children.iter().copied().collect::<HashSet<_>>();
        for child in old_children {
            if !retained.contains(&child) {
                self.unmount_element(child);
                self.diagnostics.items_unmounted += 1;
            }
        }
        let element = self
            .elements
            .get_mut(element_id.0)
            .expect("virtual list element");
        element.children = next_children;
        element.virtual_indices = next_indices;
        self.sync_render_children(element_id);
    }
    /// A controller can change independently of widget BUILD. Only mark the
    /// lazy viewport dirty when its cache window actually changes; otherwise
    /// scrolling remains a retained-transform-only operation.
    fn refresh_virtual_ranges(&mut self) {
        let pending = self
            .renders
            .iter()
            .filter_map(|(raw, render)| {
                let RenderKind::VirtualList { config } = &render.kind else {
                    return None;
                };
                let element = self.element_for_render(RenderObjectId(raw))?;
                let indices = &self.elements.get(element.0)?.virtual_indices;
                let desired = fixed_extent_materialized_range(
                    config.item_count,
                    config.item_extent,
                    config.controller.offset(),
                    render.size.height,
                    config.cache_extent,
                );
                (indices.as_slice() != desired.clone().collect::<Vec<_>>().as_slice())
                    .then_some(RenderObjectId(raw))
            })
            .collect::<Vec<_>>();
        for render in pending {
            self.mark_render_dirty(render, DirtyFlags::LAYOUT, true);
        }
    }
    fn refresh_text_fields(&mut self) {
        let pending = self
            .renders
            .iter()
            .filter_map(|(raw, render)| {
                let RenderKind::TextField { controller, .. } = &render.kind else {
                    return None;
                };
                let (content, visual) = controller.revisions();
                ((content != render.text_revision) || (visual != render.text_visual_revision))
                    .then_some(RenderObjectId(raw))
            })
            .collect::<Vec<_>>();
        for render in pending {
            // Text width can change the size seen by an unconstrained parent.
            self.mark_render_dirty(render, DirtyFlags::LAYOUT | DirtyFlags::PAINT, true);
        }
    }
    fn collect_focusable(&self, id: ElementId, out: &mut Vec<ElementId>) {
        let Some(element) = self.elements.get(id.0) else {
            return;
        };
        if matches!(
            element.widget.kind,
            WidgetKind::Button { .. } | WidgetKind::TextField { .. }
        ) {
            out.push(id);
        }
        for child in &element.children {
            self.collect_focusable(*child, out);
        }
    }
    fn text_field_ancestor(&self, mut id: ElementId) -> Option<ElementId> {
        loop {
            if self.is_text_field(id) {
                return Some(id);
            }
            id = self.parent(id)?;
        }
    }
    fn render_origin(&self, mut id: RenderObjectId) -> Offset {
        let mut origin = Offset::ZERO;
        loop {
            let node = self.renders.get(id.0).expect("live render");
            origin = origin + node.offset;
            match &node.kind {
                RenderKind::Scroll { controller } => {
                    origin = origin - Offset::new(0., controller.offset())
                }
                RenderKind::VirtualList { config } => {
                    origin = origin - Offset::new(0., config.controller.offset())
                }
                RenderKind::Translate { controller } => origin = origin + controller.offset(),
                _ => {}
            }
            let Some(parent) = node.parent else {
                return origin;
            };
            id = parent;
        }
    }
    /// World origin of a render object's viewport/picture. Unlike
    /// `render_origin`, this deliberately does not apply the object's own
    /// scrolling transform: a scrollbar is attached to that viewport, not to
    /// its scrolling content. Ancestor transforms still apply.
    fn render_viewport_origin(&self, mut id: RenderObjectId) -> Offset {
        let mut origin = Offset::ZERO;
        let mut is_self = true;
        loop {
            let node = self.renders.get(id.0).expect("live render");
            origin = origin + node.offset;
            if !is_self {
                match &node.kind {
                    RenderKind::Scroll { controller } => {
                        origin = origin - Offset::new(0., controller.offset())
                    }
                    RenderKind::VirtualList { config } => {
                        origin = origin - Offset::new(0., config.controller.offset())
                    }
                    RenderKind::Translate { controller } => origin = origin + controller.offset(),
                    _ => {}
                }
            }
            let Some(parent) = node.parent else {
                return origin;
            };
            id = parent;
            is_self = false;
        }
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
            RenderKind::Box { desired, .. } | RenderKind::Shape { desired, .. } => {
                (constraints.constrain(desired), Vec::new())
            }
            RenderKind::Decorated { desired, .. } => {
                if let Some(&child) = children.first() {
                    self.layout_render(child, constraints.loosen());
                    let child_size = self.renders.get(child.0).expect("live").size;
                    let wanted = desired.unwrap_or(child_size);
                    let size = constraints.constrain(Size::new(
                        wanted.width.max(child_size.width),
                        wanted.height.max(child_size.height),
                    ));
                    (size, vec![Offset::ZERO])
                } else {
                    (
                        constraints.constrain(desired.unwrap_or(Size::ZERO)),
                        Vec::new(),
                    )
                }
            }
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
            RenderKind::Image {
                image,
                width,
                height,
                ..
            } => {
                let intrinsic = image.decoded();
                let ratio = intrinsic.width() as f32 / intrinsic.height() as f32;
                let natural = match (width, height) {
                    (Some(w), Some(h)) => Size::new(w, h),
                    (Some(w), None) => Size::new(w, w / ratio),
                    (None, Some(h)) => Size::new(h * ratio, h),
                    (None, None) => Size::new(intrinsic.width() as f32, intrinsic.height() as f32),
                };
                (constraints.constrain(natural), Vec::new())
            }
            RenderKind::TextField {
                controller,
                desired,
                style,
                placeholder,
                multiline,
            } => {
                let value = controller.value();
                let display = text_field_display(&value, &placeholder);
                let size = constraints.constrain(desired);
                let layout = self.text_engine.layout(
                    &display,
                    &style,
                    multiline.then_some((size.width - 16.).max(0.)),
                    TextAlign::Start,
                );
                let (revision, visual_revision) = controller.revisions();
                let node = self.renders.get_mut(id.0).expect("live");
                node.text_layout = Some(layout.clone());
                node.text_revision = revision;
                node.text_visual_revision = visual_revision;
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
            RenderKind::VirtualList { config } => {
                let size = constraints.constrain(Size::new(
                    if constraints.is_width_bounded() {
                        constraints.max_width
                    } else {
                        0.
                    },
                    if constraints.is_height_bounded() {
                        constraints.max_height
                    } else {
                        0.
                    },
                ));
                config.controller.set_extents(
                    fixed_extent_content_extent(config.item_count, config.item_extent),
                    size.height,
                );
                self.materialize_virtual_children(id, &config, size, constraints);
                let materialized = self.renders.get(id.0).expect("live").children.clone();
                let item_indices = self
                    .elements
                    .get(self.element_for_render(id).expect("virtual element").0)
                    .expect("virtual element")
                    .virtual_indices
                    .clone();
                for (child, item) in materialized.into_iter().zip(item_indices) {
                    self.layout_render(
                        child,
                        Constraints::new(
                            0.,
                            constraints.max_width,
                            config.item_extent,
                            config.item_extent,
                        ),
                    );
                    let child = self.renders.get_mut(child.0).expect("live");
                    let offset =
                        Offset::new(0., (item as f64 * f64::from(config.item_extent)) as f32);
                    child.offset = offset;
                    self.compositor
                        .update_transform(child.layer, Transform::translation(offset));
                }
                self.diagnostics.lazy_layouts += 1;
                (size, Vec::new())
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
                RenderKind::Shape {
                    path, fill, stroke, ..
                } => {
                    if let Some(brush) = fill {
                        cache.push(PaintCommand::FillPath {
                            path: path.clone(),
                            brush,
                            fill_rule: FillRule::NonZero,
                        });
                    }
                    if let Some((brush, stroke)) = stroke {
                        cache.push(PaintCommand::StrokePath {
                            path,
                            brush,
                            stroke,
                        });
                    }
                }
                RenderKind::Decorated {
                    background,
                    border,
                    radius,
                    ..
                } => {
                    let rrect = RRect::new(Rect::from_origin_size(Offset::ZERO, size), radius);
                    if let Some(brush) = background {
                        cache.push(PaintCommand::RRect { rrect, brush });
                    }
                    if let Some(border) = border {
                        cache.push(PaintCommand::Border { rrect, border });
                    }
                }
                RenderKind::Button { color, .. } => {
                    let state = self.renders.get(id.0).expect("live").button_state;
                    let adjust = match state {
                        ButtonState::Normal => 0,
                        ButtonState::Hovered => 18,
                        ButtonState::Focused => 28,
                        ButtonState::Pressed => -24,
                    };
                    let shift = |value: u8| (value as i16 + adjust).clamp(0, 255) as u8;
                    cache.push(PaintCommand::RRect {
                        rrect: RRect::uniform(Rect::from_origin_size(Offset::ZERO, size), 6.),
                        brush: Color::rgba(
                            shift(color.red),
                            shift(color.green),
                            shift(color.blue),
                            color.alpha,
                        )
                        .into(),
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
                RenderKind::Image {
                    image,
                    fit,
                    alignment,
                    sampling,
                    ..
                } => {
                    let intrinsic = image.decoded();
                    let source = Rect::from_origin_size(
                        Offset::ZERO,
                        Size::new(intrinsic.width() as f32, intrinsic.height() as f32),
                    );
                    let (source, destination) = image_fit_rects(
                        source,
                        Rect::from_origin_size(Offset::ZERO, size),
                        fit,
                        alignment,
                    );
                    cache.push(PaintCommand::Image {
                        image,
                        source,
                        destination,
                        sampling,
                    });
                }
                RenderKind::TextField {
                    controller,
                    style,
                    placeholder,
                    multiline,
                    ..
                } => {
                    let (layout, focused, scroll_x, scroll_y) = {
                        let node = self.renders.get(id.0).expect("live");
                        (
                            node.text_layout.clone(),
                            node.focused,
                            node.text_scroll_x,
                            node.text_scroll_y,
                        )
                    };
                    cache.push(PaintCommand::Rect {
                        rect: Rect::from_origin_size(Offset::ZERO, size),
                        color: Color::rgba(48, 50, 63, 255),
                    });
                    let value = controller.value();
                    let display = text_field_display(&value, &placeholder);
                    let mut active_scroll_x = scroll_x;
                    let mut active_scroll_y = scroll_y;
                    if let Some(layout) = layout {
                        let (caret, caret_y, caret_height) =
                            caret_geometry(&layout, value.selection.extent);
                        let available = (size.width - 16.).max(0.);
                        if !multiline && caret - active_scroll_x > available {
                            active_scroll_x = caret - available;
                        } else if !multiline && caret < active_scroll_x {
                            active_scroll_x = caret;
                        }
                        active_scroll_x = active_scroll_x.max(0.);
                        let top = if multiline {
                            8.
                        } else {
                            ((size.height - layout.metrics.line_height) / 2.).max(0.)
                        };
                        if multiline {
                            let viewport = (size.height - 16.).max(0.);
                            if caret_y - active_scroll_y < 0. {
                                active_scroll_y = caret_y;
                            } else if caret_y + caret_height - active_scroll_y > viewport {
                                active_scroll_y = caret_y + caret_height - viewport;
                            }
                            active_scroll_y = active_scroll_y
                                .clamp(0., (layout.metrics.size.height - viewport).max(0.));
                        }
                        let selection = value.selection.range();
                        if focused && !selection.is_empty() {
                            for rect in selection_rects(
                                &layout,
                                selection,
                                active_scroll_x,
                                active_scroll_y,
                                top,
                            ) {
                                cache.push(PaintCommand::Rect {
                                    rect,
                                    color: Color::rgba(72, 120, 220, 150),
                                });
                            }
                        }
                        cache.push(PaintCommand::PushClip {
                            rect: Rect::from_origin_size(
                                Offset::new(8., 2.),
                                Size::new((size.width - 16.).max(0.), (size.height - 4.).max(0.)),
                            ),
                        });
                        cache.push(PaintCommand::PushTransform {
                            transform: Transform::translation(Offset::new(
                                8. - active_scroll_x,
                                top - active_scroll_y,
                            )),
                        });
                        let color = if display == placeholder && value.text.is_empty() {
                            Color::rgba(150, 154, 170, 255)
                        } else {
                            style.color
                        };
                        for line in layout.lines.iter() {
                            cache.push(PaintCommand::GlyphRun {
                                run: line.run.clone(),
                                color,
                            });
                        }
                        cache.push(PaintCommand::PopTransform);
                        cache.push(PaintCommand::PopClip);
                        if focused && controller.caret_visible(Instant::now()) {
                            cache.push(PaintCommand::Rect {
                                rect: Rect::from_origin_size(
                                    Offset::new(
                                        caret - active_scroll_x + 8.,
                                        top + caret_y - active_scroll_y,
                                    ),
                                    Size::new(1., caret_height),
                                ),
                                color: Color::WHITE,
                            });
                        }
                    }
                    if focused {
                        let border = Color::rgba(120, 170, 245, 255);
                        cache.push(PaintCommand::Rect {
                            rect: Rect::from_origin_size(Offset::ZERO, Size::new(size.width, 1.)),
                            color: border,
                        });
                        cache.push(PaintCommand::Rect {
                            rect: Rect::from_origin_size(
                                Offset::new(0., (size.height - 1.).max(0.)),
                                Size::new(size.width, 1.),
                            ),
                            color: border,
                        });
                    }
                    let node = self.renders.get_mut(id.0).expect("live");
                    node.text_scroll_x = active_scroll_x;
                    node.text_scroll_y = active_scroll_y;
                }
                RenderKind::Scroll { controller } => {
                    self.paint_scrollbar(id, size, &controller, &mut cache);
                }
                RenderKind::VirtualList { config } => {
                    self.paint_scrollbar(id, size, &config.controller, &mut cache);
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
    fn paint_scrollbar(
        &self,
        id: RenderObjectId,
        size: Size,
        controller: &ScrollController,
        cache: &mut DisplayList,
    ) {
        let geometry = scrollbar_geometry(size, controller, ScrollbarStyle::default());
        if !geometry.visible {
            return;
        }
        let node = self.renders.get(id.0).expect("live");
        let mut thumb = ScrollbarStyle::default().thumb_color;
        if node.scrollbar_dragging {
            thumb = Color::rgba(205, 215, 240, 235);
        } else if node.scrollbar_hovered {
            thumb = Color::rgba(190, 202, 230, 220);
        }
        cache.push(PaintCommand::RRect {
            rrect: RRect::uniform(geometry.track, ScrollbarStyle::default().width * 0.5),
            brush: ScrollbarStyle::default().track_color.into(),
        });
        cache.push(PaintCommand::RRect {
            rrect: RRect::uniform(geometry.thumb, ScrollbarStyle::default().width * 0.5),
            brush: thumb.into(),
        });
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
            RenderKind::VirtualList { config } => {
                current - Offset::new(0., config.controller.offset())
            }
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
            RenderKind::VirtualList { config } => {
                current - Offset::new(0., config.controller.offset())
            }
            RenderKind::Translate { .. } => current,
            _ => current,
        };
        for child in node.children.iter().rev() {
            if let Some(found) = self.scroll_target(*child, point, child_origin) {
                return Some(found);
            }
        }
        matches!(
            node.kind,
            RenderKind::Scroll { .. } | RenderKind::VirtualList { .. }
        )
        .then_some(id)
    }
    fn scrollbar_controller_and_geometry(
        &self,
        render: RenderObjectId,
    ) -> Option<(ScrollController, ScrollbarGeometry)> {
        let node = self.renders.get(render.0)?;
        let controller = match &node.kind {
            RenderKind::Scroll { controller } => controller.clone(),
            RenderKind::VirtualList { config } => config.controller.clone(),
            _ => return None,
        };
        let mut geometry = scrollbar_geometry(node.size, &controller, ScrollbarStyle::default());
        let origin = self.render_viewport_origin(render);
        geometry.track.origin = geometry.track.origin + origin;
        geometry.thumb.origin = geometry.thumb.origin + origin;
        Some((controller, geometry))
    }
    fn scrollbar_at(&self, point: Offset) -> Option<RenderObjectId> {
        self.renders.iter().fold(None, |found, (raw, _)| {
            let render = RenderObjectId(raw);
            found.or_else(|| {
                self.scrollbar_controller_and_geometry(render)
                    .and_then(|(_, geometry)| {
                        (geometry.visible && geometry.track.contains(point)).then_some(render)
                    })
            })
        })
    }
}

fn widget_text(widget: &Widget) -> Option<String> {
    match &widget.kind {
        WidgetKind::Text { text, .. } => Some(text.clone()),
        WidgetKind::Button { child, .. } => child.as_deref().and_then(widget_text),
        WidgetKind::Padding { child, .. }
        | WidgetKind::Align { child, .. }
        | WidgetKind::Scroll { child, .. }
        | WidgetKind::Translate { child, .. } => widget_text(child),
        WidgetKind::Flex { children, .. } => {
            let text: String = children
                .iter()
                .filter_map(widget_text)
                .collect::<Vec<_>>()
                .join(" ");
            (!text.is_empty()).then_some(text)
        }
        _ => None,
    }
}
fn render_kind(widget: &Widget) -> RenderKind {
    match &widget.kind {
        WidgetKind::Box { size, color } => RenderKind::Box {
            desired: *size,
            color: *color,
        },
        WidgetKind::Shape {
            path,
            fill,
            stroke,
            size,
        } => RenderKind::Shape {
            path: path.clone(),
            fill: fill.clone(),
            stroke: stroke.clone(),
            desired: size.unwrap_or_else(|| path.bounds().map_or(Size::ZERO, |bounds| bounds.size)),
        },
        WidgetKind::Decorated {
            size,
            background,
            border,
            radius,
            ..
        } => RenderKind::Decorated {
            desired: *size,
            background: background.clone(),
            border: *border,
            radius: *radius,
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
        WidgetKind::Image {
            image,
            width,
            height,
            fit,
            alignment,
            sampling,
        } => RenderKind::Image {
            image: image.clone(),
            width: *width,
            height: *height,
            fit: *fit,
            alignment: *alignment,
            sampling: *sampling,
        },
        WidgetKind::TextField {
            controller,
            size,
            style,
            placeholder,
            multiline,
            ..
        } => RenderKind::TextField {
            controller: controller.clone(),
            desired: *size,
            style: style.clone(),
            placeholder: placeholder.clone(),
            multiline: *multiline,
        },
        WidgetKind::Padding { padding, .. } => RenderKind::Padding { padding: *padding },
        WidgetKind::Align { alignment, .. } => RenderKind::Align {
            alignment: *alignment,
        },
        WidgetKind::Flex { axis, .. } => RenderKind::Flex { axis: *axis },
        WidgetKind::Scroll { controller, .. } => RenderKind::Scroll {
            controller: controller.clone(),
        },
        WidgetKind::VirtualList { config } => RenderKind::VirtualList {
            config: config.clone(),
        },
        WidgetKind::Translate { controller, .. } => RenderKind::Translate {
            controller: controller.clone(),
        },
    }
}

/// Returns the pixel source crop and logical destination for a fit operation.
#[must_use]
pub fn image_fit_rects(
    source: Rect,
    bounds: Rect,
    fit: ImageFit,
    alignment: Alignment,
) -> (Rect, Rect) {
    if source.size.width == 0.
        || source.size.height == 0.
        || bounds.size.width == 0.
        || bounds.size.height == 0.
    {
        return (source, Rect::from_origin_size(bounds.origin, Size::ZERO));
    }
    if fit == ImageFit::Fill {
        return (source, bounds);
    }
    let sx = bounds.size.width / source.size.width;
    let sy = bounds.size.height / source.size.height;
    let scale = match fit {
        ImageFit::Cover => sx.max(sy),
        ImageFit::None => 1.,
        ImageFit::ScaleDown => sx.min(sy).min(1.),
        _ => sx.min(sy),
    };
    let rendered = Size::new(source.size.width * scale, source.size.height * scale);
    if fit == ImageFit::Cover {
        let crop = Size::new(
            (bounds.size.width / scale).min(source.size.width),
            (bounds.size.height / scale).min(source.size.height),
        );
        let x = source.origin.x + (source.size.width - crop.width) * (alignment.x + 1.) / 2.;
        let y = source.origin.y + (source.size.height - crop.height) * (alignment.y + 1.) / 2.;
        return (Rect::from_origin_size(Offset::new(x, y), crop), bounds);
    }
    let origin = Offset::new(
        bounds.origin.x + (bounds.size.width - rendered.width) * (alignment.x + 1.) / 2.,
        bounds.origin.y + (bounds.size.height - rendered.height) * (alignment.y + 1.) / 2.,
    );
    (source, Rect::from_origin_size(origin, rendered))
}

#[cfg(test)]
mod image_fit_tests {
    use super::*;
    #[test]
    fn contain_cover_fill_and_scale_down_are_deterministic() {
        let source = Rect::from_origin_size(Offset::ZERO, Size::new(400., 200.));
        let bounds = Rect::from_origin_size(Offset::ZERO, Size::new(200., 200.));
        let (_, contain) = image_fit_rects(source, bounds, ImageFit::Contain, Alignment::CENTER);
        assert_eq!(contain.size, Size::new(200., 100.));
        let (cover_source, cover_destination) =
            image_fit_rects(source, bounds, ImageFit::Cover, Alignment::CENTER);
        assert_eq!(cover_destination, bounds);
        assert_eq!(cover_source.size, Size::new(200., 200.));
        assert_eq!(
            image_fit_rects(source, bounds, ImageFit::Fill, Alignment::CENTER).1,
            bounds
        );
        assert_eq!(
            image_fit_rects(
                Rect::from_origin_size(Offset::ZERO, Size::new(40., 20.)),
                bounds,
                ImageFit::ScaleDown,
                Alignment::CENTER
            )
            .1
            .size,
            Size::new(40., 20.)
        );
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

fn text_field_display(value: &TextEditingValue, placeholder: &str) -> String {
    if let Some(preedit) = &value.preedit {
        let range = value.selection.range();
        let mut text = value.text.clone();
        text.replace_range(range.start..range.end, preedit);
        text
    } else if value.text.is_empty() {
        placeholder.to_owned()
    } else {
        value.text.clone()
    }
}
fn line_caret_x(line: &incular_text::TextLine, byte: usize) -> f32 {
    if byte >= line.end {
        return line.width;
    }
    let mut x = 0.;
    for glyph in line.run.glyphs.iter() {
        if glyph.cluster as usize >= byte {
            break;
        }
        x = (glyph.offset.x + glyph.advance).max(x);
    }
    x
}
fn caret_for_line_x(line: &incular_text::TextLine, x: f32) -> usize {
    if x >= line.width {
        return line.end;
    }
    let mut best = 0usize;
    for glyph in line.run.glyphs.iter() {
        let midpoint = glyph.offset.x + glyph.advance / 2.;
        if x < midpoint {
            return glyph.cluster as usize;
        }
        best = (glyph.cluster as usize).max(best);
    }
    best.max(line.start)
}
fn line_for_byte(layout: &TextLayout, byte: usize) -> usize {
    layout
        .lines
        .iter()
        .position(|line| byte <= line.end)
        .unwrap_or_else(|| layout.lines.len().saturating_sub(1))
}
fn caret_geometry(layout: &TextLayout, byte: usize) -> (f32, f32, f32) {
    let index = line_for_byte(layout, byte);
    let line = layout.lines.get(index);
    (
        line.map_or(0., |line| line_caret_x(line, byte)),
        index as f32 * layout.metrics.line_height,
        layout.metrics.line_height,
    )
}
fn selection_rects(
    layout: &TextLayout,
    selection: TextRange,
    scroll_x: f32,
    scroll_y: f32,
    top: f32,
) -> Vec<Rect> {
    layout
        .lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            let start = selection.start.max(line.start);
            let end = selection.end.min(line.end);
            (start < end
                || (line.start == line.end
                    && selection.start <= line.start
                    && selection.end >= line.end))
                .then(|| {
                    Rect::from_origin_size(
                        Offset::new(
                            line_caret_x(line, start) - scroll_x + 8.,
                            index as f32 * layout.metrics.line_height - scroll_y + top,
                        ),
                        Size::new(
                            (line_caret_x(line, end) - line_caret_x(line, start)).max(1.),
                            layout.metrics.line_height,
                        ),
                    )
                })
        })
        .collect()
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
                | PaintCommand::Image { .. }
                | PaintCommand::RRect { .. }
                | PaintCommand::Border { .. }
                | PaintCommand::FillPath { .. }
                | PaintCommand::StrokePath { .. }
                | PaintCommand::PushClip { .. }
                | PaintCommand::PushClipRRect { .. }
                | PaintCommand::PushClipPath { .. }
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
                | PaintCommand::Image { .. }
                | PaintCommand::RRect { .. }
                | PaintCommand::Border { .. }
                | PaintCommand::FillPath { .. }
                | PaintCommand::StrokePath { .. }
                | PaintCommand::PushClip { .. }
                | PaintCommand::PushClipRRect { .. }
                | PaintCommand::PushClipPath { .. }
                | PaintCommand::PopClip => {}
            }
        }
        origins
    }
    fn rrect_origins(list: &DisplayList) -> Vec<Offset> {
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
                PaintCommand::RRect { rrect, .. } => {
                    origins.push(rrect.rect.origin + *transforms.last().unwrap());
                }
                _ => {}
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
            rect_origins(&tree.paint())
                .into_iter()
                .filter(|origin| origin.x < 80.)
                .collect::<Vec<_>>(),
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
        let button_origin = rrect_origins(&list)[0];
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
            rect_origins(&tree.paint())
                .into_iter()
                .filter(|origin| origin.x < 80.)
                .collect::<Vec<_>>(),
            vec![Offset::new(15., 10.), Offset::new(15., 30.)]
        );
        assert!(scroll.jump_to(10.));
        let _ = tree.update_compositor(Instant::now());
        assert_eq!(
            rect_origins(&tree.paint())
                .into_iter()
                .filter(|origin| origin.x < 80.)
                .collect::<Vec<_>>(),
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
            rect_origins(&tree.paint())
                .into_iter()
                .filter(|origin| origin.x < 80.)
                .collect::<Vec<_>>(),
            vec![
                Offset::new(0., 0.),
                Offset::new(0., 40.),
                Offset::new(0., 80.),
            ]
        );
        assert!(controller.jump_to(50.));
        let _ = tree.update_compositor(Instant::now());
        assert_eq!(
            rect_origins(&tree.paint())
                .into_iter()
                .filter(|origin| origin.x < 80.)
                .collect::<Vec<_>>(),
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
            rect_origins(&tree.paint())
                .into_iter()
                .filter(|origin| origin.x < 80.)
                .collect::<Vec<_>>(),
            vec![
                Offset::new(0., -20.),
                Offset::new(0., 20.),
                Offset::new(0., 60.),
            ]
        );
        assert!(controller.jump_to(-1.));
        assert_eq!(controller.offset(), 0.);
    }

    #[test]
    fn fixed_extent_range_handles_edges_and_large_indices() {
        assert_eq!(fixed_extent_materialized_range(0, 40., 0., 100., 80.), 0..0);
        assert_eq!(fixed_extent_materialized_range(10, 40., 0., 100., 0.), 0..3);
        assert_eq!(
            fixed_extent_materialized_range(10, 40., 40., 100., 0.),
            1..4
        );
        assert_eq!(
            fixed_extent_materialized_range(10, 40., 123.25, 100., 0.),
            3..6
        );
        assert_eq!(
            fixed_extent_materialized_range(1_000_000, 40., 35_999_960., 600., 240.),
            899_993..900_020
        );
    }

    #[test]
    fn million_item_list_materializes_only_viewport_and_cache() {
        let controller = ScrollController::new();
        let calls = Rc::new(Cell::new(0));
        let observed = calls.clone();
        let mut tree = WidgetTree::new();
        tree.mount(VirtualList::fixed_extent_with_controller(
            1_000_000,
            40.,
            controller.clone(),
            move |index| {
                observed.set(observed.get() + 1);
                Widget::box_(Size::new(80., 40.), Color::rgba(index as u8, 0, 0, 255))
            },
        ))
        .unwrap();
        tree.layout(Constraints::tight(Size::new(100., 600.)));
        let initial = tree.virtual_list_diagnostics().unwrap();
        assert!(initial.materialized_item_count < 100);
        assert_eq!(calls.get(), initial.materialized_item_count);
        assert!(controller.jump_to(900_000. * 40.));
        tree.layout(Constraints::tight(Size::new(100., 600.)));
        let jumped = tree.virtual_list_diagnostics().unwrap();
        assert!(jumped.materialized_range.contains(&900_000));
        assert!(jumped.materialized_item_count < 100);
        // Direct arithmetic builds only the destination cache range, never
        // every preceding logical item.
        assert!(calls.get() < 200);
        assert!(jumped.element_count < 100);
        assert!(jumped.render_object_count < 100);
    }

    #[test]
    fn virtual_children_retain_identity_inside_the_cache_and_release_outside() {
        let controller = ScrollController::new();
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(VirtualList::fixed_extent_with_controller(
                10_000,
                40.,
                controller.clone(),
                |i| Widget::box_(Size::new(80., 40.), Color::rgba(i as u8, 0, 0, 255)),
            ))
            .unwrap();
        let constraints = Constraints::tight(Size::new(100., 100.));
        tree.layout(constraints);
        let before = tree.children(root).unwrap().to_vec();
        assert!(controller.jump_to(3.));
        tree.layout(constraints);
        assert_eq!(tree.children(root).unwrap(), before.as_slice());
        assert!(controller.jump_to(400.));
        tree.layout(constraints);
        assert!(before.iter().any(|id| !tree.element_exists(*id)));
        assert!(before.iter().any(|id| tree.element_exists(*id)));
        let after = tree.virtual_list_diagnostics().unwrap();
        assert!(after.element_count < 30);
        assert!(tree.diagnostics().items_unmounted > 0);
    }

    #[test]
    fn virtual_count_changes_retain_valid_rows_and_release_invalid_ones() {
        let controller = ScrollController::new();
        let constraints = Constraints::tight(Size::new(100., 100.));
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(VirtualList::fixed_extent_with_controller(
                100,
                40.,
                controller.clone(),
                |_| Widget::box_(Size::new(80., 40.), Color::WHITE),
            ))
            .unwrap();
        tree.layout(constraints);
        let retained = tree.children(root).unwrap()[0];
        tree.update(
            root,
            VirtualList::fixed_extent_with_controller(80, 40., controller.clone(), |_| {
                Widget::box_(Size::new(80., 40.), Color::WHITE)
            }),
        )
        .unwrap();
        tree.layout(constraints);
        assert!(tree.element_exists(retained));
        assert!(controller.jump_to(1_000.));
        tree.layout(constraints);
        tree.update(
            root,
            VirtualList::fixed_extent_with_controller(0, 40., controller.clone(), |_| {
                Widget::box_(Size::new(80., 40.), Color::WHITE)
            }),
        )
        .unwrap();
        tree.layout(constraints);
        assert_eq!(controller.offset(), 0.);
        assert!(tree.children(root).unwrap().is_empty());
    }

    #[test]
    fn editing_uses_graphemes_and_keeps_utf8_boundaries() {
        let controller = TextEditingController::with_text("a👩‍💻é");
        controller.move_end(false);
        controller.backspace();
        assert_eq!(controller.text(), "a👩‍💻");
        controller.backspace();
        assert_eq!(controller.text(), "a");
        controller.set_selection(TextSelection { base: 1, extent: 1 });
        controller.insert("नमस्ते");
        assert_eq!(controller.text(), "aनमस्ते");
        let value = controller.value();
        assert!(controller.text().is_char_boundary(value.selection.extent));
    }

    #[test]
    fn ime_preedit_does_not_mutate_committed_text() {
        let controller = TextEditingController::with_text("hello");
        controller.set_preedit("世界", Some(TextRange::new(0, 3)));
        assert_eq!(controller.text(), "hello");
        assert_eq!(controller.value().preedit.as_deref(), Some("世界"));
        controller.commit_preedit("世界");
        assert_eq!(controller.text(), "hello世界");
        assert!(controller.value().preedit.is_none());
    }

    #[test]
    fn editing_newlines_and_boundaries_are_grapheme_safe() {
        let controller = TextEditingController::with_text("hello\nworld");
        controller.set_selection(TextSelection::collapsed(6));
        controller.backspace();
        assert_eq!(controller.text(), "helloworld");
        controller.set_selection(TextSelection { base: 2, extent: 5 });
        controller.insert("\n");
        assert_eq!(controller.text(), "he\nworld");
        controller.set_selection(TextSelection::collapsed(2));
        controller.delete();
        assert_eq!(controller.text(), "heworld");
    }

    #[test]
    fn scrollbar_geometry_and_drag_share_the_controller() {
        let controller = ScrollController::new();
        let mut tree = WidgetTree::new();
        tree.mount(Widget::scroll_view(
            controller.clone(),
            Widget::fixed_box(Size::new(100., 1_000.), Color::WHITE),
        ))
        .unwrap();
        tree.layout(Constraints::tight(Size::new(100., 100.)));
        let geometry = tree.scrollbar_diagnostics().pop().unwrap();
        assert!(geometry.visible);
        assert_eq!(geometry.thumb.size.height, 24.);
        assert_eq!(geometry.thumb.origin.y, 0.);
        assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Down, Offset::new(95., 10.)));
        assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Move, Offset::new(95., 60.)));
        assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Up, Offset::new(95., 60.)));
        assert!(controller.offset() > 0.);
        assert!(controller.offset() <= controller.max_offset());
        assert!(controller.scroll_by(0.25));
        assert!(controller.offset().fract() > 0.);
    }

    #[test]
    fn scrollbar_geometry_round_trips_offsets_and_thumb_tops() {
        let controller = ScrollController::new();
        let mut tree = WidgetTree::new();
        tree.mount(Widget::scroll_view(
            controller.clone(),
            Widget::fixed_box(Size::new(100., 4_000.), Color::WHITE),
        ))
        .unwrap();
        tree.layout(Constraints::tight(Size::new(100., 600.)));
        let geometry = tree.scrollbar_diagnostics().pop().unwrap();
        assert_eq!(geometry.thumb.size.height, 90.);
        assert_eq!(geometry.thumb_travel, 510.);
        for fraction in [0., 0.1, 0.25, 0.5, 0.75, 0.9, 1.] {
            let offset = fraction * geometry.max_scroll_extent;
            let thumb_top = geometry.thumb_top_for_offset(offset);
            assert!((geometry.offset_for_thumb_top(thumb_top) - offset).abs() < 0.01);
            assert!(
                (geometry.thumb_top_for_offset(geometry.offset_for_thumb_top(thumb_top))
                    - thumb_top)
                    .abs()
                    < 0.01
            );
        }
    }

    #[test]
    fn virtual_list_small_thumb_drag_is_continuous_and_reversible() {
        let controller = ScrollController::new();
        let mut tree = WidgetTree::new();
        tree.mount(VirtualList::fixed_extent_with_controller(
            100,
            40.,
            controller.clone(),
            |_| Widget::fixed_box(Size::new(100., 40.), Color::WHITE),
        ))
        .unwrap();
        tree.layout(Constraints::tight(Size::new(100., 600.)));
        let geometry = tree.scrollbar_diagnostics().pop().unwrap();
        assert_eq!(controller.max_offset(), 3_400.);
        assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Down, Offset::new(95., 5.)));
        for y in [10., 15., 25.] {
            assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Move, Offset::new(95., y)));
        }
        let down = controller.offset();
        assert!(down > 0. && down < controller.max_offset() * 0.1);
        assert!((down - geometry.offset_for_thumb_top(20.)).abs() < 0.01);
        assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Move, Offset::new(95., 10.)));
        assert!(controller.offset() < down);
        assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Up, Offset::new(95., 10.)));
        assert!(!tree.scrollbar_drag_diagnostics().active);
    }

    #[test]
    fn virtual_list_minimum_thumb_drag_uses_actual_travel_and_stays_bounded() {
        let controller = ScrollController::new();
        let mut tree = WidgetTree::new();
        tree.mount(VirtualList::fixed_extent_with_controller(
            1_000_000,
            40.,
            controller.clone(),
            |_| Widget::fixed_box(Size::new(100., 40.), Color::WHITE),
        ))
        .unwrap();
        tree.layout(Constraints::tight(Size::new(100., 600.)));
        let geometry = tree.scrollbar_diagnostics().pop().unwrap();
        assert_eq!(geometry.thumb.size.height, 24.);
        assert_eq!(geometry.thumb_travel, 576.);
        assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Down, Offset::new(95., 12.)));
        assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Move, Offset::new(95., 13.)));
        let one_pixel = controller.offset();
        assert!((one_pixel - geometry.max_scroll_extent / geometry.thumb_travel).abs() < 0.1);
        assert!(one_pixel < controller.max_offset());
        assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Move, Offset::new(95., 300.)));
        let middle = controller.offset();
        assert!(middle > controller.max_offset() * 0.45 && middle < controller.max_offset() * 0.55);
        tree.layout(Constraints::tight(Size::new(100., 600.)));
        let middle_rows = tree.virtual_list_diagnostics().unwrap();
        assert!(middle_rows.materialized_range.contains(&(500_000usize)));
        assert!(middle_rows.materialized_item_count < 100);
        assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Move, Offset::new(95., 588.)));
        assert_eq!(controller.offset(), controller.max_offset());
        assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Move, Offset::new(95., 300.)));
        assert!(controller.offset() < controller.max_offset());
        assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Up, Offset::new(95., 300.)));
    }

    #[test]
    fn scrollbar_does_not_scroll_when_thumb_has_no_travel() {
        let controller = ScrollController::new();
        let mut tree = WidgetTree::new();
        tree.mount(Widget::scroll_view(
            controller.clone(),
            Widget::fixed_box(Size::new(100., 1_000.), Color::WHITE),
        ))
        .unwrap();
        tree.layout(Constraints::tight(Size::new(100., 20.)));
        let geometry = tree.scrollbar_diagnostics().pop().unwrap();
        assert_eq!(geometry.thumb_travel, 0.);
        assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Down, Offset::new(95., 10.)));
        assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Move, Offset::new(95., 100.)));
        assert_eq!(controller.offset(), 0.);
    }

    #[test]
    fn scrollbar_stays_synchronized_after_wheel_and_programmatic_offset_changes() {
        let controller = ScrollController::new();
        let mut tree = WidgetTree::new();
        tree.mount(Widget::scroll_view(
            controller.clone(),
            Widget::fixed_box(Size::new(100., 4_000.), Color::WHITE),
        ))
        .unwrap();
        tree.layout(Constraints::tight(Size::new(100., 600.)));
        assert!(tree.scroll_at(Offset::new(50., 50.), Offset::new(0., 600.)));
        let after_wheel = tree.scrollbar_diagnostics().pop().unwrap();
        assert!(after_wheel.thumb.origin.y > after_wheel.track.origin.y);
        assert!(controller.jump_to(controller.max_offset() * 0.5));
        let middle = tree.scrollbar_diagnostics().pop().unwrap();
        let grab_y = middle.thumb.origin.y + middle.thumb.size.height * 0.5;
        assert!(tree.scrollbar_pointer(incular_core::PointerPhase::Down, Offset::new(95., grab_y)));
        assert!(tree.scrollbar_pointer(
            incular_core::PointerPhase::Move,
            Offset::new(95., grab_y - 10.)
        ));
        assert!(controller.offset() < controller.max_offset() * 0.5);
        let events = tree.diagnostics().scroll_events;
        assert!(tree.scrollbar_pointer(
            incular_core::PointerPhase::Move,
            Offset::new(95., grab_y - 10.)
        ));
        assert_eq!(tree.diagnostics().scroll_events, events);
    }

    #[test]
    fn textarea_uses_shaped_lines_for_pointer_and_vertical_navigation() {
        let controller = TextEditingController::with_text("abcdef\nxy\n123456");
        let mut tree = WidgetTree::new();
        let root = tree
            .mount(
                TextArea::new(controller.clone())
                    .size(Size::new(120., 100.))
                    .into(),
            )
            .unwrap();
        tree.layout(Constraints::tight(Size::new(120., 100.)));
        tree.text_field_set_caret(root, Offset::new(30., 10.), false, Instant::now());
        let initial = controller.value().selection.extent;
        assert!(initial > 0 && initial <= 6);
        assert!(tree.text_field_move_vertical(root, true, true));
        let selection = controller.value().selection;
        assert_eq!(selection.base, initial);
        assert!(selection.extent > 6 && selection.extent <= 9);
        assert!(tree.text_field_move_line_edge(root, true, false));
        assert_eq!(controller.value().selection.extent, 9);
        tree.text_field_set_caret(root, Offset::new(5., 55.), false, Instant::now());
        assert!(controller.value().selection.extent >= 10);
    }
}
