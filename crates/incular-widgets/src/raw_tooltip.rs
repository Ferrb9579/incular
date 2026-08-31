//! Renderer-neutral implementation of Flutter's `RawTooltip` contract.
//!
//! The controller owns the tooltip state machine while the widget descriptor
//! only describes how the child, semantics, and anchored overlay are composed.
//! Timing is deliberately driven by [`RawTooltipController::poll`] (or
//! [`RawTooltipController::tick`]); this crate does not create timer threads or
//! depend on an application runtime.
//!
//! # Platform-neutral limits
//!
//! `OverlayPortal` currently lowers to a retained stack when an overlay host is
//! not available. Native window adapters can lift that logical overlay into a
//! real overlay without changing this API. The existing raw input layer has
//! no window-global pointer route, so tap-to-dismiss outside the tooltip is
//! provided through an ancestor `TapRegionSurface` when one exists, plus the
//! child listener and the explicit controller dismissal methods. A host that
//! needs Flutter's application-wide outside-pointer behavior should forward
//! that event to [`RawTooltipController::dismiss_all`].
//!
//! Flutter also supplies an animation to its tooltip builder. The retained
//! widget layer has no renderer-independent animation argument for a component
//! builder, so [`TooltipComponentBuilder`] returns the overlay widget directly;
//! callers can add an existing retained transition around that widget. The
//! `enable_feedback` flag is retained as policy, but acoustic and haptic side
//! effects belong to a platform adapter and are not emitted by this crate.

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::fmt;
use std::rc::{Rc, Weak};
use std::time::{Duration, Instant};

use incular_config::Alignment;
use incular_core::Offset;
use incular_gestures::{
    GestureCallbacks, GestureRecognizerFactoryWithHandlers, PointerDeviceKind,
    PointerGestureRecognizer, TapDownDetails,
};
use incular_rendering::LayerLink;
use incular_semantics::SemanticRole;
use incular_text::RichText;

use crate::{
    CompositedTransformFollower, CompositedTransformTarget, FocusNode, FocusableActionDetector,
    HitTestBehavior, IgnorePointer, Listener, MouseRegion, OverlayPortal, RawGestureDetector,
    Semantics, TapRegion, Text, Widget,
};

thread_local! {
    /// Weak handles make the registry lifecycle-safe for mounted and dropped
    /// controllers while retaining Flutter's dismiss-all behavior.
    static OPEN_TOOLTIPS: RefCell<Vec<Weak<RefCell<ControllerState>>>> =
        const { RefCell::new(Vec::new()) };
}

/// How a raw tooltip is triggered.
///
/// Mouse and trackpad hover are intentionally independent of this setting,
/// matching Flutter's `RawTooltip`: even `Manual` still responds to hover.
/// `Focus` is an Incular addition for the retained focus API.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TooltipTriggerMode {
    /// Show from controller calls and pointer hover only.
    Manual,
    /// Show after a touch/pen tap.
    Tap,
    /// Show after a touch/pen long press.
    #[default]
    LongPress,
    /// Show while the supplied focus node is focused.
    Focus,
    /// Kept as an explicit policy value for callers that want hover-only
    /// documentation; RawTooltip hover remains active for every mode.
    Hover,
}

/// Alias that makes the widget-specific type discoverable beside
/// `TooltipTriggerMode`.
pub type RawTooltipTriggerMode = TooltipTriggerMode;

/// Current logical visibility of a tooltip.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RawTooltipVisibility {
    #[default]
    Hidden,
    Waiting,
    Visible,
    Dismissing,
}

/// Alias for consumers that prefer the shorter state name.
pub type TooltipVisibility = RawTooltipVisibility;

/// Delays used by the raw tooltip state machine.
///
/// `show_duration` is the touch/pen visibility window after a recognized tap
/// or long press. It does not represent a paint animation; renderer-neutral
/// show/hide animation is intentionally outside this module.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawTooltipDurations {
    pub wait_duration: Duration,
    pub show_duration: Duration,
    pub exit_duration: Duration,
}

impl RawTooltipDurations {
    #[must_use]
    pub const fn new(
        wait_duration: Duration,
        show_duration: Duration,
        exit_duration: Duration,
    ) -> Self {
        Self {
            wait_duration,
            show_duration,
            exit_duration,
        }
    }

    #[must_use]
    pub const fn wait(self) -> Duration {
        self.wait_duration
    }

    #[must_use]
    pub const fn show(self) -> Duration {
        self.show_duration
    }

    #[must_use]
    pub const fn exit(self) -> Duration {
        self.exit_duration
    }
}

impl Default for RawTooltipDurations {
    fn default() -> Self {
        Self {
            wait_duration: Duration::ZERO,
            show_duration: Duration::from_millis(1_500),
            exit_duration: Duration::from_millis(100),
        }
    }
}

/// Alias matching the shorter naming used by some retained widget APIs.
pub type TooltipDurations = RawTooltipDurations;

#[derive(Clone, Copy, Debug)]
enum PendingTransition {
    Show { at: Instant },
    Hide { at: Instant, force: bool },
}

