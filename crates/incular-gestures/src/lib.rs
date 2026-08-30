//! Platform-neutral gesture recognition and interaction primitives.
//!
//! This crate deliberately contains no widget-tree, runtime, or native-window
//! types. Retained widget regions and platform adapters share it as an input
//! boundary.

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::{Rc, Weak},
    time::{Duration, Instant},
};

use incular_core::{Code, KeyboardEvent, KeyboardKey, Modifiers, Offset, PointerPhase, Rect};

/// Identifies one pointer stream within a native window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GestureArenaKey {
    pub window: u64,
    pub pointer: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GestureDisposition {
    Pending,
    Accepted,
    Rejected,
    Cancelled,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GestureArenaMember(u64);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GestureArenaEntry {
    pub member: GestureArenaMember,
    pub disposition: GestureDisposition,
}
#[derive(Default)]
pub struct GestureArena {
    next: u64,
    streams: HashMap<GestureArenaKey, Vec<(GestureArenaMember, bool, GestureDisposition)>>,
}
impl GestureArena {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    pub fn add(&mut self, key: GestureArenaKey, simultaneous: bool) -> GestureArenaMember {
        self.next += 1;
        let member = GestureArenaMember(self.next);
        self.streams.entry(key).or_default().push((
            member,
            simultaneous,
            GestureDisposition::Pending,
        ));
        member
    }
    pub fn accept(
        &mut self,
        key: GestureArenaKey,
        member: GestureArenaMember,
    ) -> Vec<GestureArenaEntry> {
        let Some(entries) = self.streams.get_mut(&key) else {
            return Vec::new();
        };
        let Some(index) = entries.iter().position(|entry| entry.0 == member) else {
            return Vec::new();
        };
        if entries[index].2 != GestureDisposition::Pending {
            return entries
                .iter()
                .map(|entry| GestureArenaEntry {
                    member: entry.0,
                    disposition: entry.2,
                })
                .collect();
        }
        let simultaneous = entries[index].1;
        let blocked = entries
            .iter()
            .any(|entry| entry.2 == GestureDisposition::Accepted && !(simultaneous && entry.1));
        if blocked {
            entries[index].2 = GestureDisposition::Rejected;
        } else {
            entries[index].2 = GestureDisposition::Accepted;
            for entry in entries.iter_mut() {
                if entry.2 == GestureDisposition::Pending && !(simultaneous && entry.1) {
                    entry.2 = GestureDisposition::Rejected;
                }
            }
        }
        entries
            .iter()
            .map(|e| GestureArenaEntry {
                member: e.0,
                disposition: e.2,
            })
            .collect()
    }
    pub fn reject(&mut self, key: GestureArenaKey, member: GestureArenaMember) {
        if let Some(entries) = self.streams.get_mut(&key) {
            if let Some(entry) = entries.iter_mut().find(|e| e.0 == member) {
                entry.2 = GestureDisposition::Rejected;
            }
        }
    }
    pub fn cancel(&mut self, key: GestureArenaKey) -> Vec<GestureArenaEntry> {
        self.streams
            .remove(&key)
            .unwrap_or_default()
            .into_iter()
            .map(|(member, _, _)| GestureArenaEntry {
                member,
                disposition: GestureDisposition::Cancelled,
            })
            .collect()
    }
    #[must_use]
    pub fn entries(&self, key: GestureArenaKey) -> Vec<GestureArenaEntry> {
        self.streams
            .get(&key)
            .map(|v| {
                v.iter()
                    .map(|e| GestureArenaEntry {
                        member: e.0,
                        disposition: e.2,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// A platform-neutral pointer event.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointerEvent {
    pub pointer: u64,
    pub position: Offset,
    pub phase: PointerPhase,
    pub time: Instant,
}

/// The physical device category generating pointer events.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PointerDeviceKind {
    #[default]
    Mouse,
    Touch,
    Stylus,
    InvertedStylus,
    Trackpad,
    Unknown,
}

/// Physical pointer velocity in logical pixels per second.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Velocity {
    pub pixels_per_second: Offset,
}

impl Velocity {
    pub const ZERO: Self = Self {
        pixels_per_second: Offset::ZERO,
    };

    #[must_use]
    pub const fn new(pixels_per_second: Offset) -> Self {
        Self { pixels_per_second }
    }
}

/// Position and timing metadata for pointer tap-down events.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TapDownDetails {
    pub global_position: Offset,
    pub local_position: Offset,
    pub kind: PointerDeviceKind,
}

/// Position and timing metadata for pointer tap-up events.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TapUpDetails {
    pub global_position: Offset,
    pub local_position: Offset,
    pub kind: PointerDeviceKind,
}

/// Metadata for drag start events.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DragStartDetails {
    pub global_position: Offset,
    pub local_position: Offset,
}

/// Metadata for drag down events.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DragDownDetails {
    pub global_position: Offset,
    pub local_position: Offset,
}

/// Metadata for long-press start events.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LongPressStartDetails {
    pub global_position: Offset,
    pub local_position: Offset,
}

/// Metadata for long-press movement update events.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LongPressMoveUpdateDetails {
    pub global_position: Offset,
    pub local_position: Offset,
    pub offset_from_origin: Offset,
    pub local_offset_from_origin: Offset,
}

/// Metadata for long-press end events.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LongPressEndDetails {
    pub global_position: Offset,
    pub local_position: Offset,
    pub velocity: Velocity,
}

/// Callbacks understood by [`PointerGestureRecognizer`].
#[derive(Clone, Default)]
pub struct GestureCallbacks {
    pub on_tap: Option<Rc<dyn Fn()>>,
    pub on_tap_down: Option<Rc<dyn Fn(TapDownDetails)>>,
    pub on_tap_up: Option<Rc<dyn Fn(TapUpDetails)>>,
    pub on_tap_cancel: Option<Rc<dyn Fn()>>,
    pub on_double_tap: Option<Rc<dyn Fn()>>,
    pub on_double_tap_down: Option<Rc<dyn Fn(TapDownDetails)>>,
    pub on_double_tap_cancel: Option<Rc<dyn Fn()>>,
    pub on_long_press: Option<Rc<dyn Fn()>>,
    pub on_long_press_start: Option<Rc<dyn Fn(LongPressStartDetails)>>,
    pub on_long_press_move_update: Option<Rc<dyn Fn(LongPressMoveUpdateDetails)>>,
    pub on_long_press_up: Option<Rc<dyn Fn()>>,
    pub on_long_press_end: Option<Rc<dyn Fn(LongPressEndDetails)>>,
    pub on_pan_down: Option<Rc<dyn Fn(DragDownDetails)>>,
    pub on_pan_start: Option<Rc<dyn Fn(DragStartDetails)>>,
    pub on_pan_update: Option<Rc<dyn Fn(Offset)>>,
    /// Called once when a pan ends normally. The callback receives the total
    /// displacement, velocity, and cancellation state for the pointer stream.
    pub on_pan_end: Option<Rc<dyn Fn(DragEndDetails)>>,
    pub on_pan_cancel: Option<Rc<dyn Fn()>>,
    pub on_horizontal_drag_down: Option<Rc<dyn Fn(DragDownDetails)>>,
    pub on_horizontal_drag_start: Option<Rc<dyn Fn(DragStartDetails)>>,
    /// Receives a pan whose first slop-exceeding movement was horizontal.
    /// It competes with vertical drags in a retained `GestureDetector`.
    pub on_horizontal_drag_update: Option<Rc<dyn Fn(Offset)>>,
    /// Called once when a horizontal drag ends normally.
    pub on_horizontal_drag_end: Option<Rc<dyn Fn(DragEndDetails)>>,
    pub on_horizontal_drag_cancel: Option<Rc<dyn Fn()>>,
    pub on_vertical_drag_down: Option<Rc<dyn Fn(DragDownDetails)>>,
    pub on_vertical_drag_start: Option<Rc<dyn Fn(DragStartDetails)>>,
    /// Receives a pan whose first slop-exceeding movement was vertical.
    /// It competes with horizontal drags in a retained `GestureDetector`.
    pub on_vertical_drag_update: Option<Rc<dyn Fn(Offset)>>,
    /// Called once when a vertical drag ends normally.
    pub on_vertical_drag_end: Option<Rc<dyn Fn(DragEndDetails)>>,
    pub on_vertical_drag_cancel: Option<Rc<dyn Fn()>>,
    pub on_scale_start: Option<Rc<dyn Fn(ScaleStartDetails)>>,
    /// Receives the active focal point and relative distance for a retained
    /// multi-pointer region. Single-pointer recognizers ignore this callback.
    pub on_scale_update: Option<Rc<dyn Fn(ScaleUpdateDetails)>>,
    pub on_scale_end: Option<Rc<dyn Fn(ScaleEndDetails)>>,
    /// Called once for a retained pointer sequence when this region loses its
    /// arena claim or the platform cancels the sequence.
    pub on_cancel: Option<Rc<dyn Fn()>>,
    /// Receives a keyboard event after the retained runtime resolves the
    /// focused element. Returning `true` consumes the event and prevents
    /// outer keyboard scopes and text editing from seeing it.
    pub on_key: Option<Rc<dyn Fn(KeyboardEvent) -> bool>>,
    /// Receives key-down events that were not consumed by [`Self::on_key`].
    pub on_key_down: Option<Rc<dyn Fn(KeyboardEvent)>>,
    /// Receives repeated key-down events in addition to [`Self::on_key_down`].
    pub on_key_repeat: Option<Rc<dyn Fn(KeyboardEvent)>>,
    /// Receives key-up events that were not consumed by [`Self::on_key`].
    pub on_key_up: Option<Rc<dyn Fn(KeyboardEvent)>>,
    /// An optional typed shortcut dispatcher. Widgets owns the closure so
    /// this crate remains independent from the retained runtime.
    pub on_shortcut: Option<Rc<dyn Fn(KeyboardEvent) -> bool>>,
    /// Focus metadata associated with a keyboard listener. Keeping this on the
    /// platform-neutral callback value lets the retained tree route keyboard
    /// input without introducing a runtime dependency into gestures.
    pub focus_node: Option<FocusNode>,
    pub autofocus: bool,
    pub include_semantics: bool,
}

impl GestureCallbacks {
    /// Whether this callback set contributes an exclusive single-pointer
    /// recognizer to a gesture arena. Scale is intentionally separate so a
    /// pending pinch can coexist until one recognizer claims the stream.
    #[must_use]
    pub fn has_pointer_recognizer(&self) -> bool {
        self.on_tap.is_some()
            || self.on_tap_down.is_some()
            || self.on_tap_up.is_some()
            || self.on_tap_cancel.is_some()
            || self.on_double_tap.is_some()
            || self.on_double_tap_down.is_some()
            || self.on_double_tap_cancel.is_some()
            || self.on_long_press.is_some()
            || self.on_long_press_start.is_some()
            || self.on_long_press_move_update.is_some()
            || self.on_long_press_up.is_some()
            || self.on_long_press_end.is_some()
            || self.on_pan_down.is_some()
            || self.on_pan_start.is_some()
            || self.on_pan_update.is_some()
            || self.on_pan_end.is_some()
            || self.on_pan_cancel.is_some()
            || self.on_horizontal_drag_down.is_some()
            || self.on_horizontal_drag_start.is_some()
            || self.on_horizontal_drag_update.is_some()
            || self.on_horizontal_drag_end.is_some()
            || self.on_horizontal_drag_cancel.is_some()
            || self.on_vertical_drag_down.is_some()
            || self.on_vertical_drag_start.is_some()
            || self.on_vertical_drag_update.is_some()
            || self.on_vertical_drag_end.is_some()
            || self.on_vertical_drag_cancel.is_some()
    }

