//! Material interaction state and state-property primitives.

use std::fmt;
use std::sync::Arc;

/// Material interaction states used by state-dependent properties.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Ord, PartialOrd)]
pub enum WidgetState {
    Disabled,
    Error,
    Pressed,
    Dragged,
    Selected,
    Focused,
    Hovered,
    ScrolledUnder,
}

/// A compact set of [`WidgetState`] values.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct WidgetStates(u16);

impl WidgetStates {
    const DISABLED: u16 = 1 << 0;
    const ERROR: u16 = 1 << 1;
    const PRESSED: u16 = 1 << 2;
    const DRAGGED: u16 = 1 << 3;
    const SELECTED: u16 = 1 << 4;
    const FOCUSED: u16 = 1 << 5;
    const HOVERED: u16 = 1 << 6;
    const SCROLLED_UNDER: u16 = 1 << 7;

    #[must_use]
    pub const fn new() -> Self {
        Self(0)
    }

    #[must_use]
    pub const fn bits(self) -> u16 {
        self.0
    }

    #[must_use]
    pub const fn contains(self, state: WidgetState) -> bool {
        self.0 & state.bit() != 0
    }

    #[must_use]
    pub const fn with(mut self, state: WidgetState) -> Self {
        self.0 |= state.bit();
        self
    }

    #[must_use]
    pub const fn without(mut self, state: WidgetState) -> Self {
        self.0 &= !state.bit();
        self
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    #[must_use]
    pub const fn len(self) -> usize {
        self.0.count_ones() as usize
    }

    #[must_use]
    pub const fn from_bits_retain(bits: u16) -> Self {
        Self(bits)
    }

    /// Bridges the Material state set to the headless controls state set.
    /// Components use this at the boundary instead of maintaining a second
    /// state-resolution policy.
    #[must_use]
    pub fn into_control_state(self) -> incular_controls::ControlState {
        let mut state = incular_controls::ControlState::empty();
        if self.contains(WidgetState::Disabled) {
            state.insert(incular_controls::ControlState::DISABLED);
        }
        if self.contains(WidgetState::Pressed) {
            state.insert(incular_controls::ControlState::PRESSED);
        }
        if self.contains(WidgetState::Dragged) {
            state.insert(incular_controls::ControlState::DRAGGING);
        }
        if self.contains(WidgetState::Selected) {
            state.insert(incular_controls::ControlState::SELECTED);
        }
        if self.contains(WidgetState::Focused) {
            state.insert(incular_controls::ControlState::FOCUSED);
        }
        if self.contains(WidgetState::Hovered) {
            state.insert(incular_controls::ControlState::HOVERED);
        }
        if self.contains(WidgetState::Error) {
            state.insert(incular_controls::ControlState::INVALID);
        }
        state
    }
}

impl WidgetState {
    const fn bit(self) -> u16 {
        match self {
            Self::Disabled => WidgetStates::DISABLED,
            Self::Error => WidgetStates::ERROR,
            Self::Pressed => WidgetStates::PRESSED,
            Self::Dragged => WidgetStates::DRAGGED,
            Self::Selected => WidgetStates::SELECTED,
            Self::Focused => WidgetStates::FOCUSED,
            Self::Hovered => WidgetStates::HOVERED,
            Self::ScrolledUnder => WidgetStates::SCROLLED_UNDER,
        }
    }
}

impl std::ops::BitOr for WidgetStates {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOr<WidgetState> for WidgetStates {
    type Output = Self;

    fn bitor(self, rhs: WidgetState) -> Self::Output {
        self.with(rhs)
    }
}

impl From<WidgetState> for WidgetStates {
    fn from(value: WidgetState) -> Self {
        Self::new().with(value)
    }
}

impl From<incular_controls::ControlState> for WidgetStates {
    fn from(value: incular_controls::ControlState) -> Self {
        let mut states = Self::new();
        if value.is_disabled() {
            states = states.with(WidgetState::Disabled);
        }
        if value.is_pressed() {
            states = states.with(WidgetState::Pressed);
        }
        if value.is_dragging() {
            states = states.with(WidgetState::Dragged);
        }
        if value.is_selected() {
            states = states.with(WidgetState::Selected);
        }
        if value.is_focused() {
            states = states.with(WidgetState::Focused);
        }
        if value.is_hovered() {
            states = states.with(WidgetState::Hovered);
        }
        if value.is_invalid() {
            states = states.with(WidgetState::Error);
        }
        states
    }
}

/// A state-dependent Material value.
///
/// `All` and `Map` are allocation-free during resolution. `Resolver` is useful
/// for derived values and is intentionally kept behind an `Arc` so styles can
/// be cloned safely into retained builders.
pub enum StateProperty<T> {
    All(T),
    Map(Vec<(WidgetState, T)>),
    Resolver(Arc<dyn Fn(WidgetStates) -> T + Send + Sync + 'static>),
}

impl<T: Clone> Clone for StateProperty<T> {
    fn clone(&self) -> Self {
        match self {
            Self::All(value) => Self::All(value.clone()),
            Self::Map(values) => Self::Map(values.clone()),
            Self::Resolver(resolver) => Self::Resolver(resolver.clone()),
        }
    }
}

impl<T: fmt::Debug> fmt::Debug for StateProperty<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::All(value) => formatter.debug_tuple("All").field(value).finish(),
            Self::Map(values) => formatter.debug_tuple("Map").field(values).finish(),
            Self::Resolver(_) => formatter.write_str("Resolver(..)"),
        }
    }
}

