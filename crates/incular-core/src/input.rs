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
    Move,
    Down,
    Up,
    Cancel,
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
