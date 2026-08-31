//! Material-shaped component compositions.
//!
//! This module is intentionally a thin layer over Incular's retained widget
//! and controls crates.  It owns Material vocabulary and composition policy,
//! while layout, input dispatch, retained state, and painting remain in the
//! lower layers.  The module is kept separate from `lib.rs` so the public
//! re-export policy can evolve without changing the widget primitives.

mod app_shell;
mod chips;
mod common;
mod list_items;
mod navigation;
mod progress;
mod surfaces;

pub use app_shell::{AppBar, Scaffold, SliverAppBar};
pub use chips::{ActionChip, Chip, ChoiceChip, FilterChip, InputChip, RawChip};
pub use list_items::{Badge, CheckboxListTile, ListTile, RadioListTile, SwitchListTile};
pub use navigation::{
    BottomNavigationBar, BottomNavigationBarItem, NavigationBar, NavigationDestination,
};
pub use progress::{CircularProgressIndicator, LinearProgressIndicator};
pub use surfaces::{Card, CircleAvatar, Divider, Surface, VerticalDivider};
