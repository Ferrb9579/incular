//! Material application-shell descriptors.
//!
//! This module keeps the shell vocabulary that sits above individual Material
//! controls. It intentionally composes the retained `Widget` primitives and
//! the shared controls state machine; there is no second layout or event
//! implementation here. The navigation crate can later attach a
//! `Navigator`/`Overlay` to the route hooks without changing these widgets.

mod environment;
mod material_app;
mod navigation;
mod shell;

pub use environment::{MaterialPointerDevice, MaterialScrollBehavior};
pub use material_app::MaterialApp;
pub use navigation::{
    NavigationDrawer, NavigationDrawerDestination, NavigationRail, NavigationRailDestination,
};
pub use shell::{
    BackButton, BackButtonIcon, BottomAppBar, CloseButton, CloseButtonIcon, Drawer, DrawerButton,
    DrawerButtonIcon, DrawerHeader, EndDrawerButton, EndDrawerButtonIcon, ScaffoldMessenger,
    ScaffoldMessengerController,
};
