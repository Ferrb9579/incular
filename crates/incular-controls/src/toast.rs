//! Retained toast queue and its visual parts.
//!
//! Toasts are intentionally controller-backed: adding or dismissing an entry
//! updates the small notification subtree instead of requiring application
//! state to rebuild the whole page. The provider is still an ordinary widget,
//! so an application can place it at the window root or inside a shell.

use crate::theme::ControlTheme;
use incular_semantics::{Role as SemanticRole, SemanticState};
use incular_widgets::{
    BorderRadius, BoxDecoration, Column, Container, ExplicitSemantics, Stack, Text, Widget,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ToastId(u64);

#[derive(Clone, Debug)]
struct ToastEntry {
    id: ToastId,
    title: String,
    description: Option<String>,
}

/// Window-local retained toast state.
#[derive(Clone, Default)]
pub struct ToastController {
    entries: Rc<RefCell<Vec<ToastEntry>>>,
    next_id: Rc<Cell<u64>>,
}

impl ToastController {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a toast and returns a stable handle for later dismissal.
    pub fn push(&self, title: impl Into<String>) -> ToastId {
        self.push_with_description(title, None::<String>)
    }

    /// Adds a toast with an optional descriptive line.
    pub fn push_with_description(
        &self,
        title: impl Into<String>,
        description: Option<impl Into<String>>,
    ) -> ToastId {
        let id = ToastId(self.next_id.get().wrapping_add(1));
        self.next_id.set(id.0);
        self.entries.borrow_mut().push(ToastEntry {
            id,
            title: title.into(),
            description: description.map(Into::into),
        });
        id
    }

    pub fn dismiss(&self, id: ToastId) -> bool {
        let mut entries = self.entries.borrow_mut();
        let before = entries.len();
        entries.retain(|entry| entry.id != id);
        before != entries.len()
    }

    pub fn clear(&self) {
        self.entries.borrow_mut().clear();
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.borrow().len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.borrow().is_empty()
    }
}

#[derive(Clone)]
pub struct Provider {
    controller: ToastController,
    child: Option<Widget>,
    max_visible: usize,
}

impl Default for Provider {
    fn default() -> Self {
        Self::new()
    }
}

impl Provider {
    #[must_use]
    pub fn new() -> Self {
        Self {
            controller: ToastController::new(),
            child: None,
            max_visible: 3,
        }
    }

    #[must_use]
    pub fn controller(&self) -> ToastController {
        self.controller.clone()
    }

    #[must_use]
    pub fn with_controller(mut self, controller: ToastController) -> Self {
        self.controller = controller;
        self
    }

    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = Some(child.into());
        self
    }

    #[must_use]
    pub fn max_visible(mut self, value: usize) -> Self {
        self.max_visible = value.max(1);
        self
    }

    fn build(&self, theme: &ControlTheme) -> Widget {
        let base = self
            .child
            .clone()
            .unwrap_or_else(|| incular_widgets::SizedBox::shrink().into());
        let entries = self.controller.entries.borrow();
        if entries.is_empty() {
            return base;
        }
        let start = entries.len().saturating_sub(self.max_visible);
        let notifications: Vec<Widget> = entries[start..]
            .iter()
            .map(|entry| Root::from_entry(entry, theme))
            .collect();
        Stack::new([base, Column::new(notifications).spacing(8.).into()]).into()
    }
}

impl From<Provider> for Widget {
    fn from(value: Provider) -> Self {
        let value = Rc::new(value);
        Widget::layout_builder(move |_| value.build(&crate::theme::current_control_theme()))
    }
}

#[derive(Clone)]
pub struct Root {
    title: String,
    description: Option<String>,
    child: Option<Widget>,
}

impl Root {
    #[must_use]
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            description: None,
            child: None,
        }
    }

    #[must_use]
    pub fn description(mut self, value: impl Into<String>) -> Self {
        self.description = Some(value.into());
        self
    }

    #[must_use]
    pub fn child(mut self, value: impl Into<Widget>) -> Self {
        self.child = Some(value.into());
        self
    }

    fn from_entry(entry: &ToastEntry, theme: &ControlTheme) -> Widget {
        let content: Widget = if let Some(description) = &entry.description {
            Column::new([
                Text::new(entry.title.clone()).style(theme.typography.body_emphasis.clone()),
                Text::new(description.clone()).style(theme.typography.small.clone()),
            ])
            .spacing(3.)
            .into()
        } else {
            Text::new(entry.title.clone())
                .style(theme.typography.body.clone())
                .into()
        };
        Container::with_child(content)
            .width(theme.toast.width)
            .padding(incular_config::EdgeInsets::all(theme.toast.padding))
            .decoration(
                BoxDecoration::new()
                    .color(theme.colors.surface_elevated)
                    .border_radius(BorderRadius::circular(theme.toast.radius)),
            )
            .into()
    }
}

impl Default for Root {
    fn default() -> Self {
        Self::new("")
    }
}

impl From<Root> for Widget {
    fn from(value: Root) -> Self {
        let value = Rc::new(value);
        Widget::layout_builder(move |_| {
            let theme = crate::theme::current_control_theme();
            let entry = ToastEntry {
                id: ToastId(0),
                title: value.title.clone(),
                description: value.description.clone(),
            };
            let visual = value
                .child
                .clone()
                .unwrap_or_else(|| Root::from_entry(&entry, &theme));
            visual.semantics(
                ExplicitSemantics::new(SemanticRole::GenericContainer)
                    .label(value.title.clone())
                    .state(SemanticState {
                        enabled: true,
                        ..SemanticState::default()
                    }),
            )
        })
    }
}

#[derive(Clone)]
pub struct Title(Widget);
impl Title {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(Text::new(value).into())
    }
}
impl From<Title> for Widget {
    fn from(value: Title) -> Self {
        value.0
    }
}

#[derive(Clone)]
pub struct Description(Widget);
impl Description {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(Text::new(value).into())
    }
}
impl From<Description> for Widget {
    fn from(value: Description) -> Self {
        value.0
    }
}

pub type Action = crate::button::Button;