impl<T: PartialEq> PartialEq for StateProperty<T> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::All(a), Self::All(b)) => a == b,
            (Self::Map(a), Self::Map(b)) => a == b,
            // Resolver closures have no meaningful structural equality.
            (Self::Resolver(_), Self::Resolver(_)) => false,
            _ => false,
        }
    }
}

impl<T: Clone> StateProperty<T> {
    #[must_use]
    pub fn all(value: T) -> Self {
        Self::All(value)
    }

    #[must_use]
    pub fn from_map<const N: usize>(entries: [(WidgetState, T); N]) -> Self {
        Self::Map(entries.into_iter().collect())
    }

    #[must_use]
    pub fn resolve(&self, states: WidgetStates) -> T {
        match self {
            Self::All(value) => value.clone(),
            Self::Map(entries) => entries
                .iter()
                .filter(|(state, _)| states.contains(*state))
                .max_by_key(|(state, _)| state.priority())
                // A sparse mapper has no dedicated "normal" key. Flutter's
                // nullable state properties simply resolve to null in this
                // case; for a non-null Rust value the first entry is the
                // deterministic, allocation-free fallback. Callers that
                // need a different fallback should use `resolve_or`.
                .or_else(|| entries.first())
                .map_or_else(
                    || panic!("StateProperty::Map cannot resolve an empty map"),
                    |(_, value)| value.clone(),
                ),
            Self::Resolver(resolver) => resolver(states),
        }
    }

    #[must_use]
    pub fn resolve_or(&self, states: WidgetStates, fallback: T) -> T {
        match self {
            Self::All(value) => value.clone(),
            Self::Map(entries) => entries
                .iter()
                .filter(|(state, _)| states.contains(*state))
                .max_by_key(|(state, _)| state.priority())
                .map_or(fallback, |(_, value)| value.clone()),
            Self::Resolver(resolver) => resolver(states),
        }
    }
}

impl<T> StateProperty<T> {
    #[must_use]
    pub fn resolve_with(resolver: impl Fn(WidgetStates) -> T + Send + Sync + 'static) -> Self {
        Self::Resolver(Arc::new(resolver))
    }
}

impl<T> From<T> for StateProperty<T> {
    fn from(value: T) -> Self {
        Self::All(value)
    }
}

impl WidgetState {
    const fn priority(self) -> u8 {
        match self {
            Self::Disabled => 100,
            Self::Error => 90,
            Self::Pressed => 80,
            Self::Dragged => 75,
            Self::Selected => 70,
            Self::Focused => 60,
            Self::Hovered => 50,
            Self::ScrolledUnder => 40,
        }
    }
}

/// Compatibility spelling for Flutter's state-property abstraction.
pub type WidgetStateProperty<T> = StateProperty<T>;

/// Compatibility spelling for Flutter's `WidgetStatePropertyAll`.
pub type WidgetStatePropertyAll<T> = StateProperty<T>;

/// Pre-3.16 compatibility spellings. Flutter 3.47.1 uses `WidgetState` in
/// its public Material APIs, but keeping these aliases costs nothing and
/// allows existing Material applications to migrate without a parallel state
/// resolver or duplicate behavior implementation.
pub type MaterialState = WidgetState;
pub type MaterialStates = WidgetStates;
pub type MaterialStateProperty<T> = StateProperty<T>;
pub type MaterialStatePropertyAll<T> = StateProperty<T>;
pub type MaterialStatesController = WidgetStatesController;

/// Typed controller for mutable state sets shared by controls.
#[derive(Clone, Default)]
pub struct WidgetStatesController {
    states: std::rc::Rc<std::cell::Cell<WidgetStates>>,
    changes: incular_core::reactivity::DependencySource,
}

impl WidgetStatesController {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn states(&self) -> WidgetStates {
        self.changes.track();
        self.states.get()
    }

    pub fn set(&self, state: WidgetState, value: bool) {
        let states = if value {
            self.states.get().with(state)
        } else {
            self.states.get().without(state)
        };
        self.update(states);
    }

    pub fn update(&self, states: WidgetStates) {
        if self.states.replace(states) != states {
            self.changes.notify();
        }
    }
}

impl std::fmt::Debug for WidgetStatesController {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WidgetStatesController")
            .field("states", &self.states.get())
            .finish()
    }
}
