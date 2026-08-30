//! Material menu and dropdown components.
//!
//! This module owns the Material vocabulary and default composition for menus.
//! Hit testing, retained overlays, buttons, text editing, and type-ahead state
//! remain in the lower-level crates.  The types intentionally accept ordinary
//! [`incular_widgets::Widget`] values so an application can use custom menu rows and icons
//! without opting into a second menu renderer.

mod anchor;
mod controls;
mod dropdown;
mod popup;
mod style;

#[allow(unused_imports)]
pub use anchor::{MenuAnchor, MenuAnchorBuilder};
#[allow(unused_imports)]
pub use controls::{
    MenuBar, MenuBarBuilder, MenuController, MenuItemButton, MenuItemButtonBuilder, SubmenuButton,
    SubmenuButtonBuilder,
};
#[allow(unused_imports)]
pub use dropdown::{
    DropdownButton, DropdownButtonBuilder, DropdownButtonFormField, DropdownButtonFormFieldBuilder,
    DropdownButtonHideUnderline, DropdownButtonHideUnderlineBuilder, DropdownButtonTypedBuilder,
    DropdownMenu, DropdownMenuBuilder, DropdownMenuDecorationBuilder, DropdownMenuEntry,
    DropdownMenuEntryBuilder, DropdownMenuFormField, DropdownMenuItem, FilterCallback,
    SearchCallback,
};
#[allow(unused_imports)]
pub use popup::{
    CheckedPopupMenuItem, CheckedPopupMenuItemBuilder, PopupMenuButton, PopupMenuButtonBuilder,
    PopupMenuDivider, PopupMenuDividerBuilder, PopupMenuEntry, PopupMenuItem, PopupMenuItemBuilder,
};
#[allow(unused_imports)]
pub use style::{
    DropdownMenuThemeData, DropdownMenuThemeDataBuilder, MenuStyle, MenuStyleBuilder,
    MenuThemeData, MenuThemeDataBuilder, PopupMenuThemeData, PopupMenuThemeDataBuilder,
};
