use crate::theme::ControlTheme;
use incular_config::EdgeInsets;
use incular_core::Color;
use incular_text::TextStyle;
use incular_widgets::Border;
use std::sync::Arc;

/// Interactive state flags shared by every control.
///
/// A compact bitset keeps state transitions cheap and makes it possible for
/// a component to expose new states without growing a large collection of
/// boolean fields.  The flags are deliberately renderer independent: control
/// implementations resolve them into paint, semantics, and hit-test policy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ControlState(u16);

impl ControlState {
    pub const DISABLED: Self = Self(1 << 0);
    pub const HOVERED: Self = Self(1 << 1);
    pub const PRESSED: Self = Self(1 << 2);
    pub const FOCUSED: Self = Self(1 << 3);
    pub const CHECKED: Self = Self(1 << 4);
    pub const SELECTED: Self = Self(1 << 5);
    pub const EXPANDED: Self = Self(1 << 6);
    pub const INVALID: Self = Self(1 << 7);
    pub const READ_ONLY: Self = Self(1 << 8);
    pub const DRAGGING: Self = Self(1 << 9);
    pub const OPEN: Self = Self(1 << 10);
    pub const FOCUS_VISIBLE: Self = Self(1 << 11);

    #[must_use]
    pub const fn new() -> Self {
        Self::empty()
    }

    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    #[must_use]
    pub const fn bits(self) -> u16 {
        self.0
    }

    #[must_use]
    pub const fn from_bits_retain(bits: u16) -> Self {
        Self(bits)
    }

    #[must_use]
    pub const fn intersects(self, flags: Self) -> bool {
        self.0 & flags.0 != 0
    }

    #[must_use]
    pub const fn contains(self, flags: Self) -> bool {
        self.0 & flags.0 == flags.0
    }

    #[must_use]
    pub const fn with(self, flags: Self) -> Self {
        Self(self.0 | flags.0)
    }

    #[must_use]
    pub const fn without(self, flags: Self) -> Self {
        Self(self.0 & !flags.0)
    }

    pub const fn insert(&mut self, flags: Self) {
        self.0 |= flags.0;
    }

    pub const fn remove(&mut self, flags: Self) {
        self.0 &= !flags.0;
    }

    #[must_use]
    pub const fn is_disabled(self) -> bool {
        self.contains(Self::DISABLED)
    }
    #[must_use]
    pub const fn is_hovered(self) -> bool {
        self.contains(Self::HOVERED)
    }
    #[must_use]
    pub const fn is_pressed(self) -> bool {
        self.contains(Self::PRESSED)
    }
    #[must_use]
    pub const fn is_focused(self) -> bool {
        self.contains(Self::FOCUSED)
    }
    #[must_use]
    pub const fn is_checked(self) -> bool {
        self.contains(Self::CHECKED)
    }
    #[must_use]
    pub const fn is_selected(self) -> bool {
        self.contains(Self::SELECTED)
    }
    #[must_use]
    pub const fn is_expanded(self) -> bool {
        self.contains(Self::EXPANDED)
    }
    #[must_use]
    pub const fn is_invalid(self) -> bool {
        self.contains(Self::INVALID)
    }
    #[must_use]
    pub const fn is_read_only(self) -> bool {
        self.contains(Self::READ_ONLY)
    }
    #[must_use]
    pub const fn is_open(self) -> bool {
        self.contains(Self::OPEN)
    }
    #[must_use]
    pub const fn is_dragging(self) -> bool {
        self.contains(Self::DRAGGING)
    }
    #[must_use]
    pub const fn is_focus_visible(self) -> bool {
        self.contains(Self::FOCUS_VISIBLE)
    }

    #[must_use]
    pub const fn from_enabled(enabled: bool) -> Self {
        if enabled {
            Self::empty()
        } else {
            Self::DISABLED
        }
    }
}

