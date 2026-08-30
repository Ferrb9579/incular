//! Animated modal barrier composition and semantics.

use crate::{Animation, Color, Container, GestureDetector, HitTestBehavior, Semantics, Widget};
use incular_animation::{AnimationController, TweenValue};
use std::{cell::Cell, rc::Rc};

/// A retained, animated modal barrier.
///
/// The barrier always occupies the constraints supplied by its parent,
/// absorbs background pointer input, blocks preceding semantics, and invokes
/// its dismissal handler from either a pointer tap or an accessibility
/// activate action when dismissal is enabled.
#[derive(Clone)]
pub struct AnimatedModalBarrier {
    color: Animation<Color>,
    dismissible: bool,
    semantics_label: Option<String>,
    barrier_semantics_dismissible: bool,
    semantics_on_tap_hint: Option<String>,
    on_dismiss: Option<Rc<dyn Fn()>>,
    dismissal_handler: Option<Rc<dyn Fn() -> bool>>,
    child: Option<Widget>,
}

impl Default for AnimatedModalBarrier {
    fn default() -> Self {
        Self::new()
    }
}

impl AnimatedModalBarrier {
    /// Creates a black, half-opacity barrier with a constant color animation.
    ///
    /// `with_color`/`color_animation` are the animated constructor forms;
    /// keeping this zero-argument constructor preserves the original Incular
    /// widget spelling while still lowering to the same animated path.
    #[must_use]
    pub fn new() -> Self {
        Self::with_color(constant_color_animation(Color::rgba(0, 0, 0, 128)))
    }

    /// Creates a barrier driven by a typed color animation.
    #[must_use]
    pub fn with_color(color: Animation<Color>) -> Self {
        Self {
            color,
            dismissible: true,
            semantics_label: None,
            barrier_semantics_dismissible: true,
            semantics_on_tap_hint: None,
            on_dismiss: None,
            dismissal_handler: None,
            child: None,
        }
    }

    /// Alias for [`Self::with_color`].
    #[must_use]
    pub fn color_animation(color: Animation<Color>) -> Self {
        Self::with_color(color)
    }

    /// Sets the color animation.
    #[must_use]
    pub fn color(mut self, color: Animation<Color>) -> Self {
        self.color = color;
        self
    }

    /// Returns the current sampled barrier color.
    #[must_use]
    pub fn current_color(&self) -> Color {
        self.color.value()
    }

    /// Returns the animation driving this barrier.
    #[must_use]
    pub fn animation(&self) -> Animation<Color> {
        self.color.clone()
    }

    /// Enables or disables pointer and semantic dismissal.
    #[must_use]
    pub fn dismissible(mut self, dismissible: bool) -> Self {
        self.dismissible = dismissible;
        self
    }

    /// Sets the callback invoked by pointer and semantic dismissal.
    #[must_use]
    pub fn on_dismiss(mut self, callback: impl Fn() + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(callback));
        self
    }

    /// Installs a boolean dismissal bridge.  This is the integration seam for
    /// `incular-navigation::Overlay::dismiss_top` or a route pop operation.
    #[must_use]
    pub fn dismissal_handler(mut self, callback: impl Fn() -> bool + 'static) -> Self {
        self.dismissal_handler = Some(Rc::new(callback));
        self
    }

    /// Sets the accessibility label exposed for the modal barrier.
    #[must_use]
    pub fn semantics_label(mut self, label: impl Into<String>) -> Self {
        self.semantics_label = Some(label.into());
        self
    }

    /// Controls whether a labeled barrier exposes an accessibility dismiss
    /// action.  Pointer dismissal remains governed by `dismissible`.
    #[must_use]
    pub fn barrier_semantics_dismissible(mut self, value: bool) -> Self {
        self.barrier_semantics_dismissible = value;
        self
    }

    /// Supplies the platform-specific accessibility tap hint.
    #[must_use]
    pub fn semantics_on_tap_hint(mut self, hint: impl Into<String>) -> Self {
        self.semantics_on_tap_hint = Some(hint.into());
        self
    }

    /// Optionally places the barrier above a retained child in a stack.
    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = Some(child.into());
        self
    }

    /// Invokes the configured dismissal bridge and returns whether it handled
    /// the dismissal.  A void callback is considered handled.
    #[must_use]
    pub fn dismiss(&self) -> bool {
        if !self.dismissible {
            return false;
        }
        if let Some(callback) = &self.on_dismiss {
            callback();
            return true;
        }
        self.dismissal_handler
            .as_ref()
            .is_some_and(|callback| callback())
    }
}

impl From<AnimatedModalBarrier> for Widget {
    fn from(value: AnimatedModalBarrier) -> Self {
        let revision = Rc::new(Cell::new(0_u64));
        let weak_revision = Rc::downgrade(&revision);
        let controller = value.color.controller();
        let _listener_id = controller.add_listener(move |_| {
            if let Some(revision) = weak_revision.upgrade() {
                revision.set(revision.get().wrapping_add(1));
            }
        });

        let color = value.color;
        let dismissible = value.dismissible;
        let semantics_label = value.semantics_label;
        let semantics_dismissible = value.barrier_semantics_dismissible;
        let semantics_hint = value.semantics_on_tap_hint;
        let on_dismiss = value.on_dismiss;
        let dismissal_handler = value.dismissal_handler;
        let child = value.child;

        Widget::stateful_layout_builder(revision, move |_| {
            let dismiss = {
                let on_dismiss = on_dismiss.clone();
                let dismissal_handler = dismissal_handler.clone();
                Rc::new(move || {
                    if let Some(callback) = &on_dismiss {
                        callback();
                    } else if let Some(callback) = &dismissal_handler {
                        let _ = callback();
                    }
                })
            };

            let mut barrier: Widget = Container::new().color(color.value()).into();
            if dismissible {
                barrier = GestureDetector::new(barrier)
                    .behavior(HitTestBehavior::Opaque)
                    .on_tap({
                        let dismiss = dismiss.clone();
                        move || dismiss()
                    })
                    .into();
            } else {
                barrier = Widget::absorb_pointer(true, barrier);
            }

            if let Some(label) = &semantics_label {
                if semantics_dismissible {
                    let mut semantics = Semantics::new(barrier).label(label.clone());
                    if let Some(hint) = &semantics_hint {
                        semantics = semantics.description(hint.clone());
                    }
                    if dismissible {
                        semantics = semantics.on_tap({
                            let dismiss = dismiss.clone();
                            move || dismiss()
                        });
                    }
                    barrier = semantics.into();
                }
            }
            barrier = barrier.block_semantics();

            if let Some(child) = &child {
                crate::Stack::new([child.clone(), barrier]).into()
            } else {
                barrier
            }
        })
    }
}

fn constant_color_animation(color: Color) -> Animation<Color> {
    Animation::new(
        AnimationController::default(),
        TweenValue::new(color, color),
    )
}
