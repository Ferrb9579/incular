//! Draggable sheet extent, nested handoff, snap, and reset state.
//!
//! The sheet model mirrors Flutter's `_DraggableSheetExtent` and its custom
//! inner `ScrollPosition`: while the inner list is at its leading edge, a
//! drag can be converted into fractional sheet extent; once that extent hits
//! a bound, the remaining delta is handed to the inner controller and then to
//! parent controllers exactly once.

use std::{
    cell::RefCell,
    rc::{Rc, Weak},
    time::Duration,
};

use incular_scroll::{ScrollController, ScrollPhysics};

/// A notification emitted whenever a sheet extent changes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DraggableScrollableNotification {
    /// Configured minimum fractional extent.
    pub min_extent: f32,
    /// Configured maximum fractional extent.
    pub max_extent: f32,
    /// Current fractional extent.
    pub extent: f32,
    /// Initial fractional extent.
    pub initial_extent: f32,
    /// Whether a consumer should close when the sheet reaches its minimum.
    pub should_close_on_min_extent: bool,
    /// Notification depth supplied by the tree adapter.
    pub depth: usize,
}

type ExtentListener = Rc<dyn Fn(DraggableScrollableNotification) -> bool>;

#[derive(Default)]
struct NotificationState {
    listeners: Vec<(u64, ExtentListener)>,
    next_id: u64,
}

/// RAII subscription for sheet notifications.
pub struct DraggableNotificationSubscription {
    state: Weak<RefCell<NotificationState>>,
    id: u64,
}

impl Drop for DraggableNotificationSubscription {
    fn drop(&mut self) {
        if let Some(state) = self.state.upgrade() {
            state
                .borrow_mut()
                .listeners
                .retain(|(id, _)| *id != self.id);
        }
    }
}

/// A read-only and mutable snapshot of the sheet's fractional extent model.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DraggableSheetExtent {
    /// Minimum fractional extent.
    pub min_size: f32,
    /// Maximum fractional extent.
    pub max_size: f32,
    /// Initial fractional extent.
    pub initial_size: f32,
    /// Current fractional extent.
    pub current_size: f32,
    /// Parent pixel height multiplied by `max_size`.
    pub available_pixels: f32,
    /// Whether a user drag has changed the extent.
    pub has_dragged: bool,
    /// Whether any update has changed the extent from the initial value.
    pub has_changed: bool,
    /// Whether reaching `min_size` requests descendant closing behavior.
    pub should_close_on_min_extent: bool,
}

impl DraggableSheetExtent {
    /// Creates a validated extent model.
    #[must_use]
    pub fn new(
        min_size: f32,
        max_size: f32,
        initial_size: f32,
        should_close_on_min_extent: bool,
    ) -> Self {
        assert!(min_size.is_finite() && max_size.is_finite() && initial_size.is_finite());
        assert!(min_size >= 0.0 && min_size <= max_size && max_size <= 1.0);
        assert!(initial_size >= min_size && initial_size <= max_size);
        Self {
            min_size,
            max_size,
            initial_size,
            current_size: initial_size,
            available_pixels: 0.0,
            has_dragged: false,
            has_changed: false,
            should_close_on_min_extent,
        }
    }

    /// Whether the current extent is at the minimum within float tolerance.
    #[must_use]
    pub fn is_at_min(&self) -> bool {
        (self.current_size - self.min_size).abs() <= 1.0e-6
    }

    /// Whether the current extent is at the maximum within float tolerance.
    #[must_use]
    pub fn is_at_max(&self) -> bool {
        (self.current_size - self.max_size).abs() <= 1.0e-6
    }

    /// Returns the current sheet size in parent pixels.
    #[must_use]
    pub fn current_pixels(&self) -> f32 {
        self.size_to_pixels(self.current_size)
    }

    /// Sets the parent-height-derived pixel conversion scale.
    pub fn set_parent_height(&mut self, parent_height: f32) {
        self.available_pixels = (parent_height.max(0.0) * self.max_size).max(0.0);
    }