impl std::ops::BitOr for ControlState {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for ControlState {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl std::ops::BitAnd for ControlState {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

/// A value that can be resolved from a control's interaction state.
///
/// `Value` is allocation-free for the common static case. `States` handles
/// the usual state matrix without a callback, while `Resolver` is available
/// for application-specific state combinations.
#[derive(Clone)]
pub enum StateValue<T> {
    Value(T),
    States(StateTable<T>),
    Resolver(Arc<dyn Fn(ControlState) -> T + Send + Sync + 'static>),
}

impl<T: Clone> StateValue<T> {
    #[must_use]
    pub fn resolve(&self, state: ControlState) -> T {
        match self {
            Self::Value(value) => value.clone(),
            Self::States(table) => table.resolve(state),
            Self::Resolver(resolver) => resolver(state),
        }
    }

    #[must_use]
    pub fn states(table: StateTable<T>) -> Self {
        Self::States(table)
    }
}

impl<T> StateValue<T> {
    #[must_use]
    pub fn resolver(resolver: impl Fn(ControlState) -> T + Send + Sync + 'static) -> Self {
        Self::Resolver(Arc::new(resolver))
    }
}

impl<T> From<T> for StateValue<T> {
    fn from(value: T) -> Self {
        Self::Value(value)
    }
}

/// Fast common state table. More specific states win over less specific ones.
#[derive(Clone, Debug, PartialEq)]
pub struct StateTable<T> {
    pub normal: T,
    pub hovered: Option<T>,
    pub pressed: Option<T>,
    pub focused: Option<T>,
    pub disabled: Option<T>,
    pub checked: Option<T>,
    pub selected: Option<T>,
    pub invalid: Option<T>,
    pub expanded: Option<T>,
    pub open: Option<T>,
    pub read_only: Option<T>,
    pub dragging: Option<T>,
    pub focus_visible: Option<T>,
}

impl<T> StateTable<T> {
    #[must_use]
    pub fn new(normal: T) -> Self {
        Self {
            normal,
            hovered: None,
            pressed: None,
            focused: None,
            disabled: None,
            checked: None,
            selected: None,
            invalid: None,
            expanded: None,
            open: None,
            read_only: None,
            dragging: None,
            focus_visible: None,
        }
    }

    #[must_use]
    pub fn hovered(mut self, value: T) -> Self {
        self.hovered = Some(value);
        self
    }
    #[must_use]
    pub fn pressed(mut self, value: T) -> Self {
        self.pressed = Some(value);
        self
    }
    #[must_use]
    pub fn focused(mut self, value: T) -> Self {
        self.focused = Some(value);
        self
    }
    #[must_use]
    pub fn disabled(mut self, value: T) -> Self {
        self.disabled = Some(value);
        self
    }
    #[must_use]
    pub fn checked(mut self, value: T) -> Self {
        self.checked = Some(value);
        self
    }
    #[must_use]
    pub fn selected(mut self, value: T) -> Self {
        self.selected = Some(value);
        self
    }
    #[must_use]
    pub fn invalid(mut self, value: T) -> Self {
        self.invalid = Some(value);
        self
    }
    #[must_use]
    pub fn expanded(mut self, value: T) -> Self {
        self.expanded = Some(value);
        self
    }
    #[must_use]
    pub fn open(mut self, value: T) -> Self {
        self.open = Some(value);
        self
    }
    #[must_use]
    pub fn read_only(mut self, value: T) -> Self {
        self.read_only = Some(value);
        self
    }
    #[must_use]
    pub fn dragging(mut self, value: T) -> Self {
        self.dragging = Some(value);
        self
    }
    #[must_use]
    pub fn focus_visible(mut self, value: T) -> Self {
        self.focus_visible = Some(value);
        self
    }
}

impl<T: Clone> StateTable<T> {
    #[must_use]
    pub fn resolve(&self, state: ControlState) -> T {
        if state.is_disabled() {
            if let Some(value) = &self.disabled {
                return value.clone();
            }
        }
        if state.contains(ControlState::INVALID) {
            if let Some(value) = &self.invalid {
                return value.clone();
            }
        }
        if state.contains(ControlState::EXPANDED) {
            if let Some(value) = &self.expanded {
                return value.clone();
            }
        }
        if state.contains(ControlState::OPEN) {
            if let Some(value) = &self.open {
                return value.clone();
            }
        }
        if state.contains(ControlState::READ_ONLY) {
            if let Some(value) = &self.read_only {
                return value.clone();
            }
        }
        if state.contains(ControlState::DRAGGING) {
            if let Some(value) = &self.dragging {
                return value.clone();
            }
        }
        if state.contains(ControlState::FOCUS_VISIBLE) {
            if let Some(value) = &self.focus_visible {
                return value.clone();
            }
        }
        if state.is_pressed() {
            if let Some(value) = &self.pressed {
                return value.clone();
            }
        }
        if state.is_checked() {
            if let Some(value) = &self.checked {
                return value.clone();
            }
        }
        if state.is_selected() {
            if let Some(value) = &self.selected {
                return value.clone();
            }
        }
        if state.is_focused() {
            if let Some(value) = &self.focused {
                return value.clone();
            }
        }
        if state.is_hovered() {
            if let Some(value) = &self.hovered {
                return value.clone();
            }
        }
        self.normal.clone()
    }
}

/// Color specialization that stays compact in ordinary control styles.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StateColor {
    pub normal: Color,
    pub hovered: Option<Color>,
    pub pressed: Option<Color>,
    pub focused: Option<Color>,
    pub disabled: Option<Color>,
    pub checked: Option<Color>,
    pub selected: Option<Color>,
}

impl StateColor {
    #[must_use]
    pub const fn new(normal: Color) -> Self {
        Self {
            normal,
            hovered: None,
            pressed: None,
            focused: None,
            disabled: None,
            checked: None,
            selected: None,
        }
    }

