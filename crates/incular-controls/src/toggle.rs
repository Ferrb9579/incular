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
struct ToggleGroupScope {
    multiple: bool,
    active: Rc<Cell<Option<usize>>>,
    revision: Rc<Cell<u64>>,
}

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
        self.build_with_group(theme, None)
    }

    fn build_with_group(&self, theme: &ControlTheme, group: Option<ToggleGroupScope>) -> Widget {
        let content = self
            .child
            .clone()
            .or_else(|| {
                self.label
                    .clone()
                    .map(|label| incular_widgets::Text::new(label).into())
            })
            .unwrap_or_else(|| incular_widgets::SizedBox::shrink().into());
        let id = Rc::as_ptr(&self.pressed) as usize;
        if let Some(group) = group.as_ref()
            && !group.multiple
            && self.pressed.get()
            && group.active.get().is_none()
        {
            group.active.set(Some(id));
        }
        let pressed = group.as_ref().map_or_else(
            || self.pressed.get(),
            |group| {
                if group.multiple {
                    self.pressed.get()
                } else {
                    group.active.get() == Some(id)
                }
            },
        );
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
        let selected_visual = Widget::from(
            incular_widgets::IgnorePointer::new(Widget::controlled_opacity(
                self.selected_opacity.clone(),
                Button::with_child(content.clone())
                    .variant(ButtonVariant::Primary)
                    .enabled(true)
                    .build(theme),
            ))
            .ignoring(true),
        )
        .exclude_semantics();
        let unselected_visual = Widget::from(
            incular_widgets::IgnorePointer::new(Widget::controlled_opacity(
                self.unselected_opacity.clone(),
                Button::with_child(content)
                    .variant(ButtonVariant::Ghost)
                    .enabled(true)
                    .build(theme),
            ))
            .ignoring(true),
        )
        .exclude_semantics();
        let visual = Stack::aligned(Alignment::CENTER, [selected_visual, unselected_visual]);

        let pressed_state = self.pressed.clone();
        let revision = self.revision.clone();
        let selected_opacity = self.selected_opacity.clone();
        let unselected_opacity = self.unselected_opacity.clone();
        let callback = self.on_change.clone();
        let group_for_click = group.clone();
        let mut button = ActionSurface::with_child(visual)
            .color(Color::TRANSPARENT)
            .disabled_color(theme.colors.disabled_surface)
            .enabled(self.enabled);
        if self.enabled {
            button = button.on_click(move || {
                let next = group_for_click.as_ref().map_or_else(
                    || !pressed_state.get(),
                    |group| {
                        if group.multiple {
                            !pressed_state.get()
                        } else {
                            group.active.get() != Some(id)
                        }
                    },
                );
                pressed_state.set(next);
                revision.set(revision.get().checked_add(1).expect("revision exhausted"));
                if let Some(group) = group_for_click.as_ref()
                    && !group.multiple
                {
                    group.active.set(next.then_some(id));
                    group.revision.set(
                        group
                            .revision
                            .get()
                            .checked_add(1)
                            .expect("revision exhausted"),
                    );
                }
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
                    checked: Some(pressed.into()),
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
        Widget::stateful_layout_builder(revision, move |context, _| {
            let theme = crate::theme::current_control_theme(context);
            let group = context.depend_on::<ToggleGroupScope>();
            value.build_with_group(&theme, group)
        })
    }
}

#[derive(Clone, Default, TypedBuilder)]
pub struct Group {
    #[builder(default)]
    multiple: bool,
    #[builder(default, setter(strip_option, into))]
    child: Option<Widget>,
    #[builder(default, setter(skip))]
    controller: crate::CompositeController,
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
    #[must_use]
    pub fn navigation(&self) -> crate::CompositeController {
        self.controller.clone()
    }
}
impl From<Group> for Widget {
    fn from(value: Group) -> Self {
        let child = value
            .child
            .unwrap_or_else(|| incular_widgets::SizedBox::shrink().into());
        let revision = Rc::new(Cell::new(0_u64));
        let scope = ToggleGroupScope {
            multiple: value.multiple,
            active: Rc::new(Cell::new(None)),
            revision: revision.clone(),
        };
        let controller = value.controller;
        Widget::stateful_layout_builder(revision, move |_, _| {
            Widget::environment_scope(
                controller.clone(),
                Widget::environment_scope(scope.clone(), child.clone()),
            )
        })
    }
}
