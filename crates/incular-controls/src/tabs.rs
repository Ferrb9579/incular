//! Tab anatomy and shared composite-navigation configuration.

use crate::{CompositeController, CompositeOrientation};
use incular_semantics::{Role as SemanticRole, SemanticState};
use incular_widgets::internal::{ActionSurface, ExplicitSemantics};
use incular_widgets::{BuildContext, FocusableActionDetector, Widget};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use typed_builder::TypedBuilder;

#[derive(Clone)]
struct TabsScope {
    value: Rc<RefCell<Option<String>>>,
    automatic: bool,
    on_change: Option<Rc<dyn Fn(String) + 'static>>,
    revision: Rc<Cell<u64>>,
}

impl TabsScope {
    fn select(&self, value: String) {
        if self.value.borrow().as_ref() == Some(&value) {
            return;
        }
        self.value.replace(Some(value.clone()));
        self.revision.set(self.revision.get().wrapping_add(1));
        if let Some(callback) = self.on_change.as_ref() {
            callback(value);
        }
    }
}

#[derive(Clone, TypedBuilder)]
pub struct Root {
    #[builder(default, setter(strip_option, into))]
    value: Option<String>,
    #[builder(default)]
    orientation: CompositeOrientation,
    #[builder(default = true)]
    automatic: bool,
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
    pub fn vertical(mut self) -> Self {
        self.orientation = CompositeOrientation::Vertical;
        self
    }
    #[must_use]
    pub fn manual_activation(mut self, value: bool) -> Self {
        self.automatic = !value;
        self
    }
    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = Some(child.into());
        self
    }
    #[must_use]
    pub fn on_value_change(mut self, callback: impl Fn(String) + 'static) -> Self {
        self.on_change = Some(Rc::new(callback));
        self
    }
    #[must_use]
    pub fn navigation(&self) -> CompositeController {
        CompositeController::new().orientation_set(self.orientation)
    }
}
impl From<Root> for Widget {
    fn from(value: Root) -> Self {
        let revision = Rc::new(Cell::new(0_u64));
        let scope = TabsScope {
            value: Rc::new(RefCell::new(value.value)),
            automatic: value.automatic,
            on_change: value.on_change,
            revision: revision.clone(),
        };
        let child = value
            .child
            .unwrap_or_else(|| incular_widgets::SizedBox::shrink().into());
        let navigation = CompositeController::new().orientation_set(value.orientation);
        Widget::stateful_layout_builder(revision, move |_, _| {
            Widget::environment_scope(
                navigation.clone(),
                Widget::environment_scope(scope.clone(), child.clone()),
            )
        })
    }
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
pub struct Tab {
    #[builder(setter(into))]
    value: String,
    #[builder(setter(into))]
    child: Widget,
    #[builder(default)]
    disabled: bool,
}
impl Tab {
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
impl From<Tab> for Widget {
    fn from(value: Tab) -> Self {
        let value = Rc::new(value);
        Widget::from(incular_widgets::LayoutBuilder::new(move |context, _| {
            tab_with_context(context, &value)
        }))
    }
}

fn tab_with_context(context: &BuildContext<'_>, tab: &Tab) -> Widget {
    let scope = context.depend_on::<TabsScope>();
    let selected = scope
        .as_ref()
        .is_some_and(|scope| scope.value.borrow().as_ref() == Some(&tab.value));
    let mut action = ActionSurface::with_child(tab.child.clone())
        .label(tab.value.clone())
        .enabled(!tab.disabled);
    if !tab.disabled
        && let Some(scope) = scope.clone()
    {
        let selected_value = tab.value.clone();
        action = action.on_press(move || scope.select(selected_value.clone()));
    }
    let semantic = ExplicitSemantics::new(SemanticRole::Tab).state(SemanticState {
        enabled: !tab.disabled,
        selected,
        ..SemanticState::default()
    });
    let child = Widget::from(action);
    let Some(scope) = scope else {
        return child.semantics(semantic);
    };
    let selected_value = tab.value.clone();
    let focused: Widget = FocusableActionDetector::new(child)
        .enabled(!tab.disabled)
        .on_focus_change(move |focused| {
            if focused && scope.automatic {
                scope.select(selected_value.clone());
            }
        })
        .into();
    focused.semantics(semantic)
}
#[derive(Clone, TypedBuilder)]
pub struct Panel {
    #[builder(setter(into))]
    child: Widget,
}
impl Panel {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
}
impl From<Panel> for Widget {
    fn from(value: Panel) -> Self {
        value.child
    }
}
#[derive(Clone, TypedBuilder)]
pub struct Indicator {
    #[builder(default, setter(strip_option, into))]
    child: Option<Widget>,
}
impl Indicator {
    #[must_use]
    pub fn new() -> Self {
        Self::builder().build()
    }
    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = Some(child.into());
        self
    }
}
impl Default for Indicator {
    fn default() -> Self {
        Self::builder().build()
    }
}
impl From<Indicator> for Widget {
    fn from(value: Indicator) -> Self {
        value
            .child
            .unwrap_or_else(|| incular_widgets::SizedBox::shrink().into())
    }
}
