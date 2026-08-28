//! Shared Material state, style, density, colour, and theme primitives.
//!
//! The public surface deliberately follows Flutter's Material vocabulary while
//! keeping the implementation Rust-native.  Components use these values as
//! immutable configuration and resolve them at build time; no component owns a
//! second copy of the state-property or theme system.

use incular_config::{Brightness, Clip, Constraints, EdgeInsets};
use incular_controls::{ButtonStyle, ControlTheme, SplashFactory, StateValue, TextFieldStyle};
use incular_core::{Color, HslColor, Lerp, Offset, Size};
use incular_text::TextStyle;
use incular_widgets::internal::{ActionSurface, DropShadow};
use incular_widgets::{Border, BorderRadius, BoxDecoration, Container, Row, Text, Widget};
use std::collections::BTreeMap;
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use typed_builder::TypedBuilder;

/// Material interaction states used by state-dependent properties.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Ord, PartialOrd)]
pub enum WidgetState {
    Disabled,
    Error,
    Pressed,
    Dragged,
    Selected,
    Focused,
    Hovered,
    ScrolledUnder,
}

/// A compact set of [`WidgetState`] values.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct WidgetStates(u16);

impl WidgetStates {
    const DISABLED: u16 = 1 << 0;
    const ERROR: u16 = 1 << 1;
    const PRESSED: u16 = 1 << 2;
    const DRAGGED: u16 = 1 << 3;
    const SELECTED: u16 = 1 << 4;
    const FOCUSED: u16 = 1 << 5;
    const HOVERED: u16 = 1 << 6;
    const SCROLLED_UNDER: u16 = 1 << 7;

    #[must_use]
    pub const fn new() -> Self {
        Self(0)
    }

    #[must_use]
    pub const fn bits(self) -> u16 {
        self.0
    }

    #[must_use]
    pub const fn contains(self, state: WidgetState) -> bool {
        self.0 & state.bit() != 0
    }

    #[must_use]
    pub const fn with(mut self, state: WidgetState) -> Self {
        self.0 |= state.bit();
        self
    }

    #[must_use]
    pub const fn without(mut self, state: WidgetState) -> Self {
        self.0 &= !state.bit();
        self
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    #[must_use]
    pub const fn len(self) -> usize {
        self.0.count_ones() as usize
    }

    #[must_use]
    pub const fn from_bits_retain(bits: u16) -> Self {
        Self(bits)
    }

    /// Bridges the Material state set to the headless controls state set.
    /// Components use this at the boundary instead of maintaining a second
    /// state-resolution policy.
    #[must_use]
    pub fn into_control_state(self) -> incular_controls::ControlState {
        let mut state = incular_controls::ControlState::empty();
        if self.contains(WidgetState::Disabled) {
            state.insert(incular_controls::ControlState::DISABLED);
        }
        if self.contains(WidgetState::Pressed) {
            state.insert(incular_controls::ControlState::PRESSED);
        }
        if self.contains(WidgetState::Dragged) {
            state.insert(incular_controls::ControlState::DRAGGING);
        }
        if self.contains(WidgetState::Selected) {
            state.insert(incular_controls::ControlState::SELECTED);
        }
        if self.contains(WidgetState::Focused) {
            state.insert(incular_controls::ControlState::FOCUSED);
        }
        if self.contains(WidgetState::Hovered) {
            state.insert(incular_controls::ControlState::HOVERED);
        }
        if self.contains(WidgetState::Error) {
            state.insert(incular_controls::ControlState::INVALID);
        }
        state
    }
}

impl WidgetState {
    const fn bit(self) -> u16 {
        match self {
            Self::Disabled => WidgetStates::DISABLED,
            Self::Error => WidgetStates::ERROR,
            Self::Pressed => WidgetStates::PRESSED,
            Self::Dragged => WidgetStates::DRAGGED,
            Self::Selected => WidgetStates::SELECTED,
            Self::Focused => WidgetStates::FOCUSED,
            Self::Hovered => WidgetStates::HOVERED,
            Self::ScrolledUnder => WidgetStates::SCROLLED_UNDER,
        }
    }
}

impl std::ops::BitOr for WidgetStates {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOr<WidgetState> for WidgetStates {
    type Output = Self;

    fn bitor(self, rhs: WidgetState) -> Self::Output {
        self.with(rhs)
    }
}

impl From<WidgetState> for WidgetStates {
    fn from(value: WidgetState) -> Self {
        Self::new().with(value)
    }
}

impl From<incular_controls::ControlState> for WidgetStates {
    fn from(value: incular_controls::ControlState) -> Self {
        let mut states = Self::new();
        if value.is_disabled() {
            states = states.with(WidgetState::Disabled);
        }
        if value.is_pressed() {
            states = states.with(WidgetState::Pressed);
        }
        if value.is_dragging() {
            states = states.with(WidgetState::Dragged);
        }
        if value.is_selected() {
            states = states.with(WidgetState::Selected);
        }
        if value.is_focused() {
            states = states.with(WidgetState::Focused);
        }
        if value.is_hovered() {
            states = states.with(WidgetState::Hovered);
        }
        if value.is_invalid() {
            states = states.with(WidgetState::Error);
        }
        states
    }
}

/// A state-dependent Material value.
///
/// `All` and `Map` are allocation-free during resolution. `Resolver` is useful
/// for derived values and is intentionally kept behind an `Arc` so styles can
/// be cloned safely into retained builders.
pub enum StateProperty<T> {
    All(T),
    Map(Vec<(WidgetState, T)>),
    Resolver(Arc<dyn Fn(WidgetStates) -> T + Send + Sync + 'static>),
}

impl<T: Clone> Clone for StateProperty<T> {
    fn clone(&self) -> Self {
        match self {
            Self::All(value) => Self::All(value.clone()),
            Self::Map(values) => Self::Map(values.clone()),
            Self::Resolver(resolver) => Self::Resolver(resolver.clone()),
        }
    }
}

impl<T: fmt::Debug> fmt::Debug for StateProperty<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::All(value) => formatter.debug_tuple("All").field(value).finish(),
            Self::Map(values) => formatter.debug_tuple("Map").field(values).finish(),
            Self::Resolver(_) => formatter.write_str("Resolver(..)"),
        }
    }
}

impl<T: PartialEq> PartialEq for StateProperty<T> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::All(a), Self::All(b)) => a == b,
            (Self::Map(a), Self::Map(b)) => a == b,
            // Resolver closures have no meaningful structural equality.
            (Self::Resolver(_), Self::Resolver(_)) => false,
            _ => false,
        }
    }
}

impl<T: Clone> StateProperty<T> {
    #[must_use]
    pub fn all(value: T) -> Self {
        Self::All(value)
    }

    #[must_use]
    pub fn from_map<const N: usize>(entries: [(WidgetState, T); N]) -> Self {
        Self::Map(entries.into_iter().collect())
    }

    #[must_use]
    pub fn resolve(&self, states: WidgetStates) -> T {
        match self {
            Self::All(value) => value.clone(),
            Self::Map(entries) => entries
                .iter()
                .filter(|(state, _)| states.contains(*state))
                .max_by_key(|(state, _)| state.priority())
                // A sparse mapper has no dedicated "normal" key. Flutter's
                // nullable state properties simply resolve to null in this
                // case; for a non-null Rust value the first entry is the
                // deterministic, allocation-free fallback. Callers that
                // need a different fallback should use `resolve_or`.
                .or_else(|| entries.first())
                .map_or_else(
                    || panic!("StateProperty::Map cannot resolve an empty map"),
                    |(_, value)| value.clone(),
                ),
            Self::Resolver(resolver) => resolver(states),
        }
    }

    #[must_use]
    pub fn resolve_or(&self, states: WidgetStates, fallback: T) -> T {
        match self {
            Self::All(value) => value.clone(),
            Self::Map(entries) => entries
                .iter()
                .filter(|(state, _)| states.contains(*state))
                .max_by_key(|(state, _)| state.priority())
                .map_or(fallback, |(_, value)| value.clone()),
            Self::Resolver(resolver) => resolver(states),
        }
    }
}

impl<T> StateProperty<T> {
    #[must_use]
    pub fn resolve_with(resolver: impl Fn(WidgetStates) -> T + Send + Sync + 'static) -> Self {
        Self::Resolver(Arc::new(resolver))
    }
}

impl<T> From<T> for StateProperty<T> {
    fn from(value: T) -> Self {
        Self::All(value)
    }
}

impl WidgetState {
    const fn priority(self) -> u8 {
        match self {
            Self::Disabled => 100,
            Self::Error => 90,
            Self::Pressed => 80,
            Self::Dragged => 75,
            Self::Selected => 70,
            Self::Focused => 60,
            Self::Hovered => 50,
            Self::ScrolledUnder => 40,
        }
    }
}

/// Compatibility spelling for Flutter's state-property abstraction.
pub type WidgetStateProperty<T> = StateProperty<T>;

/// Compatibility spelling for Flutter's `WidgetStatePropertyAll`.
pub type WidgetStatePropertyAll<T> = StateProperty<T>;

/// Pre-3.16 compatibility spellings. Flutter 3.47.1 uses `WidgetState` in
/// its public Material APIs, but keeping these aliases costs nothing and
/// allows existing Material applications to migrate without a parallel state
/// resolver or duplicate behavior implementation.
pub type MaterialState = WidgetState;
pub type MaterialStates = WidgetStates;
pub type MaterialStateProperty<T> = StateProperty<T>;
pub type MaterialStatePropertyAll<T> = StateProperty<T>;
pub type MaterialStatesController = WidgetStatesController;

/// Typed controller for mutable state sets shared by controls.
#[derive(Clone, Debug, Default)]
pub struct WidgetStatesController {
    states: std::rc::Rc<std::cell::Cell<WidgetStates>>,
}

impl WidgetStatesController {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn states(&self) -> WidgetStates {
        self.states.get()
    }

    pub fn set(&self, state: WidgetState, value: bool) {
        let states = if value {
            self.states().with(state)
        } else {
            self.states().without(state)
        };
        self.states.set(states);
    }

    pub fn update(&self, states: WidgetStates) {
        self.states.set(states);
    }
}

/// Material density adjustment used by layout and hit-target calculations.
#[derive(Clone, Copy, Debug, PartialEq, TypedBuilder)]
pub struct VisualDensity {
    #[builder(default = 0.0)]
    pub horizontal: f32,
    #[builder(default = 0.0)]
    pub vertical: f32,
}

impl VisualDensity {
    pub const STANDARD: Self = Self::new(0.0, 0.0);
    pub const COMFORTABLE: Self = Self::new(-1.0, -1.0);
    pub const COMPACT: Self = Self::new(-2.0, -2.0);

    #[must_use]
    pub const fn new(horizontal: f32, vertical: f32) -> Self {
        Self {
            horizontal,
            vertical,
        }
    }

    #[must_use]
    pub fn base_size_adjustment(self) -> Offset {
        // Incular's `Size` intentionally rejects negative extents, while
        // Flutter's density adjustment is signed. `Offset` is the matching
        // renderer-neutral signed pair and keeps compact density lossless.
        Offset::new(self.horizontal * 4.0, self.vertical * 4.0)
    }

    #[must_use]
    pub fn effective_constraints(self, constraints: Constraints) -> Constraints {
        let adjustment = self.base_size_adjustment();
        Constraints::new(
            (constraints.min_width + adjustment.x).max(0.0),
            (constraints.max_width + adjustment.x).max(0.0),
            (constraints.min_height + adjustment.y).max(0.0),
            (constraints.max_height + adjustment.y).max(0.0),
        )
    }

    #[must_use]
    pub fn lerp(self, other: Self, t: f32) -> Self {
        Self::new(
            self.horizontal + (other.horizontal - self.horizontal) * t,
            self.vertical + (other.vertical - self.vertical) * t,
        )
    }
}

impl Default for VisualDensity {
    fn default() -> Self {
        Self::STANDARD
    }
}

/// Material's minimum interactive target policy.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MaterialTapTargetSize {
    #[default]
    Padded,
    ShrinkWrap,
}

impl MaterialTapTargetSize {
    #[must_use]
    pub fn minimum_size(self) -> Size {
        match self {
            Self::Padded => Size::new(48.0, 48.0),
            Self::ShrinkWrap => Size::ZERO,
        }
    }
}

/// Material surface kinds.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MaterialType {
    #[default]
    Canvas,
    Card,
    Circle,
    Button,
    Transparency,
}

/// A retained Material surface. Elevation is lowered to the shared shadow
/// renderer, so changing it affects paint/composite output rather than being a
/// passive style field.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct Material {
    #[builder(default = MaterialType::Canvas)]
    pub material_type: MaterialType,
    #[builder(default = Color::TRANSPARENT)]
    pub color: Color,
    #[builder(default = Color::rgba(0, 0, 0, 100))]
    pub shadow_color: Color,
    #[builder(default, setter(strip_option, into))]
    pub surface_tint_color: Option<Color>,
    #[builder(default = 0.0, setter(transform = |value: f32| value.max(0.0)))]
    pub elevation: f32,
    #[builder(default = BorderRadius::ZERO)]
    pub border_radius: BorderRadius,
    #[builder(default, setter(strip_option))]
    pub padding: Option<EdgeInsets>,
    #[builder(default = Clip::None)]
    pub clip_behavior: Clip,
    #[builder(default = Duration::from_millis(200))]
    pub animation_duration: Duration,
    #[builder(setter(into))]
    pub child: Widget,
}

