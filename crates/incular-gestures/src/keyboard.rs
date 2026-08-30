use std::cell::Cell;

use std::collections::HashMap;

use std::rc::Rc;

use incular_core::{Code, KeyboardEvent, KeyboardKey, Modifiers};

use crate::details::GestureCallbacks;

impl GestureCallbacks {
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