    /// Converts sheet pixels into fractional extent using Flutter's
    /// `pixelsToSize` rule.
    #[must_use]
    pub fn pixels_to_size(&self, pixels: f32) -> f32 {
        if self.available_pixels <= 0.0 {
            0.0
        } else {
            pixels / self.available_pixels * self.max_size
        }
    }

    /// Converts a fractional extent into parent pixels.
    #[must_use]
    pub fn size_to_pixels(&self, size: f32) -> f32 {
        if self.max_size <= 0.0 {
            0.0
        } else {
            size / self.max_size * self.available_pixels
        }
    }

    fn with_current_size(mut self, current_size: f32) -> Self {
        self.current_size = current_size.clamp(self.min_size, self.max_size);
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct SheetActivity {
    generation: u64,
}

#[derive(Clone)]
struct ActuatorSubscription {
    state: Weak<RefCell<ActuatorState>>,
    id: u64,
}

impl Drop for ActuatorSubscription {
    fn drop(&mut self) {
        if let Some(state) = self.state.upgrade() {
            state
                .borrow_mut()
                .listeners
                .retain(|(id, _)| *id != self.id);
        }
    }
}

type ActuatorListener = Rc<dyn Fn() -> bool>;

#[derive(Default)]
struct ActuatorState {
    listeners: Vec<(u64, ActuatorListener)>,
    next_id: u64,
}

/// A reset broadcaster for descendant draggable sheets.
#[derive(Clone, Default)]
pub struct DraggableScrollableActuator {
    state: Rc<RefCell<ActuatorState>>,
}

impl DraggableScrollableActuator {
    /// Creates an actuator with no descendants attached.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sends reset to every attached descendant and returns whether any sheet
    /// changed its extent or inner scroll position.
    pub fn reset(&self) -> bool {
        let listeners = self.state.borrow().listeners.clone();
        listeners
            .iter()
            .fold(false, |changed, (_, listener)| listener() || changed)
    }

    /// Attaches a sheet to this actuator.
    pub fn attach(&self, sheet: &DraggableScrollableState) {
        let weak_sheet = Rc::downgrade(&sheet.state);
        let listener: ActuatorListener = Rc::new(move || {
            weak_sheet.upgrade().is_some_and(|state| {
                let sheet = DraggableScrollableState { state };
                sheet.reset()
            })
        });
        let mut actuator = self.state.borrow_mut();
        let id = actuator.next_id;
        actuator.next_id = actuator.next_id.wrapping_add(1);
        actuator.listeners.push((id, listener));
        drop(actuator);
        sheet.state.borrow_mut().actuator_subscription = Some(ActuatorSubscription {
            state: Rc::downgrade(&self.state),
            id,
        });
    }
}

/// A controller for a mounted [`DraggableScrollableSheet`].
#[derive(Clone, Default)]
pub struct DraggableScrollableController {
    state: Rc<RefCell<ControllerState>>,
}

#[derive(Default)]
struct ControllerState {
    sheet: Option<Weak<RefCell<SheetState>>>,
}

impl DraggableScrollableController {
    /// Creates an unattached controller.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns whether the controller is attached to a live sheet.
    #[must_use]
    pub fn is_attached(&self) -> bool {
        self.state
            .borrow()
            .sheet
            .as_ref()
            .and_then(Weak::upgrade)
            .is_some()
    }

    /// Returns the attached sheet's current fractional size.
    #[must_use]
    pub fn size(&self) -> Option<f32> {
        self.upgrade_sheet()
            .map(|sheet| sheet.borrow().extent.current_size)
    }

    /// Returns the attached sheet's current pixel height.
    #[must_use]
    pub fn pixels(&self) -> Option<f32> {
        self.upgrade_sheet()
            .map(|sheet| sheet.borrow().extent.current_pixels())
    }

    /// Jumps the attached sheet to a fractional extent.
    pub fn jump_to(&self, size: f32) -> bool {
        self.upgrade_sheet()
            .is_some_and(|sheet| DraggableScrollableState { state: sheet }.set_size(size, false))
    }

    /// Starts a deterministic extent animation. Call [`DraggableSizeAnimation::tick`]
    /// once per frame.
    pub fn animate_to(&self, size: f32, duration: Duration) -> Option<DraggableSizeAnimation> {
        let sheet = self.upgrade_sheet()?;
        let from = sheet.borrow().extent.current_size;
        let target = sheet.borrow().extent.with_current_size(size).current_size;
        Some(DraggableSizeAnimation {
            state: Rc::downgrade(&sheet),
            from,
            target,
            duration,
            elapsed: Duration::ZERO,
            finished: false,
        })
    }

    fn attach(&self, sheet: &Rc<RefCell<SheetState>>) {
        self.state.borrow_mut().sheet = Some(Rc::downgrade(sheet));
    }

    fn upgrade_sheet(&self) -> Option<Rc<RefCell<SheetState>>> {
        self.state.borrow().sheet.as_ref().and_then(Weak::upgrade)
    }
}

/// A frame-driven sheet extent animation with an ease-out curve.
pub struct DraggableSizeAnimation {
    state: Weak<RefCell<SheetState>>,
    from: f32,
    target: f32,
    duration: Duration,
    elapsed: Duration,
    finished: bool,
}

impl DraggableSizeAnimation {
    /// Advances the animation and returns whether it is still active.
    pub fn tick(&mut self, delta: Duration) -> bool {
        if self.finished {
            return false;
        }
        self.elapsed = self.elapsed.saturating_add(delta);
        let progress = if self.duration.is_zero() {
            1.0
        } else {
            (self.elapsed.as_secs_f32() / self.duration.as_secs_f32()).clamp(0.0, 1.0)
        };
        let eased = 1.0 - (1.0 - progress).powi(3);
        let value = self.from + (self.target - self.from) * eased;
        let Some(state) = self.state.upgrade() else {
            self.finished = true;
            return false;
        };
        DraggableScrollableState { state }.set_size(value, false);
        if progress >= 1.0 {
            self.finished = true;
        }
        !self.finished
    }

    /// Returns the target fractional extent.
    #[must_use]
    pub fn target(&self) -> f32 {
        self.target
    }
}

/// Result of one nested sheet/list/parent delta handoff.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DraggableSheetDelta {
    /// Input logical content-offset delta.
    pub input_delta: f32,
    /// Portion consumed by changing sheet extent, in logical offset sign.
    pub sheet_consumed: f32,
    /// Portion consumed by the inner list.
    pub inner_consumed: f32,
    /// Portion consumed by parent controllers.
    pub parent_consumed: f32,
    /// Portion left after the complete nested chain.
    pub unconsumed: f32,
}

/// Snap configuration for a draggable sheet.
#[derive(Clone, Debug, PartialEq)]
pub struct DraggableSnap {
    /// Whether release should select a snap size.
    pub enabled: bool,
    /// Sorted fractional snap sizes. Min/max are considered implicitly.
    pub sizes: Vec<f32>,
    /// Duration used by the tree adapter for snap animation.
    pub animation_duration: Duration,
}

impl Default for DraggableSnap {
    fn default() -> Self {
        Self {
            enabled: false,
            sizes: Vec::new(),
            animation_duration: Duration::from_millis(125),
        }
    }
}

/// A configured draggable sheet whose child is built around the sheet's inner
/// [`ScrollController`].
pub struct DraggableScrollableSheet<T> {
    min_child_size: f32,
    max_child_size: f32,
    initial_child_size: f32,
    expand: bool,
    should_close_on_min_extent: bool,
    snap: DraggableSnap,
    controller: DraggableScrollableController,
    builder: Rc<dyn Fn(&ScrollController) -> T>,
}

impl<T> DraggableScrollableSheet<T> {
    /// Creates a configured sheet.
    #[must_use]
    pub fn new(builder: impl Fn(&ScrollController) -> T + 'static) -> Self {
        Self {
            min_child_size: 0.25,
            max_child_size: 1.0,
            initial_child_size: 0.5,
            expand: true,
            should_close_on_min_extent: true,
            snap: DraggableSnap::default(),
            controller: DraggableScrollableController::new(),
            builder: Rc::new(builder),
        }
    }

