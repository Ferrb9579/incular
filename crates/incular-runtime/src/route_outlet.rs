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

use incular_config::Constraints;
use incular_navigation::{ModalBarrier, Navigator, Route, RouteId, RoutePresentation};
use incular_rendering::DisplayList;
use incular_widgets::{
    AnimatedModalBarrier, ExcludeFocusTraversal, LayoutBuilder, SizedBox, Stack, Visibility,
    Widget,
    internal::{ElementId, Key, TreeError, WidgetTree},
};

use super::{
    frame::{FrameStats, Runtime},
    tasks::TaskScope,
};
use crate::{RouteTaskBinding, route_focus::RouteFocusState};

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
    /// Mount tags of directly nested outlets, newest last. Elements inside
    /// a nested outlet belong to it even when an outer tag sits above them;
    /// transitivity follows containment, so only direct children register.
    nested_roots: Rc<RefCell<Vec<Key>>>,
    task_parent: TaskScope,
    last_active: Option<RouteId>,
    namespace: u64,
    /// Monotonic tag counter. Freed tags are never reassigned (unlike the
    /// map length, which shrinks on removal), so element identity can never
    /// alias across removals. Interior so [`Self::widget`] shares by ref.
    tag_sequence: Cell<u64>,
    /// Mount tag on the outlet root, used to locate it for [`Self::attach`].
    root_key: Key,
    /// Invalidation revision bumped by every [`Self::present_frame`].
    /// Attached builders and nested stateful slots observe it to rebuild
    /// outlet content; builder-owned children survive ancestor rebuilds,
    /// so nested state and focus persist across outer frames.
    revision: Rc<Cell<u64>>,
    /// Attached mount element, if [`Self::attach`] located the outlet root.
    /// Cleared when the mount disappears so later frames degrade to
    /// frame-plus-reconcile instead of erroring forever.
    mount: Cell<Option<ElementId>>,
}

impl RouteOutlet {
    /// Hosts `navigator`, parenting route task scopes under `task_parent`
    /// (typically the window scope, so close still cancels through existing
    /// ownership).
    pub fn new(navigator: &Navigator, task_parent: &TaskScope) -> Self {
        let tags: Rc<RefCell<HashMap<RouteId, Key>>> = Rc::default();
        let tags_for_oracle = tags.clone();
        let nested_roots: Rc<RefCell<Vec<Key>>> = Rc::default();
        let nested_for_oracle = nested_roots.clone();
        let focus = RouteFocusState::new(move |tree: &WidgetTree, id: ElementId| {
            // Ancestor chain of the element, self included. Nested outlets
            // win first: anything at or below a nested outlet root belongs
            // to that outlet, even with an outer tag above it. Remaining
            // attribution is by ancestry — sibling route subtrees never
            // contain each other, so any matched own tag is the nearest.
            // Scans stay bounded: one ancestor walk plus one reverse lookup
            // per tag, on transitions and vacant retries only — never a
            // per-route sweep.
            let mut chain = Vec::new();
            let mut cursor = Some(id);
            while let Some(current) = cursor {
                chain.push(current);
                cursor = tree.parent(current);
            }
            for nested in nested_for_oracle.borrow().iter() {
                if let Some(root) = tree.element_with_key(nested)
                    && (root == id || chain.contains(&root))
                {
                    return None;
                }
            }
            tags_for_oracle.borrow().iter().find_map(|(route, key)| {
                tree.element_with_key(key)
                    .is_some_and(|root| chain.contains(&root))
                    .then_some(*route)
            })
        });
        let namespace = OUTLET_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        Self {
            navigator: navigator.clone(),
            focus,
            bindings: HashMap::new(),
            tags,
            nested_roots,
            task_parent: task_parent.clone(),
            last_active: None,
            namespace,
            tag_sequence: Cell::new(0),
            root_key: Key::String(format!("route-outlet-root-{namespace}")),
            revision: Rc::new(Cell::new(0)),
            mount: Cell::new(None),
        }
    }

    /// Registers a directly nested outlet whose content mounts inside this
    /// outlet's routes. Nested elements resolve to the nested outlet, so
    /// inner navigation can neither overwrite outer saved focus nor be
    /// restored over by it. Sibling outlets sharing only an ancestor need
    /// nothing: disjoint subtrees never attribute across. Transitivity
    /// follows containment — register each outlet with its direct parent.
    ///
    /// Nested mounting pattern: host the nested widget in a stable keyed
    /// slot built by a stateful builder reading the nested [`Self::revision`]
    /// handle. Builder-owned children survive ancestor rebuilds by
    /// framework contract, so the slot keeps its content (and the nested
    /// outlet keeps its state and focus) across outer frames, while the
    /// revision handle refreshes it on nested navigation. A plain
    /// replacing builder instead remounts the slot every outer rebuild.
    /// No nested `attach` is needed on top: present frames already drive
    /// reconciliation, and the slot builder drives mounting.
    pub fn attach_nested(&mut self, nested: &RouteOutlet) {
        let key = nested.root_key.clone();
        let mut roots = self.nested_roots.borrow_mut();
        if !roots.contains(&key) {
            roots.push(key);
        }
    }

