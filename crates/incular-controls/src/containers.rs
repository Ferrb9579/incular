use crate::styles::{CardStyle, DividerStyle};
use crate::theme::ControlTheme;
use incular_config::EdgeInsets;
use incular_widgets::{Border, BorderRadius, BoxDecoration, Container, Widget};
use std::rc::Rc;

/// Themed surface Card component.
#[derive(Clone)]
pub struct Card {
    child: Widget,
    style: CardStyle,
}

impl Card {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            style: CardStyle::default(),
        }
    }

    #[must_use]
    pub fn style(mut self, style: CardStyle) -> Self {
        self.style = style;
        self
    }

    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        let bg = self.style.background.unwrap_or(theme.colors.surface);
        let radius = self
            .style
            .border_radius
            .unwrap_or(theme.metrics.border_radius * 1.5);
        let padding = self.style.padding.unwrap_or_else(|| EdgeInsets::all(16.0));
        let border = self
            .style
            .border
            .unwrap_or_else(|| Border::new(theme.metrics.border_width, theme.colors.border));

        Container::new()
            .padding(padding)
            .decoration(
                BoxDecoration::new()
                    .color(bg)
                    .border(border)
                    .border_radius(BorderRadius::circular(radius)),
            )
            .child(self.child.clone())
            .into()
    }
}

impl From<Card> for Widget {
    fn from(value: Card) -> Self {
        let value = Rc::new(value);
        Widget::layout_builder(move |_| {
            let theme =
                incular_widgets::current_build_environment::<ControlTheme>().unwrap_or_default();
            value.build(&theme)
        })
    }
}

/// Themed horizontal divider line.
#[derive(Clone)]
pub struct Divider {
    style: DividerStyle,
}

impl Divider {
    #[must_use]
    pub fn new() -> Self {
        Self {
            style: DividerStyle::default(),
        }
    }

    #[must_use]
    pub fn style(mut self, style: DividerStyle) -> Self {
        self.style = style;
        self
    }

    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        let color = self.style.color.unwrap_or(theme.colors.border);
        let thickness = self.style.thickness.unwrap_or(1.0);
        let indent = self.style.indent.unwrap_or(0.0);
        let end_indent = self.style.end_indent.unwrap_or(0.0);

        Container::new()
            .height(thickness)
            .margin(EdgeInsets::only(indent, 0.0, end_indent, 0.0))
            .color(color)
            .into()
    }
}

impl Default for Divider {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Divider> for Widget {
    fn from(value: Divider) -> Self {
        let value = Rc::new(value);
        Widget::layout_builder(move |_| {
            let theme =
                incular_widgets::current_build_environment::<ControlTheme>().unwrap_or_default();
            value.build(&theme)
        })
    }
}
