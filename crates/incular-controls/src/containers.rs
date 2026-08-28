use crate::styles::{CardStyle, DividerStyle};
use crate::theme::ControlTheme;
use incular_config::EdgeInsets;
use incular_widgets::{Border, BorderRadius, BoxDecoration, Container, Widget};
use std::rc::Rc;
use typed_builder::TypedBuilder;

/// Themed surface Card component.
#[derive(Clone, TypedBuilder)]
pub struct Card {
    #[builder(setter(into))]
    child: Widget,
    #[builder(default)]
    style: CardStyle,
}

impl Default for Card {
    fn default() -> Self {
        Self {
            child: Widget::from(incular_widgets::SizedBox::shrink()),
            style: CardStyle::default(),
        }
    }
}

impl Card {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self::builder().child(child).build()
    }

    /// Creates a Card wrapping a child widget.
    #[must_use]
    pub fn with_child(child: impl Into<Widget>) -> Self {
        Self::new(child)
    }

    /// Replaces the card's child widget.
    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = child.into();
        self
    }

    /// Content-oriented alias for [`Card::child`].
    #[must_use]
    pub fn content(self, content: impl Into<Widget>) -> Self {
        self.child(content)
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
            let theme = incular_widgets::internal::current_build_environment::<ControlTheme>()
                .unwrap_or_default();
            value.build(&theme)
        })
    }
}

/// Themed horizontal divider line.
#[derive(Clone, TypedBuilder)]
pub struct Divider {
    #[builder(default)]
    style: DividerStyle,
}

impl Divider {
    #[must_use]
    pub fn new() -> Self {
        Self::builder().build()
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
            let theme = incular_widgets::internal::current_build_environment::<ControlTheme>()
                .unwrap_or_default();
            value.build(&theme)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use incular_config::Constraints;
    use incular_core::Size;
    use incular_widgets::Text;
    use incular_widgets::internal::WidgetTree;

    #[test]
    fn card_builder_accepts_widgets_and_preserves_child_compatibility() {
        let card = Card::builder().child(Text::new("Card")).build();
        assert_eq!(card.child.text_if_any().as_deref(), Some("Card"));

        let nested = Card::builder()
            .child(Card::new(Text::new("Nested")))
            .build();
        assert!(nested.child.text_if_any().is_none());

        let _: Card = Card::new(Text::new("New"));
        let _: Card = Card::with_child(Text::new("With child"));
        let _: Widget = card.content(Text::new("Content")).into();
        let _: Widget = Card::default().into();
    }

    #[test]
    fn card_defaults_and_style_overrides_are_stable() {
        let default = Card::default();
        assert_eq!(default.style, CardStyle::default());

        let style = CardStyle {
            padding: Some(EdgeInsets::all(8.0)),
            ..CardStyle::default()
        };
        let card = Card::builder()
            .child(Text::new("Styled"))
            .style(style.clone())
            .build();
        assert_eq!(card.style, style);

        let mut tree = WidgetTree::new();
        let root = tree.mount(card.into()).unwrap();
        tree.layout(Constraints::loose(Size::new(240.0, 120.0)));
        assert!(
            tree.render_size(tree.render_id(root).unwrap())
                .unwrap()
                .width
                > 0.0
        );
    }

    #[test]
    fn divider_builder_matches_new_and_default() {
        let new = Divider::new();
        let default = Divider::default();
        let built = Divider::builder().build();

        assert_eq!(new.style, default.style);
        assert_eq!(new.style, built.style);
        let _: Widget = built.into();
    }
}