impl Material {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            material_type: MaterialType::Canvas,
            color: Color::TRANSPARENT,
            shadow_color: Color::rgba(0, 0, 0, 100),
            surface_tint_color: None,
            elevation: 0.0,
            border_radius: BorderRadius::ZERO,
            padding: None,
            clip_behavior: Clip::None,
            animation_duration: Duration::from_millis(200),
            child: child.into(),
        }
    }

    #[must_use]
    pub fn material_type(mut self, value: MaterialType) -> Self {
        self.material_type = value;
        self
    }
    #[must_use]
    pub fn color(mut self, value: Color) -> Self {
        self.color = value;
        self
    }
    #[must_use]
    pub fn shadow_color(mut self, value: Color) -> Self {
        self.shadow_color = value;
        self
    }
    #[must_use]
    pub fn surface_tint_color(mut self, value: Color) -> Self {
        self.surface_tint_color = Some(value);
        self
    }
    #[must_use]
    pub fn elevation(mut self, value: f32) -> Self {
        self.elevation = value.max(0.0);
        self
    }
    #[must_use]
    pub fn border_radius(mut self, value: BorderRadius) -> Self {
        self.border_radius = value;
        self
    }
    /// Sets a uniform corner radius. This is a convenience alias for the
    /// canonical [`Material::border_radius`] builder.
    #[must_use]
    pub fn radius(self, value: f32) -> Self {
        self.border_radius(BorderRadius::circular(value.max(0.0)))
    }
    /// Adds inner padding around the material child.
    #[must_use]
    pub fn padding(mut self, value: EdgeInsets) -> Self {
        self.padding = Some(value);
        self
    }
    #[must_use]
    pub fn clip_behavior(mut self, value: Clip) -> Self {
        self.clip_behavior = value;
        self
    }
    #[must_use]
    pub fn animation_duration(mut self, value: Duration) -> Self {
        self.animation_duration = value;
        self
    }
}

impl From<Material> for Widget {
    fn from(value: Material) -> Self {
        let mut surface_color = value.color;
        if let Some(tint) = value.surface_tint_color {
            let amount = (0.05 + value.elevation / 240.0).min(0.20);
            surface_color = mix(surface_color, tint, amount);
        }
        let mut surface = Container::with_child(value.child)
            .decoration(
                BoxDecoration::new()
                    .color(surface_color)
                    .border_radius(value.border_radius),
            )
            .clip_behavior(value.clip_behavior);
        if let Some(padding) = value.padding {
            surface = surface.padding(padding);
        }
        if value.material_type == MaterialType::Circle {
            surface =
                surface.decoration(BoxDecoration::new().shape(incular_widgets::BoxShape::Circle));
        }
        if value.elevation > 0.0 {
            DropShadow::new(
                Offset::new(0.0, value.elevation * 0.18),
                (value.elevation * 0.55).max(1.0),
                value.shadow_color,
                surface,
            )
            .into()
        } else {
            surface.into()
        }
    }
}

/// Material ink background wrapper. The actual ink is painted by the shared
/// retained decoration/action primitives.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct Ink {
    #[builder(default, setter(strip_option))]
    pub color: Option<Color>,
    #[builder(setter(into))]
    pub child: Widget,
}

impl Ink {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            color: None,
            child: child.into(),
        }
    }
    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }
}

impl From<Ink> for Widget {
    fn from(value: Ink) -> Self {
        let mut decoration = BoxDecoration::new();
        if let Some(color) = value.color {
            decoration = decoration.color(color);
        }
        incular_widgets::DecoratedBox::new(value.child)
            .decoration(decoration)
            .into()
    }
}

/// Retained pointer/keyboard ink interaction surface.
#[derive(Clone, TypedBuilder)]
pub struct InkWell {
    #[builder(setter(into))]
    pub child: Widget,
    #[builder(
        default,
        setter(
            fn transform<F>(callback: F) -> Option<Rc<dyn Fn() + 'static>>
            where
                F: Fn() + 'static,
            {
                Some(Rc::new(callback))
            }
        )
    )]
    on_tap: Option<Rc<dyn Fn() + 'static>>,
    #[builder(default = Color::TRANSPARENT)]
    pub color: Color,
    #[builder(default = Color::TRANSPARENT)]
    pub overlay_color: Color,
}

impl fmt::Debug for InkWell {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InkWell")
            .field("color", &self.color)
            .field("overlay_color", &self.overlay_color)
            .finish()
    }
}

impl PartialEq for InkWell {
    fn eq(&self, other: &Self) -> bool {
        self.child == other.child
            && self.color == other.color
            && self.overlay_color == other.overlay_color
    }
}

impl InkWell {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            on_tap: None,
            color: Color::TRANSPARENT,
            overlay_color: Color::TRANSPARENT,
        }
    }
    #[must_use]
    pub fn on_tap(mut self, callback: impl Fn() + 'static) -> Self {
        self.on_tap = Some(std::rc::Rc::new(callback));
        self
    }
    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }
    #[must_use]
    pub fn overlay_color(mut self, color: Color) -> Self {
        self.overlay_color = color;
        self
    }
}

impl From<InkWell> for Widget {
    fn from(value: InkWell) -> Self {
        let mut surface = ActionSurface::with_child(value.child)
            .color(value.color)
            .hover_color(value.overlay_color)
            .pressed_color(value.overlay_color);
        if let Some(callback) = value.on_tap {
            surface = surface.on_click(move || callback());
        }
        surface.into()
    }
}

/// `InkResponse` shares the retained implementation with `InkWell`; the
/// distinction in Flutter is about clipping/gesture details, not a second
/// state system.
pub type InkResponse = InkWell;

/// Material theme extension payload for application-defined values.
#[derive(Clone, Debug, PartialEq)]
pub enum ThemeExtensionValue {
    Color(Color),
    Number(f32),
    Text(String),
    Bool(bool),
}

/// The complete Material 3 colour-role set.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorScheme {
    pub brightness: Brightness,
    pub primary: Color,
    pub on_primary: Color,
    pub primary_container: Color,
    pub on_primary_container: Color,
    pub primary_fixed: Color,
    pub primary_fixed_dim: Color,
    pub on_primary_fixed: Color,
    pub on_primary_fixed_variant: Color,
    pub secondary: Color,
    pub on_secondary: Color,
    pub secondary_container: Color,
    pub on_secondary_container: Color,
    pub secondary_fixed: Color,
    pub secondary_fixed_dim: Color,
    pub on_secondary_fixed: Color,
    pub on_secondary_fixed_variant: Color,
    pub tertiary: Color,
    pub on_tertiary: Color,
    pub tertiary_container: Color,
    pub on_tertiary_container: Color,
    pub tertiary_fixed: Color,
    pub tertiary_fixed_dim: Color,
    pub on_tertiary_fixed: Color,
    pub on_tertiary_fixed_variant: Color,
    pub error: Color,
    pub on_error: Color,
    pub error_container: Color,
    pub on_error_container: Color,
    pub surface: Color,
    pub on_surface: Color,
    pub surface_dim: Color,
    pub surface_bright: Color,
    pub surface_container_lowest: Color,
    pub surface_container_low: Color,
    pub surface_container: Color,
    pub surface_container_high: Color,
    pub surface_container_highest: Color,
    pub on_surface_variant: Color,
    pub outline: Color,
    pub outline_variant: Color,
    pub shadow: Color,
    pub scrim: Color,
    pub inverse_surface: Color,
    pub on_inverse_surface: Color,
    pub inverse_primary: Color,
    pub surface_tint: Color,
    /// Deprecated Flutter aliases retained for source compatibility.
    pub background: Color,
    pub on_background: Color,
    pub surface_variant: Color,
}

impl ColorScheme {
    #[must_use]
    pub fn light() -> Self {
        Self::from_seed_with_brightness(Color::rgba(103, 80, 164, 255), Brightness::Light)
    }

    #[must_use]
    pub fn dark() -> Self {
        Self::from_seed_with_brightness(Color::rgba(208, 188, 255, 255), Brightness::Dark)
    }

    #[must_use]
    pub fn from_seed(seed_color: Color) -> Self {
        // Flutter's `ColorScheme.fromSeed` defaults to light brightness.
        // Brightness is an explicit theme choice; inferring it from a seed
        // makes the standard purple seed unexpectedly produce a dark theme.
        Self::from_seed_with_brightness(seed_color, Brightness::Light)
    }

    #[must_use]
    pub fn from_seed_with_brightness(seed: Color, brightness: Brightness) -> Self {
        let hsl = seed.to_hsl();
        let primary = seed;
        let secondary = HslColor::new(
            hsl.hue + 60.0,
            (hsl.saturation * 0.7).min(1.0),
            hsl.lightness,
            1.0,
        )
        .to_color();
        let tertiary = HslColor::new(
            hsl.hue + 120.0,
            (hsl.saturation * 0.65).min(1.0),
            hsl.lightness,
            1.0,
        )
        .to_color();
        let (surface, on_surface, container, on_container) = match brightness {
            Brightness::Light => (
                Color::rgba(255, 251, 255, 255),
                Color::rgba(29, 27, 32, 255),
                HslColor::new(hsl.hue, hsl.saturation * 0.35, 0.92, 1.0).to_color(),
                Color::rgba(33, 0, 93, 255),
            ),
            Brightness::Dark => (
                Color::rgba(20, 18, 24, 255),
                Color::rgba(231, 225, 229, 255),
                HslColor::new(hsl.hue, hsl.saturation * 0.25, 0.28, 1.0).to_color(),
                Color::rgba(234, 221, 255, 255),
            ),
        };
        let on_primary = contrasting(primary);
        let on_secondary = contrasting(secondary);
        let on_tertiary = contrasting(tertiary);
        let error = match brightness {
            Brightness::Light => Color::rgba(186, 26, 26, 255),
            Brightness::Dark => Color::rgba(255, 180, 171, 255),
        };
        let on_error = contrasting(error);
        let error_container = match brightness {
            Brightness::Light => Color::rgba(255, 218, 214, 255),
            Brightness::Dark => Color::rgba(147, 0, 10, 255),
        };
        let on_error_container = contrasting(error_container);
        let surface_dim = if brightness == Brightness::Light {
            Color::rgba(222, 216, 225, 255)
        } else {
            Color::rgba(20, 18, 24, 255)
        };
        let surface_bright = if brightness == Brightness::Light {
            Color::rgba(255, 251, 255, 255)
        } else {
            Color::rgba(59, 56, 62, 255)
        };
        let outline = if brightness == Brightness::Light {
            Color::rgba(121, 116, 126, 255)
        } else {
            Color::rgba(147, 143, 153, 255)
        };
        let inverse_surface = if brightness == Brightness::Light {
            Color::rgba(50, 47, 53, 255)
        } else {
            Color::rgba(231, 225, 229, 255)
        };
        let on_inverse_surface = contrasting(inverse_surface);
        Self {
            brightness,
            primary,
            on_primary,
            primary_container: container,
            on_primary_container: on_container,
            primary_fixed: Color::rgba(234, 221, 255, 255),
            primary_fixed_dim: Color::rgba(217, 194, 255, 255),
            on_primary_fixed: Color::rgba(33, 0, 93, 255),
            on_primary_fixed_variant: Color::rgba(79, 55, 139, 255),
            secondary,
            on_secondary,
            secondary_container: mix(surface, secondary, 0.18),
            on_secondary_container: contrasting(mix(surface, secondary, 0.18)),
            secondary_fixed: Color::rgba(232, 222, 248, 255),
            secondary_fixed_dim: Color::rgba(203, 191, 219, 255),
            on_secondary_fixed: Color::rgba(30, 25, 34, 255),
            on_secondary_fixed_variant: Color::rgba(74, 68, 82, 255),
            tertiary,
            on_tertiary,
            tertiary_container: mix(surface, tertiary, 0.18),
            on_tertiary_container: contrasting(mix(surface, tertiary, 0.18)),
            tertiary_fixed: Color::rgba(255, 216, 227, 255),
            tertiary_fixed_dim: Color::rgba(239, 184, 201, 255),
            on_tertiary_fixed: Color::rgba(49, 17, 32, 255),
            on_tertiary_fixed_variant: Color::rgba(116, 53, 76, 255),
            error,
            on_error,
            error_container,
            on_error_container,
            surface,
            on_surface,
            surface_dim,
            surface_bright,
            surface_container_lowest: surface,
            surface_container_low: mix(surface, on_surface, 0.03),
            surface_container: mix(surface, on_surface, 0.06),
            surface_container_high: mix(surface, on_surface, 0.10),
            surface_container_highest: mix(surface, on_surface, 0.14),
            on_surface_variant: if brightness == Brightness::Light {
                Color::rgba(73, 69, 79, 255)
            } else {
                Color::rgba(202, 196, 208, 255)
            },
            outline,
            outline_variant: mix(outline, surface, 0.45),
            shadow: Color::BLACK,
            scrim: Color::BLACK,
            inverse_surface,
            on_inverse_surface,
            inverse_primary: if brightness == Brightness::Light {
                Color::rgba(217, 194, 255, 255)
            } else {
                Color::rgba(103, 80, 164, 255)
            },
            surface_tint: primary,
            background: surface,
            on_background: on_surface,
            surface_variant: mix(surface, on_surface, 0.10),
        }
    }

    #[must_use]
    pub fn with_primary(mut self, value: Color) -> Self {
        self.primary = value;
        self.surface_tint = value;
        self
    }

    #[must_use]
    pub fn with_surface(mut self, value: Color) -> Self {
        self.surface = value;
        self.background = value;
        self
    }

    #[must_use]
    pub fn with_on_surface(mut self, value: Color) -> Self {
        self.on_surface = value;
        self.on_background = value;
        self
    }

