//! Hosts one navigator's routes as mounted, identity-tagged content.
//!
//! [`RouteOutlet`] is the production seam between a navigation stack and a
//! window tree: it composes every mounted route's content per an explicit
//! [`OutletPlacement`] policy, drives focus save/restore/forget and route
//! task bindings from live navigator state, and derives element ownership
//! from the mounted tags — applications never maintain element-id oracles.
//!
//! Drive contract: mount [`RouteOutlet::widget`] in the window tree,
//! attach once with [`RouteOutlet::attach`], then drive every cycle with
//! the single [`RouteOutlet::present_frame`] operation — rebuild, frame,
//! and reconcile in enforced order, with tree-wide validation before and
//! after the frame. Covered routes without retention unmount (disposal);
//! retained ones stay mounted inert while invisible; only permanent
//! removal ends lifetimes.
//!
//! Frame contract: a frame runs build, layout, composite, semantics, then
//! paint, so restoration — which reconciles after the frame returns —
//! cannot make that same frame's paint or semantics. The restore applies
//! to the focus slot synchronously and flags follow-up work through the
//! existing scheduler (`set_focus` marks another frame); the next frame
//! then finalizes styling, semantics, and keyboard dispatch from restored
//! focus. No unconditional extra frame runs: follow-up is flagged when
//! reconciliation changed focus, or when the presented output is older
//! than live navigation (mid-build navigation flags exactly once per
//! revision, nested staleness wakes the root driver, and quiet frames
//! flag nothing). Errors schedule no follow-up at all, so rejected
//! stacks cannot spin a retry loop. Deferred content converges through
//! its own invalidation afterwards.
//!
//! Driving states (derived from the retained fields, not tracked
//! separately — `pending`, `consumed` vs live revision, `needs_frame`,
//! and the per-revision schedule memo):
//!
//! | Situation | pending | consumed | needs_frame | scheduled | exit |
//! | Unattached, empty | none | none | false | — | navigate → stale |
//! | Unattached, routes live | per capture | older/none | true | on success | attach → attached |
//! | Attached, idle | none | current | false | — | navigate → pending |
//! | Attempt pending | some | just composed | false | — | commit → consumed; mid-frame nav → stale |
//! | Failed (error/panic) | kept | older | true | reset, re-evaluates | retry → pending/consumed |
//! | Successful-but-stale | recaptured/none | older | true | once per rev | follow-up → consumed |
//! | Detached | kept (own) | frozen | own tree only | own claims | reattach or drive standalone |

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::{Rc, Weak},
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

/// The navigation state one frame attempt builds content from: the
/// authoritative stack revision plus the ordered route identities the
/// frame composes. Immutable once captured; refreshed (never re-read for
/// focus) at every build boundary so retries describe the content they
/// actually present. Presentation is deliberately not snapshotted: it is
/// fixed for a route identity, so revision plus identities is
/// authoritative — no boolean approximation.
#[derive(Clone, Debug, PartialEq, Eq)]
struct FrameAttempt {
    revision: u64,
    active: Option<RouteId>,
    members: Vec<RouteId>,
}

impl FrameAttempt {
    fn capture(navigator: &Navigator) -> Self {
        Self::of(navigator.revision(), &navigator.routes())
    }

    /// Snapshots exactly these routes at this revision: [`RouteOutlet::widget`]
    /// builds the attempt from the same route list it composes children
    /// from, so acquisition and composition stay together.
    fn of(revision: u64, routes: &[Route]) -> Self {
        Self {
            revision,
            active: routes.last().map(|route| route.id),
            members: routes.iter().map(|route| route.id).collect(),
        }
    }

    /// Whether the live navigator still matches this attempt closely
    /// enough to commit. Same revision means nothing moved. Otherwise the
    /// member set must match under the same active route: order-only
    /// churn (a lower-route reorder beneath an unchanged top) and
    /// same-identity child replacement keep the per-route bookkeeping
    /// valid, while membership changes — pushes, pops, removals —
    /// abandon. Push-then-pop back to the original top with no pending
    /// transition never reaches the commit; with one in flight the ids
    /// differ and the capture abandons cleanly.
    fn covers(&self, navigator: &Navigator) -> bool {
        if navigator.revision() == self.revision {
            return true;
        }
        let current = Self::capture(navigator);
        current.active == self.active
            && current.members.len() == self.members.len()
            && self.members.iter().all(|id| current.members.contains(id))
    }
}

/// One uncommitted frame transition, tracking four independent concerns:
/// - the outgoing focus save (`previous`/`saved`), which survives retries
///   untouched — rebuilding re-records consumption without re-reading;
/// - the navigation snapshot for this attempt (`attempt`), adopted from
///   what composition actually consumed (see `consumed`);
/// - newer-work detection, via the consumed revision versus live state
///   (see [`RouteOutlet::needs_frame`]);
/// - lifetime eligibility, checked at commit (incoming) and at restore
///   (records never restore for removed routes).
///
/// The save lives here — not in the records — until the frame presenting
/// it commits, so failed attempts and navigation mid-frame can never
/// disturb committed state.
#[derive(Clone, Debug, PartialEq, Eq)]
struct PendingTransition {
    previous: Option<RouteId>,
    incoming: Option<RouteId>,
    saved: Option<ElementId>,
    attempt: FrameAttempt,
}

