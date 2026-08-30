//! Retained navigation, modal, page-storage, and restoration adapters.
//!
//! The navigation crate owns route stacks and overlay entries.  This module
//! owns the widget-side lifecycle: retained callbacks are registered while a
//! scope is materialized and are removed when that retained builder is
//! dropped.  The small JSON pop-result boundary keeps this crate independent
//! from `incular-navigation` (which already depends on Widgets).

mod back_dispatch;
mod barrier;
mod page_storage;
mod pop_scopes;
mod restoration;

pub use back_dispatch::{
    BackButtonDispatcher, BackDispatchReport, BackHandlerResult, BackRegistration, PopAttempt,
};
pub use barrier::AnimatedModalBarrier;
pub use page_storage::{
    PageStorage, PageStorageBucket, PageStorageIdentifier, PageStorageKey,
    current_page_storage_bucket,
};
pub use pop_scopes::{
    BackButtonListener, NavigatorPopHandler, NavigatorPopHandlerController, PopScope,
    PopScopeController,
};
pub use restoration::{RootRestorationScope, UnmanagedRestorationScope, current_restoration_scope};