    #[must_use]
    pub fn lerp(self, other: Self, t: f32) -> Self {
        macro_rules! c {
            ($field:ident) => {
                self.$field.lerp(&other.$field, t)
            };
        }
        Self {
            brightness: if t < 0.5 {
                self.brightness
            } else {
                other.brightness
            },
            primary: c!(primary),
            on_primary: c!(on_primary),
            primary_container: c!(primary_container),
            on_primary_container: c!(on_primary_container),
            primary_fixed: c!(primary_fixed),
            primary_fixed_dim: c!(primary_fixed_dim),
            on_primary_fixed: c!(on_primary_fixed),
            on_primary_fixed_variant: c!(on_primary_fixed_variant),
            secondary: c!(secondary),
            on_secondary: c!(on_secondary),
            secondary_container: c!(secondary_container),
            on_secondary_container: c!(on_secondary_container),
            secondary_fixed: c!(secondary_fixed),
            secondary_fixed_dim: c!(secondary_fixed_dim),
            on_secondary_fixed: c!(on_secondary_fixed),
            on_secondary_fixed_variant: c!(on_secondary_fixed_variant),
            tertiary: c!(tertiary),
            on_tertiary: c!(on_tertiary),
            tertiary_container: c!(tertiary_container),
            on_tertiary_container: c!(on_tertiary_container),
            tertiary_fixed: c!(tertiary_fixed),
            tertiary_fixed_dim: c!(tertiary_fixed_dim),
            on_tertiary_fixed: c!(on_tertiary_fixed),
            on_tertiary_fixed_variant: c!(on_tertiary_fixed_variant),
            error: c!(error),
            on_error: c!(on_error),
            error_container: c!(error_container),
            on_error_container: c!(on_error_container),
            surface: c!(surface),
            on_surface: c!(on_surface),
            surface_dim: c!(surface_dim),
            surface_bright: c!(surface_bright),
            surface_container_lowest: c!(surface_container_lowest),
            surface_container_low: c!(surface_container_low),
            surface_container: c!(surface_container),
            surface_container_high: c!(surface_container_high),
            surface_container_highest: c!(surface_container_highest),
            on_surface_variant: c!(on_surface_variant),
            outline: c!(outline),
            outline_variant: c!(outline_variant),
            shadow: c!(shadow),
            scrim: c!(scrim),
            inverse_surface: c!(inverse_surface),
            on_inverse_surface: c!(on_inverse_surface),
            inverse_primary: c!(inverse_primary),
            surface_tint: c!(surface_tint),
            background: c!(background),
            on_background: c!(on_background),
            surface_variant: c!(surface_variant),
        }
    }
}

impl Default for ColorScheme {
    fn default() -> Self {
        Self::light()
    }
}

fn relative_luminance(color: Color) -> f32 {
    fn channel(value: u8) -> f32 {
        let value = value as f32 / 255.0;
        if value <= 0.03928 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * channel(color.red) + 0.7152 * channel(color.green) + 0.0722 * channel(color.blue)
}

fn alpha(color: Color, opacity: f32) -> Color {
    Color::rgba(
        color.red,
        color.green,
        color.blue,
        (color.alpha as f32 * opacity.clamp(0.0, 1.0)).round() as u8,
    )
}

fn contrasting(color: Color) -> Color {
    if relative_luminance(color) > 0.45 {
        Color::BLACK
    } else {
        Color::WHITE
    }
}

fn mix(a: Color, b: Color, amount: f32) -> Color {
    a.lerp(&b, amount.clamp(0.0, 1.0))
}

/// Material typography roles.  The text engine remains owned by
/// `incular-text`; this is only the Material role mapping.
#[derive(Clone, Debug, PartialEq)]
pub struct TextTheme {
    pub display_large: TextStyle,
    pub display_medium: TextStyle,
    pub display_small: TextStyle,
    pub headline_large: TextStyle,
    pub headline_medium: TextStyle,
    pub headline_small: TextStyle,
    pub title_large: TextStyle,
    pub title_medium: TextStyle,
    pub title_small: TextStyle,
    pub body_large: TextStyle,
    pub body_medium: TextStyle,
    pub body_small: TextStyle,
    pub label_large: TextStyle,
    pub label_medium: TextStyle,
    pub label_small: TextStyle,
}

impl Default for TextTheme {
    fn default() -> Self {
        Self::light()
    }
}

impl TextTheme {
    #[must_use]
    pub fn light() -> Self {
        Self::with_color(Color::rgba(29, 27, 32, 255))
    }

    #[must_use]
    pub fn dark() -> Self {
        Self::with_color(Color::rgba(231, 225, 229, 255))
    }

    #[must_use]
    pub fn with_color(color: Color) -> Self {
        let role = |size: f32| TextStyle::new().font_size(size).color(color);
        Self {
            display_large: role(57.0),
            display_medium: role(45.0),
            display_small: role(36.0),
            headline_large: role(32.0),
            headline_medium: role(28.0),
            headline_small: role(24.0),
            title_large: role(22.0),
            title_medium: role(16.0),
            title_small: role(14.0),
            body_large: role(16.0),
            body_medium: role(14.0),
            body_small: role(12.0),
            label_large: role(14.0),
            label_medium: role(12.0),
            label_small: role(11.0),
        }
    }

    #[must_use]
    pub fn merge(mut self, other: &Self) -> Self {
        self.display_large = self.display_large.clone().merge(&other.display_large);
        self.display_medium = self.display_medium.clone().merge(&other.display_medium);
        self.display_small = self.display_small.clone().merge(&other.display_small);
        self.headline_large = self.headline_large.clone().merge(&other.headline_large);
        self.headline_medium = self.headline_medium.clone().merge(&other.headline_medium);
        self.headline_small = self.headline_small.clone().merge(&other.headline_small);
        self.title_large = self.title_large.clone().merge(&other.title_large);
        self.title_medium = self.title_medium.clone().merge(&other.title_medium);
        self.title_small = self.title_small.clone().merge(&other.title_small);
        self.body_large = self.body_large.clone().merge(&other.body_large);
        self.body_medium = self.body_medium.clone().merge(&other.body_medium);
        self.body_small = self.body_small.clone().merge(&other.body_small);
        self.label_large = self.label_large.clone().merge(&other.label_large);
        self.label_medium = self.label_medium.clone().merge(&other.label_medium);
        self.label_small = self.label_small.clone().merge(&other.label_small);
        self
    }

    #[must_use]
    pub fn lerp(&self, other: &Self, t: f32) -> Self {
        macro_rules! style {
            ($field:ident) => {
                self.$field.lerp(&other.$field, t)
            };
        }
        Self {
            display_large: style!(display_large),
            display_medium: style!(display_medium),
            display_small: style!(display_small),
            headline_large: style!(headline_large),
            headline_medium: style!(headline_medium),
            headline_small: style!(headline_small),
            title_large: style!(title_large),
            title_medium: style!(title_medium),
            title_small: style!(title_small),
            body_large: style!(body_large),
            body_medium: style!(body_medium),
            body_small: style!(body_small),
            label_large: style!(label_large),
            label_medium: style!(label_medium),
            label_small: style!(label_small),
        }
    }

    #[must_use]
    pub fn with_body_medium(mut self, style: TextStyle) -> Self {
        self.body_medium = style;
        self
    }
}

/// Material's typography family metadata.
#[derive(Clone, Debug, PartialEq)]
pub struct Typography {
    pub black: TextTheme,
    pub white: TextTheme,
    pub english_like: TextTheme,
    pub dense: TextTheme,
    pub tall: TextTheme,
}

impl Default for Typography {
    fn default() -> Self {
        Self::material2021()
    }
}

impl Typography {
    #[must_use]
    pub fn material2021() -> Self {
        Self {
            black: TextTheme::light(),
            white: TextTheme::dark(),
            english_like: TextTheme::light(),
            dense: TextTheme::light(),
            tall: TextTheme::light(),
        }
    }
}

/// Shared fields common to Material component theme data.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ComponentThemeData {
    /// Optional complete style override used by button component themes.
    /// Non-button themes simply leave this unset; keeping it here lets the
    /// generated Flutter-shaped theme aliases share one sparse merge model.
    pub style: Option<ButtonStyle>,
    pub background_color: Option<Color>,
    /// Flutter calls this `color` for surfaces such as Card and Divider.
    /// `background_color` remains the shared spelling used by button themes.
    pub color: Option<Color>,
    pub foreground_color: Option<Color>,
    pub overlay_color: Option<Color>,
    pub shadow_color: Option<Color>,
    pub surface_tint_color: Option<Color>,
    pub elevation: Option<f32>,
    pub padding: Option<EdgeInsets>,
    pub margin: Option<EdgeInsets>,
    pub minimum_size: Option<Size>,
    pub maximum_size: Option<Size>,
    pub shape: Option<BorderRadius>,
    pub side: Option<Border>,
    pub clip_behavior: Option<Clip>,
    pub border_on_foreground: Option<bool>,
    pub center_title: Option<bool>,
    pub toolbar_height: Option<f32>,
    pub leading_width: Option<f32>,
    pub title_spacing: Option<f32>,
    pub indicator_size: Option<f32>,
    pub track_height: Option<f32>,
    pub thumb_size: Option<f32>,
    pub divider_color: Option<Color>,
    pub label_color: Option<Color>,
    pub unselected_label_color: Option<Color>,
    pub icon_color: Option<Color>,
    pub selected_icon_color: Option<Color>,
    /// State-aware colors used by selection controls and indicators. Keeping
    /// these on the shared sparse theme record lets the typed
    /// `CheckboxThemeData`/`RadioThemeData`/`SwitchThemeData` aliases expose
    /// Flutter's common property names without forking a second resolver.
    pub fill_color: Option<StateValue<Color>>,
    pub check_color: Option<StateValue<Color>>,
    pub thumb_color: Option<StateValue<Color>>,
    pub track_color: Option<StateValue<Color>>,
    pub outline_color: Option<StateValue<Color>>,
    pub active_track_color: Option<StateValue<Color>>,
    pub inactive_track_color: Option<StateValue<Color>>,
    pub disabled_color: Option<Color>,
    pub icon_size: Option<f32>,
    pub opacity: Option<f32>,
    pub mouse_cursor: Option<String>,
    pub selected_item_color: Option<Color>,
    pub unselected_item_color: Option<Color>,
    pub surface_color: Option<Color>,
    pub min_size: Option<Size>,
    pub text_style: Option<TextStyle>,
    pub animation_duration: Option<Duration>,
}

impl ComponentThemeData {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn style(mut self, value: ButtonStyle) -> Self {
        self.style = Some(value);
        self
    }

    #[must_use]
    pub fn background_color(mut self, value: Color) -> Self {
        self.background_color = Some(value);
        self
    }

    #[must_use]
    pub fn foreground_color(mut self, value: Color) -> Self {
        self.foreground_color = Some(value);
        self
    }

    #[must_use]
    pub fn color(mut self, value: Color) -> Self {
        self.color = Some(value);
        self
    }

    #[must_use]
    pub fn margin(mut self, value: EdgeInsets) -> Self {
        self.margin = Some(value);
        self
    }

    #[must_use]
    pub fn overlay_color(mut self, value: Color) -> Self {
        self.overlay_color = Some(value);
        self
    }

    #[must_use]
    pub fn fill_color(mut self, value: impl Into<StateValue<Color>>) -> Self {
        self.fill_color = Some(value.into());
        self
    }

    #[must_use]
    pub fn check_color(mut self, value: impl Into<StateValue<Color>>) -> Self {
        self.check_color = Some(value.into());
        self
    }

    #[must_use]
    pub fn thumb_color(mut self, value: impl Into<StateValue<Color>>) -> Self {
        self.thumb_color = Some(value.into());
        self
    }

    #[must_use]
    pub fn track_color(mut self, value: impl Into<StateValue<Color>>) -> Self {
        self.track_color = Some(value.into());
        self
    }

    #[must_use]
    pub fn outline_color(mut self, value: impl Into<StateValue<Color>>) -> Self {
        self.outline_color = Some(value.into());
        self
    }

    #[must_use]
    pub fn active_track_color(mut self, value: impl Into<StateValue<Color>>) -> Self {
        self.active_track_color = Some(value.into());
        self
    }

    #[must_use]
    pub fn inactive_track_color(mut self, value: impl Into<StateValue<Color>>) -> Self {
        self.inactive_track_color = Some(value.into());
        self
    }

    #[must_use]
    pub fn disabled_color(mut self, value: Color) -> Self {
        self.disabled_color = Some(value);
        self
    }

    #[must_use]
    pub fn icon_size(mut self, value: f32) -> Self {
        self.icon_size = Some(value.max(0.0));
        self
    }

    /// Flutter-shaped spelling for `IconThemeData.size`.
    #[must_use]
    pub fn size(self, value: f32) -> Self {
        self.icon_size(value)
    }

    #[must_use]
    pub fn opacity(mut self, value: f32) -> Self {
        self.opacity = Some(value.clamp(0.0, 1.0));
        self
    }

    #[must_use]
    pub fn mouse_cursor(mut self, value: impl Into<String>) -> Self {
        self.mouse_cursor = Some(value.into());
        self
    }

    #[must_use]
    pub fn elevation(mut self, value: f32) -> Self {
        self.elevation = Some(value.max(0.0));
        self
    }

    #[must_use]
    pub fn padding(mut self, value: EdgeInsets) -> Self {
        self.padding = Some(value);
        self
    }

    #[must_use]
    pub fn shape(mut self, value: BorderRadius) -> Self {
        self.shape = Some(value);
        self
    }
}

macro_rules! component_theme_aliases {
    ($($name:ident),+ $(,)?) => { $(pub type $name = ComponentThemeData;)+ };
}

component_theme_aliases!(
    ActionIconThemeData,
    AppBarThemeData,
    BadgeThemeData,
    BannerThemeData,
    MaterialBannerThemeData,
    IconThemeData,
    BottomAppBarThemeData,
    BottomNavigationBarThemeData,
    BottomSheetThemeData,
    CardThemeData,
    CarouselViewThemeData,
    CheckboxThemeData,
    ChipThemeData,
    DataTableThemeData,
    DatePickerThemeData,
    DialogThemeData,
    DividerThemeData,
    DrawerThemeData,
    ElevatedButtonThemeData,
    ExpansionTileThemeData,
    FilledButtonThemeData,
    FloatingActionButtonThemeData,
    IconButtonThemeData,
    ListTileThemeData,
    MenuBarThemeData,
    MenuButtonThemeData,
    NavigationBarThemeData,
    NavigationDrawerThemeData,
    NavigationRailThemeData,
    OutlinedButtonThemeData,
    RadioThemeData,
    SearchBarThemeData,
    SearchViewThemeData,
    SegmentedButtonThemeData,
    SnackBarThemeData,
    SwitchThemeData,
    TextButtonThemeData,
    TextSelectionThemeData,
    TimePickerThemeData,
    ToggleButtonsThemeData,
    TooltipThemeData,
    ButtonBarThemeData,
);