/// Identity of one [`RouteOutlet`], for actionable error data and host
/// bookkeeping. Assigned once per outlet; sibling outlets sharing one
/// tree never collide.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OutletId(u64);

impl std::fmt::Display for OutletId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "route-outlet-{}", self.0)
    }
}

/// A route outlet refusing to present.
///
/// Timing differs per variant, and the docs say which: pre-frame
/// rejections change nothing mounted or committed, while the post-frame
/// validation can fire after the tree mounted the frame's content — in
/// that case integration state (records, bindings, tags, saves) is still
/// untouched, but the tree holds the last frame's stack output. Hosts fix
/// the stack (portal-mount the listed routes, or keep them out of
/// outlet-driven navigators) instead of debugging silently omitted
/// content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OutletError {
    /// One or more routes use portal-managed overlay presentations, which
    /// the outlet never mounts. Carries the offending outlet alongside
    /// the route ids, so nested rejections point at the responsible
    /// navigator instead of the driving root.
    UnsupportedPresentation {
        outlet: OutletId,
        routes: Vec<RouteId>,
    },
    /// Another live runtime already drives this outlet: the outlet is
    /// attached to (or last presented by) `owner`, so presenting through
    /// `attempted` would split one lifecycle across two frame loops.
    /// Detach or drop the first runtime to release the claim — teardown
    /// releases it automatically — then drive here.
    DriverConflict {
        outlet: OutletId,
        owner: u64,
        attempted: u64,
    },
    /// This outlet nests under a live parent: drive the root instead.
    /// Separate presents would drive the child twice per frame (once
    /// directly, once through the cascade) and corrupt the pending
    /// capture. Detach first to drive it standalone.
    SeparateDrive { outlet: OutletId, parent: OutletId },
    /// The underlying rebuild or frame failed.
    Frame(TreeError),
}

impl std::fmt::Display for OutletError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedPresentation { outlet, routes } => {
                write!(
                    formatter,
                    "{outlet} cannot present portal-managed overlay routes: {routes:?} \
                     (host overlay entries in an OverlayPortal separately)"
                )
            }
            Self::DriverConflict {
                outlet,
                owner,
                attempted,
            } => {
                write!(
                    formatter,
                    "{outlet} is already driven by runtime-{owner}: refusing runtime-{attempted} \
                     (one lifecycle per runtime; release the first claim before driving here)"
                )
            }
            Self::SeparateDrive { outlet, parent } => {
                write!(
                    formatter,
                    "{outlet} nests under {parent}: drive the root instead of presenting \
                     the child separately (detach first to drive it standalone)"
                )
            }
            Self::Frame(error) => {
                write!(formatter, "route outlet frame failed: {error}")
            }
        }
    }
}

impl std::error::Error for OutletError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::UnsupportedPresentation { .. }
            | Self::DriverConflict { .. }
            | Self::SeparateDrive { .. } => None,
            Self::Frame(error) => Some(error),
        }
    }
}

impl From<TreeError> for OutletError {
    fn from(error: TreeError) -> Self {
        Self::Frame(error)
    }
}

/// Why a nested outlet attachment was rejected.
///
/// Rejection happens before any mutation, so a failed attach leaves both
/// outlets exactly as they were. The topology is a forest by design —
/// like back dispatch it must terminate, but unlike back dispatch (which
/// follows a single active chain and can share children) cascade driving
/// visits every registered child, so one child under two parents would
/// drive twice per frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutletAttachError {
    /// An outlet cannot nest inside itself.
    SelfAttachment,
    /// The child already reaches the parent through nested attachments,
    /// so the edge would close a directed cycle and cascade driving
    /// could recurse through it forever.
    Cycle,
    /// The child is already attached to a different live parent. Detach
    /// it explicitly first: cascade driving follows registration, so a
    /// second parent would drive the same outlet twice per frame.
    AlreadyAttached,
}

impl std::fmt::Display for OutletAttachError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SelfAttachment => formatter.write_str("a route outlet cannot nest inside itself"),
            Self::Cycle => formatter.write_str(
                "attaching this outlet would cycle cascade driving: \
                 it already reaches its parent",
            ),
            Self::AlreadyAttached => formatter.write_str(
                "this outlet is already attached to a different parent: \
                 detach it explicitly before reattaching",
            ),
        }
    }
}

impl std::error::Error for OutletAttachError {}

