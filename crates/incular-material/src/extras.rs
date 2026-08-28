#![allow(non_upper_case_globals)]

//! Small, dependency-free Material vocabulary shared by components.
//!
//! These values mirror the public names and semantics of Flutter's Material
//! constants without reproducing Dart's static class mechanics.  Components
//! consume the ordinary Rust enums/records directly and applications can use
//! the constants when defining themes or tests.

use incular_core::Color;
use std::time::Duration;
use typed_builder::TypedBuilder;

/// Selects which theme a Material application follows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
}

impl ThemeMode {
    #[must_use]
    pub const fn is_system(self) -> bool {
        matches!(self, Self::System)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BottomNavigationBarType {
    #[default]
    Fixed,
    Shifting,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BottomNavigationBarLandscapeLayout {
    #[default]
    Spread,
    Centered,
    Linear,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonTextTheme {
    #[default]
    Normal,
    Accent,
    Primary,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonBarLayoutBehavior {
    #[default]
    Constrained,
    Padded,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DynamicSchemeVariant {
    #[default]
    TonalSpot,
    Fidelity,
    Monochrome,
    Neutral,
    Vibrant,
    Expressive,
    Content,
    Rainbow,
    FruitSalad,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DrawerAlignment {
    #[default]
    Start,
    End,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DropdownMenuCloseBehavior {
    #[default]
    All,
    SelfOnly,
    None,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CollapseMode {
    #[default]
    Parallax,
    Pin,
    None,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StretchMode {
    #[default]
    ZoomBackground,
    BlurBackground,
    FadeTitle,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ListTileControlAffinity {
    #[default]
    Platform,
    Leading,
    Trailing,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ListTileStyle {
    #[default]
    List,
    Drawer,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ListTileTitleAlignment {
    ThreeLine,
    #[default]
    TitleHeight,
    Top,
    Center,
    Bottom,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NavigationDestinationLabelBehavior {
    #[default]
    AlwaysShow,
    AlwaysHide,
    OnlyShowSelected,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NavigationRailLabelType {
    #[default]
    None,
    Selected,
    All,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PopupMenuPosition {
    #[default]
    Over,
    Under,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RefreshIndicatorStatus {
    #[default]
    Drag,
    Armed,
    Snap,
    Refresh,
    Done,
    Canceled,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RefreshIndicatorTriggerMode {
    Anywhere,
    #[default]
    OnEdge,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SliderInteraction {
    #[default]
    TapAndSlide,
    TapOnly,
    SlideOnly,
    SlideThumb,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ShowValueIndicator {
    #[default]
    OnlyForDiscrete,
    OnlyForContinuous,
    Always,
    Never,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SnackBarBehavior {
    #[default]
    Fixed,
    Floating,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SnackBarClosedReason {
    Action,
    Dismiss,
    Swipe,
    #[default]
    Hide,
    Timeout,
    Remove,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StepState {
    #[default]
    Indexed,
    Editing,
    Complete,
    Disabled,
    Error,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StepperType {
    #[default]
    Vertical,
    Horizontal,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TabAlignment {
    Start,
    StartOffset,
    #[default]
    Fill,
    Center,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TabBarIndicatorSize {
    #[default]
    Tab,
    Label,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TabIndicatorAnimation {
    #[default]
    Linear,
    Elastic,
}

/// Keyboard/input configuration vocabulary used by Material text fields.
/// Platform adapters may translate these stable values to native IME hints.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextInputType {
    #[default]
    Text,
    Multiline,
    Number,
    Phone,
    Datetime,
    EmailAddress,
    Url,
    VisiblePassword,
    Name,
    StreetAddress,
    None,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextInputAction {
    #[default]
    Unspecified,
    None,
    Done,
    Go,
    Search,
    Send,
    Next,
    Previous,
    Continue,
    Join,
    Route,
    EmergencyCall,
    Newline,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DayPeriod {
    #[default]
    Am,
    Pm,
}

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TimeOfDayFormat {
    #[default]
    HH_colon_mm,
    HH_dot_mm,
    frenchCanadian,
    a_space_h_colon_mm,
    H_colon_mm,
    h_colon_mm_space_a,
}

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HourFormat {
    HH,
    H,
    #[default]
    h,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TimePickerEntryMode {
    #[default]
    Dial,
    Input,
    DialOnly,
    InputOnly,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScriptCategory {
    #[default]
    EnglishLike,
    Dense,
    Tall,
}

/// A named timing curve represented by its cubic-bezier control points.
#[derive(Clone, Copy, Debug, PartialEq, TypedBuilder)]
pub struct EasingCurve {
    #[builder(setter(into))]
    pub x1: f32,
    #[builder(setter(into))]
    pub y1: f32,
    #[builder(setter(into))]
    pub x2: f32,
    #[builder(setter(into))]
    pub y2: f32,
}

impl EasingCurve {
    #[must_use]
    pub const fn new(x1: f32, y1: f32, x2: f32, y2: f32) -> Self {
        Self { x1, y1, x2, y2 }
    }
}

/// Material 3 easing curves.  Rendering/animation systems may convert these
/// control points to their native curve representation.
pub struct Easing;

impl Easing {
    pub const STANDARD: EasingCurve = EasingCurve::new(0.2, 0.0, 0.0, 1.0);
    pub const STANDARD_ACCELERATE: EasingCurve = EasingCurve::new(0.3, 0.0, 1.0, 1.0);
    pub const STANDARD_DECELERATE: EasingCurve = EasingCurve::new(0.0, 0.0, 0.0, 1.0);
    pub const EMPHASIZED: EasingCurve = EasingCurve::new(0.2, 0.0, 0.0, 1.0);
    pub const EMPHASIZED_ACCELERATE: EasingCurve = EasingCurve::new(0.3, 0.0, 0.8, 0.15);
    pub const EMPHASIZED_DECELERATE: EasingCurve = EasingCurve::new(0.05, 0.7, 0.1, 1.0);
}

/// Standard Material transition durations.
pub struct Durations;

impl Durations {
    pub const SHORT_1: Duration = Duration::from_millis(50);
    pub const SHORT_2: Duration = Duration::from_millis(100);
    pub const SHORT_3: Duration = Duration::from_millis(150);
    pub const SHORT_4: Duration = Duration::from_millis(200);
    pub const MEDIUM_1: Duration = Duration::from_millis(250);
    pub const MEDIUM_2: Duration = Duration::from_millis(300);
    pub const MEDIUM_3: Duration = Duration::from_millis(350);
    pub const MEDIUM_4: Duration = Duration::from_millis(400);
    pub const LONG_1: Duration = Duration::from_millis(450);
    pub const LONG_2: Duration = Duration::from_millis(500);
    pub const LONG_3: Duration = Duration::from_millis(550);
    pub const LONG_4: Duration = Duration::from_millis(600);
    pub const EXTRA_LONG_1: Duration = Duration::from_millis(700);
    pub const EXTRA_LONG_2: Duration = Duration::from_millis(800);
    pub const EXTRA_LONG_3: Duration = Duration::from_millis(900);
    pub const EXTRA_LONG_4: Duration = Duration::from_millis(1000);
}

/// A Material swatch with a primary color and optional numeric shades.
#[derive(Clone, Copy, Debug, PartialEq, Eq, TypedBuilder)]
pub struct MaterialColor {
    #[builder(setter(into))]
    pub primary: Color,
    #[builder(setter(into))]
    pub shades: [Color; 10],
}

impl MaterialColor {
    #[must_use]
    pub const fn new(primary: Color, shades: [Color; 10]) -> Self {
        Self { primary, shades }
    }

    #[must_use]
    pub const fn shade(self, index: usize) -> Color {
        let slot = match index {
            50 => 0,
            100 => 1,
            200 => 2,
            300 => 3,
            400 => 4,
            500 => 5,
            600 => 6,
            700 => 7,
            800 => 8,
            900 => 9,
            _ => index,
        };
        if slot < self.shades.len() {
            self.shades[slot]
        } else {
            self.primary
        }
    }
}

pub type MaterialAccentColor = MaterialColor;

/// Common Material palette values.  The values are sRGBA, matching Incular's
/// renderer-independent [`Color`] representation.
pub struct Colors;

impl Colors {
    pub const TRANSPARENT: Color = Color::TRANSPARENT;
    pub const BLACK: Color = Color::BLACK;
    pub const WHITE: Color = Color::WHITE;
    pub const RED: Color = Color::rgba(244, 67, 54, 255);
    pub const PINK: Color = Color::rgba(233, 30, 99, 255);
    pub const PURPLE: Color = Color::rgba(156, 39, 176, 255);
    pub const DEEP_PURPLE: Color = Color::rgba(103, 58, 183, 255);
    pub const INDIGO: Color = Color::rgba(63, 81, 181, 255);
    pub const BLUE: Color = Color::rgba(33, 150, 243, 255);
    pub const LIGHT_BLUE: Color = Color::rgba(3, 169, 244, 255);
    pub const CYAN: Color = Color::rgba(0, 188, 212, 255);
    pub const TEAL: Color = Color::rgba(0, 150, 136, 255);
    pub const GREEN: Color = Color::rgba(76, 175, 80, 255);
    pub const LIGHT_GREEN: Color = Color::rgba(139, 195, 74, 255);
    pub const LIME: Color = Color::rgba(205, 220, 57, 255);
    pub const YELLOW: Color = Color::rgba(255, 235, 59, 255);
    pub const AMBER: Color = Color::rgba(255, 193, 7, 255);
    pub const ORANGE: Color = Color::rgba(255, 152, 0, 255);
    pub const DEEP_ORANGE: Color = Color::rgba(255, 87, 34, 255);
    pub const BROWN: Color = Color::rgba(121, 85, 72, 255);
    pub const GREY: Color = Color::rgba(158, 158, 158, 255);
    pub const BLUE_GREY: Color = Color::rgba(96, 125, 139, 255);
}

pub const K_BOTTOM_NAVIGATION_BAR_HEIGHT: f32 = 56.0;
pub const K_MIN_INTERACTIVE_DIMENSION: f32 = 48.0;
pub const K_TOOLBAR_HEIGHT: f32 = 56.0;
pub const K_RADIAL_REACTION_ALPHA: u8 = 0x1f;
pub const K_RADIAL_REACTION_RADIUS: f32 = 20.0;
pub const K_FLOATING_ACTION_BUTTON_MARGIN: f32 = 16.0;
pub const K_THEME_ANIMATION_DURATION: Duration = Duration::from_millis(200);
pub const K_TAB_SCROLL_DURATION: Duration = Duration::from_millis(300);

/// Flutter-style lower-camel aliases are provided for source generators and
/// parity tooling while the all-caps constants remain idiomatic Rust.
pub const k_bottom_navigation_bar_height: f32 = K_BOTTOM_NAVIGATION_BAR_HEIGHT;
pub const k_min_interactive_dimension: f32 = K_MIN_INTERACTIVE_DIMENSION;
pub const k_toolbar_height: f32 = K_TOOLBAR_HEIGHT;
pub const k_theme_animation_duration: Duration = K_THEME_ANIMATION_DURATION;