// Flutter exposes both `FooTheme` inherited wrappers and `FooThemeData`
// records. The retained implementation uses one sparse record per family;
// these aliases preserve the canonical entry points without creating a
// second theme-resolution system.
macro_rules! component_theme_wrappers {
    ($($name:ident = $data:ident),+ $(,)?) => { $(#[allow(dead_code)] pub type $name = $data;)+ };
}

component_theme_wrappers!(
    ActionIconTheme = ActionIconThemeData,
    AppBarTheme = AppBarThemeData,
    BadgeTheme = BadgeThemeData,
    BottomAppBarTheme = BottomAppBarThemeData,
    BottomNavigationBarTheme = BottomNavigationBarThemeData,
    BottomSheetTheme = BottomSheetThemeData,
    CardTheme = CardThemeData,
    CheckboxTheme = CheckboxThemeData,
    ChipTheme = ChipThemeData,
    DialogTheme = DialogThemeData,
    DividerTheme = DividerThemeData,
    DrawerTheme = DrawerThemeData,
    ElevatedButtonTheme = ElevatedButtonThemeData,
    FilledButtonTheme = FilledButtonThemeData,
    FloatingActionButtonTheme = FloatingActionButtonThemeData,
    IconButtonTheme = IconButtonThemeData,
    ListTileTheme = ListTileThemeData,
    MenuBarTheme = MenuBarThemeData,
    MenuButtonTheme = MenuButtonThemeData,
    NavigationBarTheme = NavigationBarThemeData,
    NavigationDrawerTheme = NavigationDrawerThemeData,
    NavigationRailTheme = NavigationRailThemeData,
    OutlinedButtonTheme = OutlinedButtonThemeData,
    RadioTheme = RadioThemeData,
    SnackBarTheme = SnackBarThemeData,
    SwitchTheme = SwitchThemeData,
    TextButtonTheme = TextButtonThemeData,
    TextSelectionTheme = TextSelectionThemeData,
    TooltipTheme = TooltipThemeData,
);

#[allow(dead_code)]
pub type PopupMenuTheme = crate::menus::PopupMenuThemeData;
#[allow(dead_code)]
pub type TabBarTheme = crate::p0_controls::TabBarThemeData;

/// Full Material input-decoration configuration.
#[derive(Clone, Debug, Default, PartialEq, TypedBuilder)]
#[builder(field_defaults(default, setter(strip_option, into)))]
pub struct InputDecorationThemeData {
    pub label_style: Option<TextStyle>,
    pub floating_label_style: Option<TextStyle>,
    pub hint_style: Option<TextStyle>,
    #[builder(default, setter(!strip_option, transform = |value: usize| Some(value.max(1))))]
    pub hint_max_lines: Option<usize>,
    pub helper_style: Option<TextStyle>,
    #[builder(default, setter(!strip_option, transform = |value: usize| Some(value.max(1))))]
    pub helper_max_lines: Option<usize>,
    pub error_style: Option<TextStyle>,
    #[builder(default, setter(!strip_option, transform = |value: usize| Some(value.max(1))))]
    pub error_max_lines: Option<usize>,
    pub counter_style: Option<TextStyle>,
    pub prefix_style: Option<TextStyle>,
    pub suffix_style: Option<TextStyle>,
    pub floating_label_behavior: Option<FloatingLabelBehavior>,
    pub floating_label_alignment: Option<FloatingLabelAlignment>,
    pub is_dense: Option<bool>,
    pub is_collapsed: Option<bool>,
    pub align_label_with_hint: Option<bool>,
    pub content_padding: Option<EdgeInsets>,
    pub filled: Option<bool>,
    pub fill_color: Option<Color>,
    pub hover_color: Option<Color>,
    pub border: Option<InputBorder>,
    pub enabled_border: Option<InputBorder>,
    pub focused_border: Option<InputBorder>,
    pub error_border: Option<InputBorder>,
    pub focused_error_border: Option<InputBorder>,
    pub disabled_border: Option<InputBorder>,
    pub constraints: Option<Size>,
}

impl InputDecorationThemeData {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn label_style(mut self, value: TextStyle) -> Self {
        self.label_style = Some(value);
        self
    }

    #[must_use]
    pub fn floating_label_style(mut self, value: TextStyle) -> Self {
        self.floating_label_style = Some(value);
        self
    }

    #[must_use]
    pub fn hint_style(mut self, value: TextStyle) -> Self {
        self.hint_style = Some(value);
        self
    }

    #[must_use]
    pub fn hint_max_lines(mut self, value: usize) -> Self {
        self.hint_max_lines = Some(value.max(1));
        self
    }

    #[must_use]
    pub fn helper_style(mut self, value: TextStyle) -> Self {
        self.helper_style = Some(value);
        self
    }

    #[must_use]
    pub fn helper_max_lines(mut self, value: usize) -> Self {
        self.helper_max_lines = Some(value);
        self
    }

    #[must_use]
    pub fn error_style(mut self, value: TextStyle) -> Self {
        self.error_style = Some(value);
        self
    }

    #[must_use]
    pub fn error_max_lines(mut self, value: usize) -> Self {
        self.error_max_lines = Some(value);
        self
    }

    #[must_use]
    pub fn counter_style(mut self, value: TextStyle) -> Self {
        self.counter_style = Some(value);
        self
    }

    #[must_use]
    pub fn prefix_style(mut self, value: TextStyle) -> Self {
        self.prefix_style = Some(value);
        self
    }

    #[must_use]
    pub fn suffix_style(mut self, value: TextStyle) -> Self {
        self.suffix_style = Some(value);
        self
    }

    #[must_use]
    pub fn floating_label_behavior(mut self, value: FloatingLabelBehavior) -> Self {
        self.floating_label_behavior = Some(value);
        self
    }

    #[must_use]
    pub fn floating_label_alignment(mut self, value: FloatingLabelAlignment) -> Self {
        self.floating_label_alignment = Some(value);
        self
    }

    #[must_use]
    pub fn is_dense(mut self, value: bool) -> Self {
        self.is_dense = Some(value);
        self
    }

    #[must_use]
    pub fn is_collapsed(mut self, value: bool) -> Self {
        self.is_collapsed = Some(value);
        self
    }

    #[must_use]
    pub fn align_label_with_hint(mut self, value: bool) -> Self {
        self.align_label_with_hint = Some(value);
        self
    }

    #[must_use]
    pub fn content_padding(mut self, value: EdgeInsets) -> Self {
        self.content_padding = Some(value);
        self
    }

    #[must_use]
    pub fn filled(mut self, value: bool) -> Self {
        self.filled = Some(value);
        self
    }

    #[must_use]
    pub fn fill_color(mut self, value: Color) -> Self {
        self.fill_color = Some(value);
        self
    }

    #[must_use]
    pub fn hover_color(mut self, value: Color) -> Self {
        self.hover_color = Some(value);
        self
    }

    #[must_use]
    pub fn border(mut self, value: InputBorder) -> Self {
        self.border = Some(value);
        self
    }

    #[must_use]
    pub fn enabled_border(mut self, value: InputBorder) -> Self {
        self.enabled_border = Some(value);
        self
    }

    #[must_use]
    pub fn focused_border(mut self, value: InputBorder) -> Self {
        self.focused_border = Some(value);
        self
    }

    #[must_use]
    pub fn error_border(mut self, value: InputBorder) -> Self {
        self.error_border = Some(value);
        self
    }

    #[must_use]
    pub fn focused_error_border(mut self, value: InputBorder) -> Self {
        self.focused_error_border = Some(value);
        self
    }

    #[must_use]
    pub fn disabled_border(mut self, value: InputBorder) -> Self {
        self.disabled_border = Some(value);
        self
    }

    #[must_use]
    pub fn constraints(mut self, value: Size) -> Self {
        self.constraints = Some(value);
        self
    }
}

/// Inherited wrapper for an [`InputDecorationThemeData`] value.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct InputDecorationTheme {
    #[builder(setter(into))]
    pub data: InputDecorationThemeData,
    #[builder(setter(into))]
    pub child: Widget,
}

impl InputDecorationTheme {
    #[must_use]
    pub fn new(data: InputDecorationThemeData, child: impl Into<Widget>) -> Self {
        Self {
            data,
            child: child.into(),
        }
    }
}

impl From<InputDecorationTheme> for Widget {
    fn from(value: InputDecorationTheme) -> Self {
        Widget::environment_scope(value.data, value.child)
    }
}

/// Per-field Material decoration. `None` fields inherit from the nearest
/// [`InputDecorationThemeData`] and then the component defaults.
#[derive(Clone, Debug, Default, PartialEq, TypedBuilder)]
pub struct InputDecoration {
    #[builder(default, setter(strip_option, into))]
    pub icon: Option<Widget>,
    #[builder(default, setter(strip_option, into))]
    pub label_text: Option<String>,
    #[builder(default, setter(strip_option, into))]
    pub hint_text: Option<String>,
    #[builder(default, setter(strip_option, into))]
    pub helper_text: Option<String>,
    #[builder(default, setter(strip_option, into))]
    pub error_text: Option<String>,
    #[builder(default, setter(strip_option, into))]
    pub counter_text: Option<String>,
    #[builder(default, setter(strip_option, into))]
    pub counter: Option<Widget>,
    #[builder(default, setter(strip_option, into))]
    pub prefix_text: Option<String>,
    #[builder(default, setter(strip_option, into))]
    pub suffix_text: Option<String>,
    #[builder(default, setter(strip_option, into))]
    pub label_style: Option<TextStyle>,
    #[builder(default, setter(strip_option, into))]
    pub floating_label_style: Option<TextStyle>,
    #[builder(default, setter(strip_option, into))]
    pub hint_style: Option<TextStyle>,
    #[builder(default, setter(transform = |value: usize| Some(value.max(1))))]
    pub hint_max_lines: Option<usize>,
    #[builder(default, setter(transform = |value: usize| Some(value.max(1))))]
    pub helper_max_lines: Option<usize>,
    #[builder(default, setter(transform = |value: usize| Some(value.max(1))))]
    pub error_max_lines: Option<usize>,
    #[builder(default, setter(strip_option, into))]
    pub helper_style: Option<TextStyle>,
    #[builder(default, setter(strip_option, into))]
    pub error_style: Option<TextStyle>,
    #[builder(default, setter(strip_option, into))]
    pub counter_style: Option<TextStyle>,
    #[builder(default, setter(strip_option, into))]
    pub prefix_style: Option<TextStyle>,
    #[builder(default, setter(strip_option, into))]
    pub suffix_style: Option<TextStyle>,
    #[builder(default, setter(strip_option, into))]
    pub prefix_icon: Option<Widget>,
    #[builder(default, setter(strip_option, into))]
    pub suffix_icon: Option<Widget>,
    #[builder(default, setter(strip_option, into))]
    pub icon_color: Option<Color>,
    #[builder(default, setter(strip_option, into))]
    pub prefix_icon_color: Option<Color>,
    #[builder(default, setter(strip_option, into))]
    pub suffix_icon_color: Option<Color>,
    #[builder(default, setter(strip_option, into))]
    pub prefix: Option<Widget>,
    #[builder(default, setter(strip_option, into))]
    pub suffix: Option<Widget>,
    #[builder(default, setter(strip_option, into))]
    pub is_dense: Option<bool>,
    #[builder(default)]
    pub is_collapsed: bool,
    #[builder(default, setter(strip_option, into))]
    pub align_label_with_hint: Option<bool>,
    #[builder(default, setter(strip_option, into))]
    pub semantic_counter_text: Option<String>,
    #[builder(default, setter(strip_option, into))]
    pub filled: Option<bool>,
    #[builder(default, setter(strip_option, into))]
    pub fill_color: Option<Color>,
    #[builder(default, setter(strip_option, into))]
    pub hover_color: Option<Color>,
    #[builder(default, setter(strip_option, into))]
    pub content_padding: Option<EdgeInsets>,
    #[builder(default, setter(strip_option, into))]
    pub floating_label_behavior: Option<FloatingLabelBehavior>,
    #[builder(default, setter(strip_option, into))]
    pub floating_label_alignment: Option<FloatingLabelAlignment>,
    #[builder(default, setter(strip_option, into))]
    pub constraints: Option<Size>,
    #[builder(default, setter(strip_option, into))]
    pub border: Option<InputBorder>,
    #[builder(default, setter(strip_option, into))]
    pub enabled_border: Option<InputBorder>,
    #[builder(default, setter(strip_option, into))]
    pub focused_border: Option<InputBorder>,
    #[builder(default, setter(strip_option, into))]
    pub error_border: Option<InputBorder>,
    #[builder(default, setter(strip_option, into))]
    pub focused_error_border: Option<InputBorder>,
    #[builder(default, setter(strip_option, into))]
    pub disabled_border: Option<InputBorder>,
}

impl InputDecoration {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    #[must_use]
    pub fn collapsed(hint: impl Into<String>) -> Self {
        Self::new().hint(hint).is_collapsed(true)
    }
    #[must_use]
    pub fn none() -> Self {
        Self::new().border(InputBorder::None).is_collapsed(true)
    }
    #[must_use]
    pub fn label(mut self, value: impl Into<String>) -> Self {
        self.label_text = Some(value.into());
        self
    }

    #[must_use]
    pub fn label_text(self, value: impl Into<String>) -> Self {
        self.label(value)
    }
    #[must_use]
    pub fn hint(mut self, value: impl Into<String>) -> Self {
        self.hint_text = Some(value.into());
        self
    }

    #[must_use]
    pub fn hint_text(self, value: impl Into<String>) -> Self {
        self.hint(value)
    }
    #[must_use]
    pub fn helper(mut self, value: impl Into<String>) -> Self {
        self.helper_text = Some(value.into());
        self
    }

    #[must_use]
    pub fn helper_text(self, value: impl Into<String>) -> Self {
        self.helper(value)
    }
    #[must_use]
    pub fn error(mut self, value: impl Into<String>) -> Self {
        self.error_text = Some(value.into());
        self
    }

    #[must_use]
    pub fn error_text(self, value: impl Into<String>) -> Self {
        self.error(value)
    }
    #[must_use]
    pub fn counter(mut self, value: impl Into<String>) -> Self {
        self.counter_text = Some(value.into());
        self
    }

    #[must_use]
    pub fn counter_text(self, value: impl Into<String>) -> Self {
        self.counter(value)
    }

    #[must_use]
    pub fn icon(mut self, value: impl Into<Widget>) -> Self {
        self.icon = Some(value.into());
        self
    }

    #[must_use]
    pub fn counter_widget(mut self, value: impl Into<Widget>) -> Self {
        self.counter = Some(value.into());
        self
    }
    #[must_use]
    pub fn prefix(mut self, value: impl Into<String>) -> Self {
        self.prefix_text = Some(value.into());
        self
    }
    #[must_use]
    pub fn suffix(mut self, value: impl Into<String>) -> Self {
        self.suffix_text = Some(value.into());
        self
    }
    #[must_use]
    pub fn label_style(mut self, value: TextStyle) -> Self {
        self.label_style = Some(value);
        self
    }
    #[must_use]
    pub fn floating_label_style(mut self, value: TextStyle) -> Self {
        self.floating_label_style = Some(value);
        self
    }

    #[must_use]
    pub fn hint_style(mut self, value: TextStyle) -> Self {
        self.hint_style = Some(value);
        self
    }

    #[must_use]
    pub fn hint_max_lines(mut self, value: usize) -> Self {
        self.hint_max_lines = Some(value.max(1));
        self
    }

    #[must_use]
    pub fn helper_max_lines(mut self, value: usize) -> Self {
        self.helper_max_lines = Some(value.max(1));
        self
    }

    #[must_use]
    pub fn error_max_lines(mut self, value: usize) -> Self {
        self.error_max_lines = Some(value.max(1));
        self
    }
    #[must_use]
    pub fn helper_style(mut self, value: TextStyle) -> Self {
        self.helper_style = Some(value);
        self
    }
    #[must_use]
    pub fn error_style(mut self, value: TextStyle) -> Self {
        self.error_style = Some(value);
        self
    }
    #[must_use]
    pub fn counter_style(mut self, value: TextStyle) -> Self {
        self.counter_style = Some(value);
        self
    }
    #[must_use]
    pub fn prefix_style(mut self, value: TextStyle) -> Self {
        self.prefix_style = Some(value);
        self
    }
    #[must_use]
    pub fn suffix_style(mut self, value: TextStyle) -> Self {
        self.suffix_style = Some(value);
        self
    }
    #[must_use]
    pub fn prefix_icon(mut self, value: impl Into<Widget>) -> Self {
        self.prefix_icon = Some(value.into());
        self
    }
    #[must_use]
    pub fn suffix_icon(mut self, value: impl Into<Widget>) -> Self {
        self.suffix_icon = Some(value.into());
        self
    }

    #[must_use]
    pub fn icon_color(mut self, value: Color) -> Self {
        self.icon_color = Some(value);
        self
    }

    #[must_use]
    pub fn prefix_icon_color(mut self, value: Color) -> Self {
        self.prefix_icon_color = Some(value);
        self
    }

    #[must_use]
    pub fn suffix_icon_color(mut self, value: Color) -> Self {
        self.suffix_icon_color = Some(value);
        self
    }
    #[must_use]
    pub fn prefix_widget(mut self, value: impl Into<Widget>) -> Self {
        self.prefix = Some(value.into());
        self
    }
    #[must_use]
    pub fn suffix_widget(mut self, value: impl Into<Widget>) -> Self {
        self.suffix = Some(value.into());
        self
    }
    #[must_use]
    pub fn is_dense(mut self, value: bool) -> Self {
        self.is_dense = Some(value);
        self
    }
    #[must_use]
    pub fn is_collapsed(mut self, value: bool) -> Self {
        self.is_collapsed = value;
        self
    }
    #[must_use]
    pub fn align_label_with_hint(mut self, value: bool) -> Self {
        self.align_label_with_hint = Some(value);
        self
    }
    #[must_use]
    pub fn semantic_counter_text(mut self, value: impl Into<String>) -> Self {
        self.semantic_counter_text = Some(value.into());
        self
    }
    #[must_use]
    pub fn filled(mut self, value: bool) -> Self {
        self.filled = Some(value);
        self
    }
    #[must_use]
    pub fn fill_color(mut self, value: Color) -> Self {
        self.fill_color = Some(value);
        self
    }
    #[must_use]
    pub fn hover_color(mut self, value: Color) -> Self {
        self.hover_color = Some(value);
        self
    }
    #[must_use]
    pub fn content_padding(mut self, value: EdgeInsets) -> Self {
        self.content_padding = Some(value);
        self
    }
    #[must_use]
    pub fn floating_label_behavior(mut self, value: FloatingLabelBehavior) -> Self {
        self.floating_label_behavior = Some(value);
        self
    }
    #[must_use]
    pub fn floating_label_alignment(mut self, value: FloatingLabelAlignment) -> Self {
        self.floating_label_alignment = Some(value);
        self
    }
    #[must_use]
    pub fn constraints(mut self, value: Size) -> Self {
        self.constraints = Some(value);
        self
    }
    #[must_use]
    pub fn border(mut self, value: InputBorder) -> Self {
        self.border = Some(value);
        self
    }
    #[must_use]
    pub fn enabled_border(mut self, value: InputBorder) -> Self {
        self.enabled_border = Some(value);
        self
    }
    #[must_use]
    pub fn focused_border(mut self, value: InputBorder) -> Self {
        self.focused_border = Some(value);
        self
    }
    #[must_use]
    pub fn error_border(mut self, value: InputBorder) -> Self {
        self.error_border = Some(value);
        self
    }
    #[must_use]
    pub fn focused_error_border(mut self, value: InputBorder) -> Self {
        self.focused_error_border = Some(value);
        self
    }
    #[must_use]
    pub fn disabled_border(mut self, value: InputBorder) -> Self {
        self.disabled_border = Some(value);
        self
    }

    /// Applies only the values present in `other`, retaining this
    /// decoration's sparse override semantics.
    #[must_use]
    pub fn merge(mut self, other: &Self) -> Self {
        macro_rules! override_field {
            ($field:ident) => {
                if other.$field.is_some() {
                    self.$field = other.$field.clone();
                }
            };
        }
        override_field!(icon);
        override_field!(label_text);
        override_field!(hint_text);
        override_field!(helper_text);
        override_field!(error_text);
        override_field!(counter_text);
        override_field!(counter);
        override_field!(prefix_text);
        override_field!(suffix_text);
        override_field!(label_style);
        override_field!(floating_label_style);
        override_field!(hint_style);
        override_field!(hint_max_lines);
        override_field!(helper_max_lines);
        override_field!(error_max_lines);
        override_field!(helper_style);
        override_field!(error_style);
        override_field!(counter_style);
        override_field!(prefix_style);
        override_field!(suffix_style);
        override_field!(prefix_icon);
        override_field!(suffix_icon);
        override_field!(icon_color);
        override_field!(prefix_icon_color);
        override_field!(suffix_icon_color);
        override_field!(prefix);
        override_field!(suffix);
        override_field!(is_dense);
        override_field!(align_label_with_hint);
        override_field!(semantic_counter_text);
        override_field!(filled);
        override_field!(fill_color);
        override_field!(hover_color);
        override_field!(content_padding);
        override_field!(floating_label_behavior);
        override_field!(floating_label_alignment);
        override_field!(constraints);
        override_field!(border);
        override_field!(enabled_border);
        override_field!(focused_border);
        override_field!(error_border);
        override_field!(focused_error_border);
        override_field!(disabled_border);
        if other.is_collapsed {
            self.is_collapsed = true;
        }
        self
    }

    #[must_use]
    pub fn apply_defaults(&self, theme: &InputDecorationThemeData) -> Self {
        let mut result = self.clone();
        if result.label_style.is_none() {
            result.label_style = theme.label_style.clone();
        }
        if result.floating_label_style.is_none() {
            result.floating_label_style = theme.floating_label_style.clone();
        }
        if result.hint_style.is_none() {
            result.hint_style = theme.hint_style.clone();
        }
        if result.hint_max_lines.is_none() {
            result.hint_max_lines = theme.hint_max_lines;
        }
        if result.helper_max_lines.is_none() {
            result.helper_max_lines = theme.helper_max_lines;
        }
        if result.error_max_lines.is_none() {
            result.error_max_lines = theme.error_max_lines;
        }
        if result.helper_style.is_none() {
            result.helper_style = theme.helper_style.clone();
        }
        if result.error_style.is_none() {
            result.error_style = theme.error_style.clone();
        }
        if result.counter_style.is_none() {
            result.counter_style = theme.counter_style.clone();
        }
        if result.prefix_style.is_none() {
            result.prefix_style = theme.prefix_style.clone();
        }
        if result.suffix_style.is_none() {
            result.suffix_style = theme.suffix_style.clone();
        }
        if result.is_dense.is_none() {
            result.is_dense = theme.is_dense;
        }
        if !result.is_collapsed {
            result.is_collapsed = theme.is_collapsed.unwrap_or(false);
        }
        if result.align_label_with_hint.is_none() {
            result.align_label_with_hint = theme.align_label_with_hint;
        }
        if result.content_padding.is_none() {
            result.content_padding = theme.content_padding;
        }
        if result.filled.is_none() {
            result.filled = theme.filled;
        }
        if result.fill_color.is_none() {
            result.fill_color = theme.fill_color;
        }
        if result.hover_color.is_none() {
            result.hover_color = theme.hover_color;
        }
        if result.floating_label_behavior.is_none() {
            result.floating_label_behavior = theme.floating_label_behavior;
        }
        if result.floating_label_alignment.is_none() {
            result.floating_label_alignment = theme.floating_label_alignment;
        }
        if result.border.is_none() {
            result.border = theme.border;
        }
        if result.enabled_border.is_none() {
            result.enabled_border = theme.enabled_border;
        }
        if result.focused_border.is_none() {
            result.focused_border = theme.focused_border;
        }
        if result.error_border.is_none() {
            result.error_border = theme.error_border;
        }
        if result.focused_error_border.is_none() {
            result.focused_error_border = theme.focused_error_border;
        }
        if result.disabled_border.is_none() {
            result.disabled_border = theme.disabled_border;
        }
        if result.constraints.is_none() {
            result.constraints = theme.constraints;
        }
        result
    }
}

/// Material's chrome-only input decorator. Editing, focus and selection stay
/// in the child (normally core `EditableText`); this widget only resolves the
/// decoration state and composes labels, icons, helper/error text, and borders.
#[derive(Clone, TypedBuilder)]
pub struct InputDecorator {
    #[builder(setter(into))]
    pub decoration: InputDecoration,
    #[builder(setter(into))]
    pub child: Widget,
    #[builder(default)]
    pub is_focused: bool,
    #[builder(default = true)]
    pub is_empty: bool,
    #[builder(default = true)]
    pub enabled: bool,
}

impl InputDecorator {
    #[must_use]
    pub fn new(decoration: InputDecoration, child: impl Into<Widget>) -> Self {
        Self {
            decoration,
            child: child.into(),
            is_focused: false,
            is_empty: true,
            enabled: true,
        }
    }

    #[must_use]
    pub fn focused(mut self, value: bool) -> Self {
        self.is_focused = value;
        self
    }

    #[must_use]
    pub fn empty(mut self, value: bool) -> Self {
        self.is_empty = value;
        self
    }

    #[must_use]
    pub fn enabled(mut self, value: bool) -> Self {
        self.enabled = value;
        self
    }

    #[must_use]
    pub fn decoration(mut self, value: InputDecoration) -> Self {
        self.decoration = value;
        self
    }

    #[must_use]
    pub fn child(mut self, value: impl Into<Widget>) -> Self {
        self.child = value.into();
        self
    }

    fn build(self) -> Widget {
        let decoration = self.decoration;
        let border = if !self.enabled {
            decoration
                .disabled_border
                .or(decoration.border)
                .unwrap_or_default()
        } else if decoration.error_text.is_some() && self.is_focused {
            decoration
                .focused_error_border
                .or(decoration.error_border)
                .or(decoration.focused_border)
                .or(decoration.border)
                .unwrap_or_default()
        } else if decoration.error_text.is_some() {
            decoration
                .error_border
                .or(decoration.border)
                .unwrap_or_default()
        } else if self.is_focused {
            decoration
                .focused_border
                .or(decoration.border)
                .unwrap_or_default()
        } else {
            decoration
                .enabled_border
                .or(decoration.border)
                .unwrap_or_default()
        };
        let (border, radius) = match border {
            InputBorder::None => (Border::new(0.0, Color::TRANSPARENT), 0.0),
            InputBorder::Underline { border, radius } | InputBorder::Outline { border, radius } => {
                (border, radius)
            }
        };
        let fill = if decoration.filled.unwrap_or(false) {
            decoration.fill_color.unwrap_or(Color::TRANSPARENT)
        } else {
            Color::TRANSPARENT
        };
        let padding = if decoration.is_collapsed {
            EdgeInsets::all(0.0)
        } else {
            decoration
                .content_padding
                .unwrap_or_else(|| EdgeInsets::symmetric(12.0, 8.0))
        };
        let mut content = Vec::with_capacity(5);
        if let Some(label) = decoration.label_text {
            if self.is_focused
                || !self.is_empty
                || decoration.floating_label_behavior == Some(FloatingLabelBehavior::Always)
            {
                content.push(Widget::from(
                    Text::new(label).style(decoration.label_style.clone().unwrap_or_default()),
                ));
            }
        }
        let mut row = Vec::with_capacity(5);
        if let Some(icon) = decoration.prefix_icon {
            row.push(icon);
        }
        if let Some(prefix) = decoration.prefix {
            row.push(prefix);
        }
        if let Some(text) = decoration.prefix_text {
            row.push(
                Text::new(text)
                    .style(decoration.prefix_style.clone().unwrap_or_default())
                    .into(),
            );
        }
        row.push(self.child);
        if let Some(text) = decoration.suffix_text {
            row.push(
                Text::new(text)
                    .style(decoration.suffix_style.clone().unwrap_or_default())
                    .into(),
            );
        }
        if let Some(suffix) = decoration.suffix {
            row.push(suffix);
        }
        if let Some(icon) = decoration.suffix_icon {
            row.push(icon);
        }
        content.push(Widget::from(incular_widgets::Row::new(row)));
        if let Some(message) = decoration.error_text {
            content.push(Widget::from(
                Text::new(message).style(decoration.error_style.unwrap_or_default()),
            ));
        } else if let Some(message) = decoration.helper_text {
            content.push(Widget::from(
                Text::new(message).style(decoration.helper_style.unwrap_or_default()),
            ));
        }
        if let Some(counter) = decoration.counter {
            content.push(counter);
        } else if let Some(counter) = decoration.counter_text {
            content.push(Widget::from(
                Text::new(counter).style(decoration.counter_style.unwrap_or_default()),
            ));
        }
        let inner: Widget = if content.len() == 1 {
            content.remove(0)
        } else {
            incular_widgets::Column::new(content).spacing(4.0).into()
        };
        let mut result = Container::new()
            .padding(padding)
            .decoration(
                BoxDecoration::new()
                    .color(fill)
                    .border(border)
                    .border_radius(BorderRadius::circular(radius)),
            )
            .child(inner);
        if let Some(size) = decoration.constraints {
            result = result.constraints(Constraints::tight(size));
        }
        if let Some(icon) = decoration.icon {
            Row::new([icon, result.into()])
                .spacing(8.0)
                .cross_axis_alignment(incular_config::CrossAxisAlignment::Center)
                .into()
        } else {
            result.into()
        }
    }
}

impl From<InputDecorator> for Widget {
    fn from(value: InputDecorator) -> Self {
        value.build()
    }
}

impl From<InputDecoration> for TextFieldStyle {
    fn from(value: InputDecoration) -> Self {
        let border = value
            .border
            .or(value.enabled_border)
            .map(|border| match border {
                InputBorder::None => Border::new(0.0, Color::TRANSPARENT),
                InputBorder::Underline { border, .. } | InputBorder::Outline { border, .. } => {
                    border
                }
            });
        let border_focused = value
            .focused_border
            .or(value.focused_error_border)
            .map(|border| match border {
                InputBorder::None => Border::new(0.0, Color::TRANSPARENT),
                InputBorder::Underline { border, .. } | InputBorder::Outline { border, .. } => {
                    border
                }
            });
        let radius = value
            .border
            .as_ref()
            .or(value.enabled_border.as_ref())
            .and_then(|border| match border {
                InputBorder::Outline { radius, .. } => Some(*radius),
                _ => None,
            });
        Self {
            background: value.fill_color,
            foreground: None,
            placeholder_color: value.label_style.as_ref().map(|style| style.color),
            border,
            border_focused,
            border_radius: radius,
            padding: value.content_padding,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FloatingLabelBehavior {
    #[default]
    Auto,
    Always,
    Never,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FloatingLabelAlignment {
    #[default]
    Start,
    Center,
}

/// Rust representation of Flutter's Material input borders.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum InputBorder {
    None,
    Underline { border: Border, radius: f32 },
    Outline { border: Border, radius: f32 },
}

impl Default for InputBorder {
    fn default() -> Self {
        Self::Underline {
            border: Border::new(1.0, Color::rgba(121, 116, 126, 255)),
            radius: 0.0,
        }
    }
}

impl InputBorder {
    #[must_use]
    pub fn underline() -> Self {
        Self::default()
    }
    #[must_use]
    pub fn outline(radius: f32, border: Border) -> Self {
        Self::Outline {
            border,
            radius: radius.max(0.0),
        }
    }
    #[must_use]
    pub fn none() -> Self {
        Self::None
    }
}

/// Flutter-shaped outline border descriptor. The renderer-neutral border is
/// retained in [`InputBorder`] so Material text fields do not need a second
/// painting implementation.
#[derive(Clone, Copy, Debug, PartialEq, TypedBuilder)]
pub struct OutlineInputBorder {
    #[builder(default = Border::new(1.0, Color::rgba(121, 116, 126, 255)))]
    pub border: Border,
    #[builder(default = 4.0, setter(transform = |value: f32| value.max(0.0)))]
    pub radius: f32,
}

impl Default for OutlineInputBorder {
    fn default() -> Self {
        Self {
            border: Border::new(1.0, Color::rgba(121, 116, 126, 255)),
            radius: 4.0,
        }
    }
}

impl OutlineInputBorder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    #[must_use]
    pub fn border(mut self, value: Border) -> Self {
        self.border = value;
        self
    }
    #[must_use]
    pub fn radius(mut self, value: f32) -> Self {
        self.radius = value.max(0.0);
        self
    }
}

impl From<OutlineInputBorder> for InputBorder {
    fn from(value: OutlineInputBorder) -> Self {
        InputBorder::outline(value.radius, value.border)
    }
}

/// Flutter-shaped underline border descriptor.
#[derive(Clone, Copy, Debug, PartialEq, TypedBuilder)]
pub struct UnderlineInputBorder {
    #[builder(default = Border::new(1.0, Color::rgba(121, 116, 126, 255)))]
    pub border: Border,
    #[builder(default = 0.0, setter(transform = |value: f32| value.max(0.0)))]
    pub radius: f32,
}

impl Default for UnderlineInputBorder {
    fn default() -> Self {
        Self {
            border: Border::new(1.0, Color::rgba(121, 116, 126, 255)),
            radius: 0.0,
        }
    }
}

impl UnderlineInputBorder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    #[must_use]
    pub fn border(mut self, value: Border) -> Self {
        self.border = value;
        self
    }
    #[must_use]
    pub fn radius(mut self, value: f32) -> Self {
        self.radius = value.max(0.0);
        self
    }
}

impl From<UnderlineInputBorder> for InputBorder {
    fn from(value: UnderlineInputBorder) -> Self {
        InputBorder::Underline {
            border: value.border,
            radius: value.radius,
        }
    }
}

/// Common shaped-border spelling used by Flutter's input border hierarchy.
pub type ShapedInputBorder = InputBorder;

/// Platform policy used by Material adaptive components.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TargetPlatform {
    #[default]
    Linux,
    Windows,
    MacOs,
    Android,
    Ios,
}

/// Marker trait for application-defined theme extensions.
pub trait ThemeExtension: Clone + fmt::Debug + PartialEq + 'static {
    fn lerp(&self, other: &Self, t: f32) -> Self;
}

