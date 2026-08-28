use crate::styles::TextFieldStyle;
use crate::theme::ControlTheme;
use incular_config::EdgeInsets;
use incular_core::Size;
use incular_text::TextStyle;
use incular_widgets::internal::TextEditingController;
use incular_widgets::{
    Border, BorderRadius, BoxDecoration, Container, EditableText as RawEditableText, Widget,
};
use std::rc::Rc;
use typed_builder::TypedBuilder;

/// Base UI terminology for a single-line text control. `TextField` remains
/// the descriptive compatibility name used by existing applications.
pub type Input = TextField;

/// Styled single-line text input field.
#[derive(Clone, Default, TypedBuilder)]
pub struct TextField {
    #[builder(default = TextEditingController::new(), setter(into))]
    controller: TextEditingController,
    #[builder(default = Size::ZERO)]
    size: Size,
    #[builder(default = String::new(), setter(into))]
    placeholder: String,
    #[builder(default)]
    style: TextFieldStyle,
    #[builder(
        default,
        setter(
            fn transform<F>(callback: F) -> Option<Rc<dyn Fn(String) + 'static>>
            where
                F: Fn(String) + 'static,
            {
                Some(Rc::new(callback))
            }
        )
    )]
    on_submit: Option<Rc<dyn Fn(String) + 'static>>,
}

impl TextField {
    #[must_use]
    pub fn new(controller: TextEditingController) -> Self {
        Self::builder().controller(controller).build()
    }

    /// Sets an explicit logical size. A zero component keeps the natural
    /// intrinsic size for that axis.
    #[must_use]
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    #[must_use]
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    #[must_use]
    pub fn style(mut self, style: TextFieldStyle) -> Self {
        self.style = style;
        self
    }

    #[must_use]
    pub fn on_submit(mut self, on_submit: impl Fn(String) + 'static) -> Self {
        self.on_submit = Some(Rc::new(on_submit));
        self
    }

    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        let bg = self.style.background.unwrap_or(theme.colors.surface);
        let fg = self.style.foreground.unwrap_or(theme.colors.foreground);
        let radius = self.style.border_radius.unwrap_or(theme.input.radius);
        let padding = self
            .style
            .padding
            .unwrap_or_else(|| theme.density.padding());
        let border = self
            .style
            .border
            .unwrap_or_else(|| Border::new(theme.input.border_width, theme.colors.border));

        let mut raw = RawEditableText::new(self.controller.clone())
            .size(self.size)
            .placeholder(self.placeholder.clone())
            .style(
                TextStyle::new()
                    .font_size(theme.typography.body.size)
                    .color(fg),
            );

        if let Some(cb) = self.on_submit.clone() {
            raw = raw.on_submit(move |value| cb(value));
        }

        Container::new()
            .padding(padding)
            .decoration(
                BoxDecoration::new()
                    .color(bg)
                    .border(border)
                    .border_radius(BorderRadius::circular(radius)),
            )
            .child(raw)
            .into()
    }
}

impl From<TextField> for Widget {
    fn from(value: TextField) -> Self {
        let value = Rc::new(value);
        Widget::layout_builder(move |_| {
            let theme = incular_widgets::internal::current_build_environment::<ControlTheme>()
                .unwrap_or_default();
            value.build(&theme)
        })
    }
}

/// Styled multiline text editing area.
#[derive(Clone, Default, TypedBuilder)]
pub struct TextArea {
    #[builder(default = TextEditingController::new(), setter(into))]
    controller: TextEditingController,
    #[builder(default = Size::ZERO)]
    size: Size,
    #[builder(default = String::new(), setter(into))]
    placeholder: String,
    #[builder(default)]
    style: TextFieldStyle,
}

impl TextArea {
    #[must_use]
    pub fn new(controller: TextEditingController) -> Self {
        Self::builder().controller(controller).build()
    }

    /// Sets an explicit logical size. A zero component keeps the natural
    /// intrinsic size for that axis.
    #[must_use]
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    #[must_use]
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    #[must_use]
    pub fn style(mut self, style: TextFieldStyle) -> Self {
        self.style = style;
        self
    }

    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        let bg = self.style.background.unwrap_or(theme.colors.surface);
        let fg = self.style.foreground.unwrap_or(theme.colors.foreground);
        let radius = self.style.border_radius.unwrap_or(theme.input.radius);
        let padding = self.style.padding.unwrap_or_else(|| EdgeInsets::all(8.0));
        let border = self
            .style
            .border
            .unwrap_or_else(|| Border::new(theme.input.border_width, theme.colors.border));

        let raw = RawEditableText::new(self.controller.clone())
            .size(self.size)
            .multiline(true)
            .placeholder(self.placeholder.clone())
            .style(
                TextStyle::new()
                    .font_size(theme.typography.body.size)
                    .line_height_multiplier(1.4)
                    .color(fg),
            );

        Container::new()
            .padding(padding)
            .decoration(
                BoxDecoration::new()
                    .color(bg)
                    .border(border)
                    .border_radius(BorderRadius::circular(radius)),
            )
            .child(raw)
            .into()
    }
}

impl From<TextArea> for Widget {
    fn from(value: TextArea) -> Self {
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

    #[test]
    fn text_field_builder_has_explicit_empty_defaults() {
        let field = TextField::builder().build();
        assert_eq!(field.size, Size::ZERO);
        assert!(field.placeholder.is_empty());
        assert_eq!(field.style, TextFieldStyle::default());
        assert!(field.on_submit.is_none());

        let default = TextField::default();
        assert_eq!(default.size, Size::ZERO);
        assert!(default.placeholder.is_empty());
        assert_eq!(default.style, TextFieldStyle::default());
        assert!(default.on_submit.is_none());
    }

    #[test]
    fn text_field_builder_preserves_controller_and_legacy_setters() {
        let controller = TextEditingController::with_text("initial");
        let style = TextFieldStyle {
            padding: Some(EdgeInsets::all(6.0)),
            ..TextFieldStyle::default()
        };
        let field = TextField::builder()
            .controller(controller.clone())
            .size(Size::new(240.0, 40.0))
            .placeholder("Search")
            .style(style.clone())
            .on_submit(|_| {})
            .build();

        assert_eq!(field.controller, controller);
        assert_eq!(field.size, Size::new(240.0, 40.0));
        assert_eq!(field.placeholder, "Search");
        assert_eq!(field.style, style);
        assert!(field.on_submit.is_some());

        let legacy = TextField::new(controller)
            .size(Size::new(120.0, 32.0))
            .placeholder("Legacy")
            .style(TextFieldStyle::default())
            .on_submit(|_| {});
        let _: Widget = legacy.into();
    }

    #[test]
    fn text_area_builder_has_defaults_and_preserves_composition() {
        let area = TextArea::builder()
            .controller(TextEditingController::with_text("notes"))
            .placeholder("Notes")
            .size(Size::new(300.0, 160.0))
            .build();

        assert_eq!(area.placeholder, "Notes");
        assert_eq!(area.size, Size::new(300.0, 160.0));
        assert_eq!(area.style, TextFieldStyle::default());

        let _: Widget = TextArea::default().into();
        let _: Widget = TextArea::new(TextEditingController::new()).into();
    }
}