    /// Whether this callback set carries retained keyboard/focus behavior.
    #[must_use]
    pub fn has_keyboard_listener(&self) -> bool {
        self.on_key.is_some()
            || self.on_key_down.is_some()
            || self.on_key_repeat.is_some()
            || self.on_key_up.is_some()
            || self.on_shortcut.is_some()
            || self.focus_node.is_some()
    }

    /// Dispatches one keyboard event through the listener callbacks. The
    /// shortcut dispatcher is nearest-scope behavior and gets first refusal;
    /// the generic callback then gets the next opportunity before the
    /// phase-specific callbacks run.
    #[must_use]
    pub fn handle_keyboard(&self, event: KeyboardEvent) -> bool {
        if self
            .on_shortcut
            .as_ref()
            .is_some_and(|callback| callback(event.clone()))
        {
            return true;
        }
        if self
            .on_key
            .as_ref()
            .is_some_and(|callback| callback(event.clone()))
        {
            return true;
        }
        if event.state.is_down() {
            let mut handled = false;
            if let Some(callback) = &self.on_key_down {
                callback(event.clone());
                handled = true;
            }
            if event.repeat {
                if let Some(callback) = &self.on_key_repeat {
                    callback(event);
                    handled = true;
                }
            }
            handled
        } else if let Some(callback) = &self.on_key_up {
            callback(event);
            true
        } else {
            false
        }
    }
}

/// The callback action selected after a recognizer has claimed its arena.
/// This is public so retained adapters can defer callbacks until arbitration
/// has completed while [`GestureDetector::handle`] remains source-compatible.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GestureAction {
    Tap,
    DoubleTap,
    LongPress,
    Pan(Offset),
    PanEnd(DragEndDetails),
    HorizontalDrag(Offset),
    HorizontalDragEnd(DragEndDetails),
    VerticalDrag(Offset),
    VerticalDragEnd(DragEndDetails),
}

/// A recognizer's claim for one pointer event.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GestureDecision {
    Pending,
    Accept(GestureAction),
    Reject,
    Cancelled,
}

/// Metadata for scale start events.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScaleStartDetails {
    pub focal_point: Offset,
    pub pointer_count: usize,
}

/// The current geometry of a two-pointer scale interaction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScaleUpdateDetails {
    pub focal_point: Offset,
    pub scale: f32,
    pub pointer_count: usize,
}

/// Metadata for scale end events.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScaleEndDetails {
    pub pointer_count: usize,
}

impl ScaleUpdateDetails {
    #[must_use]
    pub const fn new(focal_point: Offset, scale: f32) -> Self {
        Self {
            focal_point,
            scale,
            pointer_count: 2,
        }
    }
}

/// A platform-neutral two-pointer scale recognizer.
pub struct ScaleGestureDetector {
    pointers: HashMap<u64, Offset>,
    initial_distance: Option<f32>,
    on_update: Option<Rc<dyn Fn(ScaleUpdateDetails)>>,
    on_start: Option<Rc<dyn Fn(ScaleStartDetails)>>,
    on_end: Option<Rc<dyn Fn(ScaleEndDetails)>>,
    started: bool,
}
impl ScaleGestureDetector {
    #[must_use]
    pub fn new(on_update: impl Fn(ScaleUpdateDetails) + 'static) -> Self {
        Self::with_callbacks(Some(Rc::new(on_update)), None, None)
    }

    /// Creates a scale recognizer with the complete Flutter-style callback
    /// lifecycle. `new` remains the compact update-only constructor.
    #[must_use]
    pub fn with_callbacks(
        on_update: Option<Rc<dyn Fn(ScaleUpdateDetails)>>,
        on_start: Option<Rc<dyn Fn(ScaleStartDetails)>>,
        on_end: Option<Rc<dyn Fn(ScaleEndDetails)>>,
    ) -> Self {
        Self {
            pointers: HashMap::new(),
            initial_distance: None,
            on_update,
            on_start,
            on_end,
            started: false,
        }
    }
    pub fn handle(&mut self, event: PointerEvent) -> bool {
        let handled = match event.phase {
            PointerPhase::Down => true,
            PointerPhase::Move => self.pointers.contains_key(&event.pointer),
            PointerPhase::Up | PointerPhase::Cancel => self.pointers.contains_key(&event.pointer),
        };
        if let Some(details) = self.observe(event) {
            self.dispatch(details);
        }
        handled
    }
    /// Updates internal contact geometry without invoking the user callback.
    /// Retained adapters use this to wait until the scale recognizer wins its
    /// arena before exposing any scale updates.
    pub fn observe(&mut self, event: PointerEvent) -> Option<ScaleUpdateDetails> {
        match event.phase {
            PointerPhase::Down => {
                self.pointers.insert(event.pointer, event.position);
                self.reset_initial_distance();
                None
            }
            PointerPhase::Move => {
                let position = self.pointers.get_mut(&event.pointer)?;
                *position = event.position;
                let (first, second) = self.first_two()?;
                let distance = distance(first, second);
                let initial = self
                    .initial_distance
                    .get_or_insert(distance.max(f32::EPSILON));
                let details = ScaleUpdateDetails::new(
                    Offset::new((first.x + second.x) * 0.5, (first.y + second.y) * 0.5),
                    distance / *initial,
                );
                if !self.started {
                    self.started = true;
                    if let Some(callback) = &self.on_start {
                        callback(ScaleStartDetails {
                            focal_point: details.focal_point,
                            pointer_count: details.pointer_count,
                        });
                    }
                }
                Some(details)
            }
            PointerPhase::Up | PointerPhase::Cancel => {
                if self.started {
                    self.started = false;
                    if let Some(callback) = &self.on_end {
                        callback(ScaleEndDetails {
                            pointer_count: self.pointers.len(),
                        });
                    }
                }
                self.pointers.remove(&event.pointer);
                self.reset_initial_distance();
                None
            }
        }
    }
    /// Delivers a previously observed update after the caller has resolved
    /// arbitration for the relevant pointer stream.
    pub fn dispatch(&self, details: ScaleUpdateDetails) {
        if let Some(callback) = &self.on_update {
            callback(details);
        }
    }
    fn first_two(&self) -> Option<(Offset, Offset)> {
        let mut pointers = self.pointers.values().copied();
        Some((pointers.next()?, pointers.next()?))
    }
    fn reset_initial_distance(&mut self) {
        self.initial_distance = self
            .first_two()
            .map(|(first, second)| distance(first, second));
    }
}

fn distance(first: Offset, second: Offset) -> f32 {
    (first.x - second.x).hypot(first.y - second.y)
}

/// Pointer hover callbacks for desktop-capable input sources.
#[derive(Clone, Default)]
pub struct MouseRegion {
    pub on_enter: Option<Rc<dyn Fn(Offset)>>,
    pub on_exit: Option<Rc<dyn Fn(Offset)>>,
    pub on_hover: Option<Rc<dyn Fn(Offset)>>,
    inside: Rc<Cell<bool>>,
}
impl MouseRegion {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    pub fn enter(&self, position: Offset) {
        if !self.inside.replace(true) {
            if let Some(callback) = &self.on_enter {
                callback(position);
            }
        }
    }
    pub fn hover(&self, position: Offset) {
        if self.inside.get() {
            if let Some(callback) = &self.on_hover {
                callback(position);
            }
        }
    }
    pub fn exit(&self, position: Offset) {
        if self.inside.replace(false) {
            if let Some(callback) = &self.on_exit {
                callback(position);
            }
        }
    }
    #[must_use]
    pub fn is_inside(&self) -> bool {
        self.inside.get()
    }
}

/// Shared focus handle usable by controls that are rebuilt often.
///
/// For exclusive traversal use a [`FocusManager`]; calling
/// [`Self::request_focus`] directly is retained for standalone controls.
#[derive(Clone)]
pub struct FocusNode {
    state: Rc<FocusState>,
}
impl PartialEq for FocusNode {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.state, &other.state)
    }
}
impl Eq for FocusNode {}
impl std::fmt::Debug for FocusNode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FocusNode")
            .field("has_focus", &self.has_focus())
            .field("can_request_focus", &self.can_request_focus())
            .finish()
    }
}
struct FocusState {
    focused: Cell<bool>,
    can_request_focus: Cell<bool>,
    skip_traversal: Cell<bool>,
    descendants_are_focusable: Cell<bool>,
    descendants_are_traversable: Cell<bool>,
    rect: Cell<Rect>,
    traversal_order: Cell<Option<f64>>,
}

impl Default for FocusNode {
    fn default() -> Self {
        Self {
            state: Rc::new(FocusState {
                focused: Cell::new(false),
                can_request_focus: Cell::new(true),
                skip_traversal: Cell::new(false),
                descendants_are_focusable: Cell::new(true),
                descendants_are_traversable: Cell::new(true),
                rect: Cell::new(Rect::default()),
                traversal_order: Cell::new(None),
            }),
        }
    }
}
impl FocusNode {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    pub fn request_focus(&self) {
        if self.can_request_focus() {
            self.state.focused.set(true);
        }
    }
    pub fn unfocus(&self) {
        self.state.focused.set(false);
    }
    #[must_use]
    pub fn has_focus(&self) -> bool {
        self.state.focused.get()
    }
    #[must_use]
    pub fn has_primary_focus(&self) -> bool {
        self.state.focused.get()
    }
    /// Excludes or includes this node in manager-driven traversal.
    pub fn set_can_request_focus(&self, can_request_focus: bool) {
        self.state.can_request_focus.set(can_request_focus);
        if !can_request_focus {
            self.unfocus();
        }
    }
    #[must_use]
    pub fn can_request_focus(&self) -> bool {
        self.state.can_request_focus.get()
    }
    pub fn set_skip_traversal(&self, skip: bool) {
        self.state.skip_traversal.set(skip);
    }
    #[must_use]
    pub fn skip_traversal(&self) -> bool {
        self.state.skip_traversal.get()
    }
    pub fn set_descendants_are_focusable(&self, focusable: bool) {
        self.state.descendants_are_focusable.set(focusable);
    }
    #[must_use]
    pub fn descendants_are_focusable(&self) -> bool {
        self.state.descendants_are_focusable.get()
    }
    pub fn set_descendants_are_traversable(&self, traversable: bool) {
        self.state.descendants_are_traversable.set(traversable);
    }
    #[must_use]
    pub fn descendants_are_traversable(&self) -> bool {
        self.state.descendants_are_traversable.get()
    }