/// Animation/page-transition policy. It is intentionally data-only; the
/// runtime owns controllers and frame scheduling.
#[derive(Clone, Debug, Default, PartialEq, TypedBuilder)]
pub struct PageTransitionsTheme {
    #[builder(default)]
    pub duration: Duration,
    #[builder(default)]
    pub reduced_motion: bool,
}

/// Central Material theme. Component fields are present even when a component
/// has no explicit override, matching Flutter's resolved-theme shape.
#[derive(Clone, Debug, PartialEq)]
pub struct ThemeData {
    pub brightness: Brightness,
    /// Optional Cupertino bridge payload. The Cupertino crate is deliberately
    /// not a dependency of Material, so applications can carry an opaque
    /// extension value here and let the platform adapter interpret it.
    pub cupertino_override_theme: Option<ThemeExtensionValue>,
    /// Application-defined adaptive policies keyed by their Rust type name.
    pub adaptation_map: BTreeMap<String, ThemeExtensionValue>,
    pub color_scheme: ColorScheme,
    pub text_theme: TextTheme,
    pub primary_text_theme: TextTheme,
    pub typography: Typography,
    pub use_material3: bool,
    pub visual_density: VisualDensity,
    pub material_tap_target_size: MaterialTapTargetSize,
    pub platform: TargetPlatform,
    pub page_transitions_theme: PageTransitionsTheme,
    pub splash_factory: SplashFactory,
    pub apply_elevation_overlay_color: bool,
    pub canvas_color: Color,
    pub card_color: Color,
    pub disabled_color: Color,
    pub divider_color: Color,
    pub focus_color: Color,
    pub highlight_color: Color,
    pub hint_color: Color,
    pub hover_color: Color,
    pub primary_color: Color,
    pub primary_color_dark: Color,
    pub primary_color_light: Color,
    pub scaffold_background_color: Color,
    pub secondary_header_color: Color,
    pub shadow_color: Color,
    pub splash_color: Color,
    pub unselected_widget_color: Color,
    pub dialog_background_color: Color,
    pub indicator_color: Color,
    pub icon_theme: IconThemeData,
    pub primary_icon_theme: IconThemeData,
    pub input_decoration_theme: InputDecorationThemeData,
    pub action_icon_theme: Option<ActionIconThemeData>,
    pub app_bar_theme: AppBarThemeData,
    pub badge_theme: BadgeThemeData,
    pub banner_theme: BannerThemeData,
    pub bottom_app_bar_theme: BottomAppBarThemeData,
    pub bottom_navigation_bar_theme: BottomNavigationBarThemeData,
    pub bottom_sheet_theme: BottomSheetThemeData,
    pub button_theme: ButtonThemeData,
    pub card_theme: CardThemeData,
    pub carousel_view_theme: CarouselViewThemeData,
    pub checkbox_theme: CheckboxThemeData,
    pub chip_theme: ChipThemeData,
    pub data_table_theme: DataTableThemeData,
    pub date_picker_theme: DatePickerThemeData,
    pub dialog_theme: DialogThemeData,
    pub divider_theme: DividerThemeData,
    pub drawer_theme: DrawerThemeData,
    pub dropdown_menu_theme: crate::menus::DropdownMenuThemeData,
    pub elevated_button_theme: ElevatedButtonThemeData,
    pub expansion_tile_theme: ExpansionTileThemeData,
    pub filled_button_theme: FilledButtonThemeData,
    pub floating_action_button_theme: FloatingActionButtonThemeData,
    pub icon_button_theme: IconButtonThemeData,
    pub list_tile_theme: ListTileThemeData,
    pub menu_bar_theme: MenuBarThemeData,
    pub menu_button_theme: MenuButtonThemeData,
    pub menu_theme: crate::menus::MenuThemeData,
    pub navigation_bar_theme: NavigationBarThemeData,
    pub navigation_drawer_theme: NavigationDrawerThemeData,
    pub navigation_rail_theme: NavigationRailThemeData,
    pub outlined_button_theme: OutlinedButtonThemeData,
    pub popup_menu_theme: crate::menus::PopupMenuThemeData,
    pub progress_indicator_theme: crate::feedback::ProgressIndicatorThemeData,
    pub radio_theme: RadioThemeData,
    pub search_bar_theme: SearchBarThemeData,
    pub search_view_theme: SearchViewThemeData,
    pub segmented_button_theme: SegmentedButtonThemeData,
    pub slider_theme: crate::p0_controls::SliderThemeData,
    pub snack_bar_theme: SnackBarThemeData,
    pub switch_theme: SwitchThemeData,
    pub tab_bar_theme: crate::p0_controls::TabBarThemeData,
    pub text_button_theme: TextButtonThemeData,
    pub text_selection_theme: TextSelectionThemeData,
    pub time_picker_theme: TimePickerThemeData,
    pub toggle_buttons_theme: ToggleButtonsThemeData,
    pub tooltip_theme: TooltipThemeData,
    pub button_bar_theme: Option<ButtonBarThemeData>,
    pub extensions: BTreeMap<String, ThemeExtensionValue>,
}