    /// Sets the fractional extent bounds and initial value.
    #[must_use]
    pub fn extents(mut self, min: f32, max: f32, initial: f32) -> Self {
        assert!(min.is_finite() && max.is_finite() && initial.is_finite());
        assert!(min >= 0.0 && min <= max && max <= 1.0 && initial >= min && initial <= max);
        self.min_child_size = min;
        self.max_child_size = max;
        self.initial_child_size = initial;
        self
    }

    /// Configures whether the sheet fills unused parent space.
    #[must_use]
    pub fn expand(mut self, expand: bool) -> Self {
        self.expand = expand;
        self
    }

    /// Configures close-on-minimum notification behavior.
    #[must_use]
    pub fn should_close_on_min_extent(mut self, should_close: bool) -> Self {
        self.should_close_on_min_extent = should_close;
        self
    }

    /// Configures snap sizes and animation duration.
    #[must_use]
    pub fn snap(mut self, sizes: impl IntoIterator<Item = f32>, duration: Duration) -> Self {
        let mut sizes = sizes.into_iter().collect::<Vec<_>>();
        sizes.retain(|size| {
            size.is_finite() && *size >= self.min_child_size && *size <= self.max_child_size
        });
        sizes.sort_by(f32::total_cmp);
        sizes.dedup_by(|left, right| (*left - *right).abs() <= 1.0e-6);
        self.snap = DraggableSnap {
            enabled: true,
            sizes,
            animation_duration: duration,
        };
        self
    }