    #[must_use]
    pub const fn hovered(mut self, value: Color) -> Self {
        self.hovered = Some(value);
        self
    }
    #[must_use]
    pub const fn pressed(mut self, value: Color) -> Self {
        self.pressed = Some(value);
        self
    }
    #[must_use]
    pub const fn focused(mut self, value: Color) -> Self {
        self.focused = Some(value);
        self
    }
    #[must_use]
    pub const fn disabled(mut self, value: Color) -> Self {
        self.disabled = Some(value);
        self
    }
    #[must_use]
    pub const fn checked(mut self, value: Color) -> Self {
        self.checked = Some(value);
        self
    }
    #[must_use]
    pub const fn selected(mut self, value: Color) -> Self {
        self.selected = Some(value);
        self
    }

    #[must_use]
    pub fn resolve(self, state: ControlState) -> Color {
        StateTable {
            normal: self.normal,
            hovered: self.hovered,
            pressed: self.pressed,
            focused: self.focused,
            disabled: self.disabled,
            checked: self.checked,
            selected: self.selected,
            invalid: None,
            expanded: None,
            open: None,
            read_only: None,
            dragging: None,
            focus_visible: None,
        }
        .resolve(state)
    }
}

/// Visual variants for buttons.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonVariant {
    /// Standard neutral surface button.
    #[default]
    Default,
    /// Compatibility spelling for [`ButtonVariant::Default`].
    Standard,
    /// High-emphasis action button with accent background.
    Primary,
    /// Transparent chrome button for toolbars and lists.
    Ghost,
    /// Destructive action button.
    Danger,
    /// Base UI-style spelling for a destructive action.
    Destructive,
}

/// Configuration style for Button controls.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ButtonStyle {
    pub variant: ButtonVariant,
    pub background: Option<Color>,
    /// Optional state-aware background palette. When present it takes
    /// precedence over the single `background` value.
    pub background_states: Option<StateColor>,
    pub foreground: Option<Color>,
    /// Optional state-aware foreground palette.
    pub foreground_states: Option<StateColor>,
    pub border: Option<Border>,
    pub border_radius: Option<f32>,
    pub padding: Option<EdgeInsets>,
    pub height: Option<f32>,
    pub text_style: Option<TextStyle>,
}

impl ButtonStyle {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    #[must_use]
    pub fn background(mut self, background: Color) -> Self {
        self.background = Some(background);
        self
    }

    #[must_use]
    pub fn background_states(mut self, background: StateColor) -> Self {
        self.background_states = Some(background);
        self
    }