    /// Supplies the node's current layout bounds to geometry-aware traversal.
    /// Bounds are logical pixels in the same coordinate space as the widget
    /// tree. A node that never receives bounds remains in registration order.
    pub fn set_rect(&self, rect: Rect) {
        self.state.rect.set(rect);
    }

    #[must_use]
    pub fn rect(&self) -> Rect {
        self.state.rect.get()
    }

    /// Sets the explicit numeric order used by [`OrderedTraversalPolicy`].
    /// Equal or absent orders preserve widget registration order.
    pub fn set_traversal_order(&self, order: Option<f64>) {
        self.state
            .traversal_order
            .set(order.filter(|value| value.is_finite()));
    }

    #[must_use]
    pub fn traversal_order(&self) -> Option<f64> {
        self.state.traversal_order.get()
    }
}

/// Strategy for navigating keyboard focus through a collection of focus nodes.
pub trait FocusTraversalPolicy {
    fn find_first_focus(&self, nodes: &[FocusNode]) -> Option<FocusNode>;
    fn find_last_focus(&self, nodes: &[FocusNode]) -> Option<FocusNode>;
    fn next(&self, current: &FocusNode, nodes: &[FocusNode]) -> Option<FocusNode>;
    fn previous(&self, current: &FocusNode, nodes: &[FocusNode]) -> Option<FocusNode>;
}

/// Built-in traversal policies understood by retained widget groups.
///
/// The policy is intentionally a small value rather than a trait object so it
/// can cross rebuild boundaries and be stored in a widget descriptor. Custom
/// consumers can still implement [`FocusTraversalPolicy`] directly.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FocusTraversalPolicyKind {
    /// Follow the retained widget/registration order.
    #[default]
    WidgetOrder,
    /// Sort by the top edge and then the leading edge of each node.
    ReadingOrder,
    /// Sort by [`FocusNode::traversal_order`], with registration order as the
    /// stable tie breaker.
    Ordered,
}

fn traversable(nodes: &[FocusNode]) -> Vec<FocusNode> {
    nodes
        .iter()
        .filter(|node| node.can_request_focus() && !node.skip_traversal())
        .cloned()
        .collect()
}

fn reading_order(mut nodes: Vec<FocusNode>) -> Vec<FocusNode> {
    // Keep rows together even when controls have slightly different heights.
    // The tolerance is deliberately relative to the row height, which makes
    // this work for both compact desktop controls and large touch targets.
    nodes.sort_by(|left, right| {
        let left_rect = left.rect();
        let right_rect = right.rect();
        let row_tolerance = left_rect
            .size
            .height
            .max(right_rect.size.height)
            .mul_add(0.5, 1.0);
        let vertical = left_rect.origin.y - right_rect.origin.y;
        if vertical.abs() > row_tolerance {
            vertical
                .partial_cmp(&0.0)
                .unwrap_or(std::cmp::Ordering::Equal)
        } else {
            left_rect
                .origin
                .x
                .partial_cmp(&right_rect.origin.x)
                .unwrap_or(std::cmp::Ordering::Equal)
        }
    });
    nodes
}

fn explicit_order(mut nodes: Vec<FocusNode>) -> Vec<FocusNode> {
    nodes.sort_by(|left, right| {
        left.traversal_order()
            .unwrap_or(f64::INFINITY)
            .partial_cmp(&right.traversal_order().unwrap_or(f64::INFINITY))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    nodes
}

fn policy_nodes(policy: FocusTraversalPolicyKind, nodes: &[FocusNode]) -> Vec<FocusNode> {
    let nodes = traversable(nodes);
    match policy {
        FocusTraversalPolicyKind::WidgetOrder => nodes,
        FocusTraversalPolicyKind::ReadingOrder => reading_order(nodes),
        FocusTraversalPolicyKind::Ordered => explicit_order(nodes),
    }
}

/// Traversal policy following logical widget structure order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WidgetOrderTraversalPolicy;

impl FocusTraversalPolicy for WidgetOrderTraversalPolicy {
    fn find_first_focus(&self, nodes: &[FocusNode]) -> Option<FocusNode> {
        traversable(nodes).into_iter().next()
    }

    fn find_last_focus(&self, nodes: &[FocusNode]) -> Option<FocusNode> {
        traversable(nodes).into_iter().next_back()
    }

    fn next(&self, current: &FocusNode, nodes: &[FocusNode]) -> Option<FocusNode> {
        let traversable = traversable(nodes);
        if let Some(pos) = traversable.iter().position(|n| n == current) {
            traversable.get(pos + 1).cloned()
        } else {
            traversable.into_iter().next()
        }
    }

    fn previous(&self, current: &FocusNode, nodes: &[FocusNode]) -> Option<FocusNode> {
        let traversable = traversable(nodes);
        if let Some(pos) = traversable.iter().position(|n| n == current) {
            if pos > 0 {
                traversable.get(pos - 1).cloned()
            } else {
                None
            }
        } else {
            traversable.into_iter().next_back()
        }
    }
}

/// Traversal policy following natural reading order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReadingOrderTraversalPolicy;

impl FocusTraversalPolicy for ReadingOrderTraversalPolicy {
    fn find_first_focus(&self, nodes: &[FocusNode]) -> Option<FocusNode> {
        reading_order(traversable(nodes)).into_iter().next()
    }

    fn find_last_focus(&self, nodes: &[FocusNode]) -> Option<FocusNode> {
        reading_order(traversable(nodes)).into_iter().next_back()
    }

    fn next(&self, current: &FocusNode, nodes: &[FocusNode]) -> Option<FocusNode> {
        WidgetOrderTraversalPolicy.next(current, &reading_order(traversable(nodes)))
    }

    fn previous(&self, current: &FocusNode, nodes: &[FocusNode]) -> Option<FocusNode> {
        WidgetOrderTraversalPolicy.previous(current, &reading_order(traversable(nodes)))
    }
}

/// Traversal policy using explicit numeric ordering.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OrderedTraversalPolicy;

impl FocusTraversalPolicy for OrderedTraversalPolicy {
    fn find_first_focus(&self, nodes: &[FocusNode]) -> Option<FocusNode> {
        explicit_order(traversable(nodes)).into_iter().next()
    }

    fn find_last_focus(&self, nodes: &[FocusNode]) -> Option<FocusNode> {
        explicit_order(traversable(nodes)).into_iter().next_back()
    }

    fn next(&self, current: &FocusNode, nodes: &[FocusNode]) -> Option<FocusNode> {
        WidgetOrderTraversalPolicy.next(current, &explicit_order(traversable(nodes)))
    }

    fn previous(&self, current: &FocusNode, nodes: &[FocusNode]) -> Option<FocusNode> {
        WidgetOrderTraversalPolicy.previous(current, &explicit_order(traversable(nodes)))
    }
}

/// Owns a single focus scope.  Register nodes in visual traversal order, then
/// use [`Self::focus_next`] for Tab or Shift+Tab semantics.  The manager is
/// weakly registered, so unmounted/rebuilt controls cannot leak focus entries.
#[derive(Default)]
pub struct FocusManager {
    nodes: Vec<Weak<FocusState>>,
    policy: FocusTraversalPolicyKind,
}
impl FocusManager {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_policy(policy: FocusTraversalPolicyKind) -> Self {
        Self {
            nodes: Vec::new(),
            policy,
        }
    }

    pub fn set_policy(&mut self, policy: FocusTraversalPolicyKind) {
        self.policy = policy;
    }

    #[must_use]
    pub const fn policy(&self) -> FocusTraversalPolicyKind {
        self.policy
    }
    pub fn register(&mut self, node: &FocusNode) {
        self.prune();
        if !self.nodes.iter().any(|known| {
            known
                .upgrade()
                .is_some_and(|known| Rc::ptr_eq(&known, &node.state))
        }) {
            self.nodes.push(Rc::downgrade(&node.state));
        }
    }
    pub fn unregister(&mut self, node: &FocusNode) {
        self.nodes.retain(|known| {
            known
                .upgrade()
                .is_some_and(|known| !Rc::ptr_eq(&known, &node.state))
        });
        node.unfocus();
    }
    /// Gives this node exclusive focus within the scope.
    #[must_use]
    pub fn request_focus(&mut self, node: &FocusNode) -> bool {
        self.register(node);
        if !node.can_request_focus() {
            return false;
        }
        for known in self.nodes.iter().filter_map(Weak::upgrade) {
            known.focused.set(false);
        }
        node.state.focused.set(true);
        true
    }
    pub fn clear_focus(&mut self) {
        for node in self.nodes.iter().filter_map(Weak::upgrade) {
            node.focused.set(false);
        }
        self.prune();
    }
    #[must_use]
    pub fn focused(&mut self) -> Option<FocusNode> {
        self.prune();
        self.nodes
            .iter()
            .filter_map(Weak::upgrade)
            .find_map(|state| state.focused.get().then_some(FocusNode { state }))
    }
    /// Traverses to the next focusable node.  `reverse` provides Shift+Tab
    /// behavior and traversal wraps inside this scope.
    #[must_use]
    pub fn focus_next(&mut self, reverse: bool) -> Option<FocusNode> {
        self.prune();
        let nodes: Vec<_> = self
            .nodes
            .iter()
            .filter_map(Weak::upgrade)
            .map(|state| FocusNode { state })
            .collect();
        let nodes = policy_nodes(self.policy, &nodes);
        let first = nodes.first()?.clone();
        let current = nodes.iter().position(FocusNode::has_focus);
        let next = match current {
            Some(index) if reverse => nodes[(index + nodes.len() - 1) % nodes.len()].clone(),
            Some(index) => nodes[(index + 1) % nodes.len()].clone(),
            None if reverse => nodes.last()?.clone(),
            None => first,
        };
        for node in self.nodes.iter().filter_map(Weak::upgrade) {
            node.focused.set(false);
        }
        next.request_focus();
        Some(next)
    }
    #[must_use]
    pub fn registered_count(&mut self) -> usize {
        self.prune();
        self.nodes.len()
    }
    fn prune(&mut self) {
        self.nodes.retain(|node| node.strong_count() > 0);
    }
}

/// A retained focus-scope handle.
///
/// `FocusScopeNode` is the Rust-native equivalent of a scoped focus owner. It
/// owns a [`FocusManager`] behind interior mutability so a scope can be kept
/// outside rebuildable widget descriptors and shared by controls that need to
/// request focus. It intentionally does not participate in a Dart-style
/// Element hierarchy.
#[derive(Clone)]
pub struct FocusScopeNode {
    state: Rc<FocusScopeState>,
}

/// Owns one live subscription to a [`FocusScopeNode`] change stream.
///
/// The token is intentionally lightweight and single-threaded, matching the
/// retained UI model. Dropping it removes the callback from future delivery;
/// the scope prunes the corresponding weak entry on its next notification.
pub struct FocusScopeSubscription {
    _entry: Rc<FocusScopeObserverEntry>,
}

struct FocusScopeObserverEntry {
    callback: Rc<dyn Fn()>,
}

struct FocusScopeState {
    manager: RefCell<FocusManager>,
    last_focused: RefCell<Option<FocusNode>>,
    parent: RefCell<Option<Weak<FocusScopeState>>>,
    observers: RefCell<Vec<Weak<FocusScopeObserverEntry>>>,
    revision: Cell<u64>,
}

impl FocusScopeState {
    fn bump_revision(&self) {
        self.revision.set(self.revision.get().wrapping_add(1));
        let callbacks = {
            let mut observers = self.observers.borrow_mut();
            observers.retain(|observer| observer.strong_count() != 0);
            observers
                .iter()
                .filter_map(Weak::upgrade)
                .map(|observer| observer.callback.clone())
                .collect::<Vec<_>>()
        };
        for callback in callbacks {
            callback();
        }
    }
}

impl Default for FocusScopeNode {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for FocusScopeNode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut manager = self.state.manager.borrow_mut();
        formatter
            .debug_struct("FocusScopeNode")
            .field("registered_count", &manager.registered_count())
            .field("has_focus", &manager.focused().is_some())
            .finish()
    }
}

impl FocusScopeNode {
    /// Creates an empty retained focus scope.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Rc::new(FocusScopeState {
                manager: RefCell::new(FocusManager::new()),
                last_focused: RefCell::new(None),
                parent: RefCell::new(None),
                observers: RefCell::new(Vec::new()),
                revision: Cell::new(0),
            }),
        }
    }

    /// Creates a scope nested below `parent`.
    #[must_use]
    pub fn nested(parent: &Self) -> Self {
        let scope = Self::new();
        scope.set_parent(Some(parent));
        scope
    }

    /// Sets or clears this scope's parent relationship.
    pub fn set_parent(&self, parent: Option<&Self>) {
        let parent = parent.filter(|scope| !Rc::ptr_eq(&scope.state, &self.state));
        *self.state.parent.borrow_mut() = parent.map(|scope| Rc::downgrade(&scope.state));
        self.state.bump_revision();
    }

    /// Subscribes to focus registration and selection changes.
    ///
    /// Runtime builders use this to turn retained focus changes into ordinary
    /// element invalidation. Consumers outside a runtime may use the same
    /// stream for status indicators or accessibility adapters.
    #[must_use]
    pub fn observe(&self, callback: impl Fn() + 'static) -> FocusScopeSubscription {
        let entry = Rc::new(FocusScopeObserverEntry {
            callback: Rc::new(callback),
        });
        self.state
            .observers
            .borrow_mut()
            .push(Rc::downgrade(&entry));
        FocusScopeSubscription { _entry: entry }
    }

    /// Monotonic state revision, useful for diagnostics and non-runtime
    /// consumers that need to cheaply detect a focus change.
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.state.revision.get()
    }

    /// Registers a focus node in this scope.
    pub fn register(&self, node: &FocusNode) {
        self.state.manager.borrow_mut().register(node);
        self.state.bump_revision();
    }

    /// Sets the traversal policy used by this retained scope.
    pub fn set_policy(&self, policy: FocusTraversalPolicyKind) {
        self.state.manager.borrow_mut().set_policy(policy);
        self.state.bump_revision();
    }

    #[must_use]
    pub fn policy(&self) -> FocusTraversalPolicyKind {
        self.state.manager.borrow().policy()
    }

    /// Removes a focus node from this scope and clears its focus.
    pub fn unregister(&self, node: &FocusNode) {
        self.state.manager.borrow_mut().unregister(node);
        if self
            .state
            .last_focused
            .borrow()
            .as_ref()
            .is_some_and(|focused| focused == node)
        {
            self.state.last_focused.borrow_mut().take();
        }
        self.state.bump_revision();
    }

    /// Gives a node exclusive focus in this scope.
    #[must_use]
    pub fn request_focus(&self, node: &FocusNode) -> bool {
        if !node.can_request_focus() {
            return false;
        }
        let parent = self
            .state
            .parent
            .borrow()
            .as_ref()
            .and_then(Weak::upgrade)
            .map(|state| Self { state });
        if let Some(parent) = &parent {
            let mut parent_manager = parent.state.manager.borrow_mut();
            let parent_focused = parent_manager.focused();
            if parent_focused.is_some() {
                *parent.state.last_focused.borrow_mut() = parent_focused;
                parent_manager.clear_focus();
            }
        }
        let mut manager = self.state.manager.borrow_mut();
        let previous = manager.focused();
        let requested = manager.request_focus(node);
        if requested {
            *self.state.last_focused.borrow_mut() = previous;
            drop(manager);
            if let Some(parent) = &parent {
                parent.state.bump_revision();
            }
            self.state.bump_revision();
        }
        requested
    }

    /// Clears the current focus while retaining it for [`Self::restore_focus`].
    pub fn clear_focus(&self) {
        let mut manager = self.state.manager.borrow_mut();
        *self.state.last_focused.borrow_mut() = manager.focused();
        manager.clear_focus();
        drop(manager);
        self.state.bump_revision();
    }

    /// Clears this scope's focus. This is an alias matching common focus
    /// controller terminology.
    pub fn unfocus(&self) {
        self.clear_focus();
    }

    /// Returns the currently focused node in this scope.
    #[must_use]
    pub fn focused(&self) -> Option<FocusNode> {
        self.state.manager.borrow_mut().focused()
    }

    /// Moves focus to the next/previous registered node.
    #[must_use]
    pub fn focus_next(&self, reverse: bool) -> Option<FocusNode> {
        let mut manager = self.state.manager.borrow_mut();
        let previous = manager.focused();
        let next = manager.focus_next(reverse);
        if next.is_some() {
            *self.state.last_focused.borrow_mut() = previous;
            drop(manager);
            self.state.bump_revision();
        }
        next
    }

    /// Restores the last focus that was active before this scope was cleared
    /// or moved. Returns `false` when the remembered node is no longer
    /// requestable.
    #[must_use]
    pub fn restore_focus(&self) -> bool {
        let Some(node) = self.state.last_focused.borrow().clone() else {
            return false;
        };
        let mut manager = self.state.manager.borrow_mut();
        let restored = manager.request_focus(&node);
        if restored {
            *self.state.last_focused.borrow_mut() = Some(node);
            drop(manager);
            self.state.bump_revision();
        }
        restored
    }

    /// Restores the focused child remembered by this scope's parent.
    #[must_use]
    pub fn restore_parent_focus(&self) -> bool {
        self.state
            .parent
            .borrow()
            .as_ref()
            .and_then(Weak::upgrade)
            .is_some_and(|parent| {
                let scope = Self { state: parent };
                scope.restore_focus()
            })
    }

    /// Number of live nodes registered in this scope.
    #[must_use]
    pub fn registered_count(&self) -> usize {
        self.state.manager.borrow_mut().registered_count()
    }
}

