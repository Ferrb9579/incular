//! Platform-neutral gesture recognition and interaction primitives.
//!
//! This crate deliberately contains no widget-tree, runtime, or native-window
//! types. Retained widget regions and platform adapters share it as an input
//! boundary.

use std::{
    cell::Cell,
    collections::HashMap,
    rc::{Rc, Weak},
    time::{Duration, Instant},
};

use incular_core::{KeyCode, KeyEvent, Modifiers, Offset, PointerPhase};

/// A platform-neutral pointer event.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointerEvent {
    pub pointer: u64,
    pub position: Offset,
    pub phase: PointerPhase,
    pub time: Instant,
}

/// Callbacks understood by [`GestureDetector`].
#[derive(Clone, Default)]
pub struct GestureCallbacks {
    pub on_tap: Option<Rc<dyn Fn()>>,
    pub on_double_tap: Option<Rc<dyn Fn()>>,
    pub on_long_press: Option<Rc<dyn Fn()>>,
    pub on_pan_update: Option<Rc<dyn Fn(Offset)>>,
    /// Receives the active focal point and relative distance for a retained
    /// multi-pointer region. Single-pointer recognizers ignore this callback.
    pub on_scale_update: Option<Rc<dyn Fn(ScaleUpdateDetails)>>,
}

/// The current geometry of a two-pointer scale interaction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScaleUpdateDetails {
    pub focal_point: Offset,
    pub scale: f32,
}