    /// Returns the configured controller.
    #[must_use]
    pub fn controller(&self) -> DraggableScrollableController {
        self.controller.clone()
    }

    /// Returns whether the sheet should occupy unused parent space.
    #[must_use]
    pub fn expands(&self) -> bool {
        self.expand
    }

    /// Creates mounted state and the inner child value.
    #[must_use]
    pub fn mount(&self) -> (DraggableScrollableState, T) {
        let state = DraggableScrollableState::new(
            self.min_child_size,
            self.max_child_size,
            self.initial_child_size,
            self.should_close_on_min_extent,
            self.snap.clone(),
            self.controller.clone(),
        );
        let child = (self.builder)(&state.inner_controller());
        (state, child)
    }
}

struct SheetState {
    extent: DraggableSheetExtent,
    inner_controller: ScrollController,
    inner_physics: ScrollPhysics,
    parent_controllers: Vec<ScrollController>,
    listeners: Rc<RefCell<NotificationState>>,
    actuator_subscription: Option<ActuatorSubscription>,
    activity: Option<SheetActivity>,
    next_activity: u64,
    snap: DraggableSnap,
}

/// Mounted draggable sheet state.
#[derive(Clone)]
pub struct DraggableScrollableState {
    state: Rc<RefCell<SheetState>>,
}

impl DraggableScrollableState {
    /// Creates state with a newly allocated inner controller.
    #[must_use]
    pub fn new(
        min_child_size: f32,
        max_child_size: f32,
        initial_child_size: f32,
        should_close_on_min_extent: bool,
        snap: DraggableSnap,
        controller: DraggableScrollableController,
    ) -> Self {
        let state = Rc::new(RefCell::new(SheetState {
            extent: DraggableSheetExtent::new(
                min_child_size,
                max_child_size,
                initial_child_size,
                should_close_on_min_extent,
            ),
            inner_controller: ScrollController::new(),
            inner_physics: ScrollPhysics::clamping(),
            parent_controllers: Vec::new(),
            listeners: Rc::new(RefCell::new(NotificationState::default())),
            actuator_subscription: None,
            activity: None,
            next_activity: 0,
            snap,
        }));
        controller.attach(&state);
        Self { state }
    }

    /// Returns the inner list controller passed to the builder.
    #[must_use]
    pub fn inner_controller(&self) -> ScrollController {
        self.state.borrow().inner_controller.clone()
    }

