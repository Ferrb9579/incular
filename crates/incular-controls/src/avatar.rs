//! Avatar image/fallback compound control.

use crate::theme::ControlTheme;
use incular_config::Alignment;
use incular_semantics::{Role as SemanticRole, SemanticState};
use incular_widgets::internal::ExplicitSemantics;
use incular_widgets::{Border, BorderRadius, BoxDecoration, ClipOval, Container, Text, Widget};
use std::rc::Rc;
use typed_builder::TypedBuilder;

#[derive(Clone, Default, TypedBuilder)]
pub struct Root {
    #[builder(default, setter(strip_option, into))]
    image: Option<Widget>,
    #[builder(default, setter(strip_option, into))]
    fallback: Option<Widget>,
    #[builder(default, setter(strip_option, into))]
    label: Option<String>,
    #[builder(default, setter(transform = |size: f32| Some(size.max(1.))))]
    size: Option<f32>,
    #[builder(default, setter(strip_option, into))]
    child: Option<Widget>,
}

impl Root {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_child(child: impl Into<Widget>) -> Self {
        Self::default().child(child)
    }

    /// Supplies the image part. The caller can pass Incular's Image widget,
    /// an image placeholder, or any custom visual.
    #[must_use]
    pub fn image(mut self, image: impl Into<Widget>) -> Self {
        self.image = Some(image.into());
        self
    }

    /// Supplies the visual shown when no image is available.
    #[must_use]
    pub fn fallback(mut self, fallback: impl Into<Widget>) -> Self {
        self.fallback = Some(fallback.into());
        self
    }

    /// Sets the accessible name and also provides a useful text fallback when
    /// no explicit fallback part was supplied.
    #[must_use]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    #[must_use]
    pub fn size(mut self, size: f32) -> Self {
        self.size = Some(size.max(1.));
        self
    }

    /// Replaces the complete avatar surface while retaining its semantics.
    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = Some(child.into());
        self
    }

    #[must_use]
    pub fn build(&self, theme: &ControlTheme) -> Widget {
        let size = self.size.unwrap_or(theme.avatar.size).max(1.);
        let visual = self
            .child
            .clone()
            .or_else(|| self.image.clone())
            .or_else(|| self.fallback.clone())
            .or_else(|| {
                self.label.as_ref().map(|label| {
                    Text::new(initials(label))
                        .style(theme.typography.body_emphasis.clone())
                        .into()
                })
            })
            .unwrap_or_else(|| incular_widgets::SizedBox::shrink().into());

        let surface: Widget = Container::new()
            .width(size)
            .height(size)
            .alignment(Alignment::CENTER)
            .decoration(
                BoxDecoration::new()
                    .color(theme.colors.surface_variant)
                    .border(Border::new(theme.avatar.border_width, theme.colors.border))
                    .border_radius(BorderRadius::circular(theme.avatar.radius)),
            )
            .child(visual)
            .into();
        let visual: Widget = ClipOval::new(surface).into();
        visual.semantics(
            ExplicitSemantics::new(SemanticRole::Image)
                .label(self.label.clone().unwrap_or_else(|| "Avatar".to_owned()))
                .state(SemanticState {
                    enabled: true,
                    ..SemanticState::default()
                }),
        )
    }
}

impl From<Root> for Widget {
    fn from(value: Root) -> Self {
        let value = Rc::new(value);
        Widget::from(incular_widgets::LayoutBuilder::new(move |context, _| {
            let theme = crate::theme::current_control_theme(context);
            value.build(&theme)
        }))
    }
}

fn initials(label: &str) -> String {
    let mut words = label
        .split_whitespace()
        .filter_map(|word| word.chars().next());
    let first = words.next();
    let second = words.next();
    match (first, second) {
        (Some(first), Some(second)) => format!("{first}{second}").to_uppercase(),
        (Some(first), None) => first.to_uppercase().collect(),
        _ => "?".to_owned(),
    }
}
