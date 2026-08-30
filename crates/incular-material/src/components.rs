//! Material-shaped component compositions.
//!
//! This module is intentionally a thin layer over Incular's retained widget
//! and controls crates.  It owns Material vocabulary and composition policy,
//! while layout, input dispatch, retained state, and painting remain in the
//! lower layers.  The module is kept separate from `lib.rs` so the public
//! re-export policy can evolve without changing the widget primitives.

#[path = "components/app_shell.rs"]
mod app_shell;
#[path = "components/chips.rs"]
mod chips;
#[path = "components/common.rs"]
mod common;
#[path = "components/list_items.rs"]
mod list_items;
#[path = "components/navigation.rs"]
mod navigation;
#[path = "components/progress.rs"]
mod progress;
#[path = "components/surfaces.rs"]
mod surfaces;

pub use app_shell::{AppBar, Scaffold, SliverAppBar};
pub use chips::{ActionChip, Chip, ChoiceChip, FilterChip, InputChip, RawChip};
pub use list_items::{Badge, CheckboxListTile, ListTile, RadioListTile, SwitchListTile};
pub use navigation::{
    BottomNavigationBar, BottomNavigationBarItem, NavigationBar, NavigationDestination,
};
pub use progress::{CircularProgressIndicator, LinearProgressIndicator};
pub use surfaces::{Card, CircleAvatar, Divider, Surface, VerticalDivider};
