//! Free-form completion surface sharing Select/Combobox list parts.

pub use crate::popup::{Arrow, Popup, Portal, Positioner, Trigger};
pub use crate::select::{Item, List};
use crate::{Button, ControlTheme};
use incular_semantics::{Role as SemanticRole, SemanticActionKind, SemanticState};
use incular_widgets::{Widget, internal::ExplicitSemantics};
use std::rc::Rc;
use typed_builder::TypedBuilder;

#[derive(Clone, TypedBuilder)]
pub struct Root {
    #[builder(default = String::new(), setter(into))]
    query: String,
    #[builder(default, setter(strip_option, into))]
    child: Option<Widget>,
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
    on_query_change: Option<Rc<dyn Fn(String) + 'static>>,
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
    pub fn query(mut self, value: impl Into<String>) -> Self {
        self.query = value.into();
        self
    }

    #[must_use]
    pub fn child(mut self, value: impl Into<Widget>) -> Self {
        self.child = Some(value.into());
        self
    }

    #[must_use]
    pub fn on_query_change(mut self, callback: impl Fn(String) + 'static) -> Self {
        self.on_query_change = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn build(&self, _theme: &ControlTheme) -> Widget {
        let content = self
            .child
            .clone()
            .unwrap_or_else(|| Button::new(self.query.clone()).into());
        content.semantics(
            ExplicitSemantics::new(SemanticRole::TextField)
                .value(self.query.clone())
                .state(SemanticState {
                    enabled: true,
                    focusable: true,
                    editable: true,
                    ..SemanticState::default()
                })
                .actions([SemanticActionKind::Focus, SemanticActionKind::SetText]),
        )
    }
}

impl From<Root> for Widget {
    fn from(value: Root) -> Self {
        let value = Rc::new(value);
        Widget::layout_builder(move |_| value.build(&crate::theme::current_control_theme()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use incular_widgets::Text;

    #[test]
    fn builder_preserves_autocomplete_defaults_and_accepts_widgets() {
        let default = Root::default();
        let built = Root::builder().build();

        assert!(default.query.is_empty());
        assert!(built.query.is_empty());
        assert!(built.child.is_none());
        assert!(built.on_query_change.is_none());

        let root = Root::builder()
            .query("ap")
            .child(Text::new("Search"))
            .on_query_change(|_| {})
            .build();
        assert_eq!(root.query, "ap");
        assert!(root.child.is_some());
        assert!(root.on_query_change.is_some());
        let _: Widget = root.into();
    }
}
