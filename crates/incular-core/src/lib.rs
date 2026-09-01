//! Renderer- and platform-independent primitives shared by Incular.

mod arena;
mod context;
mod geometry;
/// Platform-neutral input event values and the standardized keyboard
/// vocabulary. `Key` remains the logical keyboard key inside this module;
/// the crate root re-exports it as [`KeyboardKey`] to avoid colliding with the
/// retained widget-key trait.
pub mod input;
mod key;
mod restoration;
mod widget;

pub use arena::{Arena, ArenaId};
pub use context::{BuildContext, ConsumerId, ContextGuard, DependencySnapshot, Signal};
pub use geometry::{
    ChangeImpact, Color, DirtyFlags, HslColor, HsvColor, Invalidation, Lerp, Offset, Rect, Size,
    Transform,
};
pub use input::{
    Code, ImeEvent, InputEvent, Key as KeyboardKey, KeyState, KeyboardEvent, Location, Modifiers,
    NamedKey, PRIMARY_POINTER_BUTTON, PointerDeviceKind, PointerPhase, WindowResizeDirection,
};
pub use key::{Key, KeyHandle, KeyId, KeyObject, LocalKey, StringKey, UniqueKey, ValueKey};
pub use restoration::{RestorationBackend, RestorationKey, RestorationKeyError, RestorationScope};
pub use widget::{BuildableWidget, Widget};
