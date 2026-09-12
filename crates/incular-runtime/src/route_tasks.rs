//! Binds task scopes to navigation route lifetimes.
//!
//! The binding is deliberately one-directional: the navigation crate knows
//! nothing about tasks, and this module adds no executor or cancellation
//! engine. It couples one existing [`TaskScope`] child to one neutral
//! [`RouteLifetime`](incular_navigation::RouteLifetime), so permanent route
//! removal cancels route-associated work through the scheduler machinery
//! that already discards completions of cancelled scopes.

use incular_navigation::{RouteLifetime, RouteLifetimeSubscription};

use super::tasks::TaskScope;

/// Couples one task scope to one navigation route lifetime.
///
/// [`RouteTaskBinding::bind`] creates a scope bounded by `parent` and
/// cancels it when the lifetime ends. Deactivation, keyed reorder, and
/// retained updates leave the scope alone because the lifetime survives
/// them; only permanent removal ends it. Cancelling `parent` — window
/// close, application shutdown — still cancels through the existing
/// parent-to-child ownership, and a binding made on an already-ended
/// lifetime yields an immediately cancelled scope.
///
/// Typical use: keep the binding beside the route's controller, spawn with
/// [`RuntimeSpawner::spawn_into_in`](super::tasks::RuntimeSpawner::spawn_into_in)
/// on [`Self::scope`], and let removal discard late completions instead of
/// updating a gone route.
///
/// Disposal policy: dropping the binding unregisters its removal callback,
/// so a later removal no longer cancels through this binding (detached),
/// while the scope itself follows ordinary [`TaskScope`] ownership —
/// including last-owner-drop cancellation. Retain the binding while the
/// route is mounted. Repeated removal and repeated [`TaskScope::cancel`]
/// are harmless no-ops through the idempotent lifetime and scope flags.
#[must_use]
pub struct RouteTaskBinding {
    scope: TaskScope,
    lifetime: RouteLifetime,
    _subscription: RouteLifetimeSubscription,
}

impl RouteTaskBinding {
    /// Binds a fresh scope bounded by `parent` to `lifetime`.
    pub fn bind(parent: &TaskScope, lifetime: &RouteLifetime) -> Self {
        let scope = parent.child();
        let scope_for_callback = scope.clone();
        let subscription = lifetime.on_ended(move || scope_for_callback.cancel());
        Self {
            scope,
            lifetime: lifetime.clone(),
            _subscription: subscription,
        }
    }

    /// Returns the route-bounded scope to spawn work in.
    #[must_use]
    pub fn scope(&self) -> &TaskScope {
        &self.scope
    }

    /// Returns the bound route lifetime.
    #[must_use]
    pub fn lifetime(&self) -> &RouteLifetime {
        &self.lifetime
    }
}
