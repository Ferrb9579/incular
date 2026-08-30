//! Explicit root and unmanaged restoration-scope ownership.

use crate::Widget;
use incular_core::{RestorationKey, RestorationScope as CoreRestorationScope};

#[derive(Clone)]
struct RestorationEnvironment {
    scope: Option<CoreRestorationScope>,
}

/// Reads the nearest explicit restoration scope while a deferred descendant
/// is being materialized.
#[must_use]
pub fn current_restoration_scope() -> Option<CoreRestorationScope> {
    if let Some(environment) = crate::tree::current_build_environment::<RestorationEnvironment>() {
        environment.scope
    } else {
        crate::tree::current_build_environment::<CoreRestorationScope>()
    }
}

/// Establishes an explicitly owned restoration scope without asking the
/// runtime to register or synchronize it.
#[derive(Clone)]
pub struct UnmanagedRestorationScope {
    scope: Option<CoreRestorationScope>,
    child: Widget,
}

impl UnmanagedRestorationScope {
    /// Creates a disabled unmanaged scope.  Use [`Self::with_scope`] when the
    /// caller owns an actual runtime/core restoration location.
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            scope: None,
            child: child.into(),
        }
    }

    /// Creates an unmanaged scope around an explicitly owned core scope.
    #[must_use]
    pub fn with_scope(scope: CoreRestorationScope, child: impl Into<Widget>) -> Self {
        Self {
            scope: Some(scope),
            child: child.into(),
        }
    }

    /// Creates an unmanaged scope from an optional scope; `None` deliberately
    /// shadows an outer scope and disables restoration below this node.
    #[must_use]
    pub fn with_optional_scope(
        scope: Option<CoreRestorationScope>,
        child: impl Into<Widget>,
    ) -> Self {
        Self {
            scope,
            child: child.into(),
        }
    }

    /// Supplies/replaces the explicitly owned scope.
    #[must_use]
    pub fn scope(mut self, scope: Option<CoreRestorationScope>) -> Self {
        self.scope = scope;
        self
    }

    /// Returns the owned scope, if restoration is enabled.
    #[must_use]
    pub fn restoration_scope(&self) -> Option<CoreRestorationScope> {
        self.scope.clone()
    }

    /// Returns the wrapped child descriptor.
    #[must_use]
    pub fn child(&self) -> &Widget {
        &self.child
    }
}

impl From<UnmanagedRestorationScope> for Widget {
    fn from(value: UnmanagedRestorationScope) -> Self {
        Widget::environment_scope(RestorationEnvironment { scope: value.scope }, value.child)
    }
}

/// Establishes a root restoration scope and claims a stable child path.
///
/// The explicit root scope is preferred only when no outer restoration scope
/// is visible.  This mirrors Flutter's nested-root behavior and lets the
/// runtime supply an ambient root while embedded callers provide one directly.
#[derive(Clone)]
pub struct RootRestorationScope {
    root_scope: Option<CoreRestorationScope>,
    restoration_id: Option<String>,
    child: Widget,
}

impl RootRestorationScope {
    /// Creates a root scope that resolves its root from the nearest ambient
    /// scope.  Without an ambient or explicit scope, restoration is disabled.
    #[must_use]
    pub fn new(restoration_id: impl Into<String>, child: impl Into<Widget>) -> Self {
        Self {
            root_scope: None,
            restoration_id: Some(restoration_id.into()),
            child: child.into(),
        }
    }

    /// Creates a root scope with explicit scope ownership.
    #[must_use]
    pub fn with_scope(
        root_scope: CoreRestorationScope,
        restoration_id: impl Into<String>,
        child: impl Into<Widget>,
    ) -> Self {
        Self {
            root_scope: Some(root_scope),
            restoration_id: Some(restoration_id.into()),
            child: child.into(),
        }
    }

    /// Creates a root scope from an optional owned root; `None` leaves lookup
    /// to an ambient runtime scope.
    #[must_use]
    pub fn with_optional_scope(
        root_scope: Option<CoreRestorationScope>,
        restoration_id: Option<impl Into<String>>,
        child: impl Into<Widget>,
    ) -> Self {
        Self {
            root_scope,
            restoration_id: restoration_id.map(Into::into),
            child: child.into(),
        }
    }

    /// Replaces explicit root ownership.
    #[must_use]
    pub fn scope(mut self, root_scope: CoreRestorationScope) -> Self {
        self.root_scope = Some(root_scope);
        self
    }

    /// Disables restoration below this root.
    #[must_use]
    pub fn without_restoration_id(mut self) -> Self {
        self.restoration_id = None;
        self
    }

    /// Returns the declared restoration identifier.
    #[must_use]
    pub fn restoration_id(&self) -> Option<&str> {
        self.restoration_id.as_deref()
    }

    /// Returns the explicitly owned root scope, if any.
    #[must_use]
    pub fn root_scope(&self) -> Option<CoreRestorationScope> {
        self.root_scope.clone()
    }

    /// Returns the wrapped child descriptor.
    #[must_use]
    pub fn child(&self) -> &Widget {
        &self.child
    }
}

impl From<RootRestorationScope> for Widget {
    fn from(value: RootRestorationScope) -> Self {
        let explicit_root = value.root_scope;
        let restoration_id = value.restoration_id;
        let child = value.child;
        Widget::layout_builder(move |_| {
            let root = current_restoration_scope().or_else(|| explicit_root.clone());
            let scope = match (root, restoration_id.as_deref()) {
                (Some(root), Some(restoration_id)) => RestorationKey::new(restoration_id)
                    .ok()
                    .map(|key| root.child(key)),
                _ => None,
            };
            Widget::environment_scope(RestorationEnvironment { scope }, child.clone())
        })
    }
}
