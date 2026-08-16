//! Platform-neutral input values.

use crate::{Offset, Size};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PointerPhase {
    Move,
    Down,
    Up,
    Cancel,
}

/// Platform-neutral modifiers sampled with a keyboard event. `command` is a
/// semantic shortcut modifier: Control on Linux/Windows and Command on macOS.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Modifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    pub super_key: bool,
    pub command: bool,
}

/// Physical command keys understood by the first desktop input slice. Text is
/// deliberately absent: printable Unicode arrives through [`InputEvent::Text`]
/// or IME commit rather than a key-to-character mapping.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyCode {
    Tab,
    Enter,
    Escape,
    Backspace,
    Delete,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
    Home,
    End,
    PageUp,
    PageDown,
    KeyA,
    KeyC,
    KeyV,
    KeyX,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyEvent {
    pub code: KeyCode,
    pub pressed: bool,
    pub repeat: bool,
    pub modifiers: Modifiers,
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
    Scroll {
        delta: Offset,
    },
    Key(KeyEvent),
    Text(String),
    Ime(ImeEvent),
    WindowResized {
        size: Size,
        scale_factor: f64,
    },
}
