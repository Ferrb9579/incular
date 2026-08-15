//! Controlled BUILD → LAYOUT → PAINT coordination and local reactive state.
use incular_core::{InputEvent, Offset, PointerPhase};
use incular_layout::Constraints;
use incular_painting::DisplayList;
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
/// Platform-neutral scheduler; it owns no window or GPU resource.
pub struct Runtime {
    tree: WidgetTree,
    pending: HashMap<ElementId, Widget>,
    order: VecDeque<ElementId>,
    reactive: Rc<RefCell<ReactiveQueue>>,
    builders: HashMap<ElementId, Box<dyn FnMut() -> Widget>>,
    handlers: HashMap<ActionId, Rc<dyn Fn()>>,
    next_action: u64,
    hovered_button: Option<ElementId>,
    pressed_button: Option<ElementId>,
    last_pointer: Offset,
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
            next_action: 1,
            hovered_button: None,
            pressed_button: None,
            last_pointer: Offset::ZERO,
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
            if let InputEvent::Scroll { delta } = event {
                // The platform normalizes wheel values to logical pixels. The
                // latest pointer position selects the nearest viewport.
                if self.tree.scroll_at(self.last_pointer, delta) {
                    self.frame_requested = true;
                }
            }
            return None;
        };
        self.last_pointer = position;
        let target = self
            .tree
            .hit_test(position)
            .and_then(|render| self.tree.element_for_render(render))
            .and_then(|element| self.tree.action_ancestor(element));
        match phase {
            PointerPhase::Move => {
                self.set_hover(target.map(|(element, _)| element));
                target.map(|(element, action)| EventTarget {
                    element,
                    action: Some(action),
                })
            }
            PointerPhase::Down => {
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
                if let Some(element) = self.pressed_button.take() {
                    let _ = self.tree.set_button_state(element, ButtonState::Normal);
                    self.frame_requested = true;
                }
                None
            }
        }
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
        let (composited, animations_active) = self.tree.update_compositor(now);
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
        let next = &mut self.next_action;
        widget.bind_callbacks(&mut |callback| {
            let id = ActionId(*next);
            *next += 1;
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
    use incular_core::{Color, Offset, Size};
    use incular_painting::{DisplayList, PaintCommand};
    use incular_widgets::Button;
    use std::cell::Cell;
    use std::time::{Duration, Instant};

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
                PaintCommand::PushClip { .. } | PaintCommand::PopClip => {}
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
        assert_eq!(frame.repainted_render_objects, 0);
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
        assert_eq!(frame.repainted_render_objects, 0);
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
}
