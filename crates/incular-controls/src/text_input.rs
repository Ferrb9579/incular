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
        Widget::from(incular_widgets::LayoutBuilder::new(move |context, _| {
            let theme = crate::theme::current_control_theme(context);
            value.build(&theme)
        }))
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
        Widget::from(incular_widgets::LayoutBuilder::new(move |context, _| {
            let theme = crate::theme::current_control_theme(context);
            value.build(&theme)
        }))
    }
}
