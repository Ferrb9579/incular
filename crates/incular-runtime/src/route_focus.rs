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
use incular_widgets::internal::ElementId;

use super::frame::Runtime;

/// Maps a mounted element to the route that owns it.
///
/// The mount integration supplies this attribution; the state never guesses
/// ownership, so an element without an owner is never restored. Route
/// identities are navigator-scoped, so use one state per [`Navigator`].
pub type RouteFocusOwner = Rc<dyn Fn(ElementId) -> Option<RouteId>>;

/// Remembers which element was focused per route and restores it on return.
///
/// Restore is fail-closed: the saved target is restored only while it
/// remains mounted (generational identity — a reused arena slot with a new
/// generation does not match), focusable, enabled, and owned by the
/// reactivated route. Anything else — including a record for a route that
/// never saved — resolves deterministically: an invalid target clears
/// focus (no route inherits another route's focus, matching the
/// unmounted-focus rule), while a missing record leaves focus untouched.
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
    pub fn new(owner_of: impl Fn(ElementId) -> Option<RouteId> + 'static) -> Self {
        Self {
            saved: HashMap::new(),
            owner_of: Rc::new(owner_of),
        }
    }

    /// Records the currently focused element as `route`'s focus. Call when
    /// `route` deactivates. A dead or absent focus records nothing to
    /// restore, which later restores treat as no information.
    pub fn save_focused(&mut self, runtime: &Runtime, route: RouteId) {
        let focused = runtime
            .focused_element()
            .filter(|id| runtime.tree().element_exists(*id));
        self.saved.insert(route, focused);
    }

    /// Restores `route`'s saved focus into `runtime`. Call when `route`
    /// reactivates.
    pub fn restore_saved(&mut self, runtime: &mut Runtime, route: RouteId) {
        let Some(saved) = self.saved.get(&route).copied().flatten() else {
            // No record: no information, no action. A fresh route must not
            // steal focus it never owned.
            return;
        };
        let usable = runtime.tree().element_exists(saved)
            && runtime.tree().focusable_elements().contains(&saved)
            && (self.owner_of)(saved) == Some(route);
        if usable {
            if runtime.focused_element() != Some(saved) {
                runtime.set_focus(Some(saved));
            }
        } else if runtime.focused_element().is_some() {
            runtime.set_focus(None);
        }
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
