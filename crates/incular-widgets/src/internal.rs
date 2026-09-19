//! Framework-internal retained surfaces.
//!
//! These types are exposed only so sibling framework crates can share the
//! retained implementation. They are intentionally not re-exported from the
//! public widget prelude and are not part of Incular's Flutter-facing API.

use std::any::Any;
use std::{cell::Cell, rc::Rc, time::Duration};

use incular_config::EdgeInsets;
use incular_core::{Color, Size};
use incular_text::TextStyle;

pub use crate::recursion::FramePhase;

/// Callback-bearing compilation of Flutter's semantic platform-menu model.
/// Native application-shell integrations receive only the immutable snapshot;
/// callbacks stay on the runtime/UI thread and are dispatched by stable menu
/// IDs when a native event re-enters Incular.
#[doc(hidden)]
#[derive(Clone)]
pub struct PlatformMenuCommandModel {
    snapshot: crate::PlatformMenuSnapshot,
    callbacks: crate::platform_widgets::MenuCallbackSet,
}

impl PlatformMenuCommandModel {
    pub fn compile(menus: &[crate::PlatformMenu]) -> Result<Self, crate::PlatformMenuBuildError> {
        let (snapshot, callbacks) = crate::platform_widgets::build_snapshot(menus)?;
        Ok(Self {
            snapshot,
            callbacks,
        })
    }

    #[must_use]
    pub fn snapshot(&self) -> &crate::PlatformMenuSnapshot {
        &self.snapshot
    }

    #[must_use]
    pub fn dispatch(&self, event: &crate::PlatformMenuEvent) -> crate::MenuDispatchResult {
        self.callbacks.dispatch(event)
    }
}

// The editor model is owned by `incular-text`; the internal bridge keeps the
// path available to sibling implementation crates without creating a second
// retained controller type in Widgets.
pub use incular_text::{TextEditingController, TextEditingValue, TextRange, TextSelection};

// The retained engine is deliberately kept behind this `doc(hidden)` bridge.
// Runtime/platform/control crates need to exchange element IDs and diagnostics,
// but those implementation details must not become part of the Flutter-facing
// `incular_widgets::*` namespace.  Keeping the bridge explicit also makes the
// boundary auditable: anything below this module is framework plumbing, not a
// public Widgets API.
pub use crate::advanced_scrolling::{
    CacheExtentStyle, ChangeReportingBehavior, ChildVicinity, DiagonalDragBehavior,
    DraggableAnimationStep, DraggableNotificationSubscription, DraggableScrollableActuator,
    DraggableScrollableController, DraggableScrollableNotification, DraggableScrollableSheet,
    DraggableScrollableState, DraggableSheetDelta, DraggableSheetExtent, DraggableSizeAnimation,
    DraggableSnap, DraggableSnapTarget, FixedExtentScrollController, ListWheelScrollView,
    ListWheelViewport, RawScrollbar, RawScrollbarGeometry, RawScrollbarOrientation,
    RawScrollbarStyle, TwoDimensionalChildDelegate, TwoDimensionalChildLayout,
    TwoDimensionalConstraints, TwoDimensionalScrollDelta, TwoDimensionalScrollView,
    TwoDimensionalScrollable, TwoDimensionalViewport, TwoDimensionalViewportLayout,
    WheelChildDelegate, WheelChildLayout, WheelLayout, WheelMatrix, WheelProjection,
};
pub use crate::advanced_slivers::{
    AnimatedGrid, AnimatedGridController, AnimatedItem, AnimatedItemBuilder, AnimatedItemPhase,
    AnimatedList, AnimatedListController, AnimatedRemovedItemBuilder, AutomaticKeepAlive,
    KeepAlive, KeepAliveHandle, KeepAliveNotification, KeepAliveRegistry, SliverAnimatedGrid,
    SliverAnimatedGridController, TreeRowAnimation, TreeSliver, TreeSliverController,
    TreeSliverIndentation, TreeSliverNode, TreeSliverNodeId,
};
pub use crate::forms::*;
pub use crate::gestures::PointerEvent;
pub use crate::gestures::*;
pub use crate::layout::*;
pub use crate::navigation::BackButtonDispatcher as NavigationBackButtonDispatcher;
pub use crate::navigation::{
    AnimatedModalBarrier, BackButtonListener, BackDispatchReport, BackHandlerResult,
    BackRegistration, NavigatorPopHandler, NavigatorPopHandlerController, PageStorage,
    PageStorageBucket, PageStorageIdentifier, PageStorageKey, PopAttempt, PopScope,
    PopScopeController, RootRestorationScope, UnmanagedRestorationScope,
    current_page_storage_bucket, current_restoration_scope,
};
pub use crate::painting_effects::*;
#[doc(hidden)]
pub use crate::platform_widgets::PlatformMenuBinding;
pub use crate::raw_input::*;
pub use crate::scrolling::*;
pub use crate::selection::*;
#[doc(hidden)]
pub use crate::transient::TransientPlacementOverride;
pub use crate::tree::icons;
pub use crate::tree::{
    ActionId, AnimationRetargetBridge, Blend, Blur, BlurController, BoxFit, BuildContext,
    ButtonSpec, ButtonState, ColorFilterController, ColorFiltered, DecoratedBox, Diagnostics,
    DropShadow, DropShadowController, EditableText, Effects, Element, ElementId, ExplicitSemantics,
    FadeTransition, GeneratedChildIdentity, Icon, Image, ImageFit, ImageRepeat,
    InheritedScopeValue, InvariantCategory, InvariantViolation, Key, OpacityController,
    PERFORMANCE_OVERLAY_KEY, PathView, RenderKind, RotationController, ScaleController, ScrollView,
    ScrollbarDragDiagnostics, SlideTransition, SliverViewportDiagnostics, Text,
    TextFieldInputSnapshot, TextFieldSpec, TextInputActionHint, TextInputTypeHint, Transform,
    Transition, TranslationController, TreeError, Widget, WidgetTree, image_fit_rects,
    image_repeat_destinations, performance_overlay_placeholder, render_kind,
};
#[cfg(feature = "devtools")]
pub use crate::tree::{InvalidationCause, LayoutHistoryRecord};
pub use incular_scroll::*;