/// A platform-neutral two-pointer scale recognizer.
pub struct ScaleGestureDetector {
    pointers: HashMap<u64, Offset>,
    initial_distance: Option<f32>,
    on_update: Option<Rc<dyn Fn(ScaleUpdateDetails)>>,
}
impl ScaleGestureDetector {
    #[must_use]
    pub fn new(on_update: impl Fn(ScaleUpdateDetails) + 'static) -> Self {
        Self {
            pointers: HashMap::new(),
            initial_distance: None,
            on_update: Some(Rc::new(on_update)),
        }
    }
    pub fn handle(&mut self, event: PointerEvent) -> bool {
        match event.phase {
            PointerPhase::Down => {
                self.pointers.insert(event.pointer, event.position);
                self.reset_initial_distance();
                true
            }
            PointerPhase::Move => {
                let Some(position) = self.pointers.get_mut(&event.pointer) else {
                    return false;
                };
                *position = event.position;
                let Some((first, second)) = self.first_two() else {
                    return true;
                };
                let distance = distance(first, second);
                let initial = self
                    .initial_distance
                    .get_or_insert(distance.max(f32::EPSILON));
                if let Some(callback) = &self.on_update {
                    callback(ScaleUpdateDetails {
                        focal_point: Offset::new(
                            (first.x + second.x) * 0.5,
                            (first.y + second.y) * 0.5,
                        ),
                        scale: distance / *initial,
                    });
                }
                true
            }
            PointerPhase::Up | PointerPhase::Cancel => {
                let handled = self.pointers.remove(&event.pointer).is_some();
                self.reset_initial_distance();
                handled
            }
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

#[derive(Default)]
struct FocusState {
    focused: Cell<bool>,
    can_request_focus: Cell<bool>,
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
impl Default for FocusNode {
    fn default() -> Self {
        Self {
            state: Rc::new(FocusState {
                focused: Cell::new(false),
                can_request_focus: Cell::new(true),
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
}

/// Owns a single focus scope.  Register nodes in visual traversal order, then
/// use [`Self::focus_next`] for Tab or Shift+Tab semantics.  The manager is
/// weakly registered, so unmounted/rebuilt controls cannot leak focus entries.
#[derive(Default)]
pub struct FocusManager {
    nodes: Vec<Weak<FocusState>>,
}
impl FocusManager {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
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
            .filter(|node| node.can_request_focus.get())
            .collect();
        let first = nodes.first()?.clone();
        let current = nodes.iter().position(|node| node.focused.get());
        let next = match current {
            Some(index) if reverse => nodes[(index + nodes.len() - 1) % nodes.len()].clone(),
            Some(index) => nodes[(index + 1) % nodes.len()].clone(),
            None if reverse => nodes.last()?.clone(),
            None => first,
        };
        for node in &nodes {
            node.focused.set(false);
        }
        next.focused.set(true);
        Some(FocusNode { state: next })
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

/// A keyboard shortcut key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ShortcutKey {
    pub code: KeyCode,
    pub modifiers: Modifiers,
}

/// A typed command name that can be bound independently from the shortcut
/// that invokes it.
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

/// Command-to-callback registry.  It intentionally replaces a deep
/// Intent/Action inheritance tree with typed command values.
#[derive(Default)]
pub struct Actions {
    actions: HashMap<Command, Rc<dyn Fn()>>,
}
impl Actions {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    pub fn register(&mut self, command: impl Into<Command>, action: impl Fn() + 'static) {
        self.actions.insert(command.into(), Rc::new(action));
    }
    #[must_use]
    pub fn invoke(&self, command: &Command) -> bool {
        let Some(action) = self.actions.get(command) else {
            return false;
        };
        action();
        true
    }
}
impl ShortcutKey {
    #[must_use]
    pub const fn new(code: KeyCode, modifiers: Modifiers) -> Self {
        Self { code, modifiers }
    }
}

/// A small action map that centralizes keyboard shortcut dispatch.
#[derive(Default)]
pub struct Shortcuts {
    actions: HashMap<ShortcutKey, Rc<dyn Fn()>>,
    commands: HashMap<ShortcutKey, Command>,
}
impl Shortcuts {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    pub fn register(&mut self, key: ShortcutKey, action: impl Fn() + 'static) {
        self.actions.insert(key, Rc::new(action));
    }
    /// Binds a shortcut to a command registered in [`Actions`].
    pub fn bind(&mut self, key: ShortcutKey, command: impl Into<Command>) {
        self.commands.insert(key, command.into());
    }
    pub fn handle(&self, event: KeyEvent) -> bool {
        if !event.pressed {
            return false;
        }
        let Some(action) = self
            .actions
            .get(&ShortcutKey::new(event.code, event.modifiers))
        else {
            return false;
        };
        action();
        true
    }
    /// Dispatches a command binding. Direct callback registrations continue to
    /// use [`Self::handle`] for source compatibility.
    #[must_use]
    pub fn handle_actions(&self, event: KeyEvent, actions: &Actions) -> bool {
        if !event.pressed {
            return false;
        }
        self.commands
            .get(&ShortcutKey::new(event.code, event.modifiers))
            .is_some_and(|command| actions.invoke(command))
    }
}

/// A reusable recognizer for tap, double-tap, long-press, and pan.
pub struct GestureDetector {
    callbacks: GestureCallbacks,
    down: Option<PointerEvent>,
    last_tap: Option<Instant>,
    pan_started: bool,
}
impl GestureDetector {
    pub const DOUBLE_TAP_TIMEOUT: Duration = Duration::from_millis(300);
    pub const LONG_PRESS_TIMEOUT: Duration = Duration::from_millis(500);
    pub const PAN_SLOP: f32 = 18.;
    #[must_use]
    pub fn new(callbacks: GestureCallbacks) -> Self {
        Self {
            callbacks,
            down: None,
            last_tap: None,
            pan_started: false,
        }
    }
    pub fn handle(&mut self, event: PointerEvent) -> bool {
        match event.phase {
            PointerPhase::Down => {
                self.down = Some(event);
                self.pan_started = false;
                true
            }
            PointerPhase::Move => {
                let Some(down) = self.down else {
                    return false;
                };
                let delta = event.position - down.position;
                if delta.x.hypot(delta.y) >= Self::PAN_SLOP {
                    self.pan_started = true;
                }
                if self.pan_started {
                    if let Some(callback) = &self.callbacks.on_pan_update {
                        callback(delta);
                    }
                }
                true
            }
            PointerPhase::Up => {
                let Some(down) = self.down.take() else {
                    return false;
                };
                if self.pan_started {
                    return true;
                }
                let elapsed = event.time.saturating_duration_since(down.time);
                if elapsed >= Self::LONG_PRESS_TIMEOUT {
                    if let Some(callback) = &self.callbacks.on_long_press {
                        callback();
                    }
                } else if self.last_tap.is_some_and(|tap| {
                    event.time.saturating_duration_since(tap) <= Self::DOUBLE_TAP_TIMEOUT
                }) {
                    self.last_tap = None;
                    if let Some(callback) = &self.callbacks.on_double_tap {
                        callback();
                    }
                } else {
                    self.last_tap = Some(event.time);
                    if let Some(callback) = &self.callbacks.on_tap {
                        callback();
                    }
                }
                true
            }
            PointerPhase::Cancel => {
                let handled = self.down.take().is_some();
                self.pan_started = false;
                handled
            }
        }
    }
}

/// Details emitted by a single-pointer drag recognizer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DragUpdateDetails {
    pub global_position: Offset,
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
            ShortcutKey::new(KeyCode::KeyA, Modifiers::default()),
            move || expected.set(expected.get() + 1),
        );
        assert!(shortcuts.handle(KeyEvent {
            code: KeyCode::KeyA,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::default(),
        }));
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
    fn shortcuts_can_dispatch_typed_commands() {
        let count = Rc::new(Cell::new(0));
        let mut actions = Actions::new();
        actions.register("save", {
            let count = count.clone();
            move || count.set(count.get() + 1)
        });
        let mut shortcuts = Shortcuts::new();
        shortcuts.bind(
            ShortcutKey::new(KeyCode::KeyA, Modifiers::default()),
            "save",
        );
        assert!(shortcuts.handle_actions(
            KeyEvent {
                code: KeyCode::KeyA,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::default(),
            },
            &actions,
        ));
        assert_eq!(count.get(), 1);
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
}
