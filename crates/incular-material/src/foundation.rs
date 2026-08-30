//! Shared Material state, style, density, colour, and theme primitives.
//!
//! The public surface deliberately follows Flutter's Material vocabulary while
//! keeping the implementation Rust-native. Components use these values as
//! immutable configuration and resolve them at build time; no component owns a
//! second copy of the state-property or theme system.

#[path = "foundation/helpers.rs"]
mod helpers;
#[path = "foundation/ink.rs"]
mod ink;
#[path = "foundation/input.rs"]
mod input;
#[path = "foundation/state.rs"]
mod state;
#[path = "foundation/surfaces.rs"]
mod surfaces;
#[path = "foundation/theme.rs"]
mod theme;
#[path = "foundation/theme_data.rs"]
mod theme_data;

pub use ink::*;
pub use input::*;
pub use state::*;
pub use surfaces::*;
pub use theme::*;
pub use theme_data::*;
