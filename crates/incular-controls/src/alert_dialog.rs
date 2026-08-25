//! Alert-dialog compound surface.
//!
//! Alert dialogs intentionally share the popup composition model but use a
//! dialog semantic role and do not expose an implicit outside-dismiss action.

pub use crate::popup::{Arrow, Close, Description, Popup, Positioner, Title, Trigger};
use incular_semantics::{Role as SemanticRole, SemanticState};
use incular_widgets::{ExplicitSemantics, Widget};
use std::rc::Rc;

pub use crate::popup::Portal;

#[derive(Clone)]
pub struct Root {
    open: bool,
    child: Option<Widget>,
    on_open_change: Option<Rc<dyn Fn(bool) + 'static>>,
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
            open: false,
            child: None,
            on_open_change: None,
        }
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