/// Installs a shared inherited value without wrapping it in another `Rc`.
/// Stable pointer identity therefore survives declarative theme rebuilds and
/// existing inherited-dependency tracking can suppress unaffected consumers.
#[doc(hidden)]
#[must_use]
pub fn shared_environment_scope<T: Any>(value: Rc<T>, child: Widget) -> Widget {
    Widget::shared_environment_scope(value, child)
}

/// Narrow retained bridge used by framework controls that need to preserve an
/// already-built single-axis scroll viewport without observing `WidgetKind`.
#[doc(hidden)]
#[must_use]
pub fn scroll_view_parts(widget: &Widget) -> Option<(ScrollController, Widget)> {
    match widget.kind() {
        crate::tree::WidgetKind::Scroll {
            controller, child, ..
        } => Some((controller.clone(), child.clone())),
        _ => None,
    }
}

/// Constructs the retained action node used by runtime and framework tests.
///
/// This explicit-ID form is framework plumbing; applications should compose
/// interactions with `GestureDetector` or use a Material button.
#[doc(hidden)]
pub fn action(size: Size, color: Color, action: ActionId) -> Widget {
    Widget::button(size, color, action)
}

/// Live interaction snapshot owned by a mounted retained action surface.
///
/// Sibling presentation crates can observe this state to resolve visuals, but
/// they never drive it. Pointer/focus tenure remains owned by `WidgetTree` and
/// the runtime.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ActionInteractionState {
    pub hovered: bool,
    pub pressed: bool,
    pub focused: bool,
}

/// Read-only presentation bridge for a retained action surface.
#[doc(hidden)]
#[derive(Clone, Default)]
pub struct ActionInteractionController {
    state: Rc<Cell<ActionInteractionState>>,
    revision: Rc<Cell<u64>>,
}

impl ActionInteractionController {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn state(&self) -> ActionInteractionState {
        self.state.get()
    }

    #[must_use]
    pub fn revision(&self) -> Rc<Cell<u64>> {
        self.revision.clone()
    }

    pub(crate) fn update(&self, next: ActionInteractionState) {
        if self.state.replace(next) != next {
            self.revision.set(
                self.revision
                    .get()
                    .checked_add(1)
                    .expect("revision exhausted"),
            );
        }
    }
}

impl PartialEq for ActionInteractionController {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.state, &other.state) && Rc::ptr_eq(&self.revision, &other.revision)
    }
}

impl Eq for ActionInteractionController {}

/// Renderer-neutral press-effect policy carried by a retained action surface.
/// Design-system crates map their public splash vocabulary into this enum;
/// native/runtime integrations may inspect it without depending on Material.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ActionSplashPolicy {
    #[default]
    Ripple,
    Splash,
    Sparkle,
    None,
}

/// Retained activation policy owned by the action surface.
///
/// `feedback_enabled` is an explicit platform-boundary value contract. W7
/// preserves it on the mounted action; platform capability execution is owned
/// by W8 rather than silently guessed inside a presentation crate.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActionPolicy {
    pub feedback_enabled: bool,
    pub splash: ActionSplashPolicy,
    pub transition_duration: Duration,
}

impl Default for ActionPolicy {
    fn default() -> Self {
        Self {
            feedback_enabled: true,
            splash: ActionSplashPolicy::Ripple,
            transition_duration: Duration::ZERO,
        }
    }
}