    /// Returns the navigator this outlet hosts.
    #[must_use]
    pub fn navigator(&self) -> &Navigator {
        &self.navigator
    }

    /// Invalidation revision bumped by every [`Self::present_frame`].
    /// Builders hosting outlet content observe it: the attached builder
    /// through explicit rebuilds, and nested stateful slots by reading
    /// this handle (distinct from [`Navigator::revision`], which counts
    /// stack mutations).
    #[must_use]
    pub fn revision(&self) -> Rc<Cell<u64>> {
        self.revision.clone()
    }

    /// Attaches the outlet to its mounted widget so [`Self::present_frame`]
    /// rebuilds outlet content every frame. Call once after mounting
    /// [`Self::widget`]: the mount element is located through the outlet
    /// root tag. Re-attaching replaces the previous builder harmlessly.
    ///
    /// ```rust
    /// use std::{cell::RefCell, rc::Rc};
    /// use incular_config::Constraints;
    /// use incular_core::{Color, Size};
    /// use incular_navigation::{Navigator, Page};
    /// use incular_runtime::{RouteOutlet, Runtime};
    /// use incular_widgets::{Column, Widget};
    ///
    /// let navigator = Navigator::new();
    /// let mut runtime =
    ///     Runtime::new(Column::new(Vec::<Widget>::new()).into()).unwrap();
    /// let parent = runtime.spawner().scope();
    /// let outlet = Rc::new(RefCell::new(RouteOutlet::new(&navigator, &parent)));
    /// // One-time setup: mount the outlet widget, then attach its builder.
    /// let root = runtime.tree().root().expect("root");
    /// let outlet_for_build = outlet.clone();
    /// runtime
    ///     .register_builder(root, move || {
    ///         Column::new(vec![outlet_for_build.borrow().widget()]).into()
    ///     })
    ///     .unwrap();
    /// runtime
    ///     .run_frame(Constraints::tight(Size::new(200., 200.)))
    ///     .unwrap();
    /// RouteOutlet::attach(&outlet, &mut runtime).unwrap();
    /// navigator.push_page(Page::new("a", Widget::box_(Size::new(40., 40.), Color::WHITE)));
    /// // The single recurring operation: rebuild, frame, reconcile.
    /// let _frame =
    ///     RouteOutlet::present_frame(&outlet, &mut runtime, Constraints::tight(Size::new(200., 200.)))
    ///         .unwrap();
    /// assert!(navigator.current().is_some());
    /// ```
    pub fn attach(
        outlet: &Rc<RefCell<RouteOutlet>>,
        runtime: &mut Runtime,
    ) -> Result<(), TreeError> {
        let mount = {
            let outlet = outlet.borrow();
            runtime
                .tree()
                .element_with_key(&outlet.root_key)
                .ok_or_else(|| TreeError::InvalidWidgetConfiguration {
                    widget: "RouteOutlet",
                    reason: "mount outlet.widget() in the tree before attaching".to_owned(),
                })?
        };
        // The builder carries no reactive dependencies: present_frame
        // rebuilds it explicitly, so content never refreshes behind the
        // transition bookkeeping's back.
        let outlet_for_build = Rc::clone(outlet);
        runtime.register_builder(mount, move || outlet_for_build.borrow().widget())?;
        outlet.borrow_mut().mount.set(Some(mount));
        Ok(())
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
        // The outlet root carries the mount tag so `attach` locates it;
        // route tags live on the per-route wrappers inside. The root is
        // always a stack — even empty — so mounting and unmounting the
        // last route never flips the element kind.
        let root: Widget = Stack::new(children).into();
        root.with_key(self.root_key.clone())
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

    /// Presents one production frame through the outlet: rebuilds outlet
    /// content, runs layout, then reconciles integration state with the
    /// mounted tree. This is the supported recurring operation: the save
    /// runs before the frame reconciles (rebuilding may unmount covered
    /// content and the drain clears dead focus), while the restore runs
    /// after layout when targets are eligible.
    ///
    /// Takes the outlet shared (rather than `&mut self`) because the
    /// attached builder reenters it immutably while the frame runs; short
    /// borrows never span the frame. A failed frame leaves transition
    /// bookkeeping advanced, and the next frame retries restoration only
    /// when it cannot displace valid focus.
    pub fn present_frame(
        outlet: &Rc<RefCell<Self>>,
        runtime: &mut Runtime,
        constraints: Constraints,
    ) -> Result<(DisplayList, FrameStats), TreeError> {
        let transitioned = {
            let mut outlet = outlet.borrow_mut();
            outlet.revision.set(outlet.revision.get().wrapping_add(1));
            outlet.capture_transition(runtime)
        };
        // Rebuild outlet content explicitly before framing: reactive
        // dependencies alone cannot cover content the host descriptors
        // never mention, and an explicit rebuild keeps mounting ordered
        // with the capture above. A vanished mount detaches gracefully
        // instead of erroring every frame.
        let attached = outlet.borrow().mount.get();
        if let Some(mount) = attached {
            match runtime.rebuild_from_builder(mount) {
                Ok(()) => {}
                Err(TreeError::MissingElement(_)) => {
                    outlet.borrow_mut().mount.set(None);
                }
                Err(error) => return Err(error),
            }
        }
        let output = runtime.run_frame(constraints)?;
        outlet
            .borrow_mut()
            .reconcile_presented(runtime, transitioned);
        Ok(output)
    }

    /// Reconciles integration state with live navigator state after a
    /// frame: forgets records, bindings, and tags for unmounted routes,
    /// saves the deactivated route's owned focus, binds newly mounted
    /// routes, and restores the activated route's eligible focus (whose
    /// targets mounted in the frame just presented). Manual-driving escape
    /// hatch for custom hosts; prefer [`Self::present_frame`], whose
    /// pre-frame capture also survives disposal-unmounts. Use one driver
    /// per outlet.
    pub fn after_frame(&mut self, runtime: &mut Runtime) {
        self.prune_unmounted();
        let transitioned = self.capture_transition(runtime);
        self.ensure_bindings();
        self.restore_or_retry(runtime, transitioned);
    }

    /// Restores the active route's saved focus now, for content that
    /// mounted after the transition frame. Ordinary flows go through
    /// [`Self::present_frame`] (or [`Self::after_frame`]).
    pub fn restore_active(&mut self, runtime: &mut Runtime) {
        if let Some(id) = self.navigator.current().map(|route| route.id) {
            self.focus.restore_saved(runtime, &self.navigator, id);
        }
    }

    /// Captures a pending transition: saves the deactivated route's owned
    /// focus and advances the tracked active route. Returns whether the
    /// active route changed.
    fn capture_transition(&mut self, runtime: &Runtime) -> bool {
        let active = self.navigator.current().map(|route| route.id);
        let transitioned = self.last_active != active;
        if transitioned {
            if let Some(previous) = self.last_active
                && self.navigator.lifetime_of(previous).is_some()
            {
                self.focus.save_focused(runtime, previous);
            }
            self.last_active = active;
        }
        transitioned
    }

    /// Drops records, bindings, and tags for unmounted routes. Lifetime-end
    /// notifications already fired at commit time, so bound scopes were
    /// cancelled before their bindings drop here.
    fn prune_unmounted(&mut self) {
        self.focus.retain_mounted(&self.navigator);
        self.bindings
            .retain(|id, _| self.navigator.lifetime_of(*id).is_some());
        self.tags
            .borrow_mut()
            .retain(|id, _| self.navigator.lifetime_of(*id).is_some());
    }

    /// Binds every mounted route lacking a binding. Inactive routes keep
    /// their tasks: only permanent removal ends the lifetime behind them.
    fn ensure_bindings(&mut self) {
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
    }

    /// Restores on transitions; otherwise retries a pending restore only
    /// when it cannot displace valid focus (deferred content may have
    /// mounted or enabled through ordinary invalidation since).
    fn restore_or_retry(&mut self, runtime: &mut Runtime, transitioned: bool) {
        let Some(id) = self.navigator.current().map(|route| route.id) else {
            return;
        };
        if transitioned {
            self.focus.restore_saved(runtime, &self.navigator, id);
        } else {
            self.focus.restore_if_vacant(runtime, &self.navigator, id);
        }
    }

    /// Post-frame reconciliation for [`Self::present_frame`]: prune, bind,
    /// then restore or retry without re-saving (the pre-frame capture owns
    /// saving, so a disposal-unmounted target cannot overwrite its record
    /// with nothing).
    fn reconcile_presented(&mut self, runtime: &mut Runtime, transitioned: bool) {
        self.prune_unmounted();
        self.ensure_bindings();
        self.restore_or_retry(runtime, transitioned);
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
