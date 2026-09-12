//! Hosts one navigator's routes as mounted, identity-tagged content.
//!
//! [`RouteOutlet`] is the production seam between a navigation stack and a
//! window tree: it composes every mounted route's content per an explicit
//! [`OutletPlacement`] policy, drives focus save/restore/forget and route
//! task bindings from live navigator state, and derives element ownership
//! from the mounted tags — applications never maintain element-id oracles.
//!
//! Drive contract: include [`RouteOutlet::widget`] in the window tree,
//! rebuild it after navigation (observer or revision), and call
//! [`RouteOutlet::after_frame`] after every frame presenting outlet
//! content. Covered routes without retention unmount (disposal); retained
//! ones stay mounted inert while invisible; only permanent removal ends
//! lifetimes.

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
};

use incular_navigation::{ModalBarrier, Navigator, Route, RouteId, RoutePresentation};
use incular_widgets::{
    AnimatedModalBarrier, ExcludeFocusTraversal, LayoutBuilder, SizedBox, Stack, Visibility,
    Widget,
    internal::{ElementId, Key, WidgetTree},
};

use super::{frame::Runtime, route_focus::RouteFocusState, tasks::TaskScope};
use crate::RouteTaskBinding;

/// Sequence numbering outlet key namespaces so sibling outlets sharing one
/// tree never tag two routes alike.
static OUTLET_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// How the outlet presents one route, derived from its [`RoutePresentation`].
///
/// Modes the outlet cannot honor are explicit variants, never silent
/// behavior: overlay entries are portal-managed and never outlet-mounted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OutletPlacement {
    /// Mounted stack content. `visible` paints, hits, and exposes
    /// semantics; invisible content stays mounted inert while the route
    /// retains state. `barrier` veils content below while mounted.
    Stacked {
        visible: bool,
        barrier: Option<ModalBarrier>,
    },
    /// Portal-managed overlay entries: host them in an `OverlayPortal`
    /// separately instead of mounting them here.
    ExternalOverlay,
}

impl OutletPlacement {
    /// Classifies `presentation` given whether any opaque route above
    /// covers it. Only opaque pages occlude, so translucent veils dim
    /// painted content rather than blackness.
    pub fn of(presentation: &RoutePresentation, visible: bool) -> Self {
        if presentation.is_overlay() {
            return Self::ExternalOverlay;
        }
        Self::Stacked {
            visible,
            barrier: presentation.barrier().cloned(),
        }
    }
}

/// Mounts one [`Navigator`] as presentation-aware stack content.
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

    /// Builds the outlet widget: mounted routes in stack order with each
    /// route's veil as a direct sibling preceding its content, so barrier
    /// semantics block every earlier route while the veiled route itself
    /// stays visible. The visible suffix paints from the top down to the
    /// first opaque page inclusive; retained but covered routes stay
    /// mounted inert (no paint, hit, semantics, or focus); routes without
    /// retention unmount; overlay entries are left for their portal host.
    #[must_use]
    pub fn widget(&self) -> Widget {
        let routes = self.navigator.routes();
        // Top-down visibility: everything paints until (and including) the
        // first opaque page.
        let mut visible = vec![false; routes.len()];
        let mut occluded = false;
        for (index, route) in routes.iter().enumerate().rev() {
            visible[index] = !occluded;
            if route.presentation.is_opaque() {
                occluded = true;
            }
        }
        let mut children = Vec::new();
        for (route, visible) in routes.iter().zip(visible) {
            let OutletPlacement::Stacked { visible, barrier } =
                OutletPlacement::of(&route.presentation, visible)
            else {
                continue;
            };
            if !visible && !route.presentation.maintains_state() {
                continue;
            }
            // Veil before content: semantic blocking hides preceding
            // siblings only, so the veil must precede its route's content
            // (paint and hit order still resolve top-down correctly, since
            // the veil sits above every earlier route).
            if let Some(barrier) = barrier {
                children.push(self.veil_widget(route.id, &barrier));
            }
            children.push(self.content_widget(route, visible));
        }
        if children.is_empty() {
            return SizedBox::shrink().into();
        }
        Stack::new(children).into()
    }

    /// Composes one mounted route's content inside the outlet's own
    /// visibility wrapper carrying the mount tag. The tag sits on the
    /// outlet-owned wrapper — never on the presented child — so caller
    /// keys on route content survive navigation untouched, and the tag
    /// element stays stable across visibility flips.
    fn content_widget(&self, route: &Route, visible: bool) -> Widget {
        let content = route.presented_child();
        let mut wrapped: Widget = Visibility::new(content)
            .visible(visible)
            .maintain_state(true)
            .into();
        if !visible {
            wrapped = ExcludeFocusTraversal::new(wrapped).into();
        }
        wrapped.with_key(self.tag_for(route.id))
    }

    /// Veils the outlet area for one route through the shared animated
    /// barrier mechanism: unconditional pointer interception, background
    /// semantics blocked for every preceding sibling, and dismissal that
    /// pops the barrier's own route — never whatever happens to be on top.
    /// The veil is a direct stack sibling above its content (not nested
    /// inside it) so semantic blocking reaches across routes; it is sized
    /// to the outlet area because an unsized barrier container collapses
    /// and lets taps fall through to the supposedly blocked content.
    /// Veils the outlet area for one route through the shared animated
    /// barrier mechanism: unconditional pointer interception, background
    /// semantics blocked for every preceding sibling, and dismissal that
    /// pops the barrier's own route — never whatever happens to be on top.
    /// The veil is a direct stack sibling preceding its content (not nested
    /// inside it) so semantic blocking reaches across routes while the
    /// route's own content stays visible; it is sized to the outlet area
    /// because an unsized barrier container collapses and lets taps fall
    /// through to the supposedly blocked content.
    fn veil_widget(&self, route: RouteId, barrier: &ModalBarrier) -> Widget {
        let navigator = self.navigator.clone();
        let barrier = barrier.clone();
        Widget::from(LayoutBuilder::new(move |_, constraints| {
            // Bounded hosts (windows, dialogs) size the veil to the outlet
            // area; unbounded heights fall back to the raw veil, which still
            // blocks its own content but cannot cover an unknown area.
            let size = constraints.biggest();
            let navigator = navigator.clone();
            let mut veil = AnimatedModalBarrier::with_flat_color(barrier.color)
                .dismissible(barrier.dismissible);
            if let Some(label) = barrier.label.clone() {
                veil = veil.semantics_label(label);
            }
            let veiled: Widget = veil
                .on_dismiss(move || {
                    if navigator
                        .current()
                        .is_some_and(|current| current.id == route)
                    {
                        navigator.pop();
                    }
                })
                .into();
            if size.width.is_finite() && size.height.is_finite() {
                SizedBox::from_dimensions(Some(size.width), Some(size.height), Some(veiled)).into()
            } else {
                veiled
            }
        }))
        .block_semantics()
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
