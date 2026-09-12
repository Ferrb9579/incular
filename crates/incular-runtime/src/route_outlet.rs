//! Hosts one navigator's routes as mounted, identity-tagged content.
//!
//! [`RouteOutlet`] is the production seam between a navigation stack and a
//! window tree: it builds every mounted route's content into an
//! [`IndexedStack`](incular_widgets::IndexedStack) keyed by stable
//! per-route tags, drives focus save/restore/forget and route task
//! bindings from live navigator state, and derives element ownership from
//! the mounted tags — applications never maintain element-id oracles.
//!
//! Drive contract: include [`RouteOutlet::widget`] in the window tree,
//! rebuild it after navigation (observer or revision), and call
//! [`RouteOutlet::after_frame`] after every frame presenting outlet
//! content. Inactive routes stay mounted but out of focus, semantics,
//! paint, and hit testing through the stack index; only permanent removal
//! unmounts.

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
};

use incular_navigation::{Navigator, RouteId};
use incular_widgets::{
    IndexedStack, SizedBox, Widget,
    internal::{ElementId, Key, WidgetTree},
};

use super::{frame::Runtime, route_focus::RouteFocusState, tasks::TaskScope};
use crate::RouteTaskBinding;

/// Sequence numbering outlet key namespaces so sibling outlets sharing one
/// tree never tag two routes alike.
static OUTLET_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// Mounts one [`Navigator`] as an indexed stack of route content.
///
/// The outlet owns the integration state the navigator cannot: per-route
/// mount tags feeding the focus ownership oracle, a [`RouteFocusState`]
/// driven across transitions, and one [`RouteTaskBinding`] per mounted
/// route. Use one outlet per navigator; nested navigators get nested
/// outlets and stay isolated through disjoint tags and states.
///
/// Dropping the outlet releases the integration: focus records, bindings,
/// and tags go with it (bound scopes then follow ordinary [`TaskScope`]
/// ownership, detached from later removals).
pub struct RouteOutlet {
    navigator: Navigator,
    focus: RouteFocusState,
    bindings: HashMap<RouteId, RouteTaskBinding>,
    tags: Rc<RefCell<HashMap<RouteId, Key>>>,
    task_parent: TaskScope,
    last_active: Option<RouteId>,
    namespace: u64,
    /// Monotonic tag counter. Freed tags are never reassigned (unlike the
    /// map length, which shrinks on removal), so element identity can never
    /// alias across removals. Interior so [`Self::widget`] shares by ref.
    tag_sequence: Cell<u64>,
}

impl RouteOutlet {
    /// Hosts `navigator`, parenting route task scopes under `task_parent`
    /// (typically the window scope, so close still cancels through existing
    /// ownership).
    pub fn new(navigator: &Navigator, task_parent: &TaskScope) -> Self {
        let tags: Rc<RefCell<HashMap<RouteId, Key>>> = Rc::default();
        let tags_for_oracle = tags.clone();
        let focus = RouteFocusState::new(move |tree: &WidgetTree, id: ElementId| {
            // Ancestor chain of the element, self included: the outlet tags
            // each route's content root, so the nearest tagged ancestor —
            // here, any tagged ancestor, since tags are unique per route —
            // owns the element.
            let mut chain = Vec::new();
            let mut cursor = Some(id);
            while let Some(current) = cursor {
                chain.push(current);
                cursor = tree.parent(current);
            }
            tags_for_oracle.borrow().iter().find_map(|(route, key)| {
                tree.element_with_key(key)
                    .is_some_and(|root| chain.contains(&root))
                    .then_some(*route)
            })
        });
        Self {
            navigator: navigator.clone(),
            focus,
            bindings: HashMap::new(),
            tags,
            task_parent: task_parent.clone(),
            last_active: None,
            namespace: OUTLET_SEQUENCE.fetch_add(1, Ordering::Relaxed),
            tag_sequence: Cell::new(0),
        }
    }

    /// Returns the navigator this outlet hosts.
    #[must_use]
    pub fn navigator(&self) -> &Navigator {
        &self.navigator
    }

    /// Builds the outlet widget: every mounted route's presented content in
    /// stack order, each root tagged for ownership, with the active route
    /// on top of the index. Inactive routes keep state and identity while
    /// leaving focus, semantics, paint, and hit testing.
    #[must_use]
    pub fn widget(&self) -> Widget {
        let routes = self.navigator.routes();
        if routes.is_empty() {
            return SizedBox::shrink().into();
        }
        IndexedStack::new(
            routes
                .iter()
                .map(|route| route.presented_child().with_key(self.tag_for(route.id))),
        )
        .index(routes.len() - 1)
        .into()
    }

    /// Returns the route task scope for spawning route-bound work, if the
    /// route is currently mounted and bound.
    #[must_use]
    pub fn route_task_scope(&self, route: RouteId) -> Option<TaskScope> {
        self.bindings
            .get(&route)
            .map(|binding| binding.scope().clone())
    }

    /// Reconciles integration state with live navigator state after a
    /// frame: forgets records, bindings, and tags for unmounted routes,
    /// saves the deactivated route's owned focus, binds newly mounted
    /// routes, and restores the activated route's eligible focus (whose
    /// targets mounted in the frame just presented).
    pub fn after_frame(&mut self, runtime: &mut Runtime) {
        let active = self.navigator.current().map(|route| route.id);
        // Removals first: lifetime-end notifications already fired at commit
        // time, so bound scopes were cancelled before their bindings drop
        // here, and no removed route saves or restores afterwards.
        self.focus.retain_mounted(&self.navigator);
        self.bindings
            .retain(|id, _| self.navigator.lifetime_of(*id).is_some());
        self.tags
            .borrow_mut()
            .retain(|id, _| self.navigator.lifetime_of(*id).is_some());
        if self.last_active != active
            && let Some(previous) = self.last_active
            && self.navigator.lifetime_of(previous).is_some()
        {
            self.focus.save_focused(runtime, previous);
        }
        for route in self.navigator.routes() {
            if !self.bindings.contains_key(&route.id)
                && let Some(lifetime) = self.navigator.lifetime_of(route.id)
            {
                self.bindings.insert(
                    route.id,
                    RouteTaskBinding::bind(&self.task_parent, &lifetime),
                );
            }
        }
        if let Some(id) = active
            && self.last_active != Some(id)
        {
            self.focus.restore_saved(runtime, &self.navigator, id);
        }
        self.last_active = active;
    }

    /// Restores the active route's saved focus now, for content that
    /// mounted after the transition frame. Ordinary flows go through
    /// [`Self::after_frame`].
    pub fn restore_active(&mut self, runtime: &mut Runtime) {
        if let Some(id) = self.navigator.current().map(|route| route.id) {
            self.focus.restore_saved(runtime, &self.navigator, id);
        }
    }

    /// Stable mount tag for a route, assigned once while the outlet lives.
    /// Tags are never reused, so a re-pushed route mounts fresh even under
    /// the same page key.
    fn tag_for(&self, route: RouteId) -> Key {
        if let Some(key) = self.tags.borrow().get(&route) {
            return key.clone();
        }
        let mut tags = self.tags.borrow_mut();
        if let Some(key) = tags.get(&route) {
            return key.clone();
        }
        let sequence = self.tag_sequence.get();
        self.tag_sequence.set(sequence.wrapping_add(1));
        let key = Key::String(format!("route-outlet-{}-{sequence}", self.namespace));
        tags.insert(route, key.clone());
        key
    }
}