/// The fields that are most commonly changed when deriving a theme.
#[derive(Clone, Debug, Default, PartialEq, TypedBuilder)]
#[builder(field_defaults(default, setter(strip_option, into)))]
pub struct ThemeDataPatch {
    pub brightness: Option<Brightness>,
    pub cupertino_override_theme: Option<ThemeExtensionValue>,
    pub adaptation_map: Option<BTreeMap<String, ThemeExtensionValue>>,
    pub color_scheme: Option<ColorScheme>,
    pub text_theme: Option<TextTheme>,
    pub primary_text_theme: Option<TextTheme>,
    pub icon_theme: Option<IconThemeData>,
    pub primary_icon_theme: Option<IconThemeData>,
    pub text_selection_theme: Option<TextSelectionThemeData>,
    pub use_material3: Option<bool>,
    pub visual_density: Option<VisualDensity>,
    pub material_tap_target_size: Option<MaterialTapTargetSize>,
    pub platform: Option<TargetPlatform>,
    pub page_transitions_theme: Option<PageTransitionsTheme>,
    pub splash_factory: Option<SplashFactory>,
    pub input_decoration_theme: Option<InputDecorationThemeData>,
    pub badge_theme: Option<BadgeThemeData>,
    pub app_bar_theme: Option<AppBarThemeData>,
    pub bottom_app_bar_theme: Option<BottomAppBarThemeData>,
    pub bottom_navigation_bar_theme: Option<BottomNavigationBarThemeData>,
    pub card_theme: Option<CardThemeData>,
    pub checkbox_theme: Option<CheckboxThemeData>,
    pub dialog_theme: Option<DialogThemeData>,
    pub divider_theme: Option<DividerThemeData>,
    pub drawer_theme: Option<DrawerThemeData>,
    pub dropdown_menu_theme: Option<crate::menus::DropdownMenuThemeData>,
    pub elevated_button_theme: Option<ElevatedButtonThemeData>,
    pub filled_button_theme: Option<FilledButtonThemeData>,
    pub floating_action_button_theme: Option<FloatingActionButtonThemeData>,
    pub icon_button_theme: Option<IconButtonThemeData>,
    pub list_tile_theme: Option<ListTileThemeData>,
    pub menu_bar_theme: Option<MenuBarThemeData>,
    pub menu_button_theme: Option<MenuButtonThemeData>,
    pub menu_theme: Option<crate::menus::MenuThemeData>,
    pub navigation_bar_theme: Option<NavigationBarThemeData>,
    pub navigation_drawer_theme: Option<NavigationDrawerThemeData>,
    pub navigation_rail_theme: Option<NavigationRailThemeData>,
    pub outlined_button_theme: Option<OutlinedButtonThemeData>,
    pub popup_menu_theme: Option<crate::menus::PopupMenuThemeData>,
    pub progress_indicator_theme: Option<crate::feedback::ProgressIndicatorThemeData>,
    pub radio_theme: Option<RadioThemeData>,
    pub slider_theme: Option<crate::p0_controls::SliderThemeData>,
    pub snack_bar_theme: Option<SnackBarThemeData>,
    pub switch_theme: Option<SwitchThemeData>,
    pub tab_bar_theme: Option<crate::p0_controls::TabBarThemeData>,
    pub text_button_theme: Option<TextButtonThemeData>,
    pub tooltip_theme: Option<TooltipThemeData>,
    pub button_bar_theme: Option<Option<ButtonBarThemeData>>,
}