    #[must_use]
    pub fn foreground(mut self, foreground: Color) -> Self {
        self.foreground = Some(foreground);
        self
    }

    #[must_use]
    pub fn foreground_states(mut self, foreground: StateColor) -> Self {
        self.foreground_states = Some(foreground);
        self
    }

    #[must_use]
    pub fn border(mut self, border: Border) -> Self {
        self.border = Some(border);
        self
    }

    #[must_use]
    pub fn border_radius(mut self, radius: f32) -> Self {
        self.border_radius = Some(radius);
        self
    }

    #[must_use]
    pub fn padding(mut self, padding: EdgeInsets) -> Self {
        self.padding = Some(padding);
        self
    }

    #[must_use]
    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }

    #[must_use]
    pub fn text_style(mut self, style: TextStyle) -> Self {
        self.text_style = Some(style);
        self
    }

    /// Resolves background color against active state and theme.
    #[must_use]
    pub fn resolve_background(&self, state: ControlState, theme: &ControlTheme) -> Color {
        if state.is_disabled() {
            return theme.colors.disabled_surface;
        }
        if let Some(bg) = self.background_states {
            return bg.resolve(state);
        }
        if let Some(bg) = self.background {
            return bg;
        }
        match self.variant {
            ButtonVariant::Default | ButtonVariant::Standard => {
                if state.is_pressed() {
                    theme.colors.surface_active
                } else if state.is_hovered() {
                    theme.colors.surface_elevated
                } else {
                    theme.colors.surface_variant
                }
            }
            ButtonVariant::Primary => {
                if state.is_pressed() {
                    theme.colors.accent_active
                } else if state.is_hovered() {
                    theme.colors.accent_hover
                } else {
                    theme.colors.accent
                }
            }
            ButtonVariant::Ghost => {
                if state.is_pressed() {
                    theme.colors.surface_active
                } else if state.is_hovered() {
                    theme.colors.surface_elevated
                } else {
                    Color::TRANSPARENT
                }
            }
            ButtonVariant::Danger | ButtonVariant::Destructive => {
                if state.is_pressed() {
                    theme.colors.error
                } else if state.is_hovered() {
                    Color::rgba(
                        theme.colors.error.red,
                        theme.colors.error.green,
                        theme.colors.error.blue,
                        230,
                    )
                } else {
                    theme.colors.error
                }
            }
        }
    }

    /// Resolves text foreground color against active state and theme.
    #[must_use]
    pub fn resolve_foreground(&self, state: ControlState, theme: &ControlTheme) -> Color {
        if state.is_disabled() {
            return theme.colors.disabled_foreground;
        }
        if let Some(fg) = self.foreground_states {
            return fg.resolve(state);
        }
        if let Some(fg) = self.foreground {
            return fg;
        }
        match self.variant {
            ButtonVariant::Default | ButtonVariant::Standard | ButtonVariant::Ghost => {
                theme.colors.foreground
            }
            ButtonVariant::Primary | ButtonVariant::Danger | ButtonVariant::Destructive => {
                theme.colors.accent_foreground
            }
        }
    }
}

/// Configuration style for single-line TextField and multiline TextArea.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextFieldStyle {
    pub background: Option<Color>,
    pub foreground: Option<Color>,
    pub placeholder_color: Option<Color>,
    pub border: Option<Border>,
    pub border_focused: Option<Border>,
    pub border_radius: Option<f32>,
    pub padding: Option<EdgeInsets>,
}

/// Configuration style for Checkbox, Radio, and Switch.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SelectionStyle {
    pub active_color: Option<Color>,
    pub inactive_color: Option<Color>,
    pub check_color: Option<Color>,
    pub border_radius: Option<f32>,
}

/// Configuration style for Card container.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CardStyle {
    pub background: Option<Color>,
    pub border: Option<Border>,
    pub border_radius: Option<f32>,
    pub padding: Option<EdgeInsets>,
}

/// Configuration style for Divider.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DividerStyle {
    pub color: Option<Color>,
    pub thickness: Option<f32>,
    pub indent: Option<f32>,
    pub end_indent: Option<f32>,
}