struct ControllerState {
    visible: bool,
    hovering: HashSet<u64>,
    focused: bool,
    manual: bool,
    pending: Option<PendingTransition>,
    durations: RawTooltipDurations,
    on_triggered: Option<Rc<dyn Fn()>>,
    revision: Rc<Cell<u64>>,
}

/// Shared imperative state for a [`RawTooltip`].
///
/// All transitions are synchronous and renderer-neutral. The retained tree
/// observes [`Self::revision_cell`] through `Widget::stateful_layout_builder`
/// when a visibility transition changes the overlay composition.
#[derive(Clone)]
pub struct RawTooltipController {
    state: Rc<RefCell<ControllerState>>,
}

impl Default for RawTooltipController {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for RawTooltipController {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RawTooltipController")
            .field("state", &self.state())
            .field("visible", &self.is_visible())
            .field("hovering_devices", &self.hovering_devices())
            .field("focused", &self.has_focus())
            .finish()
    }
}

impl PartialEq for RawTooltipController {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.state, &other.state)
    }
}

impl Eq for RawTooltipController {}

impl RawTooltipController {
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Rc::new(RefCell::new(ControllerState {
                visible: false,
                hovering: HashSet::new(),
                focused: false,
                manual: false,
                pending: None,
                durations: RawTooltipDurations::default(),
                on_triggered: None,
                revision: Rc::new(Cell::new(0)),
            })),
        }
    }

    #[must_use]
    pub fn with_durations(durations: RawTooltipDurations) -> Self {
        let controller = Self::new();
        controller.set_durations(durations);
        controller
    }

    /// Returns the retained invalidation cell used by `RawTooltip`.
    #[must_use]
    pub fn revision_cell(&self) -> Rc<Cell<u64>> {
        self.state.borrow().revision.clone()
    }

    /// Shows the tooltip immediately and keeps it open until an explicit hide,
    /// toggle, dismissal, or a later trigger policy changes it.
    pub fn show(&self) {
        let _ = self.show_at(Instant::now());
    }

    /// Deterministic form of [`Self::show`]. Returns whether visibility changed.
    pub fn show_at(&self, _now: Instant) -> bool {
        let (changed, callback) = {
            let mut state = self.state.borrow_mut();
            state.pending = None;
            state.manual = true;
            let changed = set_visible(&mut state, true);
            let callback = changed.then(|| state.on_triggered.clone()).flatten();
            (changed, callback)
        };
        if changed {
            self.register_open();
        }
        invoke_callback(callback);
        changed
    }

    /// Shows the tooltip if hidden, mirroring Flutter's
    /// `RawTooltipState.ensureTooltipVisible` result.
    pub fn ensure_visible(&self) -> bool {
        self.ensure_visible_at(Instant::now())
    }

    pub fn ensure_visible_at(&self, now: Instant) -> bool {
        if self.is_visible() {
            return false;
        }
        self.show_at(now)
    }

    /// Hides immediately and cancels any pending transition.
    pub fn hide(&self) {
        let _ = self.hide_at(Instant::now());
    }

    pub fn hide_at(&self, _now: Instant) -> bool {
        let mut state = self.state.borrow_mut();
        state.pending = None;
        state.manual = false;
        set_visible(&mut state, false)
    }

    /// Toggles between explicit persistent show and hide.
    pub fn toggle(&self) {
        let _ = self.toggle_at(Instant::now());
    }

    pub fn toggle_at(&self, now: Instant) -> bool {
        if self.is_visible() {
            self.hide_at(now)
        } else {
            self.show_at(now)
        }
    }

    /// Shows for a bounded programmatic lifetime. A zero duration is treated
    /// as an unbounded explicit show, matching the existing retained
    /// controller convention.
    pub fn show_for(&self, duration: Duration, now: Instant) {
        let (changed, callback) = {
            let mut state = self.state.borrow_mut();
            state.manual = false;
            state.pending = None;
            let changed = set_visible(&mut state, true);
            if duration > Duration::ZERO {
                state.pending = Some(PendingTransition::Hide {
                    at: deadline(now, duration),
                    force: true,
                });
            }
            let callback = changed.then(|| state.on_triggered.clone()).flatten();
            (changed, callback)
        };
        if changed {
            self.register_open();
        }
        invoke_callback(callback);
    }

    /// Shows from a touch or pen tap and schedules the touch visibility window
    /// after the gesture has been accepted.
    pub fn trigger_tap(&self) {
        let _ = self.trigger_tap_at(Instant::now());
    }

    pub fn trigger_tap_at(&self, now: Instant) -> bool {
        self.trigger_touch_at(now)
    }

    /// Shows from a touch or pen long press and schedules the touch visibility
    /// window after the gesture has been accepted.
    pub fn trigger_long_press(&self) {
        let _ = self.trigger_long_press_at(Instant::now());
    }

    pub fn trigger_long_press_at(&self, now: Instant) -> bool {
        self.trigger_touch_at(now)
    }

    fn trigger_touch_at(&self, now: Instant) -> bool {
        let (changed, callback) = {
            let mut state = self.state.borrow_mut();
            state.manual = false;
            state.pending = None;
            let changed = set_visible(&mut state, true);
            if state.hovering.is_empty() && !state.focused {
                state.pending = Some(PendingTransition::Hide {
                    at: deadline(now, state.durations.show_duration),
                    force: false,
                });
            }
            // Flutter invokes onTriggered for every accepted tap/long press,
            // including a retrigger while the tooltip is already visible.
            let callback = state.on_triggered.clone();
            (changed, callback)
        };
        if changed {
            self.register_open();
        }
        invoke_callback(callback);
        changed
    }

    /// Records a mouse or trackpad device entering the target or overlay.
    pub fn mouse_enter(&self, device: u64) {
        let _ = self.mouse_enter_at(device, Instant::now());
    }

    pub fn mouse_enter_at(&self, device: u64, now: Instant) -> bool {
        self.dismiss_other_hovered(now);
        let changed = {
            let mut state = self.state.borrow_mut();
            state.hovering.insert(device);
            if state.manual || state.focused {
                false
            } else if state.visible {
                // A new hover cancels both the exit delay and a touch deadline.
                if matches!(state.pending, Some(PendingTransition::Hide { .. })) {
                    state.pending = None;
                }
                false
            } else if state.durations.wait_duration.is_zero() {
                state.pending = None;
                set_visible(&mut state, true)
            } else {
                let wait_duration = state.durations.wait_duration;
                schedule_show(&mut state, deadline(now, wait_duration));
                false
            }
        };
        if changed {
            self.register_open();
        }
        changed
    }

    /// Records a mouse or trackpad device leaving the target or overlay.
    pub fn mouse_exit(&self, device: u64) {
        let _ = self.mouse_exit_at(device, Instant::now());
    }

    pub fn mouse_exit_at(&self, device: u64, now: Instant) -> bool {
        {
            let mut state = self.state.borrow_mut();
            if !state.hovering.remove(&device) || state.manual || state.focused {
                false
            } else if !state.visible {
                if matches!(state.pending, Some(PendingTransition::Show { .. })) {
                    state.pending = None;
                }
                false
            } else if state.durations.exit_duration.is_zero() {
                state.pending = None;
                set_visible(&mut state, false)
            } else {
                let exit_duration = state.durations.exit_duration;
                schedule_hide(&mut state, deadline(now, exit_duration), false);
                false
            }
        }
    }

    /// Synchronizes the optional focus trigger with a focus node.
    pub fn focus_changed(&self, focused: bool) {
        let _ = self.focus_changed_at(focused, Instant::now());
    }

    pub fn focus_changed_at(&self, focused: bool, now: Instant) -> bool {
        let changed = {
            let mut state = self.state.borrow_mut();
            state.focused = focused;
            if focused {
                state.pending = None;
                if state.manual {
                    false
                } else {
                    set_visible(&mut state, true)
                }
            } else if state.manual || !state.hovering.is_empty() {
                false
            } else if !state.visible {
                if matches!(state.pending, Some(PendingTransition::Show { .. })) {
                    state.pending = None;
                }
                false
            } else if state.durations.exit_duration.is_zero() {
                state.pending = None;
                set_visible(&mut state, false)
            } else {
                let exit_duration = state.durations.exit_duration;
                schedule_hide(&mut state, deadline(now, exit_duration), false);
                false
            }
        };
        if changed {
            self.register_open();
        }
        changed
    }

    /// Dismisses in response to an unrecognized pointer down, matching
    /// Flutter's application-global tap-to-dismiss policy as closely as the
    /// local retained input route permits.
    pub fn dismiss_by_pointer(&self) {
        let _ = self.dismiss_by_pointer_at(Instant::now());
    }

    pub fn dismiss_by_pointer_at(&self, now: Instant) -> bool {
        let mut state = self.state.borrow_mut();
        state.hovering.clear();
        state.pending = None;
        state.manual = false;
        let _ = now;
        set_visible(&mut state, false)
    }

    /// Cancels an in-progress tap/long-press trigger.
    pub fn cancel_trigger(&self) {
        let _ = self.cancel_trigger_at(Instant::now());
    }

    pub fn cancel_trigger_at(&self, now: Instant) -> bool {
        if self.is_kept_open() {
            return false;
        }
        self.hide_at(now)
    }

    /// Schedules the touch deadline after a long-press recognizer releases.
    pub fn release_touch_at(&self, now: Instant) {
        let mut state = self.state.borrow_mut();
        if state.hovering.is_empty() && !state.focused && !state.manual && state.visible {
            let show_duration = state.durations.show_duration;
            schedule_hide(&mut state, deadline(now, show_duration), false);
        }
    }

    /// Applies one scheduled state transition. Returns whether the visible
    /// flag changed and therefore whether a retained rebuild is needed.
    pub fn tick(&self, now: Instant) -> bool {
        {
            let mut state = self.state.borrow_mut();
            let Some(pending) = state.pending else {
                return false;
            };
            match pending {
                PendingTransition::Show { at } if now >= at => {
                    state.pending = None;
                    if state.manual || state.focused || !state.hovering.is_empty() {
                        set_visible(&mut state, true)
                    } else {
                        false
                    }
                }
                PendingTransition::Hide { at, force } if now >= at => {
                    if force || (!state.manual && !state.focused && state.hovering.is_empty()) {
                        state.pending = None;
                        state.manual = false;
                        set_visible(&mut state, false)
                    } else {
                        state.pending = None;
                        false
                    }
                }
                _ => false,
            }
        }
    }

    /// Convenience form for hosts whose frame clock is the system clock.
    pub fn poll(&self) -> bool {
        self.tick(Instant::now())
    }

    /// Hides all controllers that have presented a tooltip in this thread.
    /// Weak registry entries are pruned as part of the operation.
    pub fn dismiss_all() -> bool {
        Self::dismiss_all_at(Instant::now())
    }

    pub fn dismiss_all_at(now: Instant) -> bool {
        let controllers = OPEN_TOOLTIPS.with(|open| {
            let mut open = open.borrow_mut();
            let mut controllers = Vec::new();
            open.retain(|weak| {
                let Some(state) = weak.upgrade() else {
                    return false;
                };
                controllers.push(Self { state });
                true
            });
            controllers
        });
        // Do not use `Iterator::any` here: it short-circuits after the first
        // visible controller is hidden and leaves the remaining tooltips open.
        let mut changed = false;
        for controller in controllers {
            changed |= controller.hide_at(now);
        }
        changed
    }

    /// Flutter-compatible spelling for [`Self::dismiss_all`].
    pub fn dismiss_all_tooltips() -> bool {
        Self::dismiss_all()
    }

    #[must_use]
    pub fn state(&self) -> RawTooltipVisibility {
        let state = self.state.borrow();
        if state.visible {
            if matches!(state.pending, Some(PendingTransition::Hide { .. })) {
                RawTooltipVisibility::Dismissing
            } else {
                RawTooltipVisibility::Visible
            }
        } else if matches!(state.pending, Some(PendingTransition::Show { .. })) {
            RawTooltipVisibility::Waiting
        } else {
            RawTooltipVisibility::Hidden
        }
    }

    #[must_use]
    pub fn is_visible(&self) -> bool {
        self.state.borrow().visible
    }

    #[must_use]
    pub fn is_waiting(&self) -> bool {
        matches!(self.state(), RawTooltipVisibility::Waiting)
    }

    #[must_use]
    pub fn is_dismissing(&self) -> bool {
        matches!(self.state(), RawTooltipVisibility::Dismissing)
    }

    #[must_use]
    pub fn pending_deadline(&self) -> Option<Instant> {
        match self.state.borrow().pending {
            Some(PendingTransition::Show { at } | PendingTransition::Hide { at, .. }) => Some(at),
            None => None,
        }
    }

    #[must_use]
    pub fn hovering_devices(&self) -> usize {
        self.state.borrow().hovering.len()
    }

    #[must_use]
    pub fn has_focus(&self) -> bool {
        self.state.borrow().focused
    }

    pub fn set_durations(&self, durations: RawTooltipDurations) {
        self.state.borrow_mut().durations = durations;
    }

    #[must_use]
    pub fn durations(&self) -> RawTooltipDurations {
        self.state.borrow().durations
    }

    /// Installs the programmatic/touch callback used by a `RawTooltip`.
    pub fn set_on_triggered(&self, callback: impl Fn() + 'static) {
        self.state.borrow_mut().on_triggered = Some(Rc::new(callback));
    }

    pub fn clear_on_triggered(&self) {
        self.state.borrow_mut().on_triggered = None;
    }

    pub(crate) fn set_triggered_callback(&self, callback: Option<Rc<dyn Fn()>>) {
        self.state.borrow_mut().on_triggered = callback;
    }

    fn register_open(&self) {
        let weak = Rc::downgrade(&self.state);
        OPEN_TOOLTIPS.with(|open| {
            let mut open = open.borrow_mut();
            open.retain(|entry| entry.strong_count() != 0 && !Weak::ptr_eq(entry, &weak));
            open.push(weak);
        });
    }

    fn dismiss_other_hovered(&self, now: Instant) {
        let controllers = OPEN_TOOLTIPS.with(|open| {
            let mut open = open.borrow_mut();
            let mut controllers = Vec::new();
            open.retain(|weak| {
                let Some(state) = weak.upgrade() else {
                    return false;
                };
                controllers.push(Self { state });
                true
            });
            controllers
        });
        for controller in controllers {
            if controller == *self {
                continue;
            }
            let keep_open = controller.is_kept_open();
            if !keep_open {
                let _ = controller.hide_at(now);
            }
        }
    }

    fn is_kept_open(&self) -> bool {
        let state = self.state.borrow();
        state.manual || state.focused || !state.hovering.is_empty()
    }
}

