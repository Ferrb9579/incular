use crate::styles::TextFieldStyle;
use crate::theme::ControlTheme;
use incular_config::EdgeInsets;
use incular_core::Size;
use incular_widgets::internal::TextEditingController;
use incular_widgets::{
    Border, BorderRadius, BoxDecoration, BuildContext, Container, EditableText as RawEditableText,
    Widget,
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
    #[builder(default = true)]
    enabled: bool,
    #[builder(default)]
    read_only: bool,
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
    on_changed: Option<Rc<dyn Fn(String) + 'static>>,
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
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    #[must_use]
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    #[must_use]
    pub fn on_submit(mut self, on_submit: impl Fn(String) + 'static) -> Self {
        self.on_submit = Some(Rc::new(on_submit));
        self
    }

    #[must_use]
    pub fn on_changed(mut self, on_changed: impl Fn(String) + 'static) -> Self {
        self.on_changed = Some(Rc::new(on_changed));
        self
    }

    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        self.build_with_form(theme, None)
    }

    fn build_with_context(&self, context: &BuildContext<'_>, theme: &ControlTheme) -> Widget {
        self.build_with_form(theme, context.depend_on::<crate::form::FormScope>())
    }

    fn build_with_form(
        &self,
        theme: &ControlTheme,
        form: Option<crate::form::FormScope>,
    ) -> Widget {
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
        let placeholder_color = self
            .style
            .placeholder_color
            .unwrap_or(theme.colors.foreground_muted);

        let effective_enabled =
            self.enabled && !form.as_ref().is_some_and(|scope| scope.disabled());
        let mut raw = RawEditableText::new(self.controller.clone())
            .size(self.size)
            .placeholder(self.placeholder.clone())
            .placeholder_color(placeholder_color)
            .style(theme.typography.body.clone().color(fg))
            .enabled(effective_enabled)
            .read_only(self.read_only);
        if let Some(border) = self.style.border_focused {
            raw = raw.focused_border(border, radius);
        }

        if self.on_submit.is_some() || form.is_some() {
            let callback = self.on_submit.clone();
            raw = raw.on_submit(move |value| {
                if let Some(callback) = callback.as_ref() {
                    callback(value);
                }
                if let Some(form) = form.as_ref() {
                    let _ = form.submit();
                }
            });
        }

        let mut editor: Widget = raw.into();
        if let Some(callback) = self.on_changed.clone() {
            editor = editor.with_edit_callbacks(
                None,
                Some(Rc::new(move |text: &str| callback(text.to_owned()))),
            );
        }

        Container::new()
            .padding(padding)
            .decoration(
                BoxDecoration::new()
                    .color(bg)
                    .border(border)
                    .border_radius(BorderRadius::circular(radius)),
            )
            .child(editor)
            .into()
    }
}

impl From<TextField> for Widget {
    fn from(value: TextField) -> Self {
        let value = Rc::new(value);
        Widget::from(incular_widgets::LayoutBuilder::new(move |context, _| {
            let theme = crate::theme::current_control_theme(context);
            value.build_with_context(context, &theme)
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
    #[builder(default = true)]
    enabled: bool,
    #[builder(default)]
    read_only: bool,
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
    on_changed: Option<Rc<dyn Fn(String) + 'static>>,
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
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    #[must_use]
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    #[must_use]
    pub fn on_changed(mut self, on_changed: impl Fn(String) + 'static) -> Self {
        self.on_changed = Some(Rc::new(on_changed));
        self
    }

    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        self.build_with_form(theme, None)
    }

    fn build_with_context(&self, context: &BuildContext<'_>, theme: &ControlTheme) -> Widget {
        self.build_with_form(theme, context.depend_on::<crate::form::FormScope>())
    }

    fn build_with_form(
        &self,
        theme: &ControlTheme,
        form: Option<crate::form::FormScope>,
    ) -> Widget {
        let bg = self.style.background.unwrap_or(theme.colors.surface);
        let fg = self.style.foreground.unwrap_or(theme.colors.foreground);
        let radius = self.style.border_radius.unwrap_or(theme.input.radius);
        let padding = self.style.padding.unwrap_or_else(|| EdgeInsets::all(8.0));
        let border = self
            .style
            .border
            .unwrap_or_else(|| Border::new(theme.input.border_width, theme.colors.border));
        let placeholder_color = self
            .style
            .placeholder_color
            .unwrap_or(theme.colors.foreground_muted);

        let effective_enabled =
            self.enabled && !form.as_ref().is_some_and(|scope| scope.disabled());
        let mut raw = RawEditableText::new(self.controller.clone())
            .size(self.size)
            .multiline(true)
            .placeholder(self.placeholder.clone())
            .placeholder_color(placeholder_color)
            .style(theme.typography.body.clone().color(fg))
            .enabled(effective_enabled)
            .read_only(self.read_only);
        if let Some(border) = self.style.border_focused {
            raw = raw.focused_border(border, radius);
        }

        let mut editor: Widget = raw.into();
        if let Some(callback) = self.on_changed.clone() {
            editor = editor.with_edit_callbacks(
                None,
                Some(Rc::new(move |text: &str| callback(text.to_owned()))),
            );
        }

        Container::new()
            .padding(padding)
            .decoration(
                BoxDecoration::new()
                    .color(bg)
                    .border(border)
                    .border_radius(BorderRadius::circular(radius)),
            )
            .child(editor)
            .into()
    }
}

impl From<TextArea> for Widget {
    fn from(value: TextArea) -> Self {
        let value = Rc::new(value);
        Widget::from(incular_widgets::LayoutBuilder::new(move |context, _| {
            let theme = crate::theme::current_control_theme(context);
            value.build_with_context(context, &theme)
        }))
    }
}