    /// Returns an extent snapshot.
    #[must_use]
    pub fn extent(&self) -> DraggableSheetExtent {
        self.state.borrow().extent
    }

    /// Sets parent height, establishing the fractional-to-pixel conversion.
    pub fn set_parent_height(&self, parent_height: f32) {
        self.state
            .borrow_mut()
            .extent
            .set_parent_height(parent_height);
    }

    /// Configures content and viewport extents for the inner list.
    pub fn set_inner_extents(&self, content: f32, viewport: f32) {
        self.state
            .borrow()
            .inner_controller
            .update_extents_with_physics(content, viewport, self.state.borrow().inner_physics);
    }

    /// Replaces inner list physics.
    pub fn set_inner_physics(&self, physics: ScrollPhysics) {
        self.state.borrow_mut().inner_physics = physics;
    }

    /// Replaces the outer controllers that receive leftover deltas.
    pub fn set_parent_controllers(&self, controllers: impl IntoIterator<Item = ScrollController>) {
        self.state.borrow_mut().parent_controllers = controllers.into_iter().collect();
    }

    /// Adds a notification listener. Returning `true` stops propagation to
    /// later listeners, matching the existing scroll notification contract.
    pub fn add_notification_listener(
        &self,
        listener: impl Fn(DraggableScrollableNotification) -> bool + 'static,
    ) -> DraggableNotificationSubscription {
        let notifications = self.state.borrow().listeners.clone();
        let mut state = notifications.borrow_mut();
        let id = state.next_id;
        state.next_id = state.next_id.wrapping_add(1);
        state.listeners.push((id, Rc::new(listener)));
        drop(state);
        DraggableNotificationSubscription {
            state: Rc::downgrade(&notifications),
            id,
        }
    }

    /// Binds this sheet to an actuator reset channel.
    pub fn attach_actuator(&self, actuator: &DraggableScrollableActuator) {
        actuator.attach(self);
    }

    /// Begins a cancelable activity and returns its generation token.
    pub fn start_activity(&self) -> u64 {
        let mut state = self.state.borrow_mut();
        state.next_activity = state.next_activity.wrapping_add(1);
        let generation = state.next_activity;
        state.activity = Some(SheetActivity { generation });
        generation
    }

    /// Cancels the active activity.
    pub fn cancel_activity(&self) {
        self.state.borrow_mut().activity = None;
    }

    /// Returns the active activity generation, if a drag/ballistic sequence is
    /// currently owned by the sheet.
    #[must_use]
    pub fn activity_generation(&self) -> Option<u64> {
        self.state
            .borrow()
            .activity
            .map(|activity| activity.generation)
    }

    /// Applies a content-offset delta, resizing first and handing leftover
    /// movement to the inner list and then parents.
    pub fn apply_user_offset(&self, delta: f32) -> DraggableSheetDelta {
        let before = self.extent();
        let list_should_scroll = self.inner_controller().offset() > 0.0;
        let resize = !list_should_scroll
            && (!(before.is_at_min() || before.is_at_max())
                || (before.is_at_min() && delta < 0.0)
                || (before.is_at_max() && delta > 0.0));
        let mut remaining = delta;
        let mut sheet_consumed = 0.0;
        if resize {
            let pixel_delta = -delta;
            let next = before.current_size + before.pixels_to_size(pixel_delta);
            let _ = self.set_size_internal(next, true);
            let after = self.extent();
            let consumed_pixels = after.current_pixels() - before.current_pixels();
            sheet_consumed = -consumed_pixels;
            remaining -= sheet_consumed;
            if remaining.abs() <= 1.0e-4 {
                remaining = 0.0;
            }
        }

        let mut inner_consumed = 0.0;
        let mut parent_consumed = 0.0;
        if remaining.abs() > f32::EPSILON {
            let (inner_result, parents) = {
                let state = self.state.borrow();
                let result = state
                    .inner_controller
                    .apply_physics(state.inner_physics, remaining);
                (result, state.parent_controllers.clone())
            };
            inner_consumed = inner_result.consumed;
            remaining = inner_result.unconsumed;
            if remaining.abs() > f32::EPSILON && !parents.is_empty() {
                let mut coordinator = incular_scroll::NestedScrollCoordinator::new(parents);
                let result = coordinator.apply_drag_delta(remaining);
                parent_consumed = result.consumed;
                remaining = result.unconsumed;
            }
        }
        DraggableSheetDelta {
            input_delta: delta,
            sheet_consumed,
            inner_consumed,
            parent_consumed,
            unconsumed: remaining,
        }
    }