fn set_visible(state: &mut ControllerState, visible: bool) -> bool {
    if state.visible == visible {
        return false;
    }
    state.visible = visible;
    state.revision.set(state.revision.get().wrapping_add(1));
    true
}

fn schedule_show(state: &mut ControllerState, at: Instant) {
    state.pending = Some(match state.pending {
        Some(PendingTransition::Show { at: previous }) => PendingTransition::Show {
            at: previous.min(at),
        },
        _ => PendingTransition::Show { at },
    });
}

fn schedule_hide(state: &mut ControllerState, at: Instant, force: bool) {
    state.pending = Some(match state.pending {
        Some(PendingTransition::Hide {
            at: previous,
            force: previous_force,
        }) => PendingTransition::Hide {
            at: previous.min(at),
            force: previous_force || force,
        },
        _ => PendingTransition::Hide { at, force },
    });
}

fn deadline(now: Instant, duration: Duration) -> Instant {
    now.checked_add(duration).unwrap_or(now)
}

fn invoke_callback(callback: Option<Rc<dyn Fn()>>) {
    if let Some(callback) = callback {
        callback();
    }
}

fn is_touch_trigger_kind(kind: PointerDeviceKind) -> bool {
    !matches!(kind, PointerDeviceKind::Mouse)
}

fn is_trigger_pointer(mode: TooltipTriggerMode, kind: PointerDeviceKind) -> bool {
    is_touch_trigger_kind(kind)
        && matches!(
            mode,
            TooltipTriggerMode::Tap | TooltipTriggerMode::LongPress
        )
}