/// Mounts one [`Navigator`] as presentation-aware stack content.
///
/// The outlet owns the integration state the navigator cannot: per-route
/// mount tags feeding the focus ownership oracle, a [`RouteFocusState`]
/// driven across transitions, and one [`RouteTaskBinding`] per live
/// route (lifetime-owned, including unmounted-but-alive routes). Use one
/// outlet per navigator; nested navigators get nested
/// outlets and stay isolated through disjoint tags and states.
///
/// Dropping the outlet releases the integration: focus records, bindings,
/// and tags go with it (bound scopes then follow ordinary [`TaskScope`]
/// ownership, detached from later removals).
///
/// State inventory (owner → invariant): `navigator` (cloned handle —
/// identity and lifetimes live in navigation); `focus`, `bindings`,
/// `tags` (outlet — pruned to live routes on every reconcile);
/// `nested`/`parent` (attachment graph — weak both ways, single-parent,
/// acyclic); `driver` (driving runtime — claimed on drive, released on
/// detach or runtime drop); `task_parent` (host scope — never mutated);
/// `presented`/`pending` (transition bookkeeping — advance only on
/// commit); `epoch` (attempt context — one per driven frame);
/// `composing`/`consumed` (composition record — receipted mounting
/// builders only, promoted on success only); `signaled`
/// (schedule memo — edge-triggered per revision); `namespace`/`root_key`
/// (immutable
/// construction identity); `tag_sequence` (monotonic, never reused);
/// `revision` (invalidation generation — doubles as the drive counter);
/// `mount` (attached element — cleared when it vanishes).
pub struct RouteOutlet {
    navigator: Navigator,
    focus: RouteFocusState,
    bindings: HashMap<RouteId, RouteTaskBinding>,
    tags: Rc<RefCell<HashMap<RouteId, Key>>>,
    /// Directly nested outlets, weakest first. Elements inside a nested
    /// outlet belong to it even when an outer tag sits above them;
    /// transitivity follows containment, so only direct children register.
    /// Weak handles release automatically when a nested outlet drops, and
    /// dead entries prune on every present, so repeated replacement never
    /// grows metadata. Registration is what cascade driving follows: a
    /// detached outlet receives no frame work even while retained.
    nested: Rc<RefCell<Vec<Weak<RefCell<RouteOutlet>>>>>,
    /// This outlet's parent, if attached. Single-parent by design (see
    /// [`OutletAttachError`]): set on attach, cleared on detach or when
    /// the parent drops.
    parent: Rc<RefCell<Option<Weak<RefCell<RouteOutlet>>>>>,
    /// The runtime driving this outlet, with its liveness token. Claimed
    /// on attach and on every driven present (roots and cascade children
    /// alike); released on detach and automatically when the runtime
    /// drops. No global registry: ownership lives in this claim plus the
    /// runtime's own builder registration.
    driver: RefCell<Option<(u64, Weak<u64>)>>,
    task_parent: TaskScope,
    /// Last successfully presented active route. Transitions capture
    /// against this and advance it only on commit — never on failure.
    presented: Option<RouteId>,
    /// Uncommitted capture: `Some` while a frame transition is in flight.
    pending: Option<PendingTransition>,
    /// Frame attempt counter: bumped by every [`Self::begin_frame`].
    /// Necessary context for [`Self::composing`], not proof of mounting:
    /// only a receipt stamped by a mounting builder during the current
    /// attempt authorizes adoption.
    epoch: Cell<u64>,
    /// Publication slot for in-flight composition: the mounting builders
    /// write a receipt (attempt plus snapshot) here on every execution
    /// (see [`Self::compose_and_record`]). Never read directly for
    /// decisions — reconciliation promotes it (see `consumed`) only on
    /// the success path for the current attempt, so failed composition
    /// publishes nothing and discarded descriptors authorize nothing.
    composing: RefCell<Option<(u64, FrameAttempt)>>,
    /// Last composition a frame carried through reconciliation for this
    /// outlet. Comparing it against live state answers whether newer
    /// navigation still requires a frame (see [`Self::needs_frame`])
    /// independently of the pending save above. Each outlet promotes its
    /// own: nested slot builders promote when they execute and reconcile,
    /// skipped builders keep the previous publication.
    consumed: Option<FrameAttempt>,
    /// Live revision last scheduled for. Follow-up scheduling is
    /// edge-triggered per outlet: a stale revision flags exactly once no
    /// matter how many frames observe it, so unconsumable staleness
    /// (content no present can compose) idles instead of spinning. Reset
    /// whenever a present fails: a failed frame consumes nothing, so the
    /// next success must re-evaluate from scratch instead of inheriting
    /// the failure's suppression.
    signaled: Cell<u64>,
    namespace: u64,
    /// Monotonic tag counter. Freed tags are never reassigned (unlike the
    /// map length, which shrinks on removal), so element identity can never
    /// alias across removals. Interior so [`Self::widget`] shares by ref.
    tag_sequence: Cell<u64>,
    /// Mount tag on the outlet root, used to locate it for [`Self::attach`].
    root_key: Key,
    /// Invalidation generation bumped once per driven frame; nested
    /// stateful slots observe it to rebuild outlet content (builder-owned
    /// children survive ancestor rebuilds, so nested state and focus
    /// persist across outer frames). Doubles as the drive counter read by
    /// [`Self::frame_drive_count`]: one begin per present per outlet.
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
        let nested: Rc<RefCell<Vec<Weak<RefCell<RouteOutlet>>>>> = Rc::default();
        let nested_for_oracle = nested.clone();
        let focus = RouteFocusState::new(move |tree: &WidgetTree, id: ElementId| {
            // Ancestor chain of the element, self included. Nested outlets
            // win first: anything at or below a nested outlet root belongs
            // to that outlet, even with an outer tag above it. Remaining
            // attribution is by ancestry — sibling route subtrees never
            // contain each other, so any matched own tag is the nearest.
            // Scans stay bounded: one ancestor walk plus one reverse lookup
            // per tag, on transitions and vacant retries only — never a
            // per-route sweep. Dead handles simply miss.
            let mut chain = Vec::new();
            let mut cursor = Some(id);
            while let Some(current) = cursor {
                chain.push(current);
                cursor = tree.parent(current);
            }
            for nested in nested_for_oracle.borrow().iter() {
                let Some(nested) = nested.upgrade() else {
                    continue;
                };
                let nested = nested.borrow();
                if let Some(root) = tree.element_with_key(&nested.root_key)
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
            nested,
            parent: Rc::default(),
            driver: RefCell::default(),
            task_parent: task_parent.clone(),
            presented: None,
            pending: None,
            epoch: Cell::new(0),
            composing: RefCell::default(),
            consumed: None,
            signaled: Cell::new(0),
            namespace,
            // NOTE: no separate drive counter — `revision` below advances
            // once per driven frame and serves as the drive count.
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
    /// Topology is enforced before any mutation, borrowing the
    /// back-dispatch lesson without coupling to it: self-attachment and
    /// cycles are rejected (cascade driving must terminate), and each
    /// outlet has at most one live parent (cascade driving visits every
    /// registered child, so a shared child would drive twice per frame).
    /// Re-registering with the same parent is an idempotent no-op;
    /// moving parents requires [`Self::detach_nested`] first.
    /// Registration holds weakly: dropping the nested outlet releases it,
    /// and dead entries prune on every present, so repeated replacement
    /// never grows metadata.
    pub fn attach_nested(
        outlet: &Rc<RefCell<Self>>,
        nested: &Rc<RefCell<Self>>,
    ) -> Result<(), OutletAttachError> {
        if Rc::ptr_eq(outlet, nested) {
            return Err(OutletAttachError::SelfAttachment);
        }
        if nested.borrow().reaches(outlet) {
            return Err(OutletAttachError::Cycle);
        }
        {
            let child = nested.borrow();
            let mut parent = child.parent.borrow_mut();
            if let Some(current) = parent.as_ref().and_then(Weak::upgrade) {
                if Rc::ptr_eq(&current, outlet) {
                    return Ok(());
                }
                return Err(OutletAttachError::AlreadyAttached);
            }
            // A dead parent link is just a leftover: detach cleared the
            // registration but the child outlived this assignment.
            *parent = Some(Rc::downgrade(outlet));
        }
        let outlet = outlet.borrow_mut();
        let mut attached = outlet.nested.borrow_mut();
        attached.retain(|existing| {
            existing
                .upgrade()
                .is_some_and(|live| !Rc::ptr_eq(&live, nested))
        });
        attached.push(Rc::downgrade(nested));
        Ok(())
    }

    /// Detaches a nested outlet. The child keeps its own records, bindings,
    /// and tags, but the parent's cascade no longer drives it: a detached
    /// outlet receives no frame work even while application code retains
    /// its handle. Detach also releases the child's driver claim, so
    /// ownership transfers cleanly — reattaching elsewhere (or driving it
    /// standalone, including under another runtime) claims anew.
    /// Detaching a non-child is a no-op.
    pub fn detach_nested(outlet: &Rc<RefCell<Self>>, nested: &Rc<RefCell<Self>>) {
        {
            let child = nested.borrow_mut();
            let mut parent = child.parent.borrow_mut();
            let mine = parent
                .as_ref()
                .and_then(Weak::upgrade)
                .is_some_and(|current| Rc::ptr_eq(&current, outlet));
            if mine {
                parent.take();
                child.driver.borrow_mut().take();
            }
        }
        {
            let outlet = outlet.borrow_mut();
            outlet.nested.borrow_mut().retain(|existing| {
                existing
                    .upgrade()
                    .is_some_and(|live| !Rc::ptr_eq(&live, nested))
            });
        }
    }

    /// Whether `target` is reachable from this outlet by following nested
    /// attachments. Dead entries are skipped, never traversed.
    fn reaches(&self, target: &Rc<RefCell<Self>>) -> bool {
        let mut visited: Vec<*const RefCell<Self>> = Vec::new();
        let mut stack: Vec<Rc<RefCell<Self>>> = self.live_nested();
        // The outlet itself counts: self-attachment is checked separately
        // for its error, but a zero-length path must not read as a cycle.
        while let Some(node) = stack.pop() {
            if Rc::ptr_eq(&node, target) {
                return true;
            }
            let address = Rc::as_ptr(&node);
            if visited.contains(&address) {
                continue;
            }
            visited.push(address);
            stack.extend(node.borrow().live_nested());
        }
        false
    }

    /// Frames driven through this outlet, cascade included. Each
    /// [`Self::present_frame`] on the driving root advances every
    /// participating outlet by exactly one. Reads the invalidation
    /// generation, which advances once per driven frame and needs no
    /// separate counter.
    #[must_use]
    pub fn frame_drive_count(&self) -> u64 {
        self.revision.get()
    }

    /// Mounts a nested outlet's widget with the required builder identity:
    /// a stable keyed boundary driven by the nested revision, so ancestor
    /// rebuilds retain the slot instead of remounting it. Hosts compose
    /// this inside outer route content instead of assembling stateful
    /// builders by hand; no nested `attach` is needed on top. Presenting
    /// the outer outlet drives the whole tree in one frame — nested
    /// outlets never need their own `present_frame` calls.
    pub fn nested_widget(nested: &Rc<RefCell<Self>>) -> Widget {
        let (revision, namespace) = {
            let nested = nested.borrow();
            (nested.revision.clone(), nested.namespace)
        };
        let nested_for_build = Rc::clone(nested);
        Widget::stateful_layout_builder(revision, move |_, _| {
            nested_for_build.borrow().compose_and_record()
        })
        .with_key(Key::String(format!("route-outlet-slot-{namespace}")))
    }

    /// Number of live nested attachments. Dead handles prune on every
    /// present; useful for hosts composing nested outlets dynamically.
    #[must_use]
    pub fn attached_nested_count(&self) -> usize {
        self.nested
            .borrow()
            .iter()
            .filter(|weak| weak.upgrade().is_some())
            .count()
    }

    /// Live nested outlets for cascade driving.
    fn live_nested(&self) -> Vec<Rc<RefCell<Self>>> {
        self.nested
            .borrow()
            .iter()
            .filter_map(Weak::upgrade)
            .collect()
    }

    /// The live parent, if this outlet is attached under one.
    fn live_parent(&self) -> Option<Rc<RefCell<Self>>> {
        self.parent.borrow().as_ref().and_then(Weak::upgrade)
    }

    /// The live driver claim's runtime id, if a live runtime owns it. A
    /// dead token means teardown released the claim: treated as unowned.
    fn live_driver(&self) -> Option<u64> {
        self.driver
            .borrow()
            .as_ref()
            .and_then(|(id, token)| token.upgrade().is_some().then_some(*id))
    }

    /// Claims this outlet for `runtime`, superseding a dead owner's
    /// leftover. Callers verify conflicts first.
    fn claim_driver(&self, runtime: &Runtime) {
        *self.driver.borrow_mut() = Some((runtime.id(), runtime.driver_token()));
    }

    /// Rejects conflicting drivers for the whole tree before any mutation:
    /// the root must not nest under a live parent (drive the root
    /// instead), and every participant must be unclaimed or claimed by
    /// `runtime` — a live foreign claim fails with both identities.
    fn check_drivers(&self, runtime: &Runtime, is_root: bool) -> Result<(), OutletError> {
        if is_root && let Some(parent) = self.live_parent() {
            return Err(OutletError::SeparateDrive {
                outlet: self.id(),
                parent: parent.borrow().id(),
            });
        }
        if let Some(owner) = self.live_driver()
            && owner != runtime.id()
        {
            return Err(OutletError::DriverConflict {
                outlet: self.id(),
                owner,
                attempted: runtime.id(),
            });
        }
        for nested in self.live_nested() {
            nested.borrow().check_drivers(runtime, false)?;
        }
        Ok(())
    }

    /// Claims the whole tree for `runtime` after [`Self::check_drivers`]
    /// passes. Refreshes matching claims harmlessly.
    fn claim_tree(&self, runtime: &Runtime) {
        self.claim_driver(runtime);
        for nested in self.live_nested() {
            nested.borrow().claim_tree(runtime);
        }
    }

    /// Returns the navigator this outlet hosts.
    #[must_use]
    pub fn navigator(&self) -> &Navigator {
        &self.navigator
    }

    /// Returns this outlet's identity, for error data and host bookkeeping.
    #[must_use]
    pub fn id(&self) -> OutletId {
        OutletId(self.namespace)
    }

    /// Whether navigation arrived that no composition consumed yet — for
    /// this outlet or any nested outlet. True before the first composition
    /// with a non-empty stack, and after navigation newer than the last
    /// consumed snapshot (including navigation during a build, whose paint
    /// lags one frame behind the bookkeeping). False once composition
    /// consumes current state. Advisory scheduling help for hosts; it does
    /// not report failed frames (their error return does that).
    #[must_use]
    pub fn needs_frame(&self) -> bool {
        let stale = match &self.consumed {
            None => !self.navigator.routes().is_empty(),
            Some(consumed) => consumed.revision != self.navigator.revision(),
        };
        stale
            || self
                .live_nested()
                .iter()
                .any(|nested| nested.borrow().needs_frame())
    }

    /// Rejects this outlet's stack when it contains portal-managed overlay
    /// routes, which the outlet never mounts. Pure check: no state changes
    /// either way.
    fn preflight(&self) -> Result<(), OutletError> {
        let unsupported: Vec<RouteId> = self
            .navigator
            .routes()
            .iter()
            .filter(|route| route.presentation.is_overlay())
            .map(|route| route.id)
            .collect();
        if unsupported.is_empty() {
            Ok(())
        } else {
            Err(OutletError::UnsupportedPresentation {
                outlet: self.id(),
                routes: unsupported,
            })
        }
    }

    /// Validates the whole outlet tree top-down: this outlet, then every
    /// nested outlet transitively. Pure check — no revisions, captures,
    /// rebuilds, or frames happen on either outcome — so
    /// [`Self::present_frame`] runs it before driving anything and again
    /// after the frame (navigation may have moved mid-build) before
    /// committing anything. Reports the outermost offender first.
    fn preflight_tree(&self) -> Result<(), OutletError> {
        self.preflight()?;
        for nested in self.live_nested() {
            nested.borrow().preflight_tree()?;
        }
        Ok(())
    }

    /// Attaches the outlet to its mounted widget so [`Self::present_frame`]
    /// rebuilds outlet content every frame. Call once after mounting
    /// [`Self::widget`]: the mount element is located through the outlet
    /// root tag. Re-attaching replaces the previous builder harmlessly.
    ///
    /// Attachment claims the outlet for `runtime`'s driver domain: an
    /// outlet nested under a live parent, or claimed by another live
    /// runtime, is rejected before mutation. Detach (nested) or drop the
    /// owning runtime (teardown releases) before attaching elsewhere.
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
    ) -> Result<(), OutletError> {
        {
            let outlet = outlet.borrow();
            if let Some(parent) = outlet.live_parent() {
                return Err(OutletError::SeparateDrive {
                    outlet: outlet.id(),
                    parent: parent.borrow().id(),
                });
            }
            if let Some(owner) = outlet.live_driver()
                && owner != runtime.id()
            {
                return Err(OutletError::DriverConflict {
                    outlet: outlet.id(),
                    owner,
                    attempted: runtime.id(),
                });
            }
        }
        let mount = {
            let outlet = outlet.borrow();
            runtime
                .tree()
                .element_with_key(&outlet.root_key)
                .ok_or_else(|| {
                    OutletError::Frame(TreeError::InvalidWidgetConfiguration {
                        widget: "RouteOutlet",
                        reason: "mount outlet.widget() in the tree before attaching".to_owned(),
                    })
                })?
        };
        // The builder carries no reactive dependencies: present_frame
        // rebuilds it explicitly, so content never refreshes behind the
        // transition bookkeeping's back.
        let outlet_for_build = Rc::clone(outlet);
        runtime
            .register_builder(mount, move || {
                outlet_for_build.borrow().compose_and_record()
            })
            .map_err(OutletError::Frame)?;
        outlet.borrow().claim_driver(runtime);
        outlet.borrow_mut().mount.set(Some(mount));
        Ok(())
    }

    /// Builds the outlet widget: mounted routes in stack order with each
    /// route's veil as a direct sibling preceding its content, so barrier
    /// semantics block every earlier route while the veiled route itself
    /// stays visible. The visible suffix paints from the top down to the
    /// first opaque page inclusive; retained but covered routes stay
    /// Builds the outlet widget for mounting, without touching any
    /// bookkeeping: safe to call speculatively and discard. Only the
    /// mounting builders (registered by `attach`, and nested slots)
    /// acknowledge what they accept. Covered routes without retention
    /// unmount (disposal); retained ones stay mounted inert while
    /// invisible; overlay entries are left for their portal host.
    #[must_use]
    pub fn widget(&self) -> Widget {
        self.compose().0
    }

    /// Composes the widget from the current routes alongside the immutable
    /// snapshot it was built from. Pure: acquisition and composition stay
    /// together here, publication happens only where the result mounts.
    fn compose(&self) -> (Widget, FrameAttempt) {
        let routes = self.navigator.routes();
        let attempt = FrameAttempt::of(self.navigator.revision(), &routes);
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
            // Overlay entries never mount here: `present_frame` rejects
            // their stacks typed (before and after the frame), so reaching
            // this skip means setup-time composition ahead of validation —
            // still omitted, never half-mounted, and never silent through
            // the supported path.
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
            // the veil sits above every earlier route). Veils follow
            // visibility: a covered retained route keeps its content but
            // not an active veil — the veil would dim and intercept input
            // for content the user can no longer see.
            if visible && let Some(barrier) = barrier {
                children.push(self.veil_widget(route.id, &barrier));
            }
            children.push(self.content_widget(route, visible));
        }
        // The outlet root carries the mount tag so `attach` locates it;
        // route tags live on the per-route wrappers inside. The root is
        // always a stack — even empty — so mounting and unmounting the
        // last route never flips the element kind.
        let root: Widget = Stack::new(children).into();
        (root.with_key(self.root_key.clone()), attempt)
    }

    /// Composes for mounting and publishes the consumed receipt: the only
    /// path (besides [`Self::compose`]'s pure construction) that writes
    /// [`Self::composing`], stamped with the current attempt. Called
    /// solely by the mounting builders, so a discarded or speculative
    /// descriptor can never authorize a frame.
    fn compose_and_record(&self) -> Widget {
        let (widget, attempt) = self.compose();
        eprintln!(
            "PROBE-C outlet={} epoch={} rev={} n={}\n{:?}",
            self.namespace,
            self.epoch.get(),
            attempt.revision,
            attempt.members.len(),
            std::backtrace::Backtrace::force_capture()
        );
        *self.composing.borrow_mut() = Some((self.epoch.get(), attempt));
        widget
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

    /// Presents one production frame through the outlet: validates the
    /// tree, captures any pending transition, rebuilds outlet content,
    /// runs layout, then commits the transition against the mounted tree.
    /// This is the supported recurring operation: the save runs before
    /// the frame reconciles (rebuilding may unmount covered content and
    /// the drain clears dead focus), while the restore runs after layout
    /// when targets are eligible.
    ///
    /// Takes the outlet shared (rather than `&mut self`) because the
    /// attached builder reenters it immutably while the frame runs; short
    /// borrows never span the frame. Driving claims the whole tree for
    /// `runtime`: presenting a nested child directly, or through a second
    /// live runtime, fails before any mutation. A rebuild or frame failure
    /// keeps the pending capture for a safe retry instead of consuming the
    /// activation, and navigation mid-frame abandons the stale capture
    /// without touching committed records.
    pub fn present_frame(
        outlet: &Rc<RefCell<Self>>,
        runtime: &mut Runtime,
        constraints: Constraints,
    ) -> Result<(DisplayList, FrameStats), OutletError> {
        // Unsupported stacks anywhere in the tree fail before revisions,
        // capture, rebuild, frame, or reconcile — nothing mounted or
        // committed changes on this path. Driver conflicts fail next, also
        // before any mutation; the claim lands only on this success path.
        outlet.borrow().preflight_tree()?;
        outlet.borrow().check_drivers(runtime, true)?;
        outlet.borrow().claim_tree(runtime);
        {
            let mut outlet = outlet.borrow_mut();
            outlet.begin_frame(runtime);
        }
        // The fallible section runs builders, layout, and reconciliation —
        // all application-reachable code. Failures reset the schedule
        // memos (a failed present consumes nothing, so the next success
        // must re-evaluate outstanding work); panics reset and resume, so
        // a panicking frame loses no future wakeup either. Neither path
        // repairs tree state — recovery stays the retry's job.
        let driven = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            // Rebuild outlet content explicitly before framing: reactive
            // dependencies alone cannot cover content the host descriptors
            // never mention, and an explicit rebuild keeps mounting ordered
            // with the capture above. A vanished mount (unmounted content
            // prunes its builder) detaches gracefully instead of erroring —
            // or panicking — every frame. The existence check precedes the
            // rebuild because pruned builders no longer fail with
            // `MissingElement`.
            let attached = outlet
                .borrow()
                .mount
                .get()
                .filter(|mount| runtime.tree().element_exists(*mount));
            if attached.is_none() {
                outlet.borrow_mut().mount.set(None);
            }
            if let Some(mount) = attached {
                match runtime.rebuild_from_builder(mount) {
                    Ok(()) => {}
                    Err(TreeError::MissingElement(_)) => {
                        outlet.borrow_mut().mount.set(None);
                    }
                    Err(error) => return Err(OutletError::Frame(error)),
                }
            }
            // No post-rebuild sampling here: each outlet's builders publish
            // what they actually consume (see `consumed`), and
            // reconciliation adopts it. Sampling afterward would credit the
            // rebuild with navigation it never saw.
            let output = runtime.run_frame(constraints)?;
            // A preflight snapshot cannot authorize later content: builders
            // may have navigated mid-frame, so the tree re-validates before
            // anything commits. On failure the pending capture survives for
            // a retry (which preflights first anyway) and no commit, bind,
            // prune, or restore runs — the error names the offender instead
            // of silently omitting its content.
            outlet.borrow().preflight_tree()?;
            outlet.borrow_mut().reconcile_tree(runtime);
            Ok(output)
        }));
        let output = match driven {
            Ok(Ok(output)) => output,
            Ok(Err(error)) => {
                outlet.borrow().reset_signaled_tree();
                return Err(error);
            }
            Err(payload) => {
                outlet.borrow().reset_signaled_tree();
                std::panic::resume_unwind(payload);
            }
        };
        // Successful-but-stale output schedules its own follow-up: paint
        // may lag the bookkeeping by a frame, and the run_frame reset
        // wipes anything flagged mid-frame. Quiet frames schedule
        // nothing; errors never reach here.
        outlet.borrow().flag_stale_frames(runtime);
        Ok(output)
    }

    /// Captures a pending transition when none is outstanding and the
    /// active route moved since the last commit. The outgoing focus is
    /// read once into the pending slot — never into the records — alongside
    /// the attempt snapshot, so a later retry cannot overwrite it with
    /// intermediate focus, and abandoning the capture drops it without
    /// touching committed state.
    fn capture_transition(&mut self, runtime: &Runtime) {
        if self.pending.is_some() {
            return;
        }
        let active = self.navigator.current().map(|route| route.id);
        if active == self.presented {
            return;
        }
        let saved = self.presented.and_then(|previous| {
            self.navigator
                .lifetime_of(previous)
                .is_some()
                .then(|| self.focus.read_owned_focus(runtime, previous))
                .flatten()
        });
        self.pending = Some(PendingTransition {
            previous: self.presented,
            incoming: active,
            saved,
            attempt: FrameAttempt::capture(&self.navigator),
        });
    }

    /// Commits the pending transition when the frame actually presented
    /// it: the active route still matches the captured incoming route, the
    /// incoming lifetime is alive, and the attempt snapshot still covers
    /// the navigator — so the mounted content is the transition's own.
    /// Anything else — navigation mid-frame beyond order/child-only
    /// changes, or a removed incoming route — abandons the capture and
    /// re-captures fresh, leaving committed records alone. Returns whether
    /// a transition committed.
    fn commit_transition(&mut self, runtime: &mut Runtime) -> bool {
        let active = self.navigator.current().map(|route| route.id);
        let Some(pending) = self.pending.take() else {
            return false;
        };
        if active != pending.incoming
            || pending
                .incoming
                .is_some_and(|incoming| self.navigator.lifetime_of(incoming).is_none())
            || !pending.attempt.covers(&self.navigator)
        {
            // Stale capture (already taken above): re-capture fresh for the
            // route that actually won, without writing anything first.
            self.capture_transition(runtime);
            return false;
        }
        if let Some(previous) = pending.previous {
            self.focus.commit_saved(previous, pending.saved);
        }
        self.presented = active;
        self.pending = None;
        true
    }

    /// Drops records, bindings, and tags for permanently removed routes.
    /// Lifetime-end notifications already fired at commit time, so bound
    /// scopes were cancelled before their bindings drop here. Unmounted
    /// but live routes (covered, retained, disposal-unmounted) keep
    /// everything: removal is a lifetime event, not a mount event.
    fn prune_removed(&mut self) {
        self.focus.retain_mounted(&self.navigator);
        self.bindings
            .retain(|id, _| self.navigator.lifetime_of(*id).is_some());
        self.tags
            .borrow_mut()
            .retain(|id, _| self.navigator.lifetime_of(*id).is_some());
    }

    /// Binds every live route lacking a binding. Scope ownership follows
    /// the route lifetime — not mounted content: covered, retained, and
    /// even disposal-unmounted routes stay bound while alive, so their
    /// tasks outlive their pixels; only permanent removal ends them.
    /// Focus records are the mounted-content side of the integration
    /// (attributed through the live tree); bindings are the lifetime side
    /// (keyed by navigator state, including routes with no pixels).
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

    /// Restores on committed transitions; otherwise retries a pending
    /// restore only when it cannot displace valid focus (deferred content
    /// may have mounted or enabled through ordinary invalidation since).
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

    /// Promotes the in-flight composition into the published snapshot and
    /// adopts it into the pending attempt — without touching the outgoing
    /// save — so the commit judges built content, and retries describe
    /// their own build. Runs only on the success path (reconciliation),
    /// so failed composition publishes nothing; skipped builders simply
    /// re-promote their previous publication. Only mounting builders
    /// write the slot, and only the current attempt's receipt adopts: a
    /// stale receipt (failed attempt, predated capture) authorizes
    /// nothing even at a matching revision. Within the attempt, only
    /// newer-or-equal snapshots relabel. All values are immutable copies,
    /// so application callbacks running later in reconciliation cannot
    /// relabel built content.
    fn adopt_consumed(&mut self) {
        let Some((epoch, composing)) = self.composing.borrow().clone() else {
            return;
        };
        if epoch != self.epoch.get() {
            return;
        }
        if let Some(pending) = self.pending.as_mut()
            && composing.revision >= pending.attempt.revision
        {
            pending.attempt = composing.clone();
        }
        self.consumed = Some(composing);
    }

    /// Schedules follow-up frames for stale output through the runtime's
    /// own invalidation (`request_frame`) — but only for revisions never
    /// scheduled before. Successful output older than live navigation
    /// still requires a frame, and ordinary hosts must not poll
    /// [`Self::needs_frame`] to discover it; the edge trigger keeps
    /// unconsumable staleness (and every quiet frame) from scheduling
    /// anything. Errors return before reconciliation, so rejected stacks
    /// schedule no retry loop. Nested staleness wakes the root driver
    /// through the same cascade. The per-revision suppression loses no
    /// outstanding demand: failures and panics reset it, and every new
    /// navigation (including slot remounts) carries a new revision.
    fn flag_stale_frames(&self, runtime: &mut Runtime) {
        let live = self.navigator.revision();
        let stale = match &self.consumed {
            None => !self.navigator.routes().is_empty(),
            Some(consumed) => consumed.revision != live,
        };
        if stale && self.signaled.get() != live {
            self.signaled.set(live);
            runtime.request_frame();
        }
        for nested in self.live_nested() {
            nested.borrow().flag_stale_frames(runtime);
        }
    }

    /// Clears schedule memos across the tree. A failed present consumes
    /// nothing, so its suppression must not survive: the next success
    /// re-evaluates outstanding work from scratch.
    fn reset_signaled_tree(&self) {
        self.signaled.set(0);
        for nested in self.live_nested() {
            nested.borrow().reset_signaled_tree();
        }
    }

    /// Post-frame reconciliation: adopt the consumed snapshot, commit the
    /// pending transition against it, then prune, bind, and restore or
    /// retry. The commit runs before pruning so a just-committed record
    /// for a route removed mid-frame is dropped rather than resurrected.
    fn reconcile_presented(&mut self, runtime: &mut Runtime) {
        self.adopt_consumed();
        let transitioned = self.commit_transition(runtime);
        self.prune_removed();
        self.ensure_bindings();
        self.restore_or_retry(runtime, transitioned);
    }

    /// Bumps this outlet and every nested outlet, then captures pending
    /// transitions top-down. One call drives the whole outlet tree, so a
    /// single frame presents nested navigation without rendering the
    /// runtime once per outlet. The topology guarantees (acyclic,
    /// single-parent) make every participant reachable exactly once.
    fn begin_frame(&mut self, runtime: &Runtime) {
        self.epoch.set(self.epoch.get().wrapping_add(1));
        self.revision.set(self.revision.get().wrapping_add(1));
        self.capture_transition(runtime);
        for nested in self.live_nested() {
            nested.borrow_mut().begin_frame(runtime);
        }
    }

    /// Reconciles this outlet and every nested outlet after a frame,
    /// children first so the outer context wins focus ties
    /// deterministically. Dead nested handles prune here, keeping
    /// registrations bounded no matter how often hosts replace outlets.
    fn reconcile_tree(&mut self, runtime: &mut Runtime) {
        for nested in self.live_nested() {
            nested.borrow_mut().reconcile_tree(runtime);
        }
        self.reconcile_presented(runtime);
        self.nested
            .borrow_mut()
            .retain(|weak| weak.upgrade().is_some());
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