/// A keyboard shortcut key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ShortcutKey {
    pub code: Code,
    pub modifiers: Modifiers,
}

/// A command identifier that can be bound independently from the shortcut
/// that invokes it.
///
/// `Command` is the convenient string-backed default. Applications with a
/// closed command vocabulary should use the generic `Actions<C>` and
/// `Shortcuts<C>` forms with their own `C` enum instead of converting every
/// command to a string.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Command(String);
impl Command {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }
    #[must_use]
    pub fn name(&self) -> &str {
        &self.0
    }
}
impl From<&str> for Command {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

/// Bounds required by typed command registries.
///
/// This is intentionally a value bound, not an inheritance hook. A command
/// can therefore be a small enum, an interned identifier, or the compatibility
/// [`Command`] string value.
pub trait CommandId: Clone + Eq + std::hash::Hash + 'static {}

impl<T> CommandId for T where T: Clone + Eq + std::hash::Hash + 'static {}

/// A command intent passed through an action registry.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Intent<C: CommandId = Command> {
    command: C,
}

impl<C: CommandId> Intent<C> {
    /// Creates an intent for a command identifier.
    #[must_use]
    pub fn new(command: impl Into<C>) -> Self {
        Self {
            command: command.into(),
        }
    }

    /// Returns the command identifier carried by this intent.
    #[must_use]
    pub fn command(&self) -> &C {
        &self.command
    }
}

impl Intent<Command> {
    /// Returns the stable command name.
    #[must_use]
    pub fn name(&self) -> &str {
        self.command.name()
    }
}

impl<C: CommandId> From<C> for Intent<C> {
    fn from(value: C) -> Self {
        Self::new(value)
    }
}

impl From<&str> for Intent {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

/// Result of invoking an action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionResult {
    /// The action consumed the intent.
    Handled,
    /// The action declined it, allowing an outer scope to try.
    Ignored,
}

impl From<bool> for ActionResult {
    fn from(handled: bool) -> Self {
        if handled {
            Self::Handled
        } else {
            Self::Ignored
        }
    }
}

/// A Rust-native command action.
///
/// Actions are values with an enabled state and a callback. They do not form
/// a generic inheritance tree like Flutter's `Action<T>` classes; a command
/// registry composes them by scope instead.
type ActionCallback<C> = Rc<dyn Fn(&Intent<C>) -> ActionResult>;

#[derive(Clone)]
pub struct Action<C: CommandId = Command> {
    intent: Intent<C>,
    enabled: Rc<Cell<bool>>,
    callback: ActionCallback<C>,
}

impl<C: CommandId + std::fmt::Debug> std::fmt::Debug for Action<C> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Action")
            .field("intent", &self.intent)
            .field("enabled", &self.is_enabled())
            .finish_non_exhaustive()
    }
}

impl<C: CommandId> Action<C> {
    /// Creates an enabled action whose callback consumes the intent.
    #[must_use]
    pub fn new(command: impl Into<Intent<C>>, callback: impl Fn() + 'static) -> Self {
        Self::with_handler(command, move |_| {
            callback();
            ActionResult::Handled
        })
    }

    /// Creates an action with an explicit handled/ignored result.
    #[must_use]
    pub fn with_handler<R>(
        command: impl Into<Intent<C>>,
        callback: impl Fn(&Intent<C>) -> R + 'static,
    ) -> Self
    where
        R: Into<ActionResult> + 'static,
    {
        Self {
            intent: command.into(),
            enabled: Rc::new(Cell::new(true)),
            callback: Rc::new(move |intent| callback(intent).into()),
        }
    }

    /// Returns the intent this action handles.
    #[must_use]
    pub fn intent(&self) -> &Intent<C> {
        &self.intent
    }

    /// Returns the stable command identifier.
    #[must_use]
    pub fn command(&self) -> &C {
        self.intent.command()
    }

    /// Enables or disables this action without removing it from its scope.
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.set(enabled);
    }

    /// Returns whether the action can currently run.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled.get()
    }

    /// Invokes this action for an intent.
    #[must_use]
    pub fn invoke(&self, intent: &Intent<C>) -> ActionResult {
        if !self.is_enabled() || self.intent.command() != intent.command() {
            return ActionResult::Ignored;
        }
        (self.callback)(intent)
    }
}

struct ActionScope<C: CommandId> {
    actions: HashMap<C, Action<C>>,
}

/// Command-to-action registry with nearest-scope dispatch.
///
/// The last pushed scope is searched first. A missing, disabled, or
/// `Ignored` action falls through to the next outer scope, which makes nested
/// command palettes and temporary modal bindings predictable.
pub struct Actions<C: CommandId = Command> {
    scopes: Vec<ActionScope<C>>,
}

