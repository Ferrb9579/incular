//! Platform-neutral input values.

use crate::{Offset, Size};

/// Pressure normalized to the closed interval `[0, 1]`.
///
/// Native adapters normalize once at their boundary. Values outside a native
/// device's documented range are clamped, while non-finite values and invalid
/// ranges remain unavailable instead of entering retained input state.
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
pub struct NormalizedPressure(f32);

impl NormalizedPressure {
    #[must_use]
    pub fn new(value: f64) -> Option<Self> {
        value
            .is_finite()
            .then(|| Self(value.clamp(0.0, 1.0) as f32))
    }

    #[must_use]
    pub fn from_range(value: f64, maximum: f64) -> Option<Self> {
        (value.is_finite() && maximum.is_finite() && maximum > 0.0)
            .then(|| Self((value / maximum).clamp(0.0, 1.0) as f32))
    }

    #[must_use]
    pub const fn get(self) -> f32 {
        self.0
    }
}

/// Portable stylus orientation derived from native pen tilt.
///
/// `altitude` is radians above the tablet plane: `0` is parallel and `PI/2`
/// is perpendicular. `azimuth` is radians in `[0, TAU)`, clockwise from the
/// positive logical X axis in Incular's screen coordinate system (+Y down).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StylusOrientation {
    pub altitude: f32,
    pub azimuth: f32,
}

impl StylusOrientation {
    /// Converts Windows-style signed X/Y tilt angles (degrees from the surface
    /// normal) into Incular's altitude/azimuth representation.
    #[must_use]
    pub fn from_tilt_degrees(tilt_x: f64, tilt_y: f64) -> Option<Self> {
        if !tilt_x.is_finite()
            || !tilt_y.is_finite()
            || !(-90.0..=90.0).contains(&tilt_x)
            || !(-90.0..=90.0).contains(&tilt_y)
        {
            return None;
        }
        let x = tilt_x.to_radians().tan();
        let y = tilt_y.to_radians().tan();
        if !x.is_finite() || !y.is_finite() {
            return None;
        }
        let altitude = 1.0_f64.atan2(x.hypot(y));
        let azimuth = y.atan2(x).rem_euclid(std::f64::consts::TAU);
        Some(Self {
            altitude: altitude as f32,
            azimuth: azimuth as f32,
        })
    }
}

/// Stylus-only sample data with semantics shared by supported native backends.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StylusMetadata {
    pub orientation: Option<StylusOrientation>,
    /// Whether the pen's barrel/secondary switch is currently depressed.
    pub barrel_button: bool,
}

/// Optional high-fidelity data attached to one pointer sample.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PointerSampleMetadata {
    pub pressure: Option<NormalizedPressure>,
    pub stylus: Option<StylusMetadata>,
}

impl PointerSampleMetadata {
    pub const EMPTY: Self = Self {
        pressure: None,
        stylus: None,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TrackpadGesturePhase {
    Started,
    Updated,
    Ended,
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TrackpadGestureKind {
    Pinch,
    Rotation,
    Pan,
    SmartMagnify,
    Pressure,
}

/// Aggregate native trackpad gesture. These events deliberately do not contain
/// synthetic touch contacts: platforms that expose only magnification,
/// rotation, or force keep those aggregate semantics intact.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TrackpadGesture {
    Pinch {
        device: u64,
        phase: TrackpadGesturePhase,
        magnification_delta: f32,
    },
    Rotation {
        device: u64,
        phase: TrackpadGesturePhase,
        /// Counter-clockwise rotation in radians for this native update.
        delta_radians: f32,
    },
    Pan {
        device: u64,
        phase: TrackpadGesturePhase,
        delta: Offset,
    },
    SmartMagnify {
        device: u64,
    },
    Pressure {
        device: u64,
        pressure: NormalizedPressure,
        stage: i64,
    },
}

impl TrackpadGesture {
    #[must_use]
    pub const fn device(self) -> u64 {
        match self {
            Self::Pinch { device, .. }
            | Self::Rotation { device, .. }
            | Self::Pan { device, .. }
            | Self::SmartMagnify { device }
            | Self::Pressure { device, .. } => device,
        }
    }

    #[must_use]
    pub const fn kind(self) -> TrackpadGestureKind {
        match self {
            Self::Pinch { .. } => TrackpadGestureKind::Pinch,
            Self::Rotation { .. } => TrackpadGestureKind::Rotation,
            Self::Pan { .. } => TrackpadGestureKind::Pan,
            Self::SmartMagnify { .. } => TrackpadGestureKind::SmartMagnify,
            Self::Pressure { .. } => TrackpadGestureKind::Pressure,
        }
    }

    #[must_use]
    pub const fn phase(self) -> Option<TrackpadGesturePhase> {
        match self {
            Self::Pinch { phase, .. } | Self::Rotation { phase, .. } | Self::Pan { phase, .. } => {
                Some(phase)
            }
            Self::SmartMagnify { .. } | Self::Pressure { .. } => None,
        }
    }
}

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
        /// Optional pressure/orientation data supplied by the native device.
        sample: PointerSampleMetadata,
        phase: PointerPhase,
        position: Offset,
    },
    /// Native aggregate trackpad gesture. No fake pointer contacts are created
    /// from this event.
    TrackpadGesture(TrackpadGesture),
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
