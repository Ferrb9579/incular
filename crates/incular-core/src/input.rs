//! Platform-neutral input values.

use crate::{Offset, Size};

/// Standardized keyboard values shared by every platform adapter.
///
/// [`Code`] identifies the physical key position; [`Key`] is the logical key
/// value after the operating system's layout processing.  Incular does not
/// maintain a parallel keyboard vocabulary.
pub use keyboard_types::{Code, Key, KeyState, KeyboardEvent, Location, Modifiers, NamedKey};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PointerPhase {
    /// A hover-capable pointer entered the native surface. No widget-local
    /// position is implied until a subsequent [`Self::Move`] arrives.
    Enter,
    Move,
    Down,
    Up,
    Cancel,
    /// A hover-capable pointer left the native surface.
    Exit,
}

/// Physical device category for pointer input shared by platform adapters and
/// raw widget routing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PointerDeviceKind {
    #[default]
    Mouse,
    Touch,
    Stylus,
    InvertedStylus,
    Trackpad,
    Unknown,
}

/// Edge/corner selected for a compositor-owned native window resize gesture.
///
/// This lives in `incular-core` because retained widgets can describe resize
/// hit regions without depending on a platform window implementation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WindowResizeDirection {
    East,
    North,
    NorthEast,
    NorthWest,
    South,
    SouthEast,
    SouthWest,
    West,
}

/// Primary-button bit in Incular's portable pointer-button mask.
///
/// Raw pointer metadata follows Flutter's button-mask convention so the value
/// is stable across platform adapters. Zero remains valid for legacy adapters
/// that do not publish button metadata.
pub const PRIMARY_POINTER_BUTTON: u32 = 1 << 0;
/// Secondary/right mouse button.
pub const SECONDARY_POINTER_BUTTON: u32 = 1 << 1;
/// Tertiary/middle mouse button.
pub const TERTIARY_POINTER_BUTTON: u32 = 1 << 2;
/// Conventional browser/navigation back button.
pub const BACK_POINTER_BUTTON: u32 = 1 << 3;
/// Conventional browser/navigation forward button.
pub const FORWARD_POINTER_BUTTON: u32 = 1 << 4;

/// Returns the portable mask for an additional mouse button.
///
/// `index == 0` is the first button beyond primary/secondary/tertiary/back/
/// forward. The `u32` event mask has room for 27 such buttons; larger native
/// button indices are intentionally unrepresentable instead of colliding with
/// an existing bit.
#[must_use]
pub const fn additional_pointer_button_mask(index: u16) -> Option<u32> {
    let bit = 5_u32 + index as u32;
    if bit < u32::BITS {
        Some(1_u32 << bit)
    } else {
        None
    }
}

/// IME composition is separate from committed text. Byte ranges always refer
/// to valid UTF-8 boundaries in the preedit string when supplied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImeEvent {
    Preedit {
        text: String,
        selection: Option<(usize, usize)>,
    },
    Commit(String),
    End,
}

#[derive(Clone, Debug, PartialEq)]
pub enum InputEvent {
    Pointer {
        phase: PointerPhase,
        position: Offset,
    },
    /// Identified pointer input for touch, pen, and multi-contact touchpads.
    /// The existing [`Self::Pointer`] form is retained as pointer `0` for
    /// source compatibility with mouse-only adapters.
    PointerWithId {
        /// Stable for the duration of one pointer sequence.
        pointer: u64,
        phase: PointerPhase,
        position: Offset,
    },
    /// Fully identified pointer input. The two legacy pointer forms remain
    /// available for adapters that do not expose device/button metadata.
    PointerWithMetadata {
        /// Stable for the duration of one pointer sequence.
        pointer: u64,
        /// Stable identifier for the physical device, when available.
        device: u64,
        /// Physical category of the source device.
        kind: PointerDeviceKind,
        /// Pressed-button bit mask; zero is used for hover.
        buttons: u32,
        /// Button whose state changed for a Down/Up event. `None` is used for
        /// movement, surface enter/exit, cancellation, and adapters that do not
        /// expose the changed native button.
        button: Option<u32>,
        phase: PointerPhase,
        position: Offset,
    },
    Scroll {
        delta: Offset,
    },
    Key(KeyboardEvent),
    Text(String),
    Ime(ImeEvent),
    WindowResized {
        size: Size,
        scale_factor: f64,
    },
}
