//! Slider anatomy and value constraints.

use crate::theme::ControlTheme;
use incular_config::Axis;
use incular_semantics::{Role as SemanticRole, SemanticActionKind, SemanticState};
use incular_widgets::{
    Container, ExplicitSemantics, GestureDetector, HitTestBehavior, Positioned, Stack, Widget,
};
use std::cell::Cell;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Range {
    pub min: f32,
    pub max: f32,
    pub step: f32,
}
impl Default for Range {
    fn default() -> Self {
        Self {
            min: 0.,
            max: 1.,
            step: 0.01,
        }
    }
}
impl Range {
    #[must_use]
    pub fn clamp(self, value: f32) -> f32 {
        let step = self.step.max(f32::EPSILON);
        ((value.clamp(self.min, self.max) - self.min) / step)
            .round()
            .mul_add(step, self.min)
            .clamp(self.min, self.max)
    }
}

#[derive(Clone)]
pub struct Root {
    value: Rc<Cell<f32>>,
    revision: Rc<Cell<u64>>,
    range: Range,
    orientation: Axis,
    enabled: bool,
    child: Option<Widget>,
    on_change: Option<Rc<dyn Fn(f32) + 'static>>,
}
impl Default for Root {
    fn default() -> Self {
        Self::new()
    }
}
impl Root {
    #[must_use]
    pub fn new() -> Self {
        Self {
            value: Rc::new(Cell::new(0.)),
            revision: Rc::new(Cell::new(0)),
            range: Range::default(),
            orientation: Axis::Horizontal,
            enabled: true,
            child: None,
            on_change: None,
        }
    }
    #[must_use]
    pub fn value(self, value: f32) -> Self {
        self.value.set(self.range.clamp(value));
        self
    }
    #[must_use]
    pub fn range(mut self, min: f32, max: f32) -> Self {
        self.range.min = min.min(max);
        self.range.max = max.max(min);
        self.value.set(self.range.clamp(self.value.get()));
        self
    }
    #[must_use]
    pub fn step(mut self, value: f32) -> Self {
        self.range.step = value.abs().max(f32::EPSILON);
        self
    }
    #[must_use]
    pub fn vertical(mut self) -> Self {
        self.orientation = Axis::Vertical;
        self
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
    pub fn on_value_change(mut self, cb: impl Fn(f32) + 'static) -> Self {
        self.on_change = Some(Rc::new(cb));
        self
    }

    /// Resolves the default slider anatomy from the active control theme.
    /// Applications can still replace the complete visual with [`Root::child`]
    /// while retaining the descriptor's value and callback policy.
    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        if let Some(child) = &self.child {
            return child.clone();
        }

        let current_value = self.value.get();
        let ratio = ((current_value - self.range.min)
            / (self.range.max - self.range.min).max(f32::EPSILON))
        .clamp(0., 1.);
        let track_extent = 180.;
        let track_thickness = theme.slider.track_height.max(1.);
        let thumb_size = theme.slider.thumb_size.max(track_thickness);
        let track = if self.orientation == Axis::Horizontal {
            Container::new()
                .width(track_extent)
                .height(track_thickness)
                .color(theme.colors.border_strong)
        } else {
            Container::new()
                .width(track_thickness)
                .height(track_extent)
                .color(theme.colors.border_strong)
        };
        let indicator_extent = (track_extent * ratio).max(track_thickness);
        let indicator = if self.orientation == Axis::Horizontal {
            Container::new()
                .width(indicator_extent)
                .height(track_thickness)
                .color(if self.enabled {
                    theme.colors.accent
                } else {
                    theme.colors.disabled_foreground
                })
        } else {
            Container::new()
                .width(track_thickness)
                .height(indicator_extent)
                .color(if self.enabled {
                    theme.colors.accent
                } else {
                    theme.colors.disabled_foreground
                })
        };
        let thumb = Container::new()
            .width(thumb_size)
            .height(thumb_size)
            .color(theme.colors.surface)
            .border(incular_widgets::Border::new(
                theme.button.border_width,
                theme.colors.border_strong,
            ))
            .radius(theme.slider.radius);
        let visual = if self.orientation == Axis::Horizontal {
            Stack::new([
                Widget::from(track),
                Widget::from(Positioned::new(indicator).left(0.).top(0.)),
                Widget::from(
                    Positioned::new(thumb)
                        .left((track_extent * ratio - thumb_size * 0.5).max(0.))
                        .top((track_thickness - thumb_size) * 0.5),
                ),
            ])
        } else {
            Stack::new([
                Widget::from(track),
                Widget::from(Positioned::new(indicator).left(0.).bottom(0.)),
                Widget::from(
                    Positioned::new(thumb)
                        .left((track_thickness - thumb_size) * 0.5)
                        .bottom((track_extent * ratio - thumb_size * 0.5).max(0.)),
                ),
            ])
        };
        let value = self.value.clone();
        let revision = self.revision.clone();
        let range = self.range;
        let orientation = self.orientation;
        let callback = self.on_change.clone();
        let activate = {
            let value = value.clone();
            let revision = revision.clone();
            let callback = callback.clone();
            move || {
                // A tap is also a useful keyboard/accessible activation
                // fallback: advance one quantized step.
                let next = range.clamp(value.get() + range.step);
                value.set(next);
                revision.set(revision.get().wrapping_add(1));
                if let Some(callback) = &callback {
                    callback(next);
                }
            }
        };
        let drag_value = value.clone();
        let drag_revision = revision.clone();
        let drag_callback = callback.clone();
        let drag = move |delta: incular_core::Offset| {
            let travel = if orientation == Axis::Horizontal {
                delta.x / track_extent
            } else {
                -delta.y / track_extent
            };
            let next = range.clamp(current_value + travel * (range.max - range.min));
            if (next - drag_value.get()).abs() <= f32::EPSILON {
                return;
            }
            drag_value.set(next);
            drag_revision.set(drag_revision.get().wrapping_add(1));
            if let Some(callback) = &drag_callback {
                callback(next);
            }
        };
        let interactive: Widget = if self.enabled {
            let detector = GestureDetector::new(visual)
                .behavior(HitTestBehavior::Opaque)
                .on_tap(activate);
            if orientation == Axis::Horizontal {
                detector.on_horizontal_drag_update(drag).into()
            } else {
                detector.on_vertical_drag_update(drag).into()
            }
        } else {
            visual.into()
        };
        let value_text = format!("{:.3}", range.clamp(current_value));
        interactive.semantics(
            ExplicitSemantics::new(SemanticRole::Slider)
                .value(value_text)
                .state(SemanticState {
                    enabled: self.enabled,
                    focusable: self.enabled,
                    ..SemanticState::default()
                })
                .actions(if self.enabled {
                    [SemanticActionKind::Focus, SemanticActionKind::Activate].to_vec()
                } else {
                    Vec::new()
                }),
        )
    }
}
impl From<Root> for Widget {
    fn from(value: Root) -> Self {
        let value = Rc::new(value);
        let revision = value.revision.clone();
        Widget::stateful_layout_builder(revision, move |_| {
            let theme =
                incular_widgets::current_build_environment::<ControlTheme>().unwrap_or_default();
            value.build(&theme)
        })
    }
}
#[derive(Clone)]
pub struct Control {
    child: Widget,
}
impl Control {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}
impl From<Control> for Widget {
    fn from(value: Control) -> Self {
        value.child
    }
}
#[derive(Clone)]
pub struct Track {
    child: Widget,
}
impl Track {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}
impl From<Track> for Widget {
    fn from(value: Track) -> Self {
        value.child
    }
}
#[derive(Clone)]
pub struct Indicator {
    child: Widget,
}
impl Indicator {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}
impl From<Indicator> for Widget {
    fn from(value: Indicator) -> Self {
        value.child
    }
}
#[derive(Clone)]
pub struct Thumb {
    child: Widget,
}
impl Thumb {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}
impl From<Thumb> for Widget {
    fn from(value: Thumb) -> Self {
        value.child
    }
}
