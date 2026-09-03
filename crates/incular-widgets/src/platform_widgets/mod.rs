//! Platform-integrated widget descriptors.
//!
//! This module owns platform-facing widget configuration and the delegate
//! contracts used to hand serializable state to a native shell. Native menu
//! resources and OS event-loop ownership remain in the platform crates; this
//! layer retains only framework callbacks and declarative menu state.

use crate::{SizedBox, Widget};

mod menu;
pub(crate) use menu::{MenuCallbackSet, build_snapshot};

#[doc(hidden)]
pub use menu::PlatformMenuBinding;
pub(crate) use menu::PlatformMenuRetainedMarker;
pub use menu::{
    MenuDispatchResult, MenuItemId, MenuOwnerId, NoopPlatformMenuDelegate, PlatformMenu,
    PlatformMenuBar, PlatformMenuBarController, PlatformMenuBuildError, PlatformMenuDelegate,
    PlatformMenuEntry, PlatformMenuEvent, PlatformMenuItem, PlatformMenuItemGroup,
    PlatformMenuShortcut, PlatformMenuSnapshot, PlatformMenuSnapshotNode, PlatformMenuUpdate,
    ShortcutModifiers,
};