    /// Sets the current size and emits a notification if it changed.
    pub fn set_size(&self, size: f32, user_drag: bool) -> bool {
        self.set_size_internal(size, user_drag)
    }

    /// Selects the nearest snap point, with velocity choosing the next point
    /// in its direction when release has momentum.
    #[must_use]
    pub fn snap_target(&self, velocity: f32) -> Option<DraggableSnapTarget> {
        let state = self.state.borrow();
        if !state.snap.enabled {
            return None;
        }
        let mut points = Vec::with_capacity(state.snap.sizes.len() + 2);
        points.push(state.extent.min_size);
        points.extend(state.snap.sizes.iter().copied());
        points.push(state.extent.max_size);
        points.sort_by(f32::total_cmp);
        points.dedup_by(|left, right| (*left - *right).abs() <= 1.0e-6);
        let current = state.extent.current_size;
        let target = if velocity > 0.0 {
            points
                .iter()
                .copied()
                .find(|point| *point > current + 1.0e-6)
                .unwrap_or(*points.last().unwrap_or(&current))
        } else if velocity < 0.0 {
            points
                .iter()
                .rev()
                .copied()
                .find(|point| *point < current - 1.0e-6)
                .unwrap_or(points[0])
        } else {
            points
                .iter()
                .copied()
                .min_by(|left, right| (left - current).abs().total_cmp(&(right - current).abs()))
                .unwrap_or(current)
        };
        Some(DraggableSnapTarget {
            size: target,
            duration: state.snap.animation_duration,
        })
    }

    /// Applies the nearest snap target immediately.
    pub fn snap_now(&self, velocity: f32) -> Option<DraggableSnapTarget> {
        let target = self.snap_target(velocity)?;
        self.set_size(target.size, false);
        Some(target)
    }

    /// Resets the sheet to its initial extent and returns whether state changed.
    pub fn reset(&self) -> bool {
        self.cancel_activity();
        let before = self.extent();
        let _ = self.inner_controller().jump_to(0.0);
        {
            let mut state = self.state.borrow_mut();
            state.extent.has_dragged = false;
            state.extent.has_changed = false;
        }
        let changed =
            before.current_size != self.extent().initial_size || before.current_pixels() != 0.0;
        let size_changed = self.set_size_internal(self.extent().initial_size, false);
        changed || size_changed
    }

    fn set_size_internal(&self, size: f32, user_drag: bool) -> bool {
        let notification = {
            let mut state = self.state.borrow_mut();
            let next = size.clamp(state.extent.min_size, state.extent.max_size);
            if (next - state.extent.current_size).abs() <= f32::EPSILON {
                if user_drag {
                    state.extent.has_dragged = true;
                }
                return false;
            }
            state.extent.current_size = next;
            state.extent.has_changed = true;
            if user_drag {
                state.extent.has_dragged = true;
            }
            DraggableScrollableNotification {
                min_extent: state.extent.min_size,
                max_extent: state.extent.max_size,
                extent: state.extent.current_size,
                initial_extent: state.extent.initial_size,
                should_close_on_min_extent: state.extent.should_close_on_min_extent,
                depth: 0,
            }
        };
        let listeners = self.state.borrow().listeners.borrow().listeners.clone();
        for (_, listener) in listeners {
            if listener(notification) {
                break;
            }
        }
        true
    }
}

/// A resolved snap target and its configured animation duration.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DraggableSnapTarget {
    /// Target fractional size.
    pub size: f32,
    /// Configured snap animation duration.
    pub duration: Duration,
}
