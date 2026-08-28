//! Alert-dialog compound surface.
//!
//! Alert dialogs intentionally share the popup composition model but use a
//! dialog semantic role and do not expose an implicit outside-dismiss action.

pub use crate::popup::{Arrow, Close, Description, Popup, Positioner, Title, Trigger};
use incular_semantics::{Role as SemanticRole, SemanticState};
use incular_widgets::{Widget, internal::ExplicitSemantics};
use std::rc::Rc;
use typed_builder::TypedBuilder;

pub use crate::popup::Portal;

#[derive(Clone, TypedBuilder)]
pub struct Root {
    #[builder(default = false)]
    open: bool,
    #[builder(default, setter(strip_option, into))]
    child: Option<Widget>,
    #[builder(
        default,
        setter(
            fn transform<F>(callback: F) -> Option<Rc<dyn Fn(bool) + 'static>>
            where
                F: Fn(bool) + 'static,
            {
                Some(Rc::new(callback))
            }
        )
    )]
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
    pub fn child(mut self, value: impl Into<Widget>) -> Self {
        self.child = Some(value.into());
        self
    }

    #[must_use]
    pub fn on_open_change(mut self, callback: impl Fn(bool) + 'static) -> Self {
        self.on_open_change = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn is_open(&self) -> bool {
        self.open
    }
}

impl From<Root> for Widget {
    fn from(value: Root) -> Self {
        let child = value
            .child
            .unwrap_or_else(|| incular_widgets::SizedBox::shrink().into());
        let dialog = child.semantics(
            ExplicitSemantics::new(SemanticRole::Dialog)
                .state(SemanticState {
                    enabled: true,
                    expanded: Some(value.open),
                    ..SemanticState::default()
                })
                .actions([incular_semantics::SemanticActionKind::Focus]),
        );
        Widget::visibility(value.open, dialog)
    }
}

pub type Viewport = Popup;
pub type Backdrop = Portal;

#[cfg(test)]
mod tests {
    use super::*;
    use incular_widgets::Text;

    #[test]
    fn builder_preserves_alert_dialog_defaults_and_accepts_widgets() {
        let default = Root::default();
        let built = Root::builder().build();

        assert_eq!(default.is_open(), built.is_open());
        assert!(default.child.is_none());
        assert!(built.child.is_none());
        assert!(built.on_open_change.is_none());

        let root = Root::builder()
            .child(Text::new("Alert"))
            .open(true)
            .on_open_change(|_| {})
            .build();
        assert!(root.is_open());
        assert!(root.child.is_some());
        assert!(root.on_open_change.is_some());
        let _: Widget = root.into();
    }
}