/// A renderer-neutral retained overlay builder.
#[derive(Clone)]
pub struct TooltipComponentBuilder(Rc<dyn Fn() -> Widget>);

impl TooltipComponentBuilder {
    #[must_use]
    pub fn new(builder: impl Fn() -> Widget + 'static) -> Self {
        Self(Rc::new(builder))
    }

    #[must_use]
    pub fn build(&self) -> Widget {
        (self.0)()
    }

    #[must_use]
    pub fn call(&self) -> Widget {
        self.build()
    }
}

impl<F> From<F> for TooltipComponentBuilder
where
    F: Fn() -> Widget + 'static,
{
    fn from(builder: F) -> Self {
        Self::new(builder)
    }
}

impl fmt::Debug for TooltipComponentBuilder {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("TooltipComponentBuilder(..)")
    }
}

impl PartialEq for TooltipComponentBuilder {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// Alias using Flutter's property name.
pub type RawTooltipBuilder = TooltipComponentBuilder;

/// A retained, unthemed tooltip descriptor.
#[derive(Clone)]
pub struct RawTooltip {
    child: Widget,
    message: Option<String>,
    rich_message: Option<RichText>,
    tooltip_builder: Option<TooltipComponentBuilder>,
    controller: RawTooltipController,
    trigger_mode: TooltipTriggerMode,
    focus_node: FocusNode,
    enabled: bool,
    durations: RawTooltipDurations,
    enable_tap_to_dismiss: bool,
    ignore_pointer: bool,
    enable_feedback: bool,
    on_triggered: Option<Rc<dyn Fn()>>,
    semantics_tooltip: Option<String>,
    semantics_explicit: bool,
    exclude_from_semantics: bool,
    prefer_below: bool,
    vertical_offset: f32,
    target_anchor: Option<Alignment>,
    follower_anchor: Option<Alignment>,
    offset: Option<Offset>,
    layer_link: LayerLink,
}

impl fmt::Debug for RawTooltip {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RawTooltip")
            .field("child", &self.child)
            .field("message", &self.message)
            .field("rich_message", &self.rich_message)
            .field("has_tooltip_builder", &self.tooltip_builder.is_some())
            .field("controller", &self.controller)
            .field("trigger_mode", &self.trigger_mode)
            .field("enabled", &self.enabled)
            .field("durations", &self.durations)
            .field("enable_tap_to_dismiss", &self.enable_tap_to_dismiss)
            .field("ignore_pointer", &self.ignore_pointer)
            .field("enable_feedback", &self.enable_feedback)
            .field("has_on_triggered", &self.on_triggered.is_some())
            .field("semantics_tooltip", &self.semantics_tooltip)
            .field("exclude_from_semantics", &self.exclude_from_semantics)
            .field("prefer_below", &self.prefer_below)
            .field("vertical_offset", &self.vertical_offset)
            .finish()
    }
}

