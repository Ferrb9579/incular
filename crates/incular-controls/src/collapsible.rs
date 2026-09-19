//! Collapsible and accordion state containers.

use incular_semantics::{Role as SemanticRole, SemanticActionKind, SemanticState};
use incular_widgets::Widget;
use incular_widgets::internal::{ActionSurface, ExplicitSemantics};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeSet,
    rc::Rc,
};
use typed_builder::TypedBuilder;

#[derive(Clone)]
struct AccordionScope {
    multiple: bool,
    collapsible: bool,
    active: Rc<RefCell<BTreeSet<usize>>>,
    revision: Rc<Cell<u64>>,
}

#[derive(Clone)]
struct CollapsibleScope {
    id: usize,
    local_open: Rc<Cell<bool>>,
    enabled: bool,
    revision: Rc<Cell<u64>>,
    on_change: Option<Rc<dyn Fn(bool) + 'static>>,
    accordion: Option<AccordionScope>,
}

impl CollapsibleScope {
    fn is_open(&self) -> bool {
        self.accordion.as_ref().map_or_else(
            || self.local_open.get(),
            |accordion| accordion.active.borrow().contains(&self.id),
        )
    }

    fn set_open(&self, requested: bool) {
        if !self.enabled {
            return;
        }
        let previous = self.is_open();
        if let Some(accordion) = self.accordion.as_ref() {
            let mut active = accordion.active.borrow_mut();
            if requested {
                if !accordion.multiple {
                    active.clear();
                }
                active.insert(self.id);
            } else if accordion.collapsible
                || accordion.multiple && active.len() > 1
                || !active.contains(&self.id)
            {
                active.remove(&self.id);
            }
            drop(active);
        } else {
            self.local_open.set(requested);
        }

        let next = self.is_open();
        self.local_open.set(next);
        if next == previous {
            return;
        }
        self.revision.set(self.revision.get().wrapping_add(1));
        if let Some(accordion) = self.accordion.as_ref() {
            accordion
                .revision
                .set(accordion.revision.get().wrapping_add(1));
        }
        if let Some(callback) = self.on_change.as_ref() {
            callback(next);
        }
    }

    fn toggle(&self) {
        self.set_open(!self.is_open());
    }
}

#[derive(Clone, TypedBuilder)]
pub struct Root {
    #[builder(default = false)]
    open: bool,
    #[builder(default = true)]
    enabled: bool,
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
    on_change: Option<Rc<dyn Fn(bool) + 'static>>,
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
    pub fn disabled(mut self, value: bool) -> Self {
        self.enabled = !value;
        self
    }
    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = Some(child.into());
        self
    }
    #[must_use]
    pub fn on_open_change(mut self, callback: impl Fn(bool) + 'static) -> Self {
        self.on_change = Some(Rc::new(callback));
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
        let local_open = Rc::new(Cell::new(value.open));
        let revision = Rc::new(Cell::new(0_u64));
        let initialized = Rc::new(Cell::new(false));
        let id = Rc::as_ptr(&local_open) as usize;
        let enabled = value.enabled;
        let on_change = value.on_change;
        Widget::stateful_layout_builder(revision.clone(), move |context, _| {
            let accordion = context.depend_on::<AccordionScope>();
            if !initialized.get() {
                if local_open.get()
                    && let Some(accordion) = accordion.as_ref()
                {
                    let mut active = accordion.active.borrow_mut();
                    if accordion.multiple || active.is_empty() {
                        active.insert(id);
                    }
                }
                initialized.set(true);
            }
            let scope = CollapsibleScope {
                id,
                local_open: local_open.clone(),
                enabled,
                revision: revision.clone(),
                on_change: on_change.clone(),
                accordion,
            };
            let open = scope.is_open();
            Widget::environment_scope(scope, child.clone()).semantics(
                ExplicitSemantics::new(SemanticRole::GenericContainer).state(SemanticState {
                    enabled,
                    expanded: Some(open),
                    ..SemanticState::default()
                }),
            )
        })
    }
}

#[derive(Clone, TypedBuilder)]
pub struct Trigger {
    #[builder(setter(into))]
    child: Widget,
}

impl Trigger {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self::builder().child(child).build()
    }
}

impl From<Trigger> for Widget {
    fn from(value: Trigger) -> Self {
        let child = value.child;
        let label = child.semantic_text().unwrap_or_default();
        let dynamic_label = label.clone();
        Widget::from(incular_widgets::LayoutBuilder::new(move |context, _| {
            let Some(scope) = context.depend_on::<CollapsibleScope>() else {
                return child.clone();
            };
            let callback_scope = scope.clone();
            let mut action = ActionSurface::with_child(child.clone()).enabled(scope.enabled);
            if scope.enabled {
                action = action.on_click(move || callback_scope.toggle());
            }
            Widget::from(action).semantics(
                ExplicitSemantics::new(SemanticRole::Button)
                    .label(dynamic_label.clone())
                    .state(SemanticState {
                        enabled: scope.enabled,
                        focusable: scope.enabled,
                        expanded: Some(scope.is_open()),
                        ..SemanticState::default()
                    })
                    .actions(if scope.enabled {
                        vec![SemanticActionKind::Focus, SemanticActionKind::Activate]
                    } else {
                        Vec::new()
                    }),
            )
        }))
        .accessibility_label(label)
    }
}

#[derive(Clone, TypedBuilder)]
pub struct Panel {
    #[builder(setter(into))]
    child: Widget,
}

impl Panel {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self::builder().child(child).build()
    }
}

impl From<Panel> for Widget {
    fn from(value: Panel) -> Self {
        let child = value.child;
        let label = child.semantic_text().unwrap_or_default();
        Widget::from(incular_widgets::LayoutBuilder::new(move |context, _| {
            context.depend_on::<CollapsibleScope>().map_or_else(
                || child.clone(),
                |scope| {
                    if scope.is_open() {
                        child.clone()
                    } else {
                        incular_widgets::SizedBox::shrink().into()
                    }
                },
            )
        }))
        .accessibility_label(label)
    }
}

#[derive(Clone, TypedBuilder)]
pub struct Accordion {
    #[builder(setter(into))]
    child: Widget,
    #[builder(default = false)]
    multiple: bool,
    #[builder(default = true)]
    collapsible: bool,
}

impl Accordion {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self::builder().child(child).build()
    }
    #[must_use]
    pub fn multiple(mut self, value: bool) -> Self {
        self.multiple = value;
        self
    }
    #[must_use]
    pub fn collapsible(mut self, value: bool) -> Self {
        self.collapsible = value;
        self
    }
}

impl From<Accordion> for Widget {
    fn from(value: Accordion) -> Self {
        let revision = Rc::new(Cell::new(0_u64));
        let scope = AccordionScope {
            multiple: value.multiple,
            collapsible: value.collapsible,
            active: Rc::new(RefCell::new(BTreeSet::new())),
            revision: revision.clone(),
        };
        let child = value.child;
        Widget::stateful_layout_builder(revision, move |_, _| {
            Widget::environment_scope(scope.clone(), child.clone())
        })
    }
}
