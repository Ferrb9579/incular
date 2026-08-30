//! Platform-integrated widget descriptors.

use crate::{SizedBox, Widget};

mod menu;

pub use menu::{
    MemoryPlatformMenuDelegate, MenuDispatchResult, MenuItemId, MenuOwnerId,
    NoopPlatformMenuDelegate, PlatformMenu, PlatformMenuBar, PlatformMenuBarController,
    PlatformMenuBuildError, PlatformMenuCommand, PlatformMenuDelegate, PlatformMenuEntry,
    PlatformMenuItem, PlatformMenuItemGroup, PlatformMenuShortcut, PlatformMenuSnapshot,
    PlatformMenuSnapshotNode, PlatformMenuUpdate, ShortcutModifiers,
};