impl<C: CommandId> Default for Actions<C> {
    fn default() -> Self {
        Self {
            scopes: vec![ActionScope::<C> {
                actions: HashMap::new(),
            }],
        }
    }
}

impl<C: CommandId> Actions<C> {
    /// Creates an empty typed registry. Use `Actions::new()` for the
    /// string-backed default registry when no generic type is needed.
    #[must_use]
    pub fn typed() -> Self {
        Self::default()
    }

    /// Registers a simple callback in the current (nearest) scope.
    pub fn register(&mut self, command: impl Into<C>, action: impl Fn() + 'static) {
        let command = command.into();
        self.register_action(Action::new(Intent::new(command), action));
    }

    /// Registers an explicit action in the current scope.
    pub fn register_action(&mut self, action: Action<C>) {
        self.current_scope_mut()
            .actions
            .insert(action.command().clone(), action);
    }

    /// Registers a callback that can explicitly allow fallthrough.
    pub fn register_handler<R>(
        &mut self,
        command: impl Into<Intent<C>>,
        action: impl Fn(&Intent<C>) -> R + 'static,
    ) where
        R: Into<ActionResult> + 'static,
    {
        self.register_action(Action::with_handler(command, action));
    }

    /// Pushes a nested action scope. Call [`Self::pop_scope`] when the modal
    /// or temporary scope is no longer active.
    pub fn push_scope(&mut self) {
        self.scopes.push(ActionScope::<C> {
            actions: HashMap::new(),
        });
    }

    /// Pops the nearest scope, preserving the root scope.
    #[must_use]
    pub fn pop_scope(&mut self) -> bool {
        (self.scopes.len() > 1).then(|| self.scopes.pop()).is_some()
    }

    /// Runs a closure in a temporary nested scope and restores the previous
    /// scope after the closure returns, including when the closure unwinds.
    pub fn with_scope<R>(&mut self, callback: impl FnOnce(&mut Self) -> R) -> R {
        self.push_scope();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| callback(self)));
        let _ = self.pop_scope();
        match result {
            Ok(result) => result,
            Err(panic) => std::panic::resume_unwind(panic),
        }
    }

    /// Returns the number of active action scopes.
    #[must_use]
    pub fn scope_depth(&self) -> usize {
        self.scopes.len()
    }

    /// Removes a command from the nearest scope.
    pub fn unregister(&mut self, command: &C) -> bool {
        self.current_scope_mut().actions.remove(command).is_some()
    }

    #[must_use]
    pub fn invoke(&self, command: &C) -> bool {
        self.invoke_intent(&Intent::new(command.clone()))
    }

    /// Dispatches an intent from the nearest scope outward.
    #[must_use]
    pub fn invoke_intent(&self, intent: &Intent<C>) -> bool {
        self.scopes.iter().rev().any(|scope| {
            scope
                .actions
                .get(intent.command())
                .is_some_and(|action| action.invoke(intent) == ActionResult::Handled)
        })
    }

    /// Returns the nearest registered action, if any.
    #[must_use]
    pub fn action(&self, command: &C) -> Option<Action<C>> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.actions.get(command).cloned())
    }

    fn current_scope_mut(&mut self) -> &mut ActionScope<C> {
        if self.scopes.is_empty() {
            self.scopes.push(ActionScope::<C> {
                actions: HashMap::new(),
            });
        }
        self.scopes
            .last_mut()
            .expect("action registry has a root scope")
    }
}

impl Actions<Command> {
    /// Creates an empty string-backed command registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl ShortcutKey {
    #[must_use]
    pub const fn new(code: Code, modifiers: Modifiers) -> Self {
        Self { code, modifiers }
    }

    /// Builds a logical-key activator for layouts where physical codes are
    /// not stable enough (for example character shortcuts).
    #[must_use]
    pub fn logical(key: KeyboardKey, modifiers: Modifiers) -> LogicalShortcutKey {
        LogicalShortcutKey { key, modifiers }
    }
}

/// A keyboard shortcut matched against the event's logical key value.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LogicalShortcutKey {
    pub key: KeyboardKey,
    pub modifiers: Modifiers,
}

impl LogicalShortcutKey {
    #[must_use]
    pub fn new(key: KeyboardKey, modifiers: Modifiers) -> Self {
        Self { key, modifiers }
    }
}

/// Controls when a shortcut binding is eligible for a keyboard event.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ShortcutTrigger {
    /// A key-down event, including operating-system repeat events.
    #[default]
    Down,
    /// The initial key-down event, excluding repeats.
    Press,
    /// A repeated key-down event.
    Repeat,
    /// A key-up event.
    Up,
    /// Either key-down or key-up.
    Any,
}

impl ShortcutTrigger {
    #[must_use]
    pub const fn matches(self, event: &KeyboardEvent) -> bool {
        match self {
            Self::Down => event.state.is_down(),
            Self::Press => event.state.is_down() && !event.repeat,
            Self::Repeat => event.state.is_down() && event.repeat,
            Self::Up => event.state.is_up(),
            Self::Any => true,
        }
    }
}

enum ShortcutBinding<C: CommandId> {
    Callback(Rc<dyn Fn()>),
    Intent(Intent<C>),
}

struct ShortcutRegistration<C: CommandId> {
    trigger: ShortcutTrigger,
    binding: ShortcutBinding<C>,
}

struct ShortcutScope<C: CommandId> {
    physical: HashMap<ShortcutKey, Vec<ShortcutRegistration<C>>>,
    logical: HashMap<LogicalShortcutKey, Vec<ShortcutRegistration<C>>>,
}

/// A small typed shortcut map that centralizes keyboard dispatch.
pub struct Shortcuts<C: CommandId = Command> {
    scopes: Vec<ShortcutScope<C>>,
}

impl<C: CommandId> Default for Shortcuts<C> {
    fn default() -> Self {
        Self {
            scopes: vec![ShortcutScope {
                physical: HashMap::new(),
                logical: HashMap::new(),
            }],
        }
    }
}

impl<C: CommandId> Shortcuts<C> {
    /// Creates an empty typed shortcut map. Use `Shortcuts::new()` for the
    /// string-backed default map when no generic type is needed.
    #[must_use]
    pub fn typed() -> Self {
        Self::default()
    }

    pub fn register(&mut self, key: ShortcutKey, action: impl Fn() + 'static) {
        self.register_on(key, ShortcutTrigger::Down, action);
    }

    /// Registers a callback for a specific key transition.
    pub fn register_on(
        &mut self,
        key: ShortcutKey,
        trigger: ShortcutTrigger,
        action: impl Fn() + 'static,
    ) {
        Self::replace_binding(
            &mut self.current_scope_mut().physical,
            key,
            trigger,
            ShortcutBinding::Callback(Rc::new(action)),
        );
    }

    /// Registers a callback against a logical key value.
    pub fn register_logical(&mut self, key: LogicalShortcutKey, action: impl Fn() + 'static) {
        self.register_logical_on(key, ShortcutTrigger::Down, action);
    }

    /// Registers a logical-key callback for a specific key transition.
    pub fn register_logical_on(
        &mut self,
        key: LogicalShortcutKey,
        trigger: ShortcutTrigger,
        action: impl Fn() + 'static,
    ) {
        Self::replace_binding(
            &mut self.current_scope_mut().logical,
            key,
            trigger,
            ShortcutBinding::Callback(Rc::new(action)),
        );
    }

    /// Binds a shortcut to a command registered in [`Actions`].
    pub fn bind(&mut self, key: ShortcutKey, command: impl Into<C>) {
        self.bind_on(key, ShortcutTrigger::Down, Intent::new(command.into()));
    }

    /// Binds a shortcut to an intent for a specific key transition.
    pub fn bind_on(
        &mut self,
        key: ShortcutKey,
        trigger: ShortcutTrigger,
        command: impl Into<Intent<C>>,
    ) {
        Self::replace_binding(
            &mut self.current_scope_mut().physical,
            key,
            trigger,
            ShortcutBinding::Intent(command.into()),
        );
    }

    /// Binds a logical key to a typed command.
    pub fn bind_logical(&mut self, key: LogicalShortcutKey, command: impl Into<C>) {
        self.bind_logical_on(key, ShortcutTrigger::Down, Intent::new(command.into()));
    }

    /// Binds a logical key to an intent for a specific transition.
    pub fn bind_logical_on(
        &mut self,
        key: LogicalShortcutKey,
        trigger: ShortcutTrigger,
        command: impl Into<Intent<C>>,
    ) {
        Self::replace_binding(
            &mut self.current_scope_mut().logical,
            key,
            trigger,
            ShortcutBinding::Intent(command.into()),
        );
    }

    /// Removes all bindings for a shortcut key.
    pub fn clear(&mut self, key: &ShortcutKey) -> bool {
        self.current_scope_mut().physical.remove(key).is_some()
    }

    /// Removes all logical-key bindings from the nearest scope.
    pub fn clear_logical(&mut self, key: &LogicalShortcutKey) -> bool {
        self.current_scope_mut().logical.remove(key).is_some()
    }

    /// Pushes a nested shortcut scope.
    pub fn push_scope(&mut self) {
        self.scopes.push(ShortcutScope {
            physical: HashMap::new(),
            logical: HashMap::new(),
        });
    }

    /// Pops the nearest shortcut scope, preserving the root scope.
    #[must_use]
    pub fn pop_scope(&mut self) -> bool {
        (self.scopes.len() > 1).then(|| self.scopes.pop()).is_some()
    }

    /// Runs a closure in a temporary shortcut scope.
    pub fn with_scope<R>(&mut self, callback: impl FnOnce(&mut Self) -> R) -> R {
        self.push_scope();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| callback(self)));
        let _ = self.pop_scope();
        match result {
            Ok(result) => result,
            Err(panic) => std::panic::resume_unwind(panic),
        }
    }

    /// Returns the number of active shortcut scopes.
    #[must_use]
    pub fn scope_depth(&self) -> usize {
        self.scopes.len()
    }

    pub fn handle(&self, event: KeyboardEvent) -> bool {
        self.dispatch(event, None)
    }

    /// Dispatches a command binding. Direct callback registrations continue to
    /// use [`Self::handle`] for source compatibility.
    #[must_use]
    pub fn handle_actions(&self, event: KeyboardEvent, actions: &Actions<C>) -> bool {
        self.dispatch(event, Some(actions))
    }

    /// Alias emphasizing that this is the complete keyboard-event path.
    #[must_use]
    pub fn handle_event(&self, event: KeyboardEvent, actions: &Actions<C>) -> bool {
        self.handle_actions(event, actions)
    }

    fn replace_binding<K>(
        bindings: &mut HashMap<K, Vec<ShortcutRegistration<C>>>,
        key: K,
        trigger: ShortcutTrigger,
        binding: ShortcutBinding<C>,
    ) where
        K: Eq + std::hash::Hash,
    {
        let registrations = bindings.entry(key).or_default();
        registrations.retain(|registration| registration.trigger != trigger);
        registrations.push(ShortcutRegistration { trigger, binding });
    }

    fn dispatch(&self, event: KeyboardEvent, actions: Option<&Actions<C>>) -> bool {
        let physical = ShortcutKey::new(event.code, event.modifiers);
        let logical = LogicalShortcutKey::new(event.key.clone(), event.modifiers);
        for scope in self.scopes.iter().rev() {
            if scope.physical.get(&physical).is_some_and(|registrations| {
                Self::dispatch_registrations(registrations, &event, actions)
            }) || scope.logical.get(&logical).is_some_and(|registrations| {
                Self::dispatch_registrations(registrations, &event, actions)
            }) {
                return true;
            }
        }
        false
    }

    fn dispatch_registrations(
        registrations: &[ShortcutRegistration<C>],
        event: &KeyboardEvent,
        actions: Option<&Actions<C>>,
    ) -> bool {
        registrations
            .iter()
            .filter(|registration| registration.trigger.matches(event))
            .any(|registration| match &registration.binding {
                ShortcutBinding::Callback(callback) => {
                    callback();
                    true
                }
                ShortcutBinding::Intent(intent) => {
                    actions.is_some_and(|actions| actions.invoke_intent(intent))
                }
            })
    }

    fn current_scope_mut(&mut self) -> &mut ShortcutScope<C> {
        if self.scopes.is_empty() {
            self.scopes.push(ShortcutScope {
                physical: HashMap::new(),
                logical: HashMap::new(),
            });
        }
        self.scopes
            .last_mut()
            .expect("shortcut registry has a root scope")
    }
}

