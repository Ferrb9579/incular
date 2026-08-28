//! Toggle and toggle-group controls.

use crate::{Button, ButtonVariant, ControlTheme};
use incular_config::Alignment;
use incular_core::Color;
use incular_semantics::{Role as SemanticRole, SemanticActionKind, SemanticState};
use incular_widgets::internal::{ActionSurface, ExplicitSemantics, OpacityController};
use incular_widgets::{Stack, Widget};
use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};
use typed_builder::TypedBuilder;

#[derive(Clone)]
pub struct Toggle {
    pressed: Rc<Cell<bool>>,
    revision: Rc<Cell<u64>>,
    selected_opacity: OpacityController,
    unselected_opacity: OpacityController,
    enabled: bool,
    label: Option<String>,
    child: Option<Widget>,
    on_change: Option<Rc<dyn Fn(bool) + 'static>>,
}
impl Toggle {
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        let selected_opacity = OpacityController::new();
        selected_opacity.set_opacity(0.);
        let unselected_opacity = OpacityController::new();
        Self {
            pressed: Rc::new(Cell::new(false)),
            revision: Rc::new(Cell::new(0)),
            selected_opacity,
            unselected_opacity,
            enabled: true,
            label: Some(label.into()),
            child: None,
            on_change: None,
        }
    }
    #[must_use]
    pub fn pressed(self, value: bool) -> Self {
        self.pressed.set(value);
        self.selected_opacity
            .set_opacity(if value { 1. } else { 0. });
        self.unselected_opacity
            .set_opacity(if value { 0. } else { 1. });
        self
    }
    #[must_use]
    pub fn default_pressed(self, value: bool) -> Self {
        self.pressed(value)
    }
    #[must_use]
    pub fn disabled(mut self, value: bool) -> Self {
        self.enabled = !value;
        self
    }
    #[must_use]
    pub fn child(mut self, value: impl Into<Widget>) -> Self {
        self.child = Some(value.into());
        self
    }
    #[must_use]
    pub fn on_pressed_change(mut self, callback: impl Fn(bool) + 'static) -> Self {
        self.on_change = Some(Rc::new(callback));
        self
    }
    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        let content = self
            .child
            .clone()
            .or_else(|| {
                self.label
                    .clone()
                    .map(|label| incular_widgets::Text::new(label).into())
            })
            .unwrap_or_else(|| incular_widgets::SizedBox::shrink().into());
        let pressed = self.pressed.get();
        let transition = if theme.motion.reduced_motion {
            Duration::ZERO
        } else {
            Duration::from_millis(u64::from(theme.motion.pressed_duration_ms.max(140)))
        };
        let selected_target = if pressed { 1. } else { 0. };
        if (self.selected_opacity.opacity() - selected_target).abs() > f32::EPSILON {
            if transition.is_zero() {
                self.selected_opacity.set_opacity(selected_target);
                self.unselected_opacity.set_opacity(1. - selected_target);
            } else {
                let now = Instant::now();
                self.selected_opacity
                    .animate_to(selected_target, transition, now);
                self.unselected_opacity
                    .animate_to(1. - selected_target, transition, now);
            }
        }

        // Keep both visual variants mounted and cross-fade their retained
        // surfaces.  The transparent action surface below remains the sole hit
        // target and semantic node, so the transition never duplicates input
        // or accessibility actions.
        let selected_visual = Widget::ignore_pointer(
            true,
            Widget::controlled_opacity(
                self.selected_opacity.clone(),
                Button::with_child(content.clone())
                    .variant(ButtonVariant::Primary)
                    .enabled(true)
                    .build(theme),
            ),
        )
        .exclude_semantics();
        let unselected_visual = Widget::ignore_pointer(
            true,
            Widget::controlled_opacity(
                self.unselected_opacity.clone(),
                Button::with_child(content)
                    .variant(ButtonVariant::Ghost)
                    .enabled(true)
                    .build(theme),
            ),
        )
        .exclude_semantics();
        let visual = Stack::aligned(Alignment::CENTER, [selected_visual, unselected_visual]);

        let pressed_state = self.pressed.clone();
        let revision = self.revision.clone();
        let selected_opacity = self.selected_opacity.clone();
        let unselected_opacity = self.unselected_opacity.clone();
        let callback = self.on_change.clone();
        let mut button = ActionSurface::with_child(visual)
            .color(Color::TRANSPARENT)
            .disabled_color(theme.colors.disabled_surface)
            .enabled(self.enabled);
        if self.enabled {
            button = button.on_click(move || {
                let next = !pressed_state.get();
                pressed_state.set(next);
                revision.set(revision.get().wrapping_add(1));
                let selected_target = if next { 1. } else { 0. };
                if transition.is_zero() {
                    selected_opacity.set_opacity(selected_target);
                    unselected_opacity.set_opacity(1. - selected_target);
                } else {
                    let now = Instant::now();
                    selected_opacity.animate_to(selected_target, transition, now);
                    unselected_opacity.animate_to(1. - selected_target, transition, now);
                }
                if let Some(callback) = &callback {
                    callback(next);
                }
            });
        }
        let raw: Widget = button.into();
        raw.semantics(
            ExplicitSemantics::new(SemanticRole::Button)
                .label(self.label.clone().unwrap_or_default())
                .state(SemanticState {
                    enabled: self.enabled,
                    focusable: self.enabled,
                    checked: Some(pressed),
                    ..SemanticState::default()
                })
                .actions(if self.enabled {
                    vec![SemanticActionKind::Focus, SemanticActionKind::Activate]
                } else {
                    Vec::new()
                }),
        )
    }
}
impl From<Toggle> for Widget {
    fn from(value: Toggle) -> Self {
        let value = Rc::new(value);
        let revision = value.revision.clone();
        Widget::stateful_layout_builder(revision, move |_| {
            let theme = incular_widgets::internal::current_build_environment::<ControlTheme>()
                .unwrap_or_default();
            value.build(&theme)
        })
    }
}

#[derive(Clone, Default, TypedBuilder)]
pub struct Group {
    #[builder(default)]
    multiple: bool,
    #[builder(default, setter(strip_option, into))]
    child: Option<Widget>,
}

impl Group {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    #[must_use]
    pub fn with_child(child: impl Into<Widget>) -> Self {
        Self::default().child(child)
    }
    #[must_use]
    pub fn multiple(mut self, value: bool) -> Self {
        self.multiple = value;
        self
    }
    #[must_use]
    pub fn child(mut self, value: impl Into<Widget>) -> Self {
        self.child = Some(value.into());
        self
    }
}
impl From<Group> for Widget {
    fn from(value: Group) -> Self {
        value
            .child
            .unwrap_or_else(|| incular_widgets::SizedBox::shrink().into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use incular_widgets::Text;

    #[test]
    fn group_builder_uses_explicit_defaults() {
        let group = Group::builder().build();

        assert!(!group.multiple);
        assert!(group.child.is_none());
        assert!(!Group::new().multiple);
    }

    #[test]
    fn group_builder_accepts_generic_widget_children() {
        let group = Group::builder()
            .multiple(true)
            .child(Text::new("toggles"))
            .build();

        assert!(group.multiple);
        assert!(group.child.is_some());
        let _: Widget = Group::with_child(Text::new("child")).into();
    }
}