impl ThemeData {
    #[must_use]
    pub fn light() -> Self {
        Self::from_color_scheme(ColorScheme::light())
    }

    #[must_use]
    pub fn dark() -> Self {
        Self::from_color_scheme(ColorScheme::dark())
    }

    /// Creates a light theme directly in shared heap storage. ThemeData is a
    /// deliberately rich compatibility descriptor; keeping this constructor
    /// shared avoids moving its large value through the native UI stack.
    #[must_use]
    pub fn light_shared() -> Rc<Self> {
        Self::from_color_scheme_shared(ColorScheme::light())
    }

    /// Creates a dark theme directly in shared heap storage.
    #[must_use]
    pub fn dark_shared() -> Rc<Self> {
        Self::from_color_scheme_shared(ColorScheme::dark())
    }

    #[must_use]
    pub fn fallback() -> Self {
        Self::light()
    }

    #[must_use]
    pub fn from_seed(seed_color: Color) -> Self {
        Self::from_color_scheme(ColorScheme::from_seed(seed_color))
    }

    /// Creates a seeded theme directly in shared heap storage.
    #[must_use]
    pub fn from_seed_shared(seed_color: Color) -> Rc<Self> {
        Self::from_color_scheme_shared(ColorScheme::from_seed(seed_color))
    }

    /// Heap-backed counterpart to [`Self::from_color_scheme`]. Each field is
    /// written into the allocation so no full ThemeData temporary is needed
    /// on the caller's stack.
    #[must_use]
    pub fn from_color_scheme_shared(color_scheme: ColorScheme) -> Rc<Self> {
        let brightness = color_scheme.brightness;
        let text_theme = match brightness {
            Brightness::Light => TextTheme::light(),
            Brightness::Dark => TextTheme::dark(),
        };
        let control_theme = ControlTheme::default();
        let mut shared = Rc::<Self>::new_uninit();
        let ptr = Rc::get_mut(&mut shared)
            .expect("new theme allocation has one owner")
            .as_mut_ptr();
        macro_rules! write_field {
            ($field:ident, $value:expr) => {
                // `shared` is a unique, uninitialized allocation and every
                // field is written exactly once before assume_init below.
                unsafe { std::ptr::addr_of_mut!((*ptr).$field).write($value) };
            };
        }
        write_field!(brightness, brightness);
        write_field!(cupertino_override_theme, None);
        write_field!(adaptation_map, BTreeMap::new());
        write_field!(color_scheme, color_scheme);
        write_field!(text_theme, text_theme.clone());
        write_field!(primary_text_theme, text_theme);
        write_field!(typography, Typography::default());
        write_field!(use_material3, true);
        write_field!(visual_density, VisualDensity::default());
        write_field!(material_tap_target_size, MaterialTapTargetSize::default());
        write_field!(platform, TargetPlatform::default());
        write_field!(page_transitions_theme, PageTransitionsTheme::default());
        write_field!(splash_factory, SplashFactory::Ripple);
        write_field!(
            apply_elevation_overlay_color,
            brightness == Brightness::Dark
        );
        write_field!(canvas_color, color_scheme.surface);
        write_field!(card_color, color_scheme.surface_container_low);
        write_field!(disabled_color, alpha(color_scheme.on_surface, 0.38));
        write_field!(divider_color, color_scheme.outline_variant);
        write_field!(focus_color, alpha(color_scheme.primary, 0.12));
        write_field!(highlight_color, alpha(color_scheme.primary, 0.12));
        write_field!(hint_color, color_scheme.on_surface_variant);
        write_field!(hover_color, alpha(color_scheme.primary, 0.08));
        write_field!(primary_color, color_scheme.primary);
        write_field!(primary_color_dark, color_scheme.primary_container);
        write_field!(primary_color_light, color_scheme.primary_fixed);
        write_field!(scaffold_background_color, color_scheme.surface);
        write_field!(secondary_header_color, color_scheme.secondary_container);
        write_field!(shadow_color, color_scheme.shadow);
        write_field!(splash_color, alpha(color_scheme.primary, 0.16));
        write_field!(unselected_widget_color, color_scheme.on_surface_variant);
        write_field!(dialog_background_color, color_scheme.surface_container_high);
        write_field!(indicator_color, color_scheme.primary);
        write_field!(icon_theme, ComponentThemeData::default());
        write_field!(primary_icon_theme, ComponentThemeData::default());
        write_field!(input_decoration_theme, InputDecorationThemeData::default());
        write_field!(action_icon_theme, None);
        write_field!(app_bar_theme, ComponentThemeData::default());
        write_field!(badge_theme, ComponentThemeData::default());
        write_field!(banner_theme, ComponentThemeData::default());
        write_field!(bottom_app_bar_theme, ComponentThemeData::default());
        write_field!(bottom_navigation_bar_theme, ComponentThemeData::default());
        write_field!(bottom_sheet_theme, ComponentThemeData::default());
        write_field!(button_theme, ButtonThemeData::default());
        write_field!(card_theme, ComponentThemeData::default());
        write_field!(carousel_view_theme, ComponentThemeData::default());
        write_field!(checkbox_theme, ComponentThemeData::default());
        write_field!(chip_theme, ComponentThemeData::default());
        write_field!(data_table_theme, ComponentThemeData::default());
        write_field!(date_picker_theme, ComponentThemeData::default());
        write_field!(dialog_theme, ComponentThemeData::default());
        write_field!(divider_theme, ComponentThemeData::default());
        write_field!(drawer_theme, ComponentThemeData::default());
        write_field!(
            dropdown_menu_theme,
            crate::menus::DropdownMenuThemeData::default()
        );
        write_field!(elevated_button_theme, ComponentThemeData::default());
        write_field!(expansion_tile_theme, ComponentThemeData::default());
        write_field!(filled_button_theme, ComponentThemeData::default());
        write_field!(floating_action_button_theme, ComponentThemeData::default());
        write_field!(icon_button_theme, ComponentThemeData::default());
        write_field!(list_tile_theme, ComponentThemeData::default());
        write_field!(menu_bar_theme, ComponentThemeData::default());
        write_field!(menu_button_theme, ComponentThemeData::default());
        write_field!(menu_theme, crate::menus::MenuThemeData::default());
        write_field!(navigation_bar_theme, ComponentThemeData::default());
        write_field!(navigation_drawer_theme, ComponentThemeData::default());
        write_field!(navigation_rail_theme, ComponentThemeData::default());
        write_field!(outlined_button_theme, ComponentThemeData::default());
        write_field!(
            popup_menu_theme,
            crate::menus::PopupMenuThemeData::default()
        );
        write_field!(
            progress_indicator_theme,
            crate::feedback::ProgressIndicatorThemeData::default()
        );
        write_field!(radio_theme, ComponentThemeData::default());
        write_field!(search_bar_theme, ComponentThemeData::default());
        write_field!(search_view_theme, ComponentThemeData::default());
        write_field!(segmented_button_theme, ComponentThemeData::default());
        write_field!(slider_theme, crate::p0_controls::SliderThemeData::default());
        write_field!(snack_bar_theme, ComponentThemeData::default());
        write_field!(switch_theme, ComponentThemeData::default());
        write_field!(
            tab_bar_theme,
            crate::p0_controls::TabBarThemeData::default()
        );
        write_field!(text_button_theme, ComponentThemeData::default());
        write_field!(text_selection_theme, ComponentThemeData::default());
        write_field!(time_picker_theme, ComponentThemeData::default());
        write_field!(toggle_buttons_theme, ComponentThemeData::default());
        write_field!(tooltip_theme, ComponentThemeData::default());
        write_field!(button_bar_theme, None);
        write_field!(extensions, BTreeMap::new());
        let mut shared = unsafe { shared.assume_init() };
        Rc::get_mut(&mut shared)
            .expect("new theme allocation remains uniquely owned")
            .with_control_defaults(control_theme);
        shared
    }

    #[must_use]
    pub fn from_color_scheme(color_scheme: ColorScheme) -> Self {
        let brightness = color_scheme.brightness;
        let text_theme = match brightness {
            Brightness::Light => TextTheme::light(),
            Brightness::Dark => TextTheme::dark(),
        };
        let control_theme = ControlTheme::default();
        let mut theme = Self {
            brightness,
            cupertino_override_theme: None,
            adaptation_map: BTreeMap::new(),
            primary_text_theme: text_theme.clone(),
            text_theme,
            typography: Typography::default(),
            use_material3: true,
            visual_density: VisualDensity::default(),
            material_tap_target_size: MaterialTapTargetSize::default(),
            platform: TargetPlatform::default(),
            page_transitions_theme: PageTransitionsTheme::default(),
            splash_factory: SplashFactory::Ripple,
            apply_elevation_overlay_color: brightness == Brightness::Dark,
            canvas_color: color_scheme.surface,
            card_color: color_scheme.surface_container_low,
            disabled_color: alpha(color_scheme.on_surface, 0.38),
            divider_color: color_scheme.outline_variant,
            focus_color: alpha(color_scheme.primary, 0.12),
            highlight_color: alpha(color_scheme.primary, 0.12),
            hint_color: color_scheme.on_surface_variant,
            hover_color: alpha(color_scheme.primary, 0.08),
            primary_color: color_scheme.primary,
            primary_color_dark: color_scheme.primary_container,
            primary_color_light: color_scheme.primary_fixed,
            scaffold_background_color: color_scheme.surface,
            secondary_header_color: color_scheme.secondary_container,
            shadow_color: color_scheme.shadow,
            splash_color: alpha(color_scheme.primary, 0.16),
            unselected_widget_color: color_scheme.on_surface_variant,
            dialog_background_color: color_scheme.surface_container_high,
            indicator_color: color_scheme.primary,
            color_scheme,
            icon_theme: ComponentThemeData::default(),
            primary_icon_theme: ComponentThemeData::default(),
            input_decoration_theme: InputDecorationThemeData::default(),
            action_icon_theme: None,
            app_bar_theme: ComponentThemeData::default(),
            badge_theme: ComponentThemeData::default(),
            banner_theme: ComponentThemeData::default(),
            bottom_app_bar_theme: ComponentThemeData::default(),
            bottom_navigation_bar_theme: ComponentThemeData::default(),
            bottom_sheet_theme: ComponentThemeData::default(),
            button_theme: ButtonThemeData::default(),
            card_theme: ComponentThemeData::default(),
            carousel_view_theme: ComponentThemeData::default(),
            checkbox_theme: ComponentThemeData::default(),
            chip_theme: ComponentThemeData::default(),
            data_table_theme: ComponentThemeData::default(),
            date_picker_theme: ComponentThemeData::default(),
            dialog_theme: ComponentThemeData::default(),
            divider_theme: ComponentThemeData::default(),
            drawer_theme: ComponentThemeData::default(),
            dropdown_menu_theme: crate::menus::DropdownMenuThemeData::default(),
            elevated_button_theme: ComponentThemeData::default(),
            expansion_tile_theme: ComponentThemeData::default(),
            filled_button_theme: ComponentThemeData::default(),
            floating_action_button_theme: ComponentThemeData::default(),
            icon_button_theme: ComponentThemeData::default(),
            list_tile_theme: ComponentThemeData::default(),
            menu_bar_theme: ComponentThemeData::default(),
            menu_button_theme: ComponentThemeData::default(),
            menu_theme: crate::menus::MenuThemeData::default(),
            navigation_bar_theme: ComponentThemeData::default(),
            navigation_drawer_theme: ComponentThemeData::default(),
            navigation_rail_theme: ComponentThemeData::default(),
            outlined_button_theme: ComponentThemeData::default(),
            popup_menu_theme: crate::menus::PopupMenuThemeData::default(),
            progress_indicator_theme: crate::feedback::ProgressIndicatorThemeData::default(),
            radio_theme: ComponentThemeData::default(),
            search_bar_theme: ComponentThemeData::default(),
            search_view_theme: ComponentThemeData::default(),
            segmented_button_theme: ComponentThemeData::default(),
            slider_theme: crate::p0_controls::SliderThemeData::default(),
            snack_bar_theme: ComponentThemeData::default(),
            switch_theme: ComponentThemeData::default(),
            tab_bar_theme: crate::p0_controls::TabBarThemeData::default(),
            text_button_theme: ComponentThemeData::default(),
            text_selection_theme: ComponentThemeData::default(),
            time_picker_theme: ComponentThemeData::default(),
            toggle_buttons_theme: ComponentThemeData::default(),
            tooltip_theme: ComponentThemeData::default(),
            button_bar_theme: None,
            extensions: BTreeMap::new(),
        };
        theme.with_control_defaults(control_theme);
        theme
    }