impl Shortcuts<Command> {
    /// Creates an empty string-backed shortcut map.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

/// A reusable recognizer for tap, double-tap, long-press, and pan.
pub struct PointerGestureRecognizer {
    callbacks: GestureCallbacks,
    down: Option<PointerEvent>,
    last_event: Option<PointerEvent>,
    last_tap: Option<Instant>,
    pan_started: bool,
    pan_action: Option<PanAction>,
    long_press_started: bool,
}

/// Type alias for [`PointerGestureRecognizer`].
pub type GestureRecognizer = PointerGestureRecognizer;
/// Legacy compatibility alias for [`PointerGestureRecognizer`].
pub type GestureDetector = PointerGestureRecognizer;

#[derive(Clone, Copy)]
enum PanAction {
    Pan,
    Horizontal,
    Vertical,
}
impl PointerGestureRecognizer {
    pub const DOUBLE_TAP_TIMEOUT: Duration = Duration::from_millis(300);
    pub const LONG_PRESS_TIMEOUT: Duration = Duration::from_millis(500);
    pub const PAN_SLOP: f32 = 18.;
    #[must_use]
    pub fn new(callbacks: GestureCallbacks) -> Self {
        Self {
            callbacks,
            down: None,
            last_event: None,
            last_tap: None,
            pan_started: false,
            pan_action: None,
            long_press_started: false,
        }
    }
    /// Observes an event without invoking user callbacks. This lets a
    /// retained arena delay observable behavior until it has accepted this
    /// recognizer. Direct users should normally keep using [`Self::handle`].
    pub fn observe(&mut self, event: PointerEvent) -> GestureDecision {
        match event.phase {
            PointerPhase::Down => {
                let is_double_tap = self.last_tap.is_some_and(|tap| {
                    event.time.saturating_duration_since(tap) <= Self::DOUBLE_TAP_TIMEOUT
                });
                if !is_double_tap {
                    self.last_tap = None;
                }
                self.down = Some(event);
                self.last_event = Some(event);
                self.pan_started = false;
                self.pan_action = None;
                self.long_press_started = false;
                if let Some(callback) = &self.callbacks.on_tap_down {
                    callback(TapDownDetails {
                        global_position: event.position,
                        local_position: event.position,
                        kind: PointerDeviceKind::Mouse,
                    });
                }
                if is_double_tap {
                    if let Some(callback) = &self.callbacks.on_double_tap_down {
                        callback(TapDownDetails {
                            global_position: event.position,
                            local_position: event.position,
                            kind: PointerDeviceKind::Mouse,
                        });
                    }
                }
                let pan_down = DragDownDetails {
                    global_position: event.position,
                    local_position: event.position,
                };
                if let Some(callback) = &self.callbacks.on_pan_down {
                    callback(pan_down);
                }
                if let Some(callback) = &self.callbacks.on_horizontal_drag_down {
                    callback(pan_down);
                }
                if let Some(callback) = &self.callbacks.on_vertical_drag_down {
                    callback(pan_down);
                }
                GestureDecision::Pending
            }
            PointerPhase::Move => {
                let Some(down) = self.down else {
                    return GestureDecision::Reject;
                };
                let delta = event.position - down.position;
                self.last_event = Some(event);
                if !self.pan_started
                    && !self.long_press_started
                    && event.time.saturating_duration_since(down.time) >= Self::LONG_PRESS_TIMEOUT
                {
                    self.long_press_started = true;
                    if let Some(callback) = &self.callbacks.on_long_press_start {
                        callback(LongPressStartDetails {
                            global_position: down.position,
                            local_position: down.position,
                        });
                    }
                }
                if self.long_press_started && !self.pan_started {
                    if let Some(callback) = &self.callbacks.on_long_press_move_update {
                        callback(LongPressMoveUpdateDetails {
                            global_position: event.position,
                            local_position: event.position,
                            offset_from_origin: delta,
                            local_offset_from_origin: delta,
                        });
                    }
                }
                if !self.pan_started && delta.x.hypot(delta.y) >= Self::PAN_SLOP {
                    self.pan_action = if delta.x.abs() >= delta.y.abs() {
                        (self.callbacks.on_horizontal_drag_down.is_some()
                            || self.callbacks.on_horizontal_drag_start.is_some()
                            || self.callbacks.on_horizontal_drag_update.is_some()
                            || self.callbacks.on_horizontal_drag_end.is_some()
                            || self.callbacks.on_horizontal_drag_cancel.is_some())
                        .then_some(PanAction::Horizontal)
                        .or_else(|| {
                            (self.callbacks.on_pan_down.is_some()
                                || self.callbacks.on_pan_start.is_some()
                                || self.callbacks.on_pan_update.is_some()
                                || self.callbacks.on_pan_end.is_some()
                                || self.callbacks.on_pan_cancel.is_some())
                            .then_some(PanAction::Pan)
                        })
                    } else {
                        (self.callbacks.on_vertical_drag_down.is_some()
                            || self.callbacks.on_vertical_drag_start.is_some()
                            || self.callbacks.on_vertical_drag_update.is_some()
                            || self.callbacks.on_vertical_drag_end.is_some()
                            || self.callbacks.on_vertical_drag_cancel.is_some())
                        .then_some(PanAction::Vertical)
                        .or_else(|| {
                            (self.callbacks.on_pan_down.is_some()
                                || self.callbacks.on_pan_start.is_some()
                                || self.callbacks.on_pan_update.is_some()
                                || self.callbacks.on_pan_end.is_some()
                                || self.callbacks.on_pan_cancel.is_some())
                            .then_some(PanAction::Pan)
                        })
                    };
                    let start = DragStartDetails {
                        global_position: down.position,
                        local_position: down.position,
                    };
                    if let Some(callback) = &self.callbacks.on_pan_start {
                        callback(start);
                    }
                    match self.pan_action {
                        Some(PanAction::Horizontal) => {
                            if let Some(callback) = &self.callbacks.on_horizontal_drag_start {
                                callback(start);
                            }
                        }
                        Some(PanAction::Vertical) => {
                            if let Some(callback) = &self.callbacks.on_vertical_drag_start {
                                callback(start);
                            }
                        }
                        Some(PanAction::Pan) | None => {}
                    }
                    self.pan_started = self.pan_action.is_some();
                    if self.pan_action.is_none() {
                        return GestureDecision::Reject;
                    }
                }
                match self.pan_action {
                    Some(PanAction::Pan) => GestureDecision::Accept(GestureAction::Pan(delta)),
                    Some(PanAction::Horizontal) => {
                        GestureDecision::Accept(GestureAction::HorizontalDrag(delta))
                    }
                    Some(PanAction::Vertical) => {
                        GestureDecision::Accept(GestureAction::VerticalDrag(delta))
                    }
                    None => GestureDecision::Pending,
                }
            }
            PointerPhase::Up => {
                let Some(down) = self.down.take() else {
                    return GestureDecision::Reject;
                };
                if self.pan_started {
                    let previous = self.last_event.take().unwrap_or(down);
                    let elapsed = event
                        .time
                        .saturating_duration_since(previous.time)
                        .as_secs_f32();
                    let delta = event.position - previous.position;
                    let details = DragEndDetails {
                        total_delta: event.position - down.position,
                        velocity: if elapsed > 0.0 {
                            Offset::new(delta.x / elapsed, delta.y / elapsed)
                        } else {
                            Offset::ZERO
                        },
                        cancelled: false,
                    };
                    let action = match self.pan_action.take() {
                        Some(PanAction::Pan) => self
                            .callbacks
                            .on_pan_end
                            .as_ref()
                            .map(|_| GestureAction::PanEnd(details)),
                        Some(PanAction::Horizontal) => self
                            .callbacks
                            .on_horizontal_drag_end
                            .as_ref()
                            .map(|_| GestureAction::HorizontalDragEnd(details)),
                        Some(PanAction::Vertical) => self
                            .callbacks
                            .on_vertical_drag_end
                            .as_ref()
                            .map(|_| GestureAction::VerticalDragEnd(details)),
                        None => None,
                    };
                    self.pan_started = false;
                    return action.map_or(GestureDecision::Pending, GestureDecision::Accept);
                }
                self.last_event = None;
                let elapsed = event.time.saturating_duration_since(down.time);
                if elapsed >= Self::LONG_PRESS_TIMEOUT {
                    if !self.long_press_started {
                        self.long_press_started = true;
                        if let Some(callback) = &self.callbacks.on_long_press_start {
                            callback(LongPressStartDetails {
                                global_position: down.position,
                                local_position: down.position,
                            });
                        }
                    }
                    if let Some(callback) = &self.callbacks.on_long_press_up {
                        callback();
                    }
                    if let Some(callback) = &self.callbacks.on_long_press_end {
                        callback(LongPressEndDetails {
                            global_position: event.position,
                            local_position: event.position,
                            velocity: Velocity::ZERO,
                        });
                    }
                    self.long_press_started = false;
                    if self.callbacks.on_long_press.is_some()
                        || self.callbacks.on_long_press_start.is_some()
                        || self.callbacks.on_long_press_move_update.is_some()
                        || self.callbacks.on_long_press_up.is_some()
                        || self.callbacks.on_long_press_end.is_some()
                    {
                        GestureDecision::Accept(GestureAction::LongPress)
                    } else {
                        GestureDecision::Reject
                    }
                } else if self.last_tap.is_some_and(|tap| {
                    event.time.saturating_duration_since(tap) <= Self::DOUBLE_TAP_TIMEOUT
                }) && self.callbacks.on_double_tap.is_some()
                {
                    if let Some(callback) = &self.callbacks.on_tap_up {
                        callback(TapUpDetails {
                            global_position: event.position,
                            local_position: event.position,
                            kind: PointerDeviceKind::Mouse,
                        });
                    }
                    self.last_tap = None;
                    GestureDecision::Accept(GestureAction::DoubleTap)
                } else if self.callbacks.on_tap.is_some()
                    || self.callbacks.on_tap_up.is_some()
                    || self.callbacks.on_tap_cancel.is_some()
                {
                    if let Some(callback) = &self.callbacks.on_tap_up {
                        callback(TapUpDetails {
                            global_position: event.position,
                            local_position: event.position,
                            kind: PointerDeviceKind::Mouse,
                        });
                    }
                    self.last_tap = Some(event.time);
                    GestureDecision::Accept(GestureAction::Tap)
                } else {
                    GestureDecision::Reject
                }
            }
            PointerPhase::Cancel => {
                if self.down.take().is_some() {
                    self.last_event = None;
                    self.pan_started = false;
                    self.pan_action = None;
                    self.long_press_started = false;
                    GestureDecision::Cancelled
                } else {
                    GestureDecision::Reject
                }
            }
        }
    }
    /// Invokes a callback selected by [`Self::observe`] after the caller has
    /// accepted this recognizer.
    pub fn dispatch(&self, action: GestureAction) {
        match action {
            GestureAction::Tap => self.callbacks.on_tap.as_ref().map(|callback| callback()),
            GestureAction::DoubleTap => self
                .callbacks
                .on_double_tap
                .as_ref()
                .map(|callback| callback()),
            GestureAction::LongPress => self
                .callbacks
                .on_long_press
                .as_ref()
                .map(|callback| callback()),
            GestureAction::Pan(delta) => self
                .callbacks
                .on_pan_update
                .as_ref()
                .map(|callback| callback(delta)),
            GestureAction::PanEnd(details) => self
                .callbacks
                .on_pan_end
                .as_ref()
                .map(|callback| callback(details)),
            GestureAction::HorizontalDrag(delta) => self
                .callbacks
                .on_horizontal_drag_update
                .as_ref()
                .map(|callback| callback(delta)),
            GestureAction::HorizontalDragEnd(details) => self
                .callbacks
                .on_horizontal_drag_end
                .as_ref()
                .map(|callback| callback(details)),
            GestureAction::VerticalDrag(delta) => self
                .callbacks
                .on_vertical_drag_update
                .as_ref()
                .map(|callback| callback(delta)),
            GestureAction::VerticalDragEnd(details) => self
                .callbacks
                .on_vertical_drag_end
                .as_ref()
                .map(|callback| callback(details)),
        };
    }
    pub fn cancel(&self) {
        if let Some(callback) = &self.callbacks.on_tap_cancel {
            callback();
        }
        if let Some(callback) = &self.callbacks.on_double_tap_cancel {
            callback();
        }
        if let Some(callback) = &self.callbacks.on_pan_cancel {
            callback();
        }
        if let Some(callback) = &self.callbacks.on_horizontal_drag_cancel {
            callback();
        }
        if let Some(callback) = &self.callbacks.on_vertical_drag_cancel {
            callback();
        }
        if let Some(callback) = &self.callbacks.on_cancel {
            callback();
        }
    }
    pub fn handle(&mut self, event: PointerEvent) -> bool {
        match self.observe(event) {
            GestureDecision::Accept(action) => {
                self.dispatch(action);
                true
            }
            GestureDecision::Pending => self.down.is_some(),
            GestureDecision::Reject => false,
            GestureDecision::Cancelled => {
                self.cancel();
                true
            }
        }
    }
}

/// Details emitted by a single-pointer drag recognizer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DragUpdateDetails {
    pub global_position: Offset,
    pub local_position: Offset,
    pub delta: Offset,
    pub total_delta: Offset,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DragEndDetails {
    pub total_delta: Offset,
    pub velocity: Offset,
    pub cancelled: bool,
}
#[derive(Clone, Default)]
pub struct DragCallbacks {
    pub on_start: Option<Rc<dyn Fn(Offset)>>,
    pub on_update: Option<Rc<dyn Fn(DragUpdateDetails)>>,
    pub on_end: Option<Rc<dyn Fn(DragEndDetails)>>,
}
/// A platform-neutral drag recognizer with start, update, end, and cancellation.
pub struct DragGestureDetector {
    callbacks: DragCallbacks,
    active: Option<PointerEvent>,
    previous: Option<PointerEvent>,
    started: bool,
}
impl DragGestureDetector {
    pub const SLOP: f32 = 18.;
    #[must_use]
    pub fn new(callbacks: DragCallbacks) -> Self {
        Self {
            callbacks,
            active: None,
            previous: None,
            started: false,
        }
    }
    pub fn handle(&mut self, event: PointerEvent) -> bool {
        match event.phase {
            PointerPhase::Down => {
                self.active = Some(event);
                self.previous = Some(event);
                self.started = false;
                true
            }
            PointerPhase::Move => {
                let Some(start) = self.active else {
                    return false;
                };
                let previous = self.previous.replace(event).unwrap_or(start);
                let total_delta = event.position - start.position;
                if !self.started && total_delta.x.hypot(total_delta.y) >= Self::SLOP {
                    self.started = true;
                    if let Some(callback) = &self.callbacks.on_start {
                        callback(start.position);
                    }
                }
                if self.started {
                    if let Some(callback) = &self.callbacks.on_update {
                        callback(DragUpdateDetails {
                            global_position: event.position,
                            local_position: event.position,
                            delta: event.position - previous.position,
                            total_delta,
                        });
                    }
                }
                true
            }
            PointerPhase::Up | PointerPhase::Cancel => {
                let Some(start) = self.active.take() else {
                    return false;
                };
                let previous = self.previous.take().unwrap_or(start);
                let started = std::mem::take(&mut self.started);
                if started {
                    let elapsed = event
                        .time
                        .saturating_duration_since(previous.time)
                        .as_secs_f32();
                    let delta = event.position - previous.position;
                    if let Some(callback) = &self.callbacks.on_end {
                        callback(DragEndDetails {
                            total_delta: event.position - start.position,
                            velocity: if elapsed > 0. {
                                Offset::new(delta.x / elapsed, delta.y / elapsed)
                            } else {
                                Offset::ZERO
                            },
                            cancelled: matches!(event.phase, PointerPhase::Cancel),
                        });
                    }
                }
                true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn shortcuts_only_dispatch_matching_pressed_events() {
        let count = Rc::new(Cell::new(0));
        let mut shortcuts = Shortcuts::new();
        let expected = count.clone();
        shortcuts.register(
            ShortcutKey::new(Code::KeyA, Modifiers::default()),
            move || expected.set(expected.get() + 1),
        );
        assert!(shortcuts.handle(KeyboardEvent::key_down(
            incular_core::KeyboardKey::Named(incular_core::NamedKey::Unidentified),
            Code::KeyA,
        )));
        assert_eq!(count.get(), 1);
    }
    #[test]
    fn focus_manager_keeps_one_owner_and_skips_excluded_nodes() {
        let first = FocusNode::new();
        let skipped = FocusNode::new();
        let last = FocusNode::new();
        skipped.set_can_request_focus(false);
        let mut scope = FocusManager::new();
        for node in [&first, &skipped, &last] {
            scope.register(node);
        }
        assert!(scope.request_focus(&first));
        assert!(first.has_focus());
        assert_eq!(scope.focus_next(false), Some(last.clone()));
        assert!(!first.has_focus());
        assert!(last.has_focus());
        assert_eq!(scope.focus_next(true), Some(first.clone()));
        drop(last);
        assert_eq!(scope.registered_count(), 2);
    }

    #[test]
    fn focus_manager_applies_reading_and_explicit_order_policies() {
        let top_right = FocusNode::new();
        let bottom_left = FocusNode::new();
        let top_left = FocusNode::new();
        top_right.set_rect(Rect::from_origin_size(
            Offset::new(100., 0.),
            incular_core::Size::new(20., 20.),
        ));
        bottom_left.set_rect(Rect::from_origin_size(
            Offset::new(0., 50.),
            incular_core::Size::new(20., 20.),
        ));
        top_left.set_rect(Rect::from_origin_size(
            Offset::new(0., 0.),
            incular_core::Size::new(20., 20.),
        ));
        let mut reading = FocusManager::with_policy(FocusTraversalPolicyKind::ReadingOrder);
        for node in [&top_right, &bottom_left, &top_left] {
            reading.register(node);
        }
        assert_eq!(reading.focus_next(false), Some(top_left.clone()));
        assert_eq!(reading.focus_next(false), Some(top_right.clone()));
        assert_eq!(reading.focus_next(false), Some(bottom_left.clone()));

        let first = FocusNode::new();
        let second = FocusNode::new();
        first.set_traversal_order(Some(20.));
        second.set_traversal_order(Some(10.));
        let mut ordered = FocusManager::with_policy(FocusTraversalPolicyKind::Ordered);
        ordered.register(&first);
        ordered.register(&second);
        assert_eq!(ordered.focus_next(false), Some(second));
        assert_eq!(ordered.focus_next(false), Some(first));
    }
    #[test]
    fn shortcuts_can_dispatch_typed_commands() {
        let count = Rc::new(Cell::new(0));
        let mut actions = Actions::new();
        actions.register("save", {
            let count = count.clone();
            move || count.set(count.get() + 1)
        });
        let mut shortcuts = Shortcuts::new();
        shortcuts.bind(ShortcutKey::new(Code::KeyA, Modifiers::default()), "save");
        assert!(shortcuts.handle_actions(
            KeyboardEvent::key_down(
                incular_core::KeyboardKey::Named(incular_core::NamedKey::Unidentified),
                Code::KeyA,
            ),
            &actions,
        ));
        assert_eq!(count.get(), 1);
    }
    #[test]
    fn generic_commands_dispatch_ctrl_s_without_string_conversion() {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        enum EditorCommand {
            Save,
        }

        let count = Rc::new(Cell::new(0));
        let mut actions = Actions::<EditorCommand>::typed();
        let observed = count.clone();
        actions.register(EditorCommand::Save, move || {
            observed.set(observed.get() + 1)
        });
        let mut shortcuts = Shortcuts::<EditorCommand>::typed();
        shortcuts.bind(
            ShortcutKey::new(Code::KeyS, Modifiers::CONTROL),
            EditorCommand::Save,
        );

        let mut event = KeyboardEvent::key_down(
            incular_core::KeyboardKey::Named(incular_core::NamedKey::Unidentified),
            Code::KeyS,
        );
        event.modifiers = Modifiers::CONTROL;
        assert!(shortcuts.handle_actions(event, &actions));
        assert_eq!(count.get(), 1);
    }
    #[test]
    fn actions_dispatch_nearest_scope_first() {
        let observed = Rc::new(RefCell::new(Vec::new()));
        let outer_observed = observed.clone();
        let mut actions = Actions::new();
        actions.register("open", move || outer_observed.borrow_mut().push("outer"));
        actions.push_scope();
        let inner_observed = observed.clone();
        actions.register("open", move || inner_observed.borrow_mut().push("inner"));

        assert!(actions.invoke(&Command::new("open")));
        assert_eq!(&*observed.borrow(), &["inner"]);
    }

    #[test]
    fn disabled_or_ignored_actions_fall_through_to_outer_scope() {
        let observed = Rc::new(RefCell::new(Vec::new()));
        let outer_observed = observed.clone();
        let mut actions = Actions::new();
        actions.register("save", move || outer_observed.borrow_mut().push("outer"));
        actions.push_scope();
        let disabled = Action::new("save", {
            let observed = observed.clone();
            move || observed.borrow_mut().push("disabled")
        });
        disabled.set_enabled(false);
        actions.register_action(disabled);
        assert!(actions.invoke(&Command::new("save")));
        assert_eq!(&*observed.borrow(), &["outer"]);

        actions.register_handler("save", |_| ActionResult::Ignored);
        assert!(actions.invoke(&Command::new("save")));
        assert_eq!(&*observed.borrow(), &["outer", "outer"]);

        actions.register_handler("save", |_| false);
        assert!(actions.invoke(&Command::new("save")));
        assert_eq!(&*observed.borrow(), &["outer", "outer", "outer"]);

        let ignored = Action::<Command>::with_handler("save", |_| false);
        assert_eq!(
            ignored.invoke(&Intent::<Command>::from("save")),
            ActionResult::Ignored
        );
    }

    #[test]
    fn temporary_action_scope_restores_after_callback() {
        let observed = Rc::new(RefCell::new(Vec::new()));
        let outer_observed = observed.clone();
        let mut actions = Actions::new();
        actions.register("toggle", move || outer_observed.borrow_mut().push("outer"));
        actions.with_scope(|actions| {
            let inner_observed = observed.clone();
            actions.register("toggle", move || inner_observed.borrow_mut().push("inner"));
            assert_eq!(actions.scope_depth(), 2);
            assert!(actions.invoke(&Command::new("toggle")));
        });
        assert_eq!(actions.scope_depth(), 1);
        assert!(actions.invoke(&Command::new("toggle")));
        assert_eq!(&*observed.borrow(), &["inner", "outer"]);
    }

    #[test]
    fn shortcuts_distinguish_press_repeat_and_release() {
        let presses = Rc::new(Cell::new(0));
        let repeats = Rc::new(Cell::new(0));
        let releases = Rc::new(Cell::new(0));
        let mut shortcuts = Shortcuts::new();
        let observed_presses = presses.clone();
        shortcuts.register_on(
            ShortcutKey::new(Code::KeyA, Modifiers::default()),
            ShortcutTrigger::Press,
            move || observed_presses.set(observed_presses.get() + 1),
        );
        let observed_repeats = repeats.clone();
        shortcuts.register_on(
            ShortcutKey::new(Code::KeyA, Modifiers::default()),
            ShortcutTrigger::Repeat,
            move || observed_repeats.set(observed_repeats.get() + 1),
        );
        let observed_releases = releases.clone();
        shortcuts.register_on(
            ShortcutKey::new(Code::KeyA, Modifiers::default()),
            ShortcutTrigger::Up,
            move || observed_releases.set(observed_releases.get() + 1),
        );

        assert!(shortcuts.handle(KeyboardEvent::key_down(
            incular_core::KeyboardKey::Named(incular_core::NamedKey::Unidentified),
            Code::KeyA,
        )));
        let mut repeat = KeyboardEvent::key_down(
            incular_core::KeyboardKey::Named(incular_core::NamedKey::Unidentified),
            Code::KeyA,
        );
        repeat.repeat = true;
        assert!(shortcuts.handle(repeat));
        assert!(shortcuts.handle(KeyboardEvent::key_up(
            incular_core::KeyboardKey::Named(incular_core::NamedKey::Unidentified),
            Code::KeyA,
        )));
        assert_eq!(presses.get(), 1);
        assert_eq!(repeats.get(), 1);
        assert_eq!(releases.get(), 1);
    }

    #[test]
    fn logical_shortcut_and_nested_scope_fallthrough_are_supported() {
        let observed = Rc::new(Cell::new(0));
        let mut actions = Actions::new();
        let outer = observed.clone();
        actions.register("save", move || outer.set(outer.get() + 1));

        let mut shortcuts = Shortcuts::new();
        shortcuts.bind_logical(
            LogicalShortcutKey::new(
                incular_core::KeyboardKey::Character("s".into()),
                Modifiers::CONTROL,
            ),
            "save",
        );
        shortcuts.push_scope();
        shortcuts.bind_on(
            ShortcutKey::new(Code::KeyS, Modifiers::CONTROL),
            ShortcutTrigger::Press,
            Intent::new("unhandled"),
        );

        let mut event =
            KeyboardEvent::key_down(incular_core::KeyboardKey::Character("s".into()), Code::KeyS);
        event.modifiers = Modifiers::CONTROL;
        assert!(shortcuts.handle_actions(event, &actions));
        assert_eq!(observed.get(), 1);
    }

    #[test]
    fn focus_scope_node_restores_previous_focus() {
        let scope = FocusScopeNode::new();
        let first = FocusNode::new();
        let second = FocusNode::new();
        scope.register(&first);
        scope.register(&second);
        assert!(scope.request_focus(&first));
        scope.clear_focus();
        assert!(scope.focused().is_none());
        assert!(scope.restore_focus());
        assert_eq!(scope.focused(), Some(first));
        assert_eq!(scope.focus_next(false), Some(second.clone()));
        assert_eq!(scope.focused(), Some(second));
    }

    #[test]
    fn focus_scope_observer_tracks_revision_and_stops_after_drop() {
        let scope = FocusScopeNode::new();
        let node = FocusNode::new();
        let notifications = Rc::new(Cell::new(0));
        let observed = notifications.clone();
        let subscription = scope.observe(move || observed.set(observed.get() + 1));
        let initial_revision = scope.revision();

        scope.register(&node);
        assert!(scope.revision() > initial_revision);
        assert_eq!(notifications.get(), 1);
        assert!(scope.request_focus(&node));
        assert_eq!(notifications.get(), 2);

        drop(subscription);
        scope.clear_focus();
        assert_eq!(notifications.get(), 2);
        assert!(scope.revision() > initial_revision);
    }

    #[test]
    fn nested_focus_scope_restores_parent_child() {
        let parent = FocusScopeNode::new();
        let parent_child = FocusNode::new();
        parent.register(&parent_child);
        assert!(parent.request_focus(&parent_child));

        let child = FocusScopeNode::nested(&parent);
        let child_node = FocusNode::new();
        child.register(&child_node);
        assert!(child.request_focus(&child_node));
        assert!(parent.focused().is_none());
        child.unfocus();
        assert!(child.restore_parent_focus());
        assert_eq!(parent.focused(), Some(parent_child));
    }
    #[test]
    fn scale_recognizer_reports_relative_two_pointer_distance() {
        let observed = Rc::new(Cell::new(0.));
        let expected = observed.clone();
        let mut detector = ScaleGestureDetector::new(move |details| expected.set(details.scale));
        let now = Instant::now();
        for (pointer, position) in [(1, Offset::new(0., 0.)), (2, Offset::new(10., 0.))] {
            assert!(detector.handle(PointerEvent {
                pointer,
                position,
                phase: PointerPhase::Down,
                time: now,
            }));
        }
        assert!(detector.handle(PointerEvent {
            pointer: 2,
            position: Offset::new(20., 0.),
            phase: PointerPhase::Move,
            time: now,
        }));
        assert_eq!(observed.get(), 2.);
    }

    #[test]
    fn arena_rejects_competing_drag_but_keeps_compatible_scale_members() {
        let key = GestureArenaKey {
            window: 7,
            pointer: 3,
        };
        let mut arena = GestureArena::new();
        let tap = arena.add(key, false);
        let drag = arena.add(key, false);
        let entries = arena.accept(key, drag);
        assert!(
            entries
                .iter()
                .any(|entry| entry.member == drag
                    && entry.disposition == GestureDisposition::Accepted)
        );
        assert!(
            entries
                .iter()
                .any(|entry| entry.member == tap
                    && entry.disposition == GestureDisposition::Rejected)
        );
        let scale_a = arena.add(key, true);
        let scale_b = arena.add(key, true);
        let entries = arena.accept(key, scale_a);
        assert!(entries.iter().any(
            |entry| entry.member == scale_b && entry.disposition == GestureDisposition::Pending
        ));
        assert_eq!(arena.cancel(key).len(), 4);
    }
    #[test]
    fn drag_recognizer_reports_start_updates_and_end() {
        let starts = Rc::new(Cell::new(0));
        let total = Rc::new(Cell::new(Offset::ZERO));
        let ended = Rc::new(Cell::new(false));
        let mut detector = DragGestureDetector::new(DragCallbacks {
            on_start: Some({
                let starts = starts.clone();
                Rc::new(move |_| starts.set(starts.get() + 1))
            }),
            on_update: Some({
                let total = total.clone();
                Rc::new(move |details| total.set(details.total_delta))
            }),
            on_end: Some({
                let ended = ended.clone();
                Rc::new(move |details| ended.set(!details.cancelled))
            }),
        });
        let now = Instant::now();
        for (phase, position) in [
            (PointerPhase::Down, Offset::ZERO),
            (PointerPhase::Move, Offset::new(20., 0.)),
            (PointerPhase::Up, Offset::new(25., 0.)),
        ] {
            assert!(detector.handle(PointerEvent {
                pointer: 1,
                position,
                phase,
                time: now + Duration::from_millis(10),
            }));
        }
        assert_eq!(starts.get(), 1);
        assert_eq!(total.get(), Offset::new(20., 0.));
        assert!(ended.get());
    }

    #[test]
    fn pointer_recognizer_reports_drag_end_after_arena_updates() {
        let ended = Rc::new(Cell::new(None));
        let mut recognizer = PointerGestureRecognizer::new(GestureCallbacks {
            on_horizontal_drag_update: Some(Rc::new(|_| {})),
            on_horizontal_drag_end: Some({
                let ended = ended.clone();
                Rc::new(move |details| ended.set(Some(details)))
            }),
            ..GestureCallbacks::default()
        });
        let now = Instant::now();
        assert_eq!(
            recognizer.observe(PointerEvent {
                pointer: 1,
                position: Offset::ZERO,
                phase: PointerPhase::Down,
                time: now,
            }),
            GestureDecision::Pending
        );
        assert!(matches!(
            recognizer.observe(PointerEvent {
                pointer: 1,
                position: Offset::new(24., 0.),
                phase: PointerPhase::Move,
                time: now + Duration::from_millis(10),
            }),
            GestureDecision::Accept(GestureAction::HorizontalDrag(_))
        ));
        let end = recognizer.observe(PointerEvent {
            pointer: 1,
            position: Offset::new(30., 0.),
            phase: PointerPhase::Up,
            time: now + Duration::from_millis(20),
        });
        assert!(matches!(
            end,
            GestureDecision::Accept(GestureAction::HorizontalDragEnd(_))
        ));
        if let GestureDecision::Accept(action) = end {
            recognizer.dispatch(action);
        }
        let details = ended.get().expect("drag end callback");
        assert_eq!(details.total_delta, Offset::new(30., 0.));
        assert!(!details.cancelled);
    }
}
