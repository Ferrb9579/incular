//! Native drawer surface built on the retained stack/interaction primitives.

use crate::overlay::Side;
use incular_config::EdgeInsets;
use incular_core::Color;
use incular_semantics::{Role as SemanticRole, SemanticState};
use incular_widgets::internal::{ActionSurface, ExplicitSemantics};
use incular_widgets::{BorderRadius, BoxDecoration, Container, Positioned, Stack, Widget};
use std::rc::Rc;
use typed_builder::TypedBuilder;

#[derive(Clone, TypedBuilder)]
pub struct Root {
    #[builder(default = false)]
    open: bool,
    #[builder(default = Side::Right)]
    side: Side,
    #[builder(default = 320., setter(transform = |value: f32| value.max(1.)))]
    width: f32,
    #[builder(default, setter(strip_option, into))]
    child: Option<Widget>,
    #[builder(default, setter(strip_option, into))]
    panel: Option<Widget>,
    #[builder(default = true)]
    modal: bool,
    #[builder(default, setter(skip))]
    on_open_change: Option<Rc<dyn Fn(bool) + 'static>>,
}

impl Default for Root {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl Root {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn open(mut self, value: bool) -> Self {
        self.open = value;
        self
    }

    #[must_use]
    pub fn default_open(self, value: bool) -> Self {
        self.open(value)
    }

    #[must_use]
    pub fn side(mut self, value: Side) -> Self {
        self.side = value;
        self
    }

    #[must_use]
    pub fn width(mut self, value: f32) -> Self {
        self.width = value.max(1.);
        self
    }

    #[must_use]
    pub fn child(mut self, value: impl Into<Widget>) -> Self {
        self.child = Some(value.into());
        self
    }

    #[must_use]
    pub fn panel(mut self, value: impl Into<Widget>) -> Self {
        self.panel = Some(value.into());
        self
    }

    #[must_use]
    pub fn modal(mut self, value: bool) -> Self {
        self.modal = value;
        self
    }

    #[must_use]
    pub fn on_open_change(mut self, callback: impl Fn(bool) + 'static) -> Self {
        self.on_open_change = Some(Rc::new(callback));
        self
    }
}

impl From<Root> for Widget {
    fn from(value: Root) -> Self {
        let base = value
            .child
            .unwrap_or_else(|| incular_widgets::SizedBox::shrink().into());
        if !value.open {
            return base;
        }
        let mut children = vec![base];
        if value.modal {
            let close = value.on_open_change.clone();
            let hit = ActionSurface::with_child(
                Container::new()
                    .width(1.)
                    .height(1.)
                    .color(Color::rgba(0, 0, 0, 100)),
            )
            .color(Color::TRANSPARENT);
            let hit = if let Some(close) = close {
                hit.on_click(move || close(false))
            } else {
                hit
            };
            children.push(Positioned::fill(hit).into());
        }
        let panel = value
            .panel
            .unwrap_or_else(|| incular_widgets::SizedBox::shrink().into());
        let panel: Widget = Container::with_child(panel)
            .width(value.width)
            .padding(EdgeInsets::all(16.))
            .decoration(
                BoxDecoration::new()
                    .color(Color::rgba(30, 33, 42, 255))
                    .border_radius(BorderRadius::circular(6.)),
            )
            .into();
        let panel = match value.side {
            Side::Left => Positioned::new(panel).left(0.).top(0.).bottom(0.),
            Side::Right => Positioned::new(panel).right(0.).top(0.).bottom(0.),
            Side::Top => Positioned::new(panel).left(0.).top(0.).right(0.),
            Side::Bottom => Positioned::new(panel).left(0.).right(0.).bottom(0.),
        };
        children.push(panel.into());
        let visual: Widget = Stack::new(children).into();
        visual.semantics(
            ExplicitSemantics::new(SemanticRole::Dialog).state(SemanticState {
                enabled: true,
                expanded: Some(true),
                ..SemanticState::default()
            }),
        )
    }
}

pub type Trigger = crate::popup::Trigger;
pub type Portal = crate::popup::Portal;
pub type Popup = crate::popup::Popup;

#[cfg(test)]
mod tests {
    use super::*;
    use incular_widgets::{Text, Widget};

    #[test]
    fn builder_uses_explicit_defaults_and_generic_children() {
        let default = Root::builder().build();
        assert!(!default.open);
        assert_eq!(default.side, Side::Right);
        assert_eq!(default.width, 320.);
        assert!(default.child.is_none());
        assert!(default.panel.is_none());
        assert!(default.modal);
        assert!(default.on_open_change.is_none());

        let drawer = Root::builder()
            .open(true)
            .side(Side::Left)
            .width(0.)
            .child(Text::new("page"))
            .panel(Text::new("drawer"))
            .modal(false)
            .build();
        assert_eq!(drawer.width, 1.);
        assert!(drawer.child.is_some());
        assert!(drawer.panel.is_some());
        assert!(!drawer.modal);

        let _: Widget = Root::new().child(Text::new("legacy")).into();
    }
}
