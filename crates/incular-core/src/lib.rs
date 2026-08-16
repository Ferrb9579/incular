//! Renderer- and platform-independent primitives shared by Incular.

mod arena;
mod context;
mod geometry;
mod input;
mod key;
mod widget;

pub use arena::{Arena, ArenaId};
pub use context::{BuildContext, ConsumerId, ContextGuard, DependencySnapshot, Signal};
pub use geometry::{Color, DirtyFlags, Offset, Rect, Size, Transform};
pub use input::{ImeEvent, InputEvent, KeyCode, KeyEvent, Modifiers, PointerPhase};
pub use key::{Key, KeyHandle, KeyId, KeyObject, LocalKey, StringKey, UniqueKey, ValueKey};
pub use widget::{BuildableWidget, Widget};
