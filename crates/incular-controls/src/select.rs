//! Select/listbox parts.
pub use crate::popup::{Arrow, Popup, Portal, Positioner, Trigger};
use incular_semantics::{Role as SemanticRole, SemanticState};
use incular_widgets::internal::{ActionSurface, ExplicitSemantics};
use incular_widgets::{BuildContext, Widget};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use typed_builder::TypedBuilder;

#[derive(Clone)]
struct SelectScope {
    value: Rc<RefCell<Option<String>>>,
    on_change: Option<Rc<dyn Fn(String) + 'static>>,
    revision: Rc<Cell<u64>>,
}

impl SelectScope {
    fn select(&self, value: String) {
        if self.value.borrow().as_ref() == Some(&value) {
            return;
        }
        self.value.replace(Some(value.clone()));
        self.revision.set(
            self.revision
                .get()
                .checked_add(1)
                .expect("revision exhausted"),
        );
        if let Some(callback) = self.on_change.as_ref() {
            callback(value);
        }
    }
}

#[derive(Clone, TypedBuilder)]
pub struct Root {
    #[builder(default, setter(strip_option, into))]
    value: Option<String>,
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
    on_change: Option<Rc<dyn Fn(String) + 'static>>,
}
impl Default for Root {
    fn default() -> Self {
        Self::builder().build()
    }
}
impl Root {
    #[must_use]
    pub fn new() -> Self {
        Self::builder().build()
    }
    #[must_use]
    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }
    #[must_use]
    pub fn default_value(self, value: impl Into<String>) -> Self {
        self.value(value)
    }
    #[must_use]
    pub fn child(mut self, value: impl Into<Widget>) -> Self {
        self.child = Some(value.into());
        self
    }
    #[must_use]
    pub fn on_value_change(mut self, cb: impl Fn(String) + 'static) -> Self {
        self.on_change = Some(Rc::new(cb));
        self
    }
}
impl From<Root> for Widget {
    fn from(value: Root) -> Self {
        let selected = Rc::new(RefCell::new(value.value));
        let revision = Rc::new(Cell::new(0_u64));
        let scope = SelectScope {
            value: selected.clone(),
            on_change: value.on_change,
            revision: revision.clone(),
        };
        let child = value.child;
        Widget::stateful_layout_builder(revision, move |_, _| {
            let content = child.clone().unwrap_or_else(|| {
                let label = selected
                    .borrow()
                    .clone()
                    .unwrap_or_else(|| "Select".to_owned());
                incular_controls_button(label)
            });
            Widget::environment_scope(scope.clone(), content)
        })
    }
}

fn incular_controls_button(label: String) -> Widget {
    crate::Button::new(label).into()
}
#[derive(Clone, TypedBuilder)]
pub struct List {
    #[builder(setter(into))]
    child: Widget,
}
impl List {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}
impl From<List> for Widget {
    fn from(value: List) -> Self {
        value.child
    }
}
#[derive(Clone, TypedBuilder)]
pub struct Item {
    #[builder(setter(into))]
    value: String,
    #[builder(setter(into))]
    child: Widget,
    #[builder(default)]
    disabled: bool,
}
impl Item {
    #[must_use]
    pub fn new(value: impl Into<String>, child: impl Into<Widget>) -> Self {
        Self {
            value: value.into(),
            child: child.into(),
            disabled: false,
        }
    }
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }
    #[must_use]
    pub fn is_disabled(&self) -> bool {
        self.disabled
    }
    #[must_use]
    pub fn disabled(mut self, value: bool) -> Self {
        self.disabled = value;
        self
    }
}
impl From<Item> for Widget {
    fn from(value: Item) -> Self {
        let value = Rc::new(value);
        Widget::from(incular_widgets::LayoutBuilder::new(move |context, _| {
            item_with_context(context, &value)
        }))
    }
}

fn item_with_context(context: &BuildContext<'_>, item: &Item) -> Widget {
    let scope = context.depend_on::<SelectScope>();
    let selected = scope
        .as_ref()
        .is_some_and(|scope| scope.value.borrow().as_ref() == Some(&item.value));
    let mut action = ActionSurface::with_child(item.child.clone())
        .label(item.value.clone())
        .enabled(!item.disabled);
    if !item.disabled
        && let Some(scope) = scope
    {
        let selected_value = item.value.clone();
        action = action.on_press(move || scope.select(selected_value.clone()));
    }
    Widget::from(action).semantics(ExplicitSemantics::new(SemanticRole::ListItem).state(
        SemanticState {
            enabled: !item.disabled,
            selected,
            ..SemanticState::default()
        },
    ))
}
