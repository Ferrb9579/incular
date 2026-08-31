//! Flutter-shaped Material button components.
//!
//! The retained input, focus, semantics, and repaint behavior is provided by
//! [`incular_controls::Button`].  This module owns the Material vocabulary and
//! the default/configuration layer around that headless primitive.  Keeping the
//! two layers separate means a button can be themed with [`ThemeData`] without
//! moving Material policy into `incular-widgets` or duplicating the controls
//! event implementation.

mod common;
mod icon_floating;
mod style;
mod variants;

pub use icon_floating::{FloatingActionButton, IconButton};
pub use style::{ButtonStyleConfig, StyleFrom, style_from};
pub use variants::{ElevatedButton, FilledButton, OutlinedButton, TextButton};
