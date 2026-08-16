//! Controlled BUILD → LAYOUT → PAINT coordination and local reactive state.
use incular_accessibility::{SemanticAction, SemanticNodeId};
use incular_core::{ImeEvent, InputEvent, KeyCode, KeyEvent, Offset, PointerPhase};
use incular_layout::Constraints;
use incular_painting::DisplayList;
use incular_platform::{Clipboard, MemoryClipboard};
use incular_widgets::{
    ActionId, ButtonState, Diagnostics, ElementId, TreeError, Widget, WidgetTree,
};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet, VecDeque},
    rc::{Rc, Weak},
    time::Instant,
};

trait Dependency {
    fn remove(&self, element: ElementId);
}
struct ReactiveQueue {
    queued: HashSet<ElementId>,
    order: VecDeque<ElementId>,
    dependencies: HashMap<ElementId, Vec<Weak<dyn Dependency>>>,
}
impl ReactiveQueue {
    fn enqueue(&mut self, id: ElementId) {
        if self.queued.insert(id) {
            self.order.push_back(id);
        }
    }
    fn take(&mut self) -> Option<ElementId> {
        let id = self.order.pop_front()?;
        self.queued.remove(&id);
        Some(id)
    }
    fn refresh(&mut self, id: ElementId) {
        if let Some(deps) = self.dependencies.remove(&id) {
            for dep in deps {
                if let Some(dep) = dep.upgrade() {
                    dep.remove(id);
                }
            }
        }
    }
    fn record(&mut self, id: ElementId, dep: Weak<dyn Dependency>) {
        let entries = self.dependencies.entry(id).or_default();
        if !entries.iter().any(|current| current.ptr_eq(&dep)) {
            entries.push(dep);
        }
    }
    fn forget(&mut self, id: ElementId) {
        self.refresh(id);
        self.queued.remove(&id);
    }
}
struct BuildScope {
    element: ElementId,
    queue: Weak<RefCell<ReactiveQueue>>,
}
thread_local! { static BUILD_SCOPE: RefCell<Option<BuildScope>> = const { RefCell::new(None) }; }
struct SignalInner<T> {
    value: RefCell<T>,
    dependents: RefCell<HashSet<ElementId>>,
    queue: RefCell<Weak<RefCell<ReactiveQueue>>>,
}
impl<T> Dependency for SignalInner<T> {
    fn remove(&self, element: ElementId) {
        self.dependents.borrow_mut().remove(&element);
    }
}
/// Shared state. A read in a registered builder subscribes that Element.
#[derive(Clone)]
pub struct Signal<T> {
    inner: Rc<SignalInner<T>>,
}
impl<T: 'static> Signal<T> {
    /// Creates single-threaded application state. It attaches to the runtime
    /// that first reads it during a build; a signal is therefore not shared
    /// between independent applications.
    #[must_use]
    pub fn new(value: T) -> Self {
        Self {
            inner: Rc::new(SignalInner {
                value: RefCell::new(value),
                dependents: RefCell::new(HashSet::new()),
                queue: RefCell::new(Weak::new()),
            }),
        }
    }
    #[must_use]
    pub fn with_runtime(value: T, runtime: &Runtime) -> Self {
        let signal = Self::new(value);
        *signal.inner.queue.borrow_mut() = Rc::downgrade(&runtime.reactive);
        signal
    }
    #[must_use]
    pub fn get(&self) -> T
    where
        T: Clone,
    {
        BUILD_SCOPE.with(|scope| {
            if let Some(scope) = scope.borrow().as_ref() {
                if self.inner.queue.borrow().upgrade().is_none() {
                    *self.inner.queue.borrow_mut() = scope.queue.clone();
                }
                self.inner.dependents.borrow_mut().insert(scope.element);
                let dependency: Rc<dyn Dependency> = self.inner.clone();
                if let Some(queue) = scope.queue.upgrade() {
                    queue
                        .borrow_mut()
                        .record(scope.element, Rc::downgrade(&dependency));
                }
            }
        });
        self.inner.value.borrow().clone()
    }
    pub fn set(&self, value: T) -> bool
    where
        T: PartialEq,
    {
        if *self.inner.value.borrow() == value {
            return false;
        }
        *self.inner.value.borrow_mut() = value;
        if let Some(queue) = self.inner.queue.borrow().upgrade() {
            let ids: Vec<_> = self.inner.dependents.borrow().iter().copied().collect();
            let mut queue = queue.borrow_mut();
            for id in ids {
                queue.enqueue(id);
            }
        }
        true
    }
    #[must_use]
    pub fn dependent_count(&self) -> usize {
        self.inner.dependents.borrow().len()
    }
    /// Mutates the value once and schedules only the Elements that read it.
    pub fn update(&self, update: impl FnOnce(&mut T)) {
        update(&mut self.inner.value.borrow_mut());
        if let Some(queue) = self.inner.queue.borrow().upgrade() {
            let ids: Vec<_> = self.inner.dependents.borrow().iter().copied().collect();
            let mut queue = queue.borrow_mut();
            for id in ids {
                queue.enqueue(id);
            }
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameStats {
    pub updated_elements: usize,
    pub rebuilt_elements: u64,
    pub laid_out_render_objects: u64,
    pub repainted_render_objects: u64,
    pub composited: u64,
    pub active_animations: u64,
    pub display_list_commands: usize,
    pub requested_another_frame: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventTarget {
    pub element: ElementId,
    pub action: Option<ActionId>,
}
/// Debug-facing focus state without exposing native event-loop details.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FocusDiagnostics {
    pub focused_element: Option<ElementId>,
    pub text_pointer_capture: Option<ElementId>,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EditingDiagnostics {
    pub key_down_received: u64,
    pub key_up_received: u64,
    pub backspace_commands: u64,
    pub delete_commands: u64,
    pub text_commits: u64,
    pub ime_events: u64,
}
/// Platform-neutral scheduler; it owns no window or GPU resource.
pub struct Runtime {
    tree: WidgetTree,
    pending: HashMap<ElementId, Widget>,
    order: VecDeque<ElementId>,
    reactive: Rc<RefCell<ReactiveQueue>>,
    builders: HashMap<ElementId, Box<dyn FnMut() -> Widget>>,
    handlers: HashMap<ActionId, Rc<dyn Fn()>>,
    hovered_button: Option<ElementId>,
    pressed_button: Option<ElementId>,
    last_pointer: Offset,
    focused: Option<ElementId>,
    captured_text_field: Option<ElementId>,
    clipboard: Box<dyn Clipboard>,
    editing_diagnostics: EditingDiagnostics,
    frame_requested: bool,
}
impl Runtime {
    pub fn new(root: Widget) -> Result<Self, TreeError> {
        let mut tree = WidgetTree::new();
        tree.mount(root)?;
        Ok(Self {
            tree,
            pending: HashMap::new(),
            order: VecDeque::new(),
            reactive: Rc::new(RefCell::new(ReactiveQueue {
                queued: HashSet::new(),
                order: VecDeque::new(),
                dependencies: HashMap::new(),
            })),
            builders: HashMap::new(),
            handlers: HashMap::new(),
            hovered_button: None,
            pressed_button: None,
            last_pointer: Offset::ZERO,
            focused: None,
            captured_text_field: None,
            clipboard: Box::new(MemoryClipboard::default()),
            editing_diagnostics: EditingDiagnostics::default(),
            frame_requested: true,
        })
    }
    #[must_use]
    pub fn tree(&self) -> &WidgetTree {
        &self.tree
    }
    #[must_use]
    pub fn tree_mut(&mut self) -> &mut WidgetTree {
        &mut self.tree
    }
    #[must_use]
    pub fn focused_element(&self) -> Option<ElementId> {
        self.focused
    }
    /// Dispatches an owned semantic action through the same control state used
    /// by pointer and keyboard input. Native adapters queue these requests;
    /// they never borrow mutable element storage.
    pub fn dispatch_semantic_action(
        &mut self,
        node: SemanticNodeId,
        action: SemanticAction,
    ) -> bool {
        let Some(element) = self.tree.element_for_semantic_node(node) else {
            return false;
        };
        self.tree.note_semantic_action();
        let handled = match action {
            SemanticAction::Focus => {
                self.set_focus(Some(element));
                true
            }
            SemanticAction::Activate => self
                .tree
                .action_for_element(element)
                .and_then(|action| self.handlers.get(&action).cloned())
                .map(|callback| {
                    callback();
                    true
                })
                .unwrap_or(false),
            SemanticAction::SetText(text) => self
                .tree
                .text_controller(element)
                .map(|controller| {
                    controller.set_text(text);
                    true
                })
                .unwrap_or(false),
            SemanticAction::SetSelection { base, extent } => self
                .tree
                .text_controller(element)
                .map(|controller| {
                    let length = controller.text().len();
                    controller.set_selection(incular_widgets::TextSelection {
                        base: base.min(length),
                        extent: extent.min(length),
                    });
                    true
                })
                .unwrap_or(false),
            SemanticAction::ScrollForward => self.tree.semantic_scroll(element, true),
            SemanticAction::ScrollBackward => self.tree.semantic_scroll(element, false),
            SemanticAction::Increment | SemanticAction::Decrement => false,
        };
        if handled {
            self.frame_requested = true;
        }
        handled
    }
    #[must_use]
    pub fn focus_diagnostics(&self) -> FocusDiagnostics {
        FocusDiagnostics {
            focused_element: self.focused,
            text_pointer_capture: self.captured_text_field,
        }
    }
    #[must_use]
    pub const fn editing_diagnostics(&self) -> EditingDiagnostics {
        self.editing_diagnostics
    }
    pub fn set_clipboard(&mut self, clipboard: Box<dyn Clipboard>) {
        self.clipboard = clipboard;
    }
    pub fn set_clipboard_text(&mut self, text: impl Into<String>) {
        self.clipboard.set_text(text.into());
    }
    pub fn register_builder(
        &mut self,
        id: ElementId,
        builder: impl FnMut() -> Widget + 'static,
    ) -> Result<(), TreeError> {
        if !self.tree.element_exists(id) {
            return Err(TreeError::MissingElement(id));
        }
        self.builders.insert(id, Box::new(builder));
        self.rebuild_from_builder(id)
    }
    pub fn schedule_update(&mut self, id: ElementId, widget: Widget) -> Result<(), TreeError> {
        if !self.tree.element_exists(id) {
            return Err(TreeError::MissingElement(id));
        }
        self.tree.mark_build(id)?;
        let mut widget = widget;
        self.prepare_widget(&mut widget);
        if self.pending.insert(id, widget).is_none() {
            self.order.push_back(id);
        }
        self.frame_requested = true;
        Ok(())
    }
    #[must_use]
    pub fn frame_requested(&self) -> bool {
        self.frame_requested || !self.reactive.borrow().queued.is_empty()
    }
    #[must_use]
    pub fn handle_input(&mut self, event: InputEvent) -> Option<EventTarget> {
        let InputEvent::Pointer { phase, position } = event else {
            match event {
                InputEvent::Scroll { delta } => {
                    // The platform normalizes wheel values to logical pixels. The
                    // latest pointer position selects the nearest viewport.
                    if self.tree.scroll_at(self.last_pointer, delta) {
                        self.frame_requested = true;
                    }
                }
                InputEvent::Key(key) => {
                    self.handle_key(key);
                }
                InputEvent::Text(text) => {
                    self.insert_text(&text);
                }
                InputEvent::Ime(ime) => {
                    self.handle_ime(ime);
                }
                InputEvent::WindowResized { .. } => {}
                InputEvent::Pointer { .. } => unreachable!(),
            }
            return None;
        };
        self.last_pointer = position;
        if self.tree.scrollbar_pointer(phase, position) {
            self.frame_requested = true;
            return None;
        }
        let text_target = self.tree.text_field_at(position);
        let target = self
            .tree
            .hit_test(position)
            .and_then(|render| self.tree.element_for_render(render))
            .and_then(|element| self.tree.action_ancestor(element));
        match phase {
            PointerPhase::Move => {
                if let Some(field) = self.captured_text_field {
                    self.tree
                        .text_field_set_caret(field, position, true, Instant::now());
                    self.frame_requested = true;
                }
                self.set_hover(target.map(|(element, _)| element));
                target.map(|(element, action)| EventTarget {
                    element,
                    action: Some(action),
                })
            }
            PointerPhase::Down => {
                self.set_focus(text_target);
                self.captured_text_field = text_target;
                if let Some(field) = text_target {
                    self.tree
                        .text_field_set_caret(field, position, false, Instant::now());
                    self.frame_requested = true;
                    return Some(EventTarget {
                        element: field,
                        action: None,
                    });
                }
                self.set_hover(target.map(|(element, _)| element));
                self.pressed_button = target.map(|(element, _)| element);
                if let Some(element) = self.pressed_button {
                    let _ = self.tree.set_button_state(element, ButtonState::Pressed);
                    self.frame_requested = true;
                }
                target.map(|(element, action)| EventTarget {
                    element,
                    action: Some(action),
                })
            }
            PointerPhase::Up => {
                self.captured_text_field = None;
                let pressed = self.pressed_button.take();
                let valid = pressed
                    .zip(target)
                    .filter(|(pressed, (target, _))| pressed == target)
                    .map(|(_, target)| target);
                if let Some(element) = pressed {
                    let _ = self.tree.set_button_state(
                        element,
                        if self.hovered_button == Some(element) {
                            ButtonState::Hovered
                        } else {
                            ButtonState::Normal
                        },
                    );
                }
                if let Some((element, action)) = valid {
                    if let Some(callback) = self.handlers.get(&action).cloned() {
                        callback();
                        self.frame_requested = true;
                    }
                    Some(EventTarget {
                        element,
                        action: Some(action),
                    })
                } else {
                    None
                }
            }
            PointerPhase::Cancel => {
                self.captured_text_field = None;
                if let Some(element) = self.pressed_button.take() {
                    let _ = self.tree.set_button_state(element, ButtonState::Normal);
                    self.frame_requested = true;
                }
                None
            }
        }
    }
    fn set_focus(&mut self, next: Option<ElementId>) {
        if self.focused == next {
            return;
        }
        if let Some(previous) = self.focused {
            let _ = self.tree.set_focused(previous, false, Instant::now());
        }
        self.focused = next;
        if let Some(current) = next {
            let _ = self.tree.set_focused(current, true, Instant::now());
        }
        self.frame_requested = true;
    }
    fn focus_next(&mut self, reverse: bool) {
        let fields = self.tree.focusable_elements();
        if fields.is_empty() {
            return;
        }
        let index = self
            .focused
            .and_then(|focused| fields.iter().position(|id| *id == focused));
        let next = match index {
            Some(index) if reverse => fields[(index + fields.len() - 1) % fields.len()],
            Some(index) => fields[(index + 1) % fields.len()],
            None if reverse => *fields.last().expect("not empty"),
            None => fields[0],
        };
        self.set_focus(Some(next));
    }
    fn handle_key(&mut self, event: KeyEvent) {
        if event.pressed {
            self.editing_diagnostics.key_down_received += 1;
        } else {
            self.editing_diagnostics.key_up_received += 1;
        }
        if !event.pressed {
            return;
        }
        if event.code == KeyCode::Tab {
            self.focus_next(event.modifiers.shift);
            return;
        }
        let Some(field) = self.focused.filter(|id| self.tree.is_text_field(*id)) else {
            return;
        };
        let Some(controller) = self.tree.text_controller(field) else {
            return;
        };
        let extend = event.modifiers.shift;
        if event.modifiers.command {
            match event.code {
                KeyCode::KeyA => controller.select_all(),
                KeyCode::KeyC => self.clipboard.set_text(controller.selected_text()),
                KeyCode::KeyX => {
                    self.clipboard.set_text(controller.selected_text());
                    controller.replace_selection("");
                }
                KeyCode::KeyV => {
                    if let Some(text) = self.clipboard.get_text() {
                        controller.insert(&text);
                    }
                }
                _ => return,
            }
        } else {
            match event.code {
                KeyCode::Backspace => {
                    controller.backspace();
                    self.editing_diagnostics.backspace_commands += 1;
                }
                KeyCode::Delete => {
                    controller.delete();
                    self.editing_diagnostics.delete_commands += 1;
                }
                KeyCode::ArrowLeft => controller.move_left(extend),
                KeyCode::ArrowRight => controller.move_right(extend),
                KeyCode::ArrowUp => {
                    let _ = self.tree.text_field_move_vertical(field, false, extend);
                }
                KeyCode::ArrowDown => {
                    let _ = self.tree.text_field_move_vertical(field, true, extend);
                }
                KeyCode::Home => {
                    if !self.tree.text_field_move_line_edge(field, false, extend) {
                        controller.move_home(extend);
                    }
                }
                KeyCode::End => {
                    if !self.tree.text_field_move_line_edge(field, true, extend) {
                        controller.move_end(extend);
                    }
                }
                KeyCode::Enter => {
                    if self.tree.is_multiline_text_field(field) {
                        controller.insert("\n");
                    } else {
                        self.tree.submit_text_field(field);
                    }
                }
                _ => return,
            }
        }
        controller.reset_caret(Instant::now());
        self.frame_requested = true;
    }
    fn insert_text(&mut self, text: &str) {
        let Some(field) = self.focused.filter(|id| self.tree.is_text_field(*id)) else {
            return;
        };
        if text.chars().any(char::is_control) {
            return;
        }
        if let Some(controller) = self.tree.text_controller(field) {
            controller.insert(text);
            self.editing_diagnostics.text_commits += 1;
            controller.reset_caret(Instant::now());
            self.frame_requested = true;
        }
    }
    fn handle_ime(&mut self, event: ImeEvent) {
        self.editing_diagnostics.ime_events += 1;
        let Some(field) = self.focused.filter(|id| self.tree.is_text_field(*id)) else {
            return;
        };
        let Some(controller) = self.tree.text_controller(field) else {
            return;
        };
        match event {
            ImeEvent::Preedit { text, selection } => controller.set_preedit(
                text,
                selection.map(|(start, end)| incular_widgets::TextRange::new(start, end)),
            ),
            ImeEvent::Commit(text) => controller.commit_preedit(&text),
            ImeEvent::End => controller.clear_preedit(),
        }
        controller.reset_caret(Instant::now());
        self.frame_requested = true;
    }
    pub fn run_frame(
        &mut self,
        constraints: Constraints,
    ) -> Result<(DisplayList, FrameStats), TreeError> {
        self.run_frame_at(constraints, Instant::now())
    }
    /// Deterministic frame entry point used by animation tests and embedders
    /// with an existing monotonic clock.
    pub fn run_frame_at(
        &mut self,
        constraints: Constraints,
        now: Instant,
    ) -> Result<(DisplayList, FrameStats), TreeError> {
        let before = self.tree.diagnostics();
        let mut updated = 0;
        while let Some(id) = self.order.pop_front() {
            if let Some(widget) = self.pending.remove(&id) {
                if self.tree.element_exists(id) {
                    self.tree.update(id, widget)?;
                    updated += 1;
                }
            }
        }
        while let Some(id) = { self.reactive.borrow_mut().take() } {
            if self.tree.element_exists(id) && self.builders.contains_key(&id) {
                self.rebuild_from_builder(id)?;
                updated += 1;
            }
        }
        for id in self.tree.take_unmounted() {
            self.builders.remove(&id);
            self.reactive.borrow_mut().forget(id);
        }
        self.prune_handlers();
        self.tree.layout(constraints);
        for (action, handler) in self.tree.take_pending_handlers() {
            self.handlers.insert(action, handler);
        }
        // Lazy viewport expiry occurs during layout, after the ordinary dirty
        // queue drain above. Release those builder subscriptions and callbacks
        // in the same frame rather than retaining one stale cache generation.
        for id in self.tree.take_unmounted() {
            self.builders.remove(&id);
            self.reactive.borrow_mut().forget(id);
        }
        self.prune_handlers();
        let (composited, animations_active) = self.tree.update_compositor(now);
        self.tree.update_semantics();
        let display_list = self.tree.paint();
        self.frame_requested = !self.pending.is_empty()
            || !self.reactive.borrow().queued.is_empty()
            || animations_active;
        let after = self.tree.diagnostics();
        Ok((
            display_list.clone(),
            FrameStats {
                updated_elements: updated,
                rebuilt_elements: after.rebuilds - before.rebuilds,
                laid_out_render_objects: after.layouts - before.layouts,
                repainted_render_objects: after.paints - before.paints,
                composited: u64::from(composited),
                active_animations: u64::from(animations_active),
                display_list_commands: display_list.len(),
                requested_another_frame: self.frame_requested,
            },
        ))
    }
    #[must_use]
    pub fn diagnostics(&self) -> Diagnostics {
        self.tree.diagnostics()
    }
    fn rebuild_from_builder(&mut self, id: ElementId) -> Result<(), TreeError> {
        self.reactive.borrow_mut().refresh(id);
        let old = BUILD_SCOPE.with(|scope| {
            scope.replace(Some(BuildScope {
                element: id,
                queue: Rc::downgrade(&self.reactive),
            }))
        });
        let widget = self.builders.get_mut(&id).expect("registered builder")();
        BUILD_SCOPE.with(|scope| {
            scope.replace(old);
        });
        let mut widget = widget;
        self.prepare_widget(&mut widget);
        self.tree.update(id, widget)?;
        self.prune_handlers();
        Ok(())
    }
    fn prepare_widget(&mut self, widget: &mut Widget) {
        let handlers = &mut self.handlers;
        let tree = &mut self.tree;
        widget.bind_callbacks(&mut |callback| {
            let id = tree.allocate_action();
            handlers.insert(id, callback);
            id
        });
    }
    fn prune_handlers(&mut self) {
        let active = self.tree.action_ids();
        self.handlers.retain(|id, _| active.contains(id));
    }
    fn set_hover(&mut self, next: Option<ElementId>) {
        if self.hovered_button == next {
            return;
        }
        if let Some(previous) = self.hovered_button {
            let _ = self.tree.set_button_state(previous, ButtonState::Normal);
        }
        self.hovered_button = next;
        if let Some(current) = next {
            if self.pressed_button != Some(current) {
                let _ = self.tree.set_button_state(current, ButtonState::Hovered);
            }
        }
        self.frame_requested = true;
    }
}

/// Framework-controlled build scope for declarative roots. It intentionally
/// exposes no element IDs or scheduler handles. Signals use the runtime's
/// scoped collector while a builder executes; reads outside a build simply
/// return the current value and create no dependency.
pub struct BuildContext {
    _private: (),
}
impl BuildContext {
    fn new() -> Self {
        Self { _private: () }
    }
}

/// Owns a declarative root builder. The runner consumes it, so applications do
/// not retain mutable access to `Runtime`.
pub struct Application {
    runtime: Runtime,
}
impl Application {
    pub fn new(
        build: impl FnMut(&mut BuildContext) -> Widget + 'static,
    ) -> Result<Self, TreeError> {
        let build = Rc::new(RefCell::new(build));
        let initial = (build.borrow_mut())(&mut BuildContext::new());
        let mut runtime = Runtime::new(initial)?;
        let root = runtime.tree().root().expect("new runtime has root");
        let closure = build.clone();
        runtime.register_builder(root, move || {
            (closure.borrow_mut())(&mut BuildContext::new())
        })?;
        Ok(Self { runtime })
    }
    #[must_use]
    pub fn into_runtime(self) -> Runtime {
        self.runtime
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use incular_accessibility::{Role as SemanticRole, SemanticAction};
    use incular_core::{Color, Offset, Size};
    use incular_painting::{DisplayList, PaintCommand};
    use incular_widgets::{Button, VirtualList};
    use std::cell::Cell;
    use std::time::{Duration, Instant};

    fn semantic_node(
        runtime: &Runtime,
        role: SemanticRole,
    ) -> incular_accessibility::SemanticNodeId {
        runtime
            .tree()
            .semantics()
            .iter()
            .find_map(|(id, node)| (node.role == role).then_some(id))
            .expect("semantic node")
    }

    #[test]
    fn semantic_actions_share_logical_button_and_editing_state() {
        use incular_widgets::{TextEditingController, TextField};
        let hits = Rc::new(Cell::new(0));
        let controller = TextEditingController::with_text("Ada");
        let mut runtime = Runtime::new(Widget::column(vec![
            Button::new("Increment")
                .on_press({
                    let hits = hits.clone();
                    move || hits.set(hits.get() + 1)
                })
                .into(),
            TextField::new(controller.clone()).into(),
        ]))
        .unwrap();
        let root = runtime.tree().root().unwrap();
        runtime
            .schedule_update(
                root,
                Widget::column(vec![
                    Button::new("Increment")
                        .on_press({
                            let hits = hits.clone();
                            move || hits.set(hits.get() + 1)
                        })
                        .into(),
                    TextField::new(controller.clone()).into(),
                ]),
            )
            .unwrap();
        runtime
            .run_frame(Constraints::tight(Size::new(300., 200.)))
            .unwrap();
        let button = semantic_node(&runtime, SemanticRole::Button);
        let field = semantic_node(&runtime, SemanticRole::TextField);
        assert!(runtime.dispatch_semantic_action(button, SemanticAction::Activate));
        assert_eq!(hits.get(), 1);
        assert!(runtime.dispatch_semantic_action(field, SemanticAction::Focus));
        assert_eq!(
            runtime.focused_element(),
            runtime.tree().element_for_semantic_node(field)
        );
        assert!(runtime.dispatch_semantic_action(field, SemanticAction::SetText("hello".into())));
        assert!(
            runtime.dispatch_semantic_action(
                field,
                SemanticAction::SetSelection { base: 1, extent: 4 }
            )
        );
        assert_eq!(controller.text(), "hello");
        assert_eq!(
            controller.value().selection,
            incular_widgets::TextSelection { base: 1, extent: 4 }
        );
    }

    #[test]
    fn virtual_list_semantics_are_bounded_and_follow_materialization() {
        let controller = incular_widgets::ScrollController::new();
        let mut runtime = Runtime::new(VirtualList::fixed_extent_with_controller(
            1_000_000,
            40.,
            controller.clone(),
            |index| Button::new(format!("Item {index}")),
        ))
        .unwrap();
        runtime
            .run_frame(Constraints::tight(Size::new(200., 600.)))
            .unwrap();
        let initial = runtime.tree().semantics().len();
        assert!(initial < 100, "{initial}");
        let list = semantic_node(&runtime, SemanticRole::List);
        assert_eq!(
            runtime
                .tree()
                .semantics()
                .node(list)
                .unwrap()
                .state
                .set_size,
            Some(1_000_000)
        );
        controller.jump_to(900_000. * 40.);
        runtime
            .run_frame(Constraints::tight(Size::new(200., 600.)))
            .unwrap();
        assert!(runtime.tree().semantics().len() < 100);
        assert!(runtime.tree().semantics().iter().any(|(_, node)| {
            node.label
                .as_deref()
                .is_some_and(|label| label.contains("900000"))
        }));
    }

    #[test]
    fn semantic_scroll_uses_existing_controller() {
        let controller = incular_widgets::ScrollController::new();
        let mut runtime = Runtime::new(incular_widgets::ScrollView::vertical(
            controller.clone(),
            Widget::fixed_box(Size::new(100., 2000.), Color::WHITE),
        ))
        .unwrap();
        runtime
            .run_frame(Constraints::tight(Size::new(100., 200.)))
            .unwrap();
        let scroll = semantic_node(&runtime, SemanticRole::ScrollView);
        assert!(runtime.dispatch_semantic_action(scroll, SemanticAction::ScrollForward));
        assert!(controller.offset() > 0.);
        assert!(runtime.dispatch_semantic_action(scroll, SemanticAction::ScrollBackward));
        assert_eq!(controller.offset(), 0.);
    }

    fn picture_origins(list: &DisplayList) -> (Offset, Offset) {
        let mut transforms = vec![Offset::ZERO];
        let mut rect = None;
        let mut glyph = None;
        for command in list.commands() {
            match command {
                PaintCommand::PushTransform { transform } => {
                    transforms.push(*transforms.last().unwrap() + transform.translation);
                }
                PaintCommand::PopTransform => {
                    transforms.pop();
                }
                PaintCommand::Rect { rect: bounds, .. } => {
                    rect = Some(bounds.origin + *transforms.last().unwrap());
                }
                PaintCommand::GlyphRun { run, .. } => {
                    glyph = Some(run.origin + *transforms.last().unwrap());
                }
                PaintCommand::Image { .. }
                | PaintCommand::RRect { .. }
                | PaintCommand::Border { .. }
                | PaintCommand::FillPath { .. }
                | PaintCommand::StrokePath { .. }
                | PaintCommand::PushClip { .. }
                | PaintCommand::PushClipRRect { .. }
                | PaintCommand::PushClipPath { .. }
                | PaintCommand::PopClip
                | PaintCommand::PushOpacity { .. }
                | PaintCommand::PopOpacity
                | PaintCommand::PushBlur { .. }
                | PaintCommand::PushDropShadow { .. }
                | PaintCommand::PushColorFilter { .. }
                | PaintCommand::PushBlend { .. }
                | PaintCommand::PopEffect => {}
            }
        }
        (rect.unwrap(), glyph.unwrap())
    }
    #[test]
    fn signals_schedule_only_subscribed_element_once() {
        let mut runtime = Runtime::new(Widget::row(vec![
            Widget::box_(Size::new(1., 1.), Color::WHITE),
            Widget::box_(Size::new(2., 2.), Color::WHITE),
        ]))
        .unwrap();
        let root = runtime.tree().root().unwrap();
        let children = runtime.tree().children(root).unwrap().to_vec();
        let signal = Signal::with_runtime(2_u32, &runtime);
        let state = signal.clone();
        runtime
            .register_builder(children[1], move || {
                Widget::box_(Size::new(state.get() as f32, 2.), Color::WHITE)
            })
            .unwrap();
        assert_eq!(signal.dependent_count(), 1);
        assert!(signal.set(3));
        let (_, stats) = runtime
            .run_frame(Constraints::tight(Size::new(20., 20.)))
            .unwrap();
        assert_eq!(stats.updated_elements, 1);
        assert!(!runtime.tree().is_build_dirty(children[0]));
        assert!(!signal.set(3));
    }
    #[test]
    fn hit_test_resolves_button_action() {
        let mut runtime = Runtime::new(Widget::button(
            Size::new(10., 10.),
            Color::WHITE,
            ActionId(1),
        ))
        .unwrap();
        runtime
            .run_frame(Constraints::tight(Size::new(20., 20.)))
            .unwrap();
        let _ = runtime.handle_input(InputEvent::Pointer {
            phase: PointerPhase::Down,
            position: Offset::new(5., 5.),
        });
        assert_eq!(
            runtime
                .handle_input(InputEvent::Pointer {
                    phase: PointerPhase::Up,
                    position: Offset::new(5., 5.)
                })
                .unwrap()
                .action,
            Some(ActionId(1))
        );
    }
    #[test]
    fn wheel_updates_only_retained_scroll_transform() {
        let controller = incular_widgets::ScrollController::new();
        let child = Widget::column(
            (0..8)
                .map(|_| Widget::box_(Size::new(80., 40.), Color::WHITE))
                .collect::<Vec<_>>(),
        );
        let mut runtime = Runtime::new(Widget::scroll_view(controller.clone(), child)).unwrap();
        let constraints = Constraints::tight(Size::new(100., 100.));
        let (_, initial) = runtime.run_frame(constraints).unwrap();
        assert!(
            initial.laid_out_render_objects > 0
                && initial.repainted_render_objects > 0
                && initial.composited > 0
        );
        let _ = runtime.handle_input(InputEvent::Pointer {
            phase: PointerPhase::Move,
            position: Offset::new(10., 10.),
        });
        let _ = runtime.handle_input(InputEvent::Scroll {
            delta: Offset::new(0., 60.),
        });
        let (_, frame) = runtime.run_frame(constraints).unwrap();
        assert_eq!(frame.rebuilt_elements, 0);
        assert_eq!(frame.laid_out_render_objects, 0);
        assert!(frame.repainted_render_objects <= 1); // overlay scrollbar only
        assert!(frame.composited > 0);
        assert_eq!(controller.offset(), 60.);
    }
    #[test]
    fn translated_button_hit_tests_at_its_visible_position_without_repaint() {
        let controller = incular_widgets::TranslationController::new();
        controller.set_offset(Offset::new(0., 30.));
        let mut runtime = Runtime::new(Widget::translate(
            controller.clone(),
            Widget::button(Size::new(20., 20.), Color::WHITE, ActionId(9)),
        ))
        .unwrap();
        let constraints = Constraints::tight(Size::new(100., 100.));
        let _ = runtime.run_frame(constraints).unwrap();
        let (_, frame) = runtime.run_frame(constraints).unwrap();
        assert_eq!(frame.repainted_render_objects, 0);
        assert!(
            runtime
                .handle_input(InputEvent::Pointer {
                    phase: PointerPhase::Down,
                    position: Offset::new(5., 35.)
                })
                .is_some()
        );
        assert!(
            runtime
                .handle_input(InputEvent::Pointer {
                    phase: PointerPhase::Down,
                    position: Offset::new(5., 5.)
                })
                .is_none()
        );
    }
    #[test]
    fn scrolled_button_hits_at_visible_not_old_location() {
        let controller = incular_widgets::ScrollController::new();
        let content = Widget::column(vec![
            Widget::box_(Size::new(80., 160.), Color::WHITE),
            Widget::button(Size::new(20., 20.), Color::WHITE, ActionId(12)),
        ]);
        let mut runtime = Runtime::new(Widget::scroll_view(controller.clone(), content)).unwrap();
        let constraints = Constraints::tight(Size::new(100., 100.));
        let _ = runtime.run_frame(constraints).unwrap();
        assert!(controller.jump_to(80.));
        let (_, frame) = runtime.run_frame(constraints).unwrap();
        assert!(frame.repainted_render_objects <= 1); // overlay scrollbar only
        assert_eq!(
            runtime
                .handle_input(InputEvent::Pointer {
                    phase: PointerPhase::Down,
                    position: Offset::new(5., 85.)
                })
                .unwrap()
                .action,
            Some(ActionId(12))
        );
        assert!(
            runtime
                .handle_input(InputEvent::Pointer {
                    phase: PointerPhase::Down,
                    position: Offset::new(5., 165.)
                })
                .is_none()
        );
    }
    #[test]
    fn animation_ticks_request_frames_without_rebuild_or_paint() {
        let controller = incular_widgets::TranslationController::new();
        let mut runtime = Runtime::new(Widget::translate(
            controller.clone(),
            Widget::text("warm text"),
        ))
        .unwrap();
        let constraints = Constraints::tight(Size::new(100., 100.));
        let origin = Instant::now();
        let _ = runtime.run_frame_at(constraints, origin).unwrap();
        let text_before = runtime.tree().text_diagnostics();
        controller.animate_to(Offset::new(50., 0.), Duration::from_millis(1000), origin);
        let (_, frame) = runtime
            .run_frame_at(constraints, origin + Duration::from_millis(500))
            .unwrap();
        assert_eq!(frame.rebuilt_elements, 0);
        assert_eq!(frame.laid_out_render_objects, 0);
        assert_eq!(frame.repainted_render_objects, 0);
        assert!(
            frame.composited > 0 && frame.active_animations > 0 && frame.requested_another_frame
        );
        assert_eq!(runtime.tree().text_diagnostics(), text_before);
    }
    #[test]
    fn retained_card_text_and_background_move_together_without_repaint() {
        let controller = incular_widgets::TranslationController::new();
        let mut runtime = Runtime::new(Widget::padding(
            incular_layout::EdgeInsets {
                left: 20.,
                top: 10.,
                right: 0.,
                bottom: 0.,
            },
            Widget::translate(
                controller.clone(),
                Widget::column(vec![
                    Widget::box_(Size::new(80., 20.), Color::WHITE),
                    Widget::text("cached card text"),
                ]),
            ),
        ))
        .unwrap();
        let constraints = Constraints::tight(Size::new(200., 100.));
        let (before, _) = runtime.run_frame(constraints).unwrap();
        let text_before = runtime.tree().text_diagnostics();
        let (before_rect, before_glyph) = picture_origins(&before);
        controller.set_offset(Offset::new(50., 0.));
        let (after, frame) = runtime.run_frame(constraints).unwrap();
        let (after_rect, after_glyph) = picture_origins(&after);
        assert_eq!(frame.rebuilt_elements, 0);
        assert_eq!(frame.laid_out_render_objects, 0);
        assert_eq!(frame.repainted_render_objects, 0);
        assert!(frame.composited > 0);
        assert_eq!(after_rect - before_rect, Offset::new(50., 0.));
        assert_eq!(after_glyph - before_glyph, Offset::new(50., 0.));
        assert_eq!(runtime.tree().text_diagnostics(), text_before);
    }
    #[test]
    fn unmount_removes_signal_subscription() {
        let mut runtime = Runtime::new(Widget::row(vec![Widget::box_(
            Size::new(2., 2.),
            Color::WHITE,
        )]))
        .unwrap();
        let root = runtime.tree().root().unwrap();
        let child = runtime.tree().children(root).unwrap()[0];
        let signal = Signal::with_runtime(1_u32, &runtime);
        let state = signal.clone();
        runtime
            .register_builder(child, move || {
                Widget::box_(Size::new(state.get() as f32, 2.), Color::WHITE)
            })
            .unwrap();
        runtime
            .schedule_update(root, Widget::row(Vec::new()))
            .unwrap();
        runtime
            .run_frame(Constraints::tight(Size::new(20., 20.)))
            .unwrap();
        assert_eq!(signal.dependent_count(), 0);
    }
    #[test]
    fn declarative_button_dispatches_only_a_completed_press() {
        let hits = Rc::new(Cell::new(0));
        let callback_hits = hits.clone();
        let app = Application::new(move |_| {
            Button::new("Add")
                .on_press({
                    let callback_hits = callback_hits.clone();
                    move || callback_hits.set(callback_hits.get() + 1)
                })
                .into()
        })
        .unwrap();
        let mut runtime = app.into_runtime();
        runtime
            .run_frame(Constraints::tight(Size::new(120., 60.)))
            .unwrap();
        let _ = runtime.handle_input(InputEvent::Pointer {
            phase: PointerPhase::Down,
            position: Offset::new(10., 10.),
        });
        let _ = runtime.handle_input(InputEvent::Pointer {
            phase: PointerPhase::Up,
            position: Offset::new(121., 50.),
        });
        assert_eq!(hits.get(), 0);
        let _ = runtime.handle_input(InputEvent::Pointer {
            phase: PointerPhase::Down,
            position: Offset::new(10., 10.),
        });
        let _ = runtime.handle_input(InputEvent::Pointer {
            phase: PointerPhase::Up,
            position: Offset::new(10., 10.),
        });
        assert_eq!(hits.get(), 1);
    }

    #[test]
    fn virtual_list_keeps_small_scrolls_compositor_only_and_direct_jumps_bounded() {
        let controller = incular_widgets::ScrollController::new();
        let calls = Rc::new(Cell::new(0));
        let observed = calls.clone();
        let mut runtime = Runtime::new(VirtualList::fixed_extent_with_controller(
            1_000_000,
            40.,
            controller.clone(),
            move |index| {
                observed.set(observed.get() + 1);
                Widget::text(format!("Item {index}"))
            },
        ))
        .unwrap();
        let constraints = Constraints::tight(Size::new(120., 100.));
        let (_, initial) = runtime.run_frame(constraints).unwrap();
        assert!(initial.laid_out_render_objects > 0 && initial.repainted_render_objects > 0);
        let warm_calls = calls.get();
        let warm_text = runtime.tree().text_diagnostics();
        assert!(controller.jump_to(3.));
        let (_, small) = runtime.run_frame(constraints).unwrap();
        assert_eq!(calls.get(), warm_calls);
        assert_eq!(small.rebuilt_elements, 0);
        assert_eq!(small.laid_out_render_objects, 0);
        assert!(small.repainted_render_objects <= 1); // overlay scrollbar only
        assert!(small.composited > 0);
        assert_eq!(runtime.tree().text_diagnostics(), warm_text);

        let boundary_before = runtime.diagnostics();
        assert!(controller.jump_to(40.));
        let (_, boundary) = runtime.run_frame(constraints).unwrap();
        assert_eq!(calls.get(), warm_calls + 1);
        let boundary_after = runtime.diagnostics();
        assert_eq!(boundary_after.items_built - boundary_before.items_built, 1);
        assert_eq!(
            boundary_after.items_mounted - boundary_before.items_mounted,
            1
        );
        assert_eq!(
            boundary_after.items_unmounted - boundary_before.items_unmounted,
            0
        );
        assert!(boundary.laid_out_render_objects <= 2);
        assert!(boundary.repainted_render_objects <= 2);

        assert!(controller.jump_to(900_000. * 40.));
        let (_, jumped) = runtime.run_frame(constraints).unwrap();
        let diagnostics = runtime.tree().virtual_list_diagnostics().unwrap();
        assert!(diagnostics.materialized_range.contains(&900_000));
        assert!(diagnostics.materialized_item_count < 100);
        assert!(calls.get() < 200);
        assert!(jumped.laid_out_render_objects < 100);
        assert!(diagnostics.picture_layer_count < 250);
    }

    #[test]
    fn virtual_button_rows_hit_their_logical_index_and_drop_stale_handlers() {
        let controller = incular_widgets::ScrollController::new();
        let hit = Rc::new(Cell::new(None));
        let observed = hit.clone();
        let mut runtime = Runtime::new(VirtualList::fixed_extent_with_controller(
            2_000,
            40.,
            controller.clone(),
            move |index| {
                let hit = observed.clone();
                Button::new(format!("Item {index}")).on_press(move || hit.set(Some(index)))
            },
        ))
        .unwrap();
        let constraints = Constraints::tight(Size::new(120., 40.));
        runtime.run_frame(constraints).unwrap();
        assert!(controller.jump_to(500. * 40.));
        runtime.run_frame(constraints).unwrap();
        let down = runtime
            .handle_input(InputEvent::Pointer {
                phase: PointerPhase::Down,
                position: Offset::new(10., 10.),
            })
            .unwrap();
        let _ = runtime.handle_input(InputEvent::Pointer {
            phase: PointerPhase::Up,
            position: Offset::new(10., 10.),
        });
        assert_eq!(hit.get(), Some(500));
        let stale_action = down.action.unwrap();
        assert!(controller.jump_to(1_500. * 40.));
        runtime.run_frame(constraints).unwrap();
        assert!(!runtime.handlers.contains_key(&stale_action));
    }

    #[test]
    fn long_virtual_scroll_keeps_retained_resources_bounded() {
        let controller = incular_widgets::ScrollController::new();
        let mut runtime = Runtime::new(VirtualList::fixed_extent_with_controller(
            50_000,
            40.,
            controller.clone(),
            move |index| Button::new(format!("Item {index}")),
        ))
        .unwrap();
        let constraints = Constraints::tight(Size::new(120., 120.));
        runtime.run_frame(constraints).unwrap();
        for index in (97..10_000).step_by(97) {
            assert!(controller.jump_to(index as f32 * 40.));
            runtime.run_frame(constraints).unwrap();
            let view = runtime.tree().virtual_list_diagnostics().unwrap();
            assert!(view.materialized_item_count < 30);
            assert!(view.element_count < 100);
            assert!(view.render_object_count < 100);
            assert!(view.picture_layer_count < 200);
            assert!(runtime.handlers.len() < 30);
        }
    }

    #[test]
    fn focus_routes_text_shortcuts_and_ime_without_rebuilding_tree() {
        use incular_core::{ImeEvent, KeyCode, KeyEvent, Modifiers};
        use incular_widgets::{TextEditingController, TextField};
        let first = TextEditingController::new();
        let second = TextEditingController::new();
        let mut runtime = Runtime::new(Widget::column(vec![
            TextField::new(first.clone())
                .size(Size::new(120., 40.))
                .into(),
            TextField::new(second.clone())
                .size(Size::new(120., 40.))
                .into(),
        ]))
        .unwrap();
        let constraints = Constraints::tight(Size::new(160., 100.));
        runtime.run_frame(constraints).unwrap();
        let before = runtime.tree().diagnostics();
        let _ = runtime.handle_input(InputEvent::Pointer {
            phase: PointerPhase::Down,
            position: Offset::new(5., 5.),
        });
        let _ = runtime.handle_input(InputEvent::Text("café".into()));
        let _ = runtime.handle_input(InputEvent::Ime(ImeEvent::Preedit {
            text: "世界".into(),
            selection: None,
        }));
        assert_eq!(first.text(), "café");
        let _ = runtime.handle_input(InputEvent::Ime(ImeEvent::Commit("世界".into())));
        assert_eq!(first.text(), "café世界");
        let _ = runtime.handle_input(InputEvent::Key(KeyEvent {
            code: KeyCode::Tab,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::default(),
        }));
        assert_ne!(runtime.focused_element(), runtime.tree().root());
        let _ = runtime.handle_input(InputEvent::Text("next".into()));
        assert_eq!(second.text(), "next");
        let _ = runtime.handle_input(InputEvent::Key(KeyEvent {
            code: KeyCode::KeyA,
            pressed: true,
            repeat: false,
            modifiers: Modifiers {
                command: true,
                ..Modifiers::default()
            },
        }));
        let _ = runtime.handle_input(InputEvent::Key(KeyEvent {
            code: KeyCode::KeyX,
            pressed: true,
            repeat: false,
            modifiers: Modifiers {
                command: true,
                ..Modifiers::default()
            },
        }));
        assert_eq!(second.text(), "");
        let _ = runtime.handle_input(InputEvent::Key(KeyEvent {
            code: KeyCode::KeyV,
            pressed: true,
            repeat: false,
            modifiers: Modifiers {
                command: true,
                ..Modifiers::default()
            },
        }));
        assert_eq!(second.text(), "next");
        let (_, stats) = runtime.run_frame(constraints).unwrap();
        assert_eq!(runtime.tree().diagnostics().rebuilds, before.rebuilds);
        // The two fields plus their flex ancestor are repainted; unrelated
        // widget descriptions were not rebuilt.
        assert!(stats.repainted_render_objects <= 3);
    }

    #[test]
    fn focused_native_style_backspace_repeat_and_delete_edit_the_buffer() {
        use incular_core::{KeyCode, KeyEvent, Modifiers};
        use incular_widgets::{TextEditingController, TextField};
        let controller = TextEditingController::with_text("abc");
        let mut runtime = Runtime::new(TextField::new(controller.clone()).into()).unwrap();
        let constraints = Constraints::tight(Size::new(140., 50.));
        runtime.run_frame(constraints).unwrap();
        let _ = runtime.handle_input(InputEvent::Pointer {
            phase: PointerPhase::Down,
            position: Offset::new(100., 5.),
        });
        for expected in ["ab", "a", "", ""] {
            let _ = runtime.handle_input(InputEvent::Key(KeyEvent {
                code: KeyCode::Backspace,
                pressed: true,
                repeat: true,
                modifiers: Modifiers::default(),
            }));
            assert_eq!(controller.text(), expected);
        }
        controller.set_text("é👩‍💻");
        let _ = runtime.handle_input(InputEvent::Key(KeyEvent {
            code: KeyCode::Backspace,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::default(),
        }));
        assert_eq!(controller.text(), "é");
        controller.set_selection(incular_widgets::TextSelection::collapsed(0));
        let _ = runtime.handle_input(InputEvent::Key(KeyEvent {
            code: KeyCode::Delete,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::default(),
        }));
        assert_eq!(controller.text(), "");
        assert_eq!(runtime.editing_diagnostics().backspace_commands, 5);
        assert_eq!(runtime.editing_diagnostics().delete_commands, 1);
    }

    #[test]
    fn multiline_enter_replaces_selection_while_single_line_submits() {
        use incular_core::{KeyCode, KeyEvent, Modifiers};
        use incular_widgets::{TextArea, TextEditingController, TextField, TextSelection};
        let single = TextEditingController::with_text("one");
        let multi = TextEditingController::with_text("ab cdef");
        let submitted = Rc::new(RefCell::new(0));
        let observed = submitted.clone();
        let mut runtime = Runtime::new(Widget::column(vec![
            TextField::new(single.clone())
                .on_submit(move |_| *observed.borrow_mut() += 1)
                .into(),
            TextArea::new(multi.clone()).height(100.).into(),
        ]))
        .unwrap();
        runtime
            .run_frame(Constraints::tight(Size::new(300., 200.)))
            .unwrap();
        let enter = || {
            InputEvent::Key(KeyEvent {
                code: KeyCode::Enter,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::default(),
            })
        };
        let _ = runtime.handle_input(InputEvent::Pointer {
            phase: PointerPhase::Down,
            position: Offset::new(5., 5.),
        });
        let _ = runtime.handle_input(enter());
        assert_eq!(single.text(), "one");
        assert_eq!(*submitted.borrow(), 1);
        let _ = runtime.handle_input(InputEvent::Pointer {
            phase: PointerPhase::Down,
            position: Offset::new(5., 50.),
        });
        multi.set_selection(TextSelection { base: 2, extent: 5 });
        let _ = runtime.handle_input(enter());
        assert_eq!(multi.text(), "ab\nef");
        let _ = runtime.handle_input(InputEvent::Key(KeyEvent {
            code: KeyCode::Enter,
            pressed: true,
            repeat: false,
            modifiers: Modifiers {
                shift: true,
                ..Modifiers::default()
            },
        }));
        assert_eq!(multi.text(), "ab\n\nef");
    }
}