/// Internal action surface used by control implementations.
///
/// Flutter has no `widgets::ActionSurface`. Material's raw button is
/// `RawMaterialButton`; the public Material crate provides that name as a
/// compatibility surface over this retained implementation.
#[doc(hidden)]
pub struct ActionSurface {
    pub(crate) label: String,
    pub(crate) callback: Option<Rc<dyn Fn()>>,
    pub(crate) hover_callback: Option<Rc<dyn Fn()>>,
    pub(crate) exit_callback: Option<Rc<dyn Fn()>>,
    pub(crate) color: Color,
    pub(crate) hover_color: Option<Color>,
    pub(crate) pressed_color: Option<Color>,
    pub(crate) focused_color: Option<Color>,
    pub(crate) disabled_color: Option<Color>,
    pub(crate) enabled: bool,
    pub(crate) focusable_when_disabled: bool,
    pub(crate) size: Size,
    pub(crate) label_style: TextStyle,
    pub(crate) padding: EdgeInsets,
    pub(crate) content: Option<Widget>,
    pub(crate) interaction: Option<ActionInteractionController>,
    pub(crate) policy: ActionPolicy,
    pub(crate) mouse_cursor: crate::MouseCursor,
}

impl ActionSurface {
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            callback: None,
            hover_callback: None,
            exit_callback: None,
            color: Color::TRANSPARENT,
            hover_color: None,
            pressed_color: None,
            focused_color: None,
            disabled_color: None,
            enabled: true,
            focusable_when_disabled: false,
            size: Size::ZERO,
            label_style: TextStyle::default(),
            padding: EdgeInsets::ZERO,
            content: None,
            interaction: None,
            policy: ActionPolicy::default(),
            mouse_cursor: crate::MouseCursor::Defer,
        }
    }

    #[must_use]
    pub fn with_child(child: impl Into<Widget>) -> Self {
        Self {
            label: String::new(),
            content: Some(child.into()),
            ..Self::new("")
        }
    }

    /// Supplies the accessible name for a custom-content action surface.
    ///
    /// A child widget is visual content; it is not necessarily a semantic
    /// label. Keeping the two values separate matches Flutter's low-level
    /// button APIs and lets simulations, accessibility adapters, and keyboard
    /// navigation address icon-only or custom-content buttons reliably.
    #[must_use]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    #[must_use]
    pub fn on_press(mut self, callback: impl Fn() + 'static) -> Self {
        self.callback = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn on_click(self, callback: impl Fn() + 'static) -> Self {
        self.on_press(callback)
    }

    #[must_use]
    pub fn on_hover(mut self, callback: impl Fn() + 'static) -> Self {
        self.hover_callback = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn on_exit(mut self, callback: impl Fn() + 'static) -> Self {
        self.exit_callback = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    #[must_use]
    pub fn hover_color(mut self, color: Color) -> Self {
        self.hover_color = Some(color);
        self
    }

    #[must_use]
    pub fn pressed_color(mut self, color: Color) -> Self {
        self.pressed_color = Some(color);
        self
    }

    #[must_use]
    pub fn focused_color(mut self, color: Color) -> Self {
        self.focused_color = Some(color);
        self
    }

    #[must_use]
    pub fn disabled_color(mut self, color: Color) -> Self {
        self.disabled_color = Some(color);
        self
    }

    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    #[must_use]
    pub fn focusable_when_disabled(mut self, value: bool) -> Self {
        self.focusable_when_disabled = value;
        self
    }

    #[must_use]
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    #[must_use]
    pub fn label_style(mut self, style: TextStyle) -> Self {
        self.label_style = style;
        self
    }

    #[must_use]
    pub fn padding(mut self, padding: EdgeInsets) -> Self {
        self.padding = padding;
        self
    }

    #[must_use]
    pub fn content(mut self, content: impl Into<Widget>) -> Self {
        self.content = Some(content.into());
        self
    }

    /// Binds a presentation observer to this mounted action surface.
    #[must_use]
    pub fn interaction_controller(mut self, controller: ActionInteractionController) -> Self {
        self.interaction = Some(controller);
        self
    }

    #[must_use]
    pub fn policy(mut self, policy: ActionPolicy) -> Self {
        self.policy = policy;
        self
    }

    #[must_use]
    pub fn mouse_cursor(mut self, cursor: crate::MouseCursor) -> Self {
        self.mouse_cursor = cursor;
        self
    }
}

/// Returns the explicit action policy when `widget` is the retained action
/// node itself. Framework tests use this to prove design-system setters reach
/// the neutral behavior owner without exposing `WidgetKind` publicly.
#[doc(hidden)]
#[must_use]
pub fn action_policy(widget: &Widget) -> Option<ActionPolicy> {
    match widget.kind() {
        crate::tree::WidgetKind::Button(spec) => Some(spec.policy),
        _ => None,
    }
}

impl From<ActionSurface> for Widget {
    fn from(value: ActionSurface) -> Self {
        Widget::action_surface(value)
    }
}