impl RawTooltip {
    /// Creates a text-backed raw tooltip. An empty message disables the
    /// overlay, matching Flutter's empty-tooltip behavior.
    #[must_use]
    pub fn new(message: impl Into<String>, child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            message: Some(message.into()),
            rich_message: None,
            tooltip_builder: None,
            controller: RawTooltipController::new(),
            trigger_mode: TooltipTriggerMode::LongPress,
            focus_node: FocusNode::new(),
            enabled: true,
            durations: RawTooltipDurations::default(),
            enable_tap_to_dismiss: true,
            ignore_pointer: false,
            enable_feedback: true,
            on_triggered: None,
            semantics_tooltip: None,
            semantics_explicit: false,
            exclude_from_semantics: false,
            prefer_below: true,
            vertical_offset: 0.0,
            target_anchor: None,
            follower_anchor: None,
            offset: None,
            layer_link: LayerLink::default(),
        }
    }

    #[must_use]
    pub fn empty(child: impl Into<Widget>) -> Self {
        Self::new(String::new(), child)
    }

    #[must_use]
    pub fn with_rich_message(rich_message: RichText, child: impl Into<Widget>) -> Self {
        let mut tooltip = Self::empty(child);
        tooltip.rich_message = Some(rich_message);
        tooltip
    }

    #[must_use]
    pub fn from_rich_message(rich_message: RichText, child: impl Into<Widget>) -> Self {
        Self::with_rich_message(rich_message, child)
    }

    #[must_use]
    pub fn with_builder(
        child: impl Into<Widget>,
        builder: impl Into<TooltipComponentBuilder>,
    ) -> Self {
        let mut tooltip = Self::empty(child);
        tooltip.tooltip_builder = Some(builder.into());
        tooltip
    }

    #[must_use]
    pub fn with_tooltip_builder(
        builder: impl Into<TooltipComponentBuilder>,
        child: impl Into<Widget>,
    ) -> Self {
        Self::with_builder(child, builder)
    }

    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = child.into();
        self
    }

    #[must_use]
    pub fn set_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self.rich_message = None;
        self.tooltip_builder = None;
        self
    }

    #[must_use]
    pub fn clear_message(mut self) -> Self {
        self.message = None;
        self.rich_message = None;
        self.tooltip_builder = None;
        self
    }

    #[must_use]
    pub fn set_rich_message(mut self, rich_message: RichText) -> Self {
        self.message = None;
        self.rich_message = Some(rich_message);
        self.tooltip_builder = None;
        self
    }

    #[must_use]
    pub fn set_tooltip_builder(mut self, builder: impl Into<TooltipComponentBuilder>) -> Self {
        self.message = None;
        self.rich_message = None;
        self.tooltip_builder = Some(builder.into());
        self
    }

    #[must_use]
    pub fn content(self, builder: impl Into<TooltipComponentBuilder>) -> Self {
        self.set_tooltip_builder(builder)
    }

    #[must_use]
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    #[must_use]
    pub fn rich_message(&self) -> Option<&RichText> {
        self.rich_message.as_ref()
    }

    #[must_use]
    pub fn tooltip_builder(&self) -> Option<&TooltipComponentBuilder> {
        self.tooltip_builder.as_ref()
    }

    #[must_use]
    pub fn controller(mut self, controller: RawTooltipController) -> Self {
        self.controller = controller;
        self
    }

    #[must_use]
    pub fn controller_ref(&self) -> RawTooltipController {
        self.controller.clone()
    }

    #[must_use]
    pub fn focus_node(mut self, node: FocusNode) -> Self {
        self.focus_node = node;
        self
    }

    #[must_use]
    pub fn trigger_mode(mut self, mode: TooltipTriggerMode) -> Self {
        self.trigger_mode = mode;
        self
    }

    #[must_use]
    pub fn trigger_mode_value(&self) -> TooltipTriggerMode {
        self.trigger_mode
    }

    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    #[must_use]
    pub fn durations(mut self, durations: RawTooltipDurations) -> Self {
        self.durations = durations;
        self
    }

    #[must_use]
    pub fn wait_duration(mut self, duration: Duration) -> Self {
        self.durations.wait_duration = duration;
        self
    }

    #[must_use]
    pub fn show_duration(mut self, duration: Duration) -> Self {
        self.durations.show_duration = duration;
        self
    }

    #[must_use]
    pub fn exit_duration(mut self, duration: Duration) -> Self {
        self.durations.exit_duration = duration;
        self
    }

    #[must_use]
    pub fn hover_delay(self, duration: Duration) -> Self {
        self.wait_duration(duration)
    }

    #[must_use]
    pub fn touch_delay(self, duration: Duration) -> Self {
        self.show_duration(duration)
    }

    #[must_use]
    pub fn dismiss_delay(self, duration: Duration) -> Self {
        self.exit_duration(duration)
    }

    #[must_use]
    pub fn durations_value(&self) -> RawTooltipDurations {
        self.durations
    }

    #[must_use]
    pub fn wait_duration_value(&self) -> Duration {
        self.durations.wait_duration
    }

    #[must_use]
    pub fn show_duration_value(&self) -> Duration {
        self.durations.show_duration
    }

    #[must_use]
    pub fn exit_duration_value(&self) -> Duration {
        self.durations.exit_duration
    }

    #[must_use]
    pub fn enable_tap_to_dismiss(mut self, enabled: bool) -> Self {
        self.enable_tap_to_dismiss = enabled;
        self
    }

    #[must_use]
    pub fn dismissible(self, enabled: bool) -> Self {
        self.enable_tap_to_dismiss(enabled)
    }

    #[must_use]
    pub fn is_tap_to_dismiss_enabled(&self) -> bool {
        self.enable_tap_to_dismiss
    }

    #[must_use]
    pub fn ignore_pointer(mut self, ignore: bool) -> Self {
        self.ignore_pointer = ignore;
        self
    }

    #[must_use]
    pub fn ignores_pointer(&self) -> bool {
        self.ignore_pointer
    }

    #[must_use]
    pub fn enable_feedback(mut self, enabled: bool) -> Self {
        self.enable_feedback = enabled;
        self
    }

    #[must_use]
    pub fn feedback_enabled(&self) -> bool {
        self.enable_feedback
    }

    #[must_use]
    pub fn on_triggered(mut self, callback: impl Fn() + 'static) -> Self {
        self.on_triggered = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn semantics_tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.semantics_tooltip = Some(tooltip.into());
        self.semantics_explicit = true;
        self
    }

    #[must_use]
    pub fn clear_semantics_tooltip(mut self) -> Self {
        self.semantics_tooltip = None;
        self.semantics_explicit = true;
        self
    }

    #[must_use]
    pub fn exclude_from_semantics(mut self, exclude: bool) -> Self {
        self.exclude_from_semantics = exclude;
        self
    }

    #[must_use]
    pub fn without_semantics(self) -> Self {
        self.exclude_from_semantics(true)
    }

    #[must_use]
    pub fn prefer_below(mut self, prefer_below: bool) -> Self {
        self.prefer_below = prefer_below;
        self
    }

    #[must_use]
    pub fn vertical_offset(mut self, offset: f32) -> Self {
        self.vertical_offset = if offset.is_finite() { offset } else { 0.0 };
        self
    }

    #[must_use]
    pub fn target_anchor(mut self, anchor: Alignment) -> Self {
        self.target_anchor = Some(anchor);
        self
    }

    #[must_use]
    pub fn follower_anchor(mut self, anchor: Alignment) -> Self {
        self.follower_anchor = Some(anchor);
        self
    }

    #[must_use]
    pub fn offset(mut self, offset: Offset) -> Self {
        self.offset = Some(offset);
        self
    }

    #[must_use]
    pub fn position_anchors(
        mut self,
        target_anchor: Alignment,
        follower_anchor: Alignment,
        offset: Offset,
    ) -> Self {
        self.target_anchor = Some(target_anchor);
        self.follower_anchor = Some(follower_anchor);
        self.offset = Some(offset);
        self
    }

    #[must_use]
    pub fn layer_link(mut self, link: LayerLink) -> Self {
        self.layer_link = link;
        self
    }

    #[must_use]
    pub fn layer_link_ref(&self) -> LayerLink {
        self.layer_link.clone()
    }

    fn has_overlay_content(&self) -> bool {
        self.tooltip_builder.is_some()
            || self
                .message
                .as_deref()
                .is_some_and(|message| !message.is_empty())
            || self
                .rich_message
                .as_ref()
                .is_some_and(|message| !message.plain_text().is_empty())
    }

    fn semantic_value(&self) -> Option<String> {
        if self.exclude_from_semantics {
            return None;
        }
        let value = if self.semantics_explicit {
            self.semantics_tooltip.clone()
        } else if let Some(message) = self.message.as_ref() {
            Some(message.clone())
        } else {
            self.rich_message.as_ref().map(RichText::plain_text)
        };
        value.filter(|value| !value.is_empty())
    }

    fn overlay_content(&self) -> Widget {
        if let Some(builder) = self.tooltip_builder.as_ref() {
            builder.build()
        } else if let Some(message) = self.rich_message.as_ref() {
            message.clone().into()
        } else if let Some(message) = self.message.as_ref() {
            Text::new(message.clone()).into()
        } else {
            crate::SizedBox::shrink().into()
        }
    }

    fn build(&self) -> Widget {
        self.controller.set_durations(self.durations);
        if self.trigger_mode == TooltipTriggerMode::Focus {
            let _ = self
                .controller
                .focus_changed_at(self.focus_node.has_focus(), Instant::now());
        }

        let active = self.enabled && self.has_overlay_content();
        if !active {
            return self.child.clone();
        }

        let controller = self.controller.clone();
        let mut anchor: Widget =
            CompositedTransformTarget::new(self.layer_link.clone(), self.child.clone()).into();

        // RawTooltip listens for hover on every trigger mode. The same device
        // set is used by the overlay region below so moving into the popup
        // does not start the exit delay.
        let hover_controller = controller.clone();
        let exit_controller = controller.clone();
        anchor = MouseRegion::new(anchor)
            .on_enter(move |event| {
                let _ = hover_controller.mouse_enter_at(event.device, event.time);
            })
            .on_exit(move |event| {
                let _ = exit_controller.mouse_exit_at(event.device, event.time);
            })
            .into();

        let uses_touch_gesture = matches!(
            self.trigger_mode,
            TooltipTriggerMode::Tap | TooltipTriggerMode::LongPress
        );
        if uses_touch_gesture || self.enable_tap_to_dismiss {
            let last_event_time = Rc::new(Cell::new(Instant::now()));
            let last_kind = Rc::new(Cell::new(PointerDeviceKind::Mouse));
            let down_controller = controller.clone();
            let mode = self.trigger_mode;
            let dismissible = self.enable_tap_to_dismiss;
            let down_time = last_event_time.clone();
            let down_kind = last_kind.clone();
            let mut listener = Listener::new(anchor).behavior(HitTestBehavior::Opaque);
            listener = listener.on_pointer_down(move |event| {
                down_time.set(event.time);
                down_kind.set(event.kind);
                if dismissible && !is_trigger_pointer(mode, event.kind) {
                    let _ = down_controller.dismiss_by_pointer_at(event.time);
                }
            });
            let up_time = last_event_time.clone();
            listener = listener
                .on_pointer_up(move |event| up_time.set(event.time))
                .on_pointer_cancel({
                    let cancel_time = last_event_time.clone();
                    move |event| cancel_time.set(event.time)
                });
            anchor = listener.into();

            if uses_touch_gesture {
                let tap_controller = controller.clone();
                let tap_time = last_event_time.clone();
                let tap_kind = last_kind.clone();
                let tap_down_kind = last_kind.clone();
                let long_controller = controller.clone();
                let long_time = last_event_time.clone();
                let long_kind = last_kind.clone();
                let cancel_controller = controller.clone();
                let callbacks = GestureCallbacks {
                    on_tap_down: Some(Rc::new(move |details: TapDownDetails| {
                        tap_down_kind.set(details.kind);
                    })),
                    on_tap: (self.trigger_mode == TooltipTriggerMode::Tap).then(|| {
                        Rc::new(move || {
                            if is_touch_trigger_kind(tap_kind.get()) {
                                let _ = tap_controller.trigger_tap_at(tap_time.get());
                            }
                        }) as Rc<dyn Fn()>
                    }),
                    on_long_press: (self.trigger_mode == TooltipTriggerMode::LongPress).then(
                        || {
                            Rc::new(move || {
                                if is_touch_trigger_kind(long_kind.get()) {
                                    let _ = long_controller.trigger_long_press_at(long_time.get());
                                }
                            }) as Rc<dyn Fn()>
                        },
                    ),
                    on_long_press_up: (self.trigger_mode == TooltipTriggerMode::LongPress).then(
                        || {
                            let release_controller = controller.clone();
                            let release_time = last_event_time.clone();
                            Rc::new(move || release_controller.release_touch_at(release_time.get()))
                                as Rc<dyn Fn()>
                        },
                    ),
                    on_cancel: self.enable_tap_to_dismiss.then(|| {
                        let cancel_time = last_event_time.clone();
                        Rc::new(move || {
                            let _ = cancel_controller.cancel_trigger_at(cancel_time.get());
                        }) as Rc<dyn Fn()>
                    }),
                    ..GestureCallbacks::default()
                };
                let factory = GestureRecognizerFactoryWithHandlers::new(
                    {
                        let callbacks = callbacks.clone();
                        move || PointerGestureRecognizer::new(callbacks.clone())
                    },
                    |_recognizer| {},
                );
                anchor = RawGestureDetector::new(anchor)
                    .gesture(factory)
                    .behavior(HitTestBehavior::DeferToChild)
                    .into();
            }
        }

        if self.trigger_mode == TooltipTriggerMode::Focus {
            let focus_controller = controller.clone();
            let focus_node = self.focus_node.clone();
            anchor = FocusableActionDetector::new(anchor)
                .focus_node(focus_node)
                .enabled(self.enabled)
                .on_focus_change(move |focused| {
                    let _ = focus_controller.focus_changed_at(focused, Instant::now());
                })
                .into();
        }

        if let Some(semantics) = self.semantic_value() {
            // The retained semantics collector only emits a wrapper when it
            // has a role. A role-less `Semantics::tooltip` annotation is
            // therefore transparent and its description never reaches the
            // retained tree. GenericContainer is the renderer-neutral
            // equivalent of Flutter's containerized tooltip annotation.
            anchor = Semantics::new(anchor)
                .role(SemanticRole::GenericContainer)
                .tooltip(semantics)
                .into();
        }

        if self.enable_tap_to_dismiss {
            let outside_controller = controller.clone();
            anchor = TapRegion::new(anchor)
                .on_tap_outside(move |event| {
                    let _ = outside_controller.dismiss_by_pointer_at(event.time);
                })
                .into();
        }

        let mut popup = self.overlay_content();
        let popup_hover_controller = controller.clone();
        let popup_exit_controller = controller.clone();
        popup = MouseRegion::new(popup)
            .on_enter(move |event| {
                let _ = popup_hover_controller.mouse_enter_at(event.device, event.time);
            })
            .on_exit(move |event| {
                let _ = popup_exit_controller.mouse_exit_at(event.device, event.time);
            })
            .into();
        popup = IgnorePointer::new(popup)
            .ignoring(self.ignore_pointer)
            .into();

        let target_anchor = self.target_anchor.unwrap_or(if self.prefer_below {
            Alignment::BOTTOM_CENTER
        } else {
            Alignment::TOP_CENTER
        });
        let follower_anchor = self.follower_anchor.unwrap_or(if self.prefer_below {
            Alignment::TOP_CENTER
        } else {
            Alignment::BOTTOM_CENTER
        });
        let popup_offset = self.offset.unwrap_or_else(|| {
            let vertical = if self.prefer_below {
                self.vertical_offset
            } else {
                -self.vertical_offset
            };
            Offset::new(0.0, vertical)
        });
        let follower = CompositedTransformFollower::new(self.layer_link.clone(), popup)
            .show_when_unlinked(false)
            .target_anchor(target_anchor)
            .follower_anchor(follower_anchor)
            .offset(popup_offset);

        OverlayPortal::new(anchor)
            .overlay_child(follower)
            .show(self.controller.is_visible())
            .into()
    }
}

impl From<RawTooltip> for Widget {
    fn from(value: RawTooltip) -> Self {
        let value = Rc::new(value);
        value.controller.set_durations(value.durations);
        value
            .controller
            .set_triggered_callback(value.on_triggered.clone());
        let revision = value.controller.revision_cell();
        Widget::stateful_layout_builder(revision, move |_, _| value.build())
    }
}