    fn with_control_defaults(&mut self, control_theme: ControlTheme) {
        self.elevated_button_theme.elevation = Some(control_theme.elevation.popup.min(6.0));
        self.card_theme.elevation = Some(control_theme.elevation.popup.min(1.0));
    }

    #[must_use]
    pub fn copy_with(mut self, patch: ThemeDataPatch) -> Self {
        if let Some(value) = patch.brightness {
            self.brightness = value;
        }
        if let Some(value) = patch.cupertino_override_theme {
            self.cupertino_override_theme = Some(value);
        }
        if let Some(value) = patch.adaptation_map {
            self.adaptation_map = value;
        }
        if let Some(value) = patch.color_scheme {
            self.brightness = value.brightness;
            self.color_scheme = value;
        }
        if let Some(value) = patch.text_theme {
            self.text_theme = value;
        }
        if let Some(value) = patch.primary_text_theme {
            self.primary_text_theme = value;
        }
        if let Some(value) = patch.icon_theme {
            self.icon_theme = value;
        }
        if let Some(value) = patch.primary_icon_theme {
            self.primary_icon_theme = value;
        }
        if let Some(value) = patch.text_selection_theme {
            self.text_selection_theme = value;
        }
        if let Some(value) = patch.use_material3 {
            self.use_material3 = value;
        }
        if let Some(value) = patch.visual_density {
            self.visual_density = value;
        }
        if let Some(value) = patch.material_tap_target_size {
            self.material_tap_target_size = value;
        }
        if let Some(value) = patch.platform {
            self.platform = value;
        }
        if let Some(value) = patch.page_transitions_theme {
            self.page_transitions_theme = value;
        }
        if let Some(value) = patch.splash_factory {
            self.splash_factory = value;
        }
        if let Some(value) = patch.input_decoration_theme {
            self.input_decoration_theme = value;
        }
        macro_rules! component_patch {
            ($field:ident) => {
                if let Some(value) = patch.$field {
                    self.$field = value;
                }
            };
        }
        component_patch!(badge_theme);
        component_patch!(app_bar_theme);
        component_patch!(bottom_app_bar_theme);
        component_patch!(bottom_navigation_bar_theme);
        component_patch!(card_theme);
        component_patch!(checkbox_theme);
        component_patch!(dialog_theme);
        component_patch!(divider_theme);
        component_patch!(drawer_theme);
        component_patch!(dropdown_menu_theme);
        component_patch!(elevated_button_theme);
        component_patch!(filled_button_theme);
        component_patch!(floating_action_button_theme);
        component_patch!(icon_button_theme);
        component_patch!(list_tile_theme);
        component_patch!(menu_bar_theme);
        component_patch!(menu_button_theme);
        component_patch!(menu_theme);
        component_patch!(navigation_bar_theme);
        component_patch!(navigation_drawer_theme);
        component_patch!(navigation_rail_theme);
        component_patch!(outlined_button_theme);
        component_patch!(popup_menu_theme);
        component_patch!(progress_indicator_theme);
        component_patch!(radio_theme);
        component_patch!(slider_theme);
        component_patch!(snack_bar_theme);
        component_patch!(switch_theme);
        component_patch!(tab_bar_theme);
        component_patch!(text_button_theme);
        component_patch!(tooltip_theme);
        if let Some(value) = patch.button_bar_theme {
            self.button_bar_theme = value;
        }
        self
    }

    #[must_use]
    pub fn with_color_scheme(mut self, value: ColorScheme) -> Self {
        self.brightness = value.brightness;
        self.color_scheme = value;
        self
    }
    #[must_use]
    pub fn with_use_material3(mut self, value: bool) -> Self {
        self.use_material3 = value;
        self
    }
    #[must_use]
    pub fn with_visual_density(mut self, value: VisualDensity) -> Self {
        self.visual_density = value;
        self
    }
    #[must_use]
    pub fn with_tap_target_size(mut self, value: MaterialTapTargetSize) -> Self {
        self.material_tap_target_size = value;
        self
    }
    #[must_use]
    pub fn with_platform(mut self, value: TargetPlatform) -> Self {
        self.platform = value;
        self
    }
    #[must_use]
    pub fn with_page_transitions_theme(mut self, value: PageTransitionsTheme) -> Self {
        self.page_transitions_theme = value;
        self
    }
    #[must_use]
    pub fn with_splash_factory(mut self, value: SplashFactory) -> Self {
        self.splash_factory = value;
        self
    }
    #[must_use]
    pub fn with_input_decoration_theme(mut self, value: InputDecorationThemeData) -> Self {
        self.input_decoration_theme = value;
        self
    }
    #[must_use]
    pub fn with_button_bar_theme(mut self, value: Option<ButtonBarThemeData>) -> Self {
        self.button_bar_theme = value;
        self
    }
    #[must_use]
    pub fn with_extension(mut self, key: impl Into<String>, value: ThemeExtensionValue) -> Self {
        self.extensions.insert(key.into(), value);
        self
    }

    #[must_use]
    pub fn lerp(&self, other: &Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let mut result = if t < 0.5 { self.clone() } else { other.clone() };
        result.brightness = if t < 0.5 {
            self.brightness
        } else {
            other.brightness
        };
        result.color_scheme = self.color_scheme.lerp(other.color_scheme, t);
        result.text_theme = self.text_theme.lerp(&other.text_theme, t);
        result.primary_text_theme = self.primary_text_theme.lerp(&other.primary_text_theme, t);
        result.visual_density = self.visual_density.lerp(other.visual_density, t);
        result
    }

    /// Converts the Material theme into the existing controls theme. This is
    /// the only bridge from Material defaults to headless control behavior.
    #[must_use]
    pub fn control_theme(&self) -> ControlTheme {
        let mut controls = if self.brightness == Brightness::Light {
            ControlTheme::light()
        } else {
            ControlTheme::dark()
        };
        controls = controls.with_palette(incular_controls::ControlColors {
            background: self.scaffold_background_color,
            surface: self.color_scheme.surface,
            surface_variant: self.color_scheme.surface_container,
            surface_elevated: self.color_scheme.surface_container_high,
            surface_active: self.color_scheme.surface_container_highest,
            foreground: self.color_scheme.on_surface,
            foreground_muted: self.color_scheme.on_surface_variant,
            foreground_disabled: self.disabled_color,
            accent: self.color_scheme.primary,
            accent_hover: alpha(self.color_scheme.primary, 0.92),
            accent_active: self.color_scheme.primary_container,
            accent_foreground: self.color_scheme.on_primary,
            border: self.color_scheme.outline,
            border_subtle: self.color_scheme.outline_variant,
            border_strong: self.color_scheme.outline,
            hover_overlay: self.hover_color,
            pressed_overlay: self.splash_color,
            focus_ring: self.focus_color,
            selection: alpha(self.color_scheme.primary, 0.24),
            disabled_surface: self.color_scheme.surface_container_highest,
            disabled_foreground: self.disabled_color,
            error: self.color_scheme.error,
            warning: Color::rgba(160, 100, 0, 255),
            success: Color::rgba(30, 120, 60, 255),
            info: self.color_scheme.primary,
        });
        controls.density = if self.visual_density == VisualDensity::COMPACT {
            incular_controls::ControlDensity::Compact
        } else if self.visual_density == VisualDensity::COMFORTABLE {
            incular_controls::ControlDensity::Comfortable
        } else {
            incular_controls::ControlDensity::Standard
        };
        // Slider mechanics stay in `incular-controls`; Material's sparse
        // slider theme supplies the dimensions and common state colors used
        // by that shared renderer.
        if let Some(height) = self.slider_theme.track_height {
            controls.slider.track_height = height.max(1.0);
        }
        if let Some(size) = self.slider_theme.thumb_size {
            controls.slider.thumb_size = size.max(1.0);
        }
        if let Some(color) = self
            .slider_theme
            .active_track_color
            .as_ref()
            .map(|property| property.resolve(WidgetStates::default()))
        {
            controls.colors.accent = color;
        }
        if let Some(color) = self
            .slider_theme
            .inactive_track_color
            .as_ref()
            .map(|property| property.resolve(WidgetStates::default()))
        {
            controls.colors.border_strong = color;
        }
        if let Some(color) = self.slider_theme.disabled_active_track_color {
            controls.colors.disabled_foreground = color;
        }
        if let Some(size) = self.checkbox_theme.icon_size {
            controls.checkbox.indicator_size = size.max(1.0);
        }
        if let Some(size) = self.radio_theme.icon_size {
            controls.radio.indicator_size = size.max(1.0);
        }
        if let Some(width) = self.switch_theme.minimum_size.map(|size| size.width) {
            controls.switch.width = width.max(1.0);
        }
        if let Some(height) = self.switch_theme.minimum_size.map(|size| size.height) {
            controls.switch.height = height.max(1.0);
        }
        if let Some(size) = self.switch_theme.thumb_size {
            controls.switch.thumb_size = size.max(1.0);
        }
        if let Some(fill) = self
            .checkbox_theme
            .fill_color
            .as_ref()
            .map(|property| property.resolve(incular_controls::ControlState::empty()))
        {
            controls.colors.accent = fill;
        }
        if let Some(check) = self
            .checkbox_theme
            .check_color
            .as_ref()
            .map(|property| property.resolve(incular_controls::ControlState::empty()))
        {
            controls.colors.accent_foreground = check;
        }
        controls
    }
}

impl Default for ThemeData {
    fn default() -> Self {
        Self::light()
    }
}

/// Inherited Material theme scope.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct Theme {
    #[builder(setter(into))]
    pub data: ThemeData,
    #[builder(setter(into))]
    pub child: Widget,
}

impl Theme {
    #[must_use]
    pub fn new(data: ThemeData, child: impl Into<Widget>) -> Self {
        Self {
            data,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn of() -> Option<ThemeData> {
        Self::of_shared().map(|theme| (*theme).clone()).or_else(|| {
            incular_widgets::internal::current_build_environment_boxed::<ThemeData>()
                .map(|theme| *theme)
        })
    }

    /// Reads the ambient theme without copying the large theme descriptor onto
    /// the native UI stack. Material application roots use this shared form.
    #[must_use]
    pub(crate) fn of_shared() -> Option<Rc<ThemeData>> {
        incular_widgets::internal::current_build_environment::<Rc<ThemeData>>()
    }

    /// Installs one shared theme descriptor plus the derived control scopes.
    pub(crate) fn scope_shared(data: Rc<ThemeData>, child: Widget) -> Widget {
        let controls = data.control_theme();
        let input_decoration = data.input_decoration_theme.clone();
        Widget::environment_scope(
            data,
            Widget::environment_scope(input_decoration, Widget::environment_scope(controls, child)),
        )
    }
}

impl From<Theme> for Widget {
    fn from(value: Theme) -> Self {
        Theme::scope_shared(Rc::new(value.data), value.child)
    }
}

/// Data-only animated theme descriptor. The runtime can interpolate the
/// supplied endpoints without forcing components to own animation state.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct AnimatedTheme {
    #[builder(setter(into))]
    pub data: ThemeData,
    pub duration: Duration,
    #[builder(setter(into))]
    pub child: Widget,
}

impl AnimatedTheme {
    #[must_use]
    pub fn new(data: ThemeData, duration: Duration, child: impl Into<Widget>) -> Self {
        Self {
            data,
            duration,
            child: child.into(),
        }
    }
}

impl From<AnimatedTheme> for Widget {
    fn from(value: AnimatedTheme) -> Self {
        Theme::new(value.data, value.child).into()
    }
}

/// Theme data for the legacy `ButtonTheme` API.
#[derive(Clone, Debug, Default, PartialEq, TypedBuilder)]
#[builder(field_defaults(default, setter(strip_option, into)))]
pub struct ButtonThemeData {
    pub style: Option<ButtonStyle>,
    pub height: Option<f32>,
    pub min_width: Option<f32>,
    pub shape: Option<BorderRadius>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_property_prefers_more_specific_state() {
        let property =
            StateProperty::from_map([(WidgetState::Hovered, 1_u32), (WidgetState::Pressed, 2_u32)]);
        let states = WidgetStates::new()
            .with(WidgetState::Hovered)
            .with(WidgetState::Pressed);
        assert_eq!(property.resolve(states), 2);
    }

    #[test]
    fn visual_density_adjusts_both_axes() {
        let constraints = Constraints::tight(Size::new(48.0, 48.0));
        let compact = VisualDensity::COMPACT.effective_constraints(constraints);
        assert_eq!(compact.min_width, 40.0);
        assert_eq!(compact.min_height, 40.0);
    }

    #[test]
    fn seeded_scheme_has_complete_surface_roles() {
        let scheme = ColorScheme::from_seed(Color::rgba(0x67, 0x50, 0xa4, 255));
        assert_eq!(scheme.background, scheme.surface);
        assert_eq!(scheme.on_background, scheme.on_surface);
        assert_ne!(scheme.primary, Color::TRANSPARENT);
        assert_ne!(scheme.surface_container_highest, Color::TRANSPARENT);
    }

    #[test]
    fn theme_precedence_is_sparse() {
        let theme = ThemeData::light().copy_with(ThemeDataPatch {
            use_material3: Some(false),
            ..ThemeDataPatch::default()
        });
        assert!(!theme.use_material3);
        assert_eq!(theme.color_scheme, ColorScheme::light());
    }
}
