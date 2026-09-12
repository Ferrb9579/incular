//! Per-route focus save/restore policy over navigation lifetimes.
//!
//! The state is deliberately pull-driven: it owns the save records and the
//! restore validation, while the mount integration drives it — save the
//! outgoing route on deactivation, restore the incoming route on
//! reactivation, and drop records through [`RouteFocusState::forget`] or
//! [`RouteFocusState::retain_mounted`] on permanent removal. There is no
//! background watcher and no second focus engine; restoration funnels
//! through the runtime's own focus slot.

use std::{collections::HashMap, rc::Rc};

use incular_navigation::{Navigator, RouteId};
use incular_widgets::internal::{ElementId, WidgetTree};

use super::frame::Runtime;

/// Maps a mounted element to the route that owns it.
///
/// The mount integration supplies this attribution from mounted structure
/// (keys, scope containment); the state never guesses ownership, so an
/// element without an owner is never restored. Route identities are
/// navigator-scoped, so use one state per [`Navigator`].
pub type RouteFocusOwner = Rc<dyn Fn(&WidgetTree, ElementId) -> Option<RouteId>>;

/// Remembers which element was focused per route and restores it on return.
///
/// Three record states, three restore behaviors:
/// - No saved information (never saved, forgotten, or saved with nothing
///   owned): restore leaves focus untouched, except for genuinely orphaned
///   focus (see below).
/// - Previously saved focus that is no longer eligible: the deterministic
///   fallback clears focus, but only when current focus is itself orphaned —
///   an invalid saved record never steals unrelated focus.
/// - Focus currently owned by another route: saves reject it (a route
///   records only its own focus) and restores leave it alone while its
///   owner stays mounted.
///
/// Eligibility reuses the tree's authoritative focus rule instead of a
/// parallel predicate: the target must be live in the arena (detached
/// elements fail here, including reused slots whose generation changed)
/// and a member of [`WidgetTree::focusable_elements`], which already
/// encodes enabled state, `ExcludeFocus`, and inactive subtrees
/// (`IndexedStack` non-indexed children, `descendants_are_focusable`).
/// Hidden-but-mounted subtrees (`Visibility`, `Offstage`) stay eligible by
/// framework design — they retain focus — while ownership comes from the
/// [`RouteFocusOwner`] oracle.
///
/// Route identity and element identity stay separate throughout: records
/// are keyed by [`RouteId`], targets are [`ElementId`] values validated
/// against the live tree on every restore, never by index or name.
///
/// Dropping the state forgets all records; later restores are impossible,
/// which is the teardown policy.
pub struct RouteFocusState {
    saved: HashMap<RouteId, Option<ElementId>>,
    owner_of: RouteFocusOwner,
}

impl RouteFocusState {
    /// Creates focus state attributing elements through `owner_of`.
    pub fn new(owner_of: impl Fn(&WidgetTree, ElementId) -> Option<RouteId> + 'static) -> Self {
        Self {
            saved: HashMap::new(),
            owner_of: Rc::new(owner_of),
        }
    }

    /// Records the currently focused element as `route`'s focus. Call when
    /// `route` deactivates. Only focus owned by `route` is recorded: focus
    /// owned elsewhere (or by nothing) records no information rather than
    /// a foreign snapshot, so retained records never lie about ownership.
    /// Absent and rejected records restore identically (orphan sweep), so
    /// validation is record hygiene, not a second restore predicate.
    pub fn save_focused(&mut self, runtime: &Runtime, route: RouteId) {
        let focused = runtime.focused_element().filter(|id| {
            runtime.tree().element_exists(*id)
                && (self.owner_of)(runtime.tree(), *id) == Some(route)
        });
        self.saved.insert(route, focused);
    }

    /// Restores `route`'s saved focus into `runtime`. Call when `route`
    /// reactivates. A removed `route` restores nothing even with a leftover
    /// record, so cleanup never depends on the host having forgotten first.
    pub fn restore_saved(&mut self, runtime: &mut Runtime, navigator: &Navigator, route: RouteId) {
        if navigator.lifetime_of(route).is_none() {
            return;
        }
        if let Some(saved) = self.saved.get(&route).copied().flatten()
            && Self::eligible(runtime, &self.owner_of, route, saved)
        {
            if runtime.focused_element() != Some(saved) {
                runtime.set_focus(Some(saved));
            }
            return;
        }
        if let Some(current) = runtime.focused_element()
            && Self::orphaned(runtime, navigator, &self.owner_of, current)
        {
            runtime.set_focus(None);
        }
    }

    /// Eligibility is the tree's own rule plus attribution: live, focusable
    /// (hence enabled and in an active subtree), and owned by `route`.
    fn eligible(
        runtime: &Runtime,
        owner_of: &RouteFocusOwner,
        route: RouteId,
        id: ElementId,
    ) -> bool {
        runtime.tree().element_exists(id)
            && runtime.tree().focusable_elements().contains(&id)
            && owner_of(runtime.tree(), id) == Some(route)
    }

    /// Orphaned focus has no live owner to keep it: a dead or unfocusable
    /// target, or a target whose owning route is no longer mounted. Focus
    /// owned by a mounted route — or by nothing attributable — is left
    /// alone.
    fn orphaned(
        runtime: &Runtime,
        navigator: &Navigator,
        owner_of: &RouteFocusOwner,
        id: ElementId,
    ) -> bool {
        !runtime.tree().element_exists(id)
            || !runtime.tree().focusable_elements().contains(&id)
            || owner_of(runtime.tree(), id)
                .is_some_and(|owner| navigator.lifetime_of(owner).is_none())
    }

    /// Drops `route`'s record on permanent removal. A removed route never
    /// restores: without this call a stale record would outlive its route.
    pub fn forget(&mut self, route: RouteId) {
        self.saved.remove(&route);
    }

    /// Drops records for routes no longer mounted in `navigator`. Call
    /// after declarative reconciliation, which can remove routes without
    /// an explicit per-route removal call.
    pub fn retain_mounted(&mut self, navigator: &Navigator) {
        self.saved
            .retain(|route, _| navigator.lifetime_of(*route).is_some());
    }
}
