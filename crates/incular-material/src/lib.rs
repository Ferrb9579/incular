//! Material-layer controls for Incular.
//!
//! Flutter's Material library owns button families, `TextField`,
//! `TextFormField`, `SelectableText`, `SelectionArea`, and `Autocomplete`.
//! They are deliberately not part of the renderer-neutral
//! [`incular_widgets`] surface.

mod app_shell;
mod buttons;
#[path = "components.rs"]
mod component_impl;
mod extras;
mod feedback;
mod foundation;
mod menus;
mod p0_controls;

pub use app_shell::{
    BackButton, BackButtonIcon, BottomAppBar, CloseButton, CloseButtonIcon, Drawer, DrawerButton,
    DrawerButtonIcon, DrawerHeader, EndDrawerButton, EndDrawerButtonIcon, MaterialApp,
    MaterialPointerDevice, MaterialScrollBehavior, NavigationDrawer, NavigationDrawerDestination,
    NavigationRail, NavigationRailDestination, ScaffoldMessenger, ScaffoldMessengerController,
};
pub use feedback::{
    AlertDialog, Dialog, DialogHandle, DialogResultHandle, DialogRoute, ProgressIndicatorStrokeCap,
    ProgressIndicatorTheme, ProgressIndicatorThemeData, SimpleDialog, SimpleDialogOption, SnackBar,
    SnackBarAction, Tooltip, TooltipController, TooltipTriggerMode,
    current_progress_indicator_theme, show_dialog, show_dialog_result,
};
pub use menus::{
    CheckedPopupMenuItem, DropdownButton, DropdownButtonBuilder, DropdownButtonFormField,
    DropdownButtonHideUnderline, DropdownMenu, DropdownMenuDecorationBuilder, DropdownMenuEntry,
    DropdownMenuFormField, DropdownMenuItem, DropdownMenuThemeData, FilterCallback, MenuAnchor,
    MenuBar, MenuController, MenuItemButton, MenuStyle, MenuThemeData, PopupMenuButton,
    PopupMenuDivider, PopupMenuEntry, PopupMenuItem, PopupMenuThemeData, SearchCallback,
    SubmenuButton,
};
pub use p0_controls::{
    Checkbox, Icons, Radio, RadioGroup, RangeLabels, RangeSlider, RangeValues, Slider,
    SliderThemeData, Switch, Tab, TabBar, TabBarThemeData, TabBarView, TabController,
};

pub use buttons::{
    ButtonStyleConfig, ElevatedButton, FilledButton, FloatingActionButton, IconButton,
    OutlinedButton, StyleFrom, TextButton, style_from,
};
pub use component_impl::{
    ActionChip, AppBar, Badge, BottomNavigationBar, BottomNavigationBarItem, Card,
    CheckboxListTile, Chip, ChoiceChip, CircleAvatar, CircularProgressIndicator, Divider,
    FilterChip, InputChip, LinearProgressIndicator, ListTile, NavigationBar, NavigationDestination,
    RadioListTile, RawChip, Scaffold, SliverAppBar, Surface, SwitchListTile, VerticalDivider,
};
pub use extras::*;
pub use foundation::*;

use incular_config::TextDirection;
use incular_core::{Color, Size};
use incular_semantics::Role as SemanticRole;
use incular_text::{TextAlign, TextEditingValue, TextSelection, TextStyle};
use incular_widgets::internal::{SelectionAreaController, TextEditingController};
use incular_widgets::{
    AbsorbPointer, AutovalidateMode, Border, BorderRadius, BoxDecoration, Container,
    DefaultSelectionStyle, Directionality, EditableText as RawEditableText, FocusNode, Form,
    FormField, GestureDetector, HitTestBehavior, Semantics, Text, TextInputActionHint,
    TextInputFormatter, TextInputTypeHint, Widget,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use typed_builder::TypedBuilder;

type Validator = Rc<dyn Fn(&str) -> Option<String>>;
type ChangedCallback = Rc<dyn Fn(&str)>;

fn text_input_type_hint(input_type: TextInputType) -> TextInputTypeHint {
    match input_type {
        TextInputType::Text
        | TextInputType::Datetime
        | TextInputType::Name
        | TextInputType::StreetAddress
        | TextInputType::None => TextInputTypeHint::Text,
        TextInputType::Multiline => TextInputTypeHint::Multiline,
        TextInputType::Number => TextInputTypeHint::Number,
        TextInputType::Phone => TextInputTypeHint::Phone,
        TextInputType::EmailAddress => TextInputTypeHint::Email,
        TextInputType::Url => TextInputTypeHint::Url,
        TextInputType::VisiblePassword => TextInputTypeHint::Password,
    }
}

fn text_input_action_hint(action: TextInputAction) -> TextInputActionHint {
    match action {
        TextInputAction::Unspecified => TextInputActionHint::Unspecified,
        TextInputAction::None => TextInputActionHint::None,
        TextInputAction::Done
        | TextInputAction::Continue
        | TextInputAction::Join
        | TextInputAction::EmergencyCall => TextInputActionHint::Done,
        TextInputAction::Go | TextInputAction::Route => TextInputActionHint::Go,
        TextInputAction::Search => TextInputActionHint::Search,
        TextInputAction::Send => TextInputActionHint::Send,
        TextInputAction::Next => TextInputActionHint::Next,
        TextInputAction::Previous => TextInputActionHint::Previous,
        TextInputAction::Newline => TextInputActionHint::Newline,
    }
}

pub use incular_controls::theme::TypographyTokens;
pub use incular_controls::theme::{
    AutocompleteTheme, AvatarTheme, CheckboxTheme, ContextMenuTheme, DrawerTheme, InputTheme,
    MenuTheme, MenubarTheme, MeterTheme, NavigationMenuTheme, PopupTheme, ProgressTheme,
    RadioTheme, ScrollAreaTheme, ScrollbarTheme, SliderTheme, SwitchTheme, TabsTheme, ToastTheme,
    ToggleGroupTheme, TooltipTheme,
};
pub use incular_controls::toast::{Provider as ToastProvider, ToastController, ToastId};
pub use incular_controls::{
    AlertDialogTheme, Avatar, ButtonLayerBuilder, ButtonStyle, ButtonTheme, ButtonVariant,
    CardStyle, CheckboxGroupTheme, CheckedState, CompositeController, CompositeItem,
    CompositeOrientation, ControlColors, ControlDensity, ControlIcon, ControlMetrics,
    ControlMotion, ControlState, ControlTheme, ControlThemeScope, ControlTypography, DividerStyle,
    ElevationTokens, GhostButton, GroupTheme, IconAlignment, Meter, MotionTokens, PaletteTokens,
    PrimaryButton, Progress, RadiusTokens, Scrollbar, SelectionStyle, SpacingTokens, SplashFactory,
    StateColor, StateTable, StateValue, TapTargetSize, TextFieldStyle, Toast, alert_dialog, app,
    autocomplete as control_autocomplete, avatar, checkbox, checkbox_group, collapsible, combobox,
    composite, containers, context_menu, current_control_theme, dialog, drawer, field, fieldset,
    form, icons, menu, menubar, meter, navigation_menu, number_field, otp_field, overlay, popover,
    popup, preview_card, progress, scroll_area, scrollbar, select, selection, separator, slider,
    styles, switch, tabs, theme, toast, toggle, toggle_group, toolbar, tooltip,
};
pub use incular_widgets::IconTheme;

/// Material's lower-level button entry point. Use a concrete variant when the
/// intended emphasis is known.
pub type MaterialButton = incular_controls::Button;

/// Flutter's low-level Material button entry point.
///
/// This is intentionally located in the Material package: Flutter's
/// `RawMaterialButton` is a Material widget, not a `widgets` primitive. New
/// code should normally prefer `ElevatedButton`, `FilledButton`,
/// `OutlinedButton`, or `TextButton`.
pub type RawMaterialButton = incular_widgets::internal::ActionSurface;

// Canonical names for the component roots that are implemented in the
// shared controls engine.  Keeping these aliases in the Material package
// gives applications a stable, Flutter-shaped import surface while the
// renderer-neutral widget crate remains free of design-system vocabulary.
pub type CheckboxGroup = incular_controls::checkbox_group::Root;
pub type Collapsible = incular_controls::collapsible::Root;
pub type ComboBox = incular_controls::combobox::Root;
pub type ContextMenu = incular_controls::context_menu::Root;
pub type Field = incular_controls::field::Root;
pub type FieldSet = incular_controls::fieldset::Root;
pub type Menu = incular_controls::menu::Root;
pub type NavigationMenu = incular_controls::navigation_menu::Root;
pub type NumberField = incular_controls::number_field::Root;
pub type OtpField = incular_controls::otp_field::Root;
pub type Popover = incular_controls::popover::Root;
pub type Popup = incular_controls::popup::Root;
pub type PreviewCard = incular_controls::preview_card::Root;
pub type ProgressIndicator = incular_controls::progress::Root;
pub type ScrollArea = incular_controls::scroll_area::Root;
pub type Select = incular_controls::select::Root;
pub type Separator = incular_controls::separator::Separator;
pub type Toggle = incular_controls::toggle::Toggle;
pub type ToggleGroup = incular_controls::toggle_group::Root;
pub type Toolbar = incular_controls::toolbar::Root;
pub type TooltipProvider = incular_controls::tooltip::Provider;

/// Read-only text with Material selection behavior.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct SelectableText {
    #[builder(setter(into))]
    text: String,
    #[builder(default)]
    style: TextStyle,
    #[builder(default = incular_text::TextAlign::Start)]
    align: incular_text::TextAlign,
}

impl Default for SelectableText {
    fn default() -> Self {
        Self {
            text: String::new(),
            style: TextStyle::default(),
            align: incular_text::TextAlign::Start,
        }
    }
}

impl SelectableText {
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self::builder().text(text).build()
    }

    #[must_use]
    pub fn style(mut self, style: TextStyle) -> Self {
        self.style = style;
        self
    }

    #[must_use]
    pub fn color(mut self, color: incular_core::Color) -> Self {
        self.style.color = color;
        self
    }

    #[must_use]
    pub fn align(mut self, align: incular_text::TextAlign) -> Self {
        self.align = align;
        self
    }
}

impl From<SelectableText> for Widget {
    fn from(value: SelectableText) -> Self {
        Widget::selectable_text_styled(value.text, value.style, value.align)
    }
}

/// Material selection area coordinating selectable descendants.
#[derive(Clone, Debug, PartialEq)]
pub struct SelectionArea {
    controller: SelectionAreaController,
    child: Widget,
}

impl SelectionArea {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self::with_controller(SelectionAreaController::new(), child)
    }

    #[must_use]
    pub fn with_controller(controller: SelectionAreaController, child: impl Into<Widget>) -> Self {
        Self {
            controller,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn controller(&self) -> SelectionAreaController {
        self.controller.clone()
    }
}

impl From<SelectionArea> for Widget {
    fn from(value: SelectionArea) -> Self {
        Widget::selection_area(value.controller, value.child)
    }
}

/// Material autocomplete model. Presentation is composed from ordinary
/// Material popup/list controls, matching Flutter's builder-oriented API.
pub struct Autocomplete<T> {
    options: Vec<T>,
    label: Rc<dyn Fn(&T) -> String>,
    query: String,
    selected: Option<usize>,
}

impl<T> Autocomplete<T> {
    #[must_use]
    pub fn new(
        options: impl IntoIterator<Item = T>,
        label: impl Fn(&T) -> String + 'static,
    ) -> Self {
        Self {
            options: options.into_iter().collect(),
            label: Rc::new(label),
            query: String::new(),
            selected: None,
        }
    }

    pub fn set_query(&mut self, query: impl Into<String>) {
        self.query = query.into();
        self.selected = None;
    }

    #[must_use]
    pub fn query(&self) -> &str {
        &self.query
    }

    #[must_use]
    pub fn matching_indices(&self) -> Vec<usize> {
        let needle = self.query.to_lowercase();
        self.options
            .iter()
            .enumerate()
            .filter_map(|(index, option)| {
                ((self.label)(option).to_lowercase().contains(&needle)).then_some(index)
            })
            .collect()
    }

    #[must_use]
    pub fn option(&self, index: usize) -> Option<&T> {
        self.options.get(index)
    }

    #[must_use]
    pub fn select(&mut self, index: usize) -> Option<&T> {
        self.options.get(index)?;
        self.selected = Some(index);
        self.options.get(index)
    }

    #[must_use]
    pub fn selected(&self) -> Option<&T> {
        self.selected.and_then(|index| self.options.get(index))
    }
}

impl Autocomplete<String> {
    #[must_use]
    pub fn strings(options: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self::new(options.into_iter().map(Into::into), |value| value.clone())
    }
}

/// Material text input.
///
/// Flutter's Material `TextField` is the themed wrapper around the core
/// `widgets/EditableText` primitive.  The same split is kept here: the
/// controller, caret, selection, and IME state remain in the core widget while
/// this descriptor owns Material chrome and the Material text-style API.
#[derive(Clone)]
pub struct TextField {
    controller: TextEditingController,
    size: Size,
    placeholder: String,
    decoration: TextFieldStyle,
    material_decoration: Option<InputDecoration>,
    style: TextStyle,
    on_submit: Option<Rc<dyn Fn(String) + 'static>>,
    on_editing_complete: Option<Rc<dyn Fn() + 'static>>,
    on_changed: Option<ChangedCallback>,
    enabled: bool,
    read_only: bool,
    obscure_text: bool,
    autofocus: bool,
    max_length: Option<usize>,
    min_lines: Option<usize>,
    max_lines: Option<usize>,
    expands: bool,
    text_align: TextAlign,
    text_direction: Option<TextDirection>,
    cursor_color: Option<incular_core::Color>,
    selection_color: Option<incular_core::Color>,
    cursor_width: Option<f32>,
    cursor_height: Option<f32>,
    cursor_radius: Option<f32>,
    show_cursor: Option<bool>,
    keyboard_type: TextInputType,
    text_input_action: TextInputAction,
    input_formatters: Vec<Rc<dyn TextInputFormatter>>,
    focus_node: Option<FocusNode>,
    can_request_focus: bool,
    semantic_label: Option<String>,
    helper_text: Option<String>,
    error_text: Option<String>,
    on_tap: Option<Rc<dyn Fn() + 'static>>,
}

impl TextField {
    #[must_use]
    pub fn new(controller: TextEditingController) -> Self {
        Self {
            controller,
            size: Size::ZERO,
            placeholder: String::new(),
            decoration: TextFieldStyle::default(),
            material_decoration: None,
            style: TextStyle::default(),
            on_submit: None,
            on_editing_complete: None,
            on_changed: None,
            enabled: true,
            read_only: false,
            obscure_text: false,
            autofocus: false,
            max_length: None,
            min_lines: None,
            max_lines: Some(1),
            expands: false,
            text_align: TextAlign::Start,
            text_direction: None,
            cursor_color: None,
            selection_color: None,
            cursor_width: None,
            cursor_height: None,
            cursor_radius: None,
            show_cursor: None,
            keyboard_type: TextInputType::Text,
            text_input_action: TextInputAction::Unspecified,
            input_formatters: Vec::new(),
            // Keep a retained node even when callers do not provide one.  It
            // gives the Material decorator a real focus source for focused
            // borders and keeps ordinary fields keyboard-addressable without
            // requiring an explicit FocusNode in every application.
            focus_node: Some(FocusNode::new()),
            can_request_focus: true,
            semantic_label: None,
            helper_text: None,
            error_text: None,
            on_tap: None,
        }
    }

    /// Creates an editor with an internally-owned controller.  Use
    /// [`TextField::new`] when the application needs controller ownership.
    #[must_use]
    pub fn uncontrolled() -> Self {
        Self::new(TextEditingController::new())
    }

    /// Explicit constructor spelling for controller-owned fields.
    #[must_use]
    pub fn with_controller(controller: TextEditingController) -> Self {
        Self::new(controller)
    }

    /// Replaces the retained controller while preserving the descriptor's
    /// Material configuration.
    #[must_use]
    pub fn controller(mut self, controller: TextEditingController) -> Self {
        self.controller = controller;
        self
    }

    /// Sets the logical size. A zero component keeps the natural size on that
    /// axis, matching the core `EditableText` primitive.
    #[must_use]
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    #[must_use]
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Sets Material decoration (surface, border, radius, and padding).
    #[must_use]
    pub fn decoration(mut self, decoration: impl Into<TextFieldStyle>) -> Self {
        self.decoration = decoration.into();
        self
    }

    /// Sets Flutter-shaped decoration while retaining the lower-level style
    /// conversion used by the existing renderer-neutral controls.
    #[must_use]
    pub fn input_decoration(mut self, decoration: InputDecoration) -> Self {
        if self.placeholder.is_empty() {
            if let Some(hint) = decoration.hint_text.clone() {
                self.placeholder = hint;
            }
        }
        self.semantic_label = decoration.label_text.clone();
        self.helper_text = decoration.helper_text.clone();
        self.error_text = decoration.error_text.clone();
        self.decoration = decoration.clone().into();
        self.material_decoration = Some(decoration);
        self
    }

    /// Sets the text style used by the editable content.
    #[must_use]
    pub fn style(mut self, style: TextStyle) -> Self {
        self.style = style;
        self
    }

    /// Sets the maximum number of lines. Values greater than one enable
    /// multiline editing; `None` means an unbounded multiline editor.
    #[must_use]
    pub fn max_lines(mut self, max_lines: Option<usize>) -> Self {
        self.max_lines = max_lines.map(|lines| lines.max(1));
        self
    }

    /// Convenience for a multiline Material text field. Flutter represents a
    /// text area with `TextField(maxLines: ...)`, rather than a separate
    /// `TextArea` widget.
    #[must_use]
    pub fn multiline(self, multiline: bool) -> Self {
        self.max_lines(if multiline { None } else { Some(1) })
    }

    #[must_use]
    pub fn on_submit(mut self, on_submit: impl Fn(String) + 'static) -> Self {
        self.on_submit = Some(Rc::new(on_submit));
        self
    }

    #[must_use]
    pub fn on_editing_complete(mut self, callback: impl Fn() + 'static) -> Self {
        self.on_editing_complete = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn on_changed(mut self, on_changed: impl Fn(&str) + 'static) -> Self {
        self.on_changed = Some(Rc::new(on_changed));
        self
    }

    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
    #[must_use]
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }
    #[must_use]
    pub fn obscure_text(mut self, obscure: bool) -> Self {
        self.obscure_text = obscure;
        self
    }
    #[must_use]
    pub fn autofocus(mut self, autofocus: bool) -> Self {
        self.autofocus = autofocus;
        self
    }
    #[must_use]
    pub fn max_length(mut self, max_length: Option<usize>) -> Self {
        self.max_length = max_length;
        self
    }

    #[must_use]
    pub fn min_lines(mut self, lines: Option<usize>) -> Self {
        self.min_lines = lines.map(|value| value.max(1));
        self
    }

    #[must_use]
    pub fn expands(mut self, value: bool) -> Self {
        self.expands = value;
        self
    }

    #[must_use]
    pub fn text_align(mut self, value: TextAlign) -> Self {
        self.text_align = value;
        self
    }

    #[must_use]
    pub fn text_direction(mut self, value: TextDirection) -> Self {
        self.text_direction = Some(value);
        self
    }

    #[must_use]
    pub fn cursor_color(mut self, value: incular_core::Color) -> Self {
        self.cursor_color = Some(value);
        self
    }

    #[must_use]
    pub fn selection_color(mut self, value: incular_core::Color) -> Self {
        self.selection_color = Some(value);
        self
    }

    #[must_use]
    pub fn cursor_width(mut self, value: f32) -> Self {
        self.cursor_width = Some(value.max(0.0));
        self
    }

    #[must_use]
    pub fn cursor_height(mut self, value: Option<f32>) -> Self {
        self.cursor_height = value.map(|height| height.max(0.0));
        self
    }

    #[must_use]
    pub fn cursor_radius(mut self, value: f32) -> Self {
        self.cursor_radius = Some(value.max(0.0));
        self
    }

    #[must_use]
    pub fn show_cursor(mut self, value: bool) -> Self {
        self.show_cursor = Some(value);
        self
    }

    #[must_use]
    pub fn keyboard_type(mut self, value: TextInputType) -> Self {
        self.keyboard_type = value;
        self
    }

    #[must_use]
    pub fn text_input_action(mut self, value: TextInputAction) -> Self {
        self.text_input_action = value;
        self
    }

    #[must_use]
    pub fn input_formatter(mut self, value: impl TextInputFormatter + 'static) -> Self {
        self.input_formatters.push(Rc::new(value));
        self
    }

    #[must_use]
    pub fn input_formatters(
        mut self,
        values: impl IntoIterator<Item = Rc<dyn TextInputFormatter>>,
    ) -> Self {
        self.input_formatters.extend(values);
        self
    }

    /// Updates the controller's selection without taking ownership of the
    /// core editing algorithm.
    #[must_use]
    pub fn selection(self, value: TextSelection) -> Self {
        self.controller.set_selection(value);
        self
    }

    #[must_use]
    pub fn text_editing_value(self, value: TextEditingValue) -> Self {
        self.controller.set_value(value);
        self
    }

    #[must_use]
    pub fn helper_text(mut self, value: impl Into<String>) -> Self {
        self.helper_text = Some(value.into());
        self
    }

    #[must_use]
    pub fn error_text(mut self, value: impl Into<String>) -> Self {
        self.error_text = Some(value.into());
        self
    }

    #[must_use]
    pub fn on_tap(mut self, callback: impl Fn() + 'static) -> Self {
        self.on_tap = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn focus_node(mut self, value: FocusNode) -> Self {
        self.focus_node = Some(value);
        self
    }

    #[must_use]
    pub fn can_request_focus(mut self, value: bool) -> Self {
        self.can_request_focus = value;
        self
    }

    #[must_use]
    pub fn semantic_label(mut self, label: impl Into<String>) -> Self {
        self.semantic_label = Some(label.into());
        self
    }

    fn build(&self, theme: &ControlTheme) -> Widget {
        let background = self.decoration.background.unwrap_or(theme.colors.surface);
        let foreground = self.decoration.foreground.unwrap_or({
            if self.style.has_explicit_color {
                self.style.color
            } else {
                theme.colors.foreground
            }
        });
        let radius = self.decoration.border_radius.unwrap_or(theme.input.radius);
        let padding = self
            .decoration
            .padding
            .unwrap_or_else(|| theme.density.padding());
        let border = self
            .decoration
            .border
            .unwrap_or_else(|| Border::new(theme.input.border_width, theme.colors.border));
        let multiline = self.max_lines.is_none_or(|lines| lines != 1);

        let mut raw = RawEditableText::new(self.controller.clone())
            .size(self.size)
            .multiline(multiline)
            .min_lines(self.min_lines)
            .max_lines(self.max_lines)
            .expands(self.expands)
            .text_align(self.text_align)
            .enabled(self.enabled)
            .read_only(self.read_only)
            .obscure_text(self.obscure_text)
            .cursor_width(self.cursor_width.unwrap_or(1.0))
            .cursor_height(self.cursor_height)
            .cursor_radius(self.cursor_radius.unwrap_or(0.0))
            .show_cursor(self.show_cursor.unwrap_or(true))
            .cursor_color(self.cursor_color.unwrap_or(Color::WHITE))
            .selection_color(
                self.selection_color
                    .unwrap_or(Color::rgba(72, 120, 220, 150)),
            )
            .placeholder(self.placeholder.clone())
            .style(self.style.clone().color(foreground))
            .input_type(text_input_type_hint(self.keyboard_type))
            .input_action(text_input_action_hint(self.text_input_action));
        if self.on_submit.is_some() || self.on_editing_complete.is_some() {
            let on_submit = self.on_submit.clone();
            let on_editing_complete = self.on_editing_complete.clone();
            raw = raw.on_submit(move |value| {
                if let Some(callback) = &on_submit {
                    callback(value);
                }
                if let Some(callback) = &on_editing_complete {
                    callback();
                }
            });
        }

        let mut editor: Widget = raw.into();
        if let Some(callback) = self.on_tap.clone() {
            editor = GestureDetector::new(editor)
                .behavior(HitTestBehavior::DeferToChild)
                .on_tap(move || callback())
                .into();
        }
        // A disabled or read-only Material field must not mutate its retained
        // controller from pointer input.  Keyboard/IME policy remains in the
        // platform editor bridge, while this wrapper enforces the public
        // interaction contract immediately.
        if !self.enabled || self.read_only {
            editor = AbsorbPointer::new(editor).absorbing(true).into();
        }

        let mut content: Widget = if let Some(decoration) = self.material_decoration.clone() {
            let decoration_theme =
                incular_widgets::internal::current_build_environment::<InputDecorationThemeData>()
                    .unwrap_or_default();
            let mut decoration = decoration.apply_defaults(&decoration_theme);
            if decoration.error_text.is_none() {
                decoration.error_text = self.error_text.clone();
            }
            InputDecorator::new(decoration, editor)
                .focused(self.focus_node.as_ref().is_some_and(FocusNode::has_focus))
                .empty(self.controller.text().is_empty())
                .enabled(self.enabled)
                .into()
        } else {
            Container::new()
                .padding(padding)
                .decoration(
                    BoxDecoration::new()
                        .color(background)
                        .border(border)
                        .border_radius(BorderRadius::circular(radius)),
                )
                .child(editor)
                .into()
        };
        // InputDecorator owns Material labels and helper/error rows.  The
        // outer compatibility label is only needed for the renderer-neutral
        // TextFieldStyle path; adding it here as well duplicates labels.
        if self.material_decoration.is_none()
            && (self.label_text().is_some()
                || self.helper_text.is_some()
                || self.error_text.is_some())
        {
            let mut children = Vec::with_capacity(4);
            if let Some(label) = self.label_text() {
                children.push(Widget::from(Text::new(label)));
            }
            children.push(content);
            if let Some(helper) = self.helper_text.clone().or_else(|| self.error_text.clone()) {
                children.push(Widget::from(Text::new(helper)));
            }
            content = incular_widgets::Column::new(children).spacing(4.0).into();
        }
        if let Some(direction) = self.text_direction {
            content = Directionality::new(direction, content).into();
        }
        if self.cursor_color.is_some() || self.selection_color.is_some() {
            let mut selection = DefaultSelectionStyle::new(content);
            if let Some(color) = self.cursor_color {
                selection = selection.cursor_color(color);
            }
            if let Some(color) = self.selection_color {
                selection = selection.selection_color(color);
            }
            content = selection.into();
        }
        if let Some(node) = self.focus_node.as_ref() {
            node.set_can_request_focus(self.can_request_focus);
        }
        let value = self.controller.text();
        let mut semantics = Semantics::new(content)
            .role(SemanticRole::TextField)
            .label(
                self.semantic_label
                    .clone()
                    .unwrap_or_else(|| self.placeholder.clone()),
            )
            .value(value)
            .enabled(self.enabled)
            .read_only(self.read_only)
            .obscured(self.obscure_text)
            .multiline(multiline);
        if let Some(node) = self.focus_node.clone() {
            semantics = Semantics::new(
                incular_widgets::Focus::new(semantics)
                    .node(node)
                    .autofocus(self.autofocus),
            )
            .role(SemanticRole::TextField);
        }
        semantics.into()
    }

    fn label_text(&self) -> Option<String> {
        self.semantic_label.clone()
    }
}

impl Default for TextField {
    fn default() -> Self {
        Self::uncontrolled()
    }
}

impl From<TextField> for Widget {
    fn from(value: TextField) -> Self {
        let controller = value.controller.clone();
        if value.on_changed.is_some()
            || value.max_length.is_some()
            || !value.input_formatters.is_empty()
        {
            let callback = value.on_changed.clone();
            let limit = value.max_length;
            let formatters = value.input_formatters.clone();
            let controller_for_listener = controller.clone();
            let previous = Rc::new(RefCell::new(controller.value()));
            let normalizing = Rc::new(Cell::new(false));
            controller.add_listener(move |current| {
                // Applying a formatter may update the same retained
                // controller synchronously. Ignore that nested notification;
                // the outer pass emits one callback for the effective value.
                if normalizing.get() {
                    return;
                }
                let old = previous.replace(current.clone());
                let mut next = current.clone();
                for formatter in &formatters {
                    next = formatter.format_edit_update(&old, &next);
                }
                if let Some(limit) = limit {
                    if next.text.chars().count() > limit {
                        next.text = next.text.chars().take(limit).collect();
                    }
                }
                if next != *current {
                    normalizing.set(true);
                    controller_for_listener.set_value(next);
                    normalizing.set(false);
                    if let Some(callback) = &callback {
                        callback(&controller_for_listener.text());
                    }
                    return;
                }
                if let Some(callback) = &callback {
                    callback(&current.text);
                }
            });
        }
        let value = Rc::new(value);
        Widget::layout_builder(move |_| {
            let theme = incular_widgets::internal::current_build_environment::<ControlTheme>()
                .unwrap_or_default();
            value.build(&theme)
        })
    }
}

/// Material form field convenience wrapper around [`TextField`]. Validation
/// remains owned by the core `Form`/`FormField` model; the callbacks are kept
/// on the descriptor so applications can attach them while composing a form.
#[derive(Clone)]
pub struct TextFormField {
    controller: Option<TextEditingController>,
    validator: Option<Validator>,
    on_saved: Option<Rc<dyn Fn(String)>>,
    on_submit: Option<Rc<dyn Fn(String)>>,
    on_editing_complete: Option<Rc<dyn Fn()>>,
    on_changed: Option<ChangedCallback>,
    on_tap: Option<Rc<dyn Fn()>>,
    autovalidate_mode: AutovalidateMode,
    placeholder: String,
    style: Option<TextStyle>,
    decoration: Option<TextFieldStyle>,
    input_decoration: Option<InputDecoration>,
    size: Size,
    max_lines: Option<usize>,
    min_lines: Option<usize>,
    expands: bool,
    text_align: TextAlign,
    text_direction: Option<TextDirection>,
    cursor_color: Option<incular_core::Color>,
    selection_color: Option<incular_core::Color>,
    cursor_width: Option<f32>,
    cursor_height: Option<f32>,
    cursor_radius: Option<f32>,
    show_cursor: Option<bool>,
    keyboard_type: TextInputType,
    text_input_action: TextInputAction,
    input_formatters: Vec<Rc<dyn TextInputFormatter>>,
    focus_node: Option<FocusNode>,
    can_request_focus: bool,
    enabled: bool,
    read_only: bool,
    obscure_text: bool,
    autofocus: bool,
    max_length: Option<usize>,
}

impl Default for TextFormField {
    fn default() -> Self {
        Self::new()
    }
}

impl TextFormField {
    #[must_use]
    pub fn new() -> Self {
        Self {
            controller: None,
            validator: None,
            on_saved: None,
            on_submit: None,
            on_editing_complete: None,
            on_changed: None,
            on_tap: None,
            autovalidate_mode: AutovalidateMode::Disabled,
            placeholder: String::new(),
            style: None,
            decoration: None,
            input_decoration: None,
            size: Size::new(200.0, 36.0),
            max_lines: Some(1),
            min_lines: None,
            expands: false,
            text_align: TextAlign::Start,
            text_direction: None,
            cursor_color: None,
            selection_color: None,
            cursor_width: None,
            cursor_height: None,
            cursor_radius: None,
            show_cursor: None,
            keyboard_type: TextInputType::Text,
            text_input_action: TextInputAction::Unspecified,
            input_formatters: Vec::new(),
            focus_node: None,
            can_request_focus: true,
            enabled: true,
            read_only: false,
            obscure_text: false,
            autofocus: false,
            max_length: None,
        }
    }

    #[must_use]
    pub fn controller(mut self, controller: TextEditingController) -> Self {
        self.controller = Some(controller);
        self
    }

    #[must_use]
    pub fn with_controller(self, controller: TextEditingController) -> Self {
        self.controller(controller)
    }

    #[must_use]
    pub fn validator(mut self, validator: impl Fn(&str) -> Option<String> + 'static) -> Self {
        self.validator = Some(Rc::new(validator));
        self
    }

    /// Registers the callback used by an enclosing [`Form`] when its save
    /// operation succeeds.  Call [`Self::register_with_form`] with the same
    /// descriptor to connect this callback to the retained form registry.
    #[must_use]
    pub fn on_saved(mut self, callback: impl Fn(String) + 'static) -> Self {
        self.on_saved = Some(Rc::new(callback));
        self
    }

    /// Connects this field's controller, validator, autovalidation policy, and
    /// save callback to an explicit core form controller.  This is the
    /// Rust-native equivalent of Flutter's implicit `Form.of(context)` lookup
    /// and keeps registration independent of widget rebuilds.
    #[must_use]
    pub fn register_with_form(&self, form: &Form) -> FormField {
        let controller = self.controller.clone().unwrap_or_default();
        let mut field = form.register(controller);
        if let Some(validator) = self.validator.clone() {
            field = field.validator(move |text| validator(text));
        }
        if self.autovalidate_mode != AutovalidateMode::Disabled {
            field = field.autovalidate(self.autovalidate_mode);
        }
        if let Some(callback) = self.on_saved.clone() {
            field = field.on_saved(move |text| callback(text));
        }
        field
    }

    #[must_use]
    pub fn on_submit(mut self, on_submit: impl Fn(String) + 'static) -> Self {
        self.on_submit = Some(Rc::new(on_submit));
        self
    }

    #[must_use]
    pub fn on_editing_complete(mut self, callback: impl Fn() + 'static) -> Self {
        self.on_editing_complete = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn on_changed(mut self, on_changed: impl Fn(&str) + 'static) -> Self {
        self.on_changed = Some(Rc::new(on_changed));
        self
    }

    #[must_use]
    pub fn on_tap(mut self, callback: impl Fn() + 'static) -> Self {
        self.on_tap = Some(Rc::new(callback));
        self
    }

    #[must_use]
    pub fn autovalidate_mode(mut self, mode: AutovalidateMode) -> Self {
        self.autovalidate_mode = mode;
        self
    }

    #[must_use]
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    #[must_use]
    pub fn style(mut self, style: TextStyle) -> Self {
        self.style = Some(style);
        self
    }

    #[must_use]
    pub fn decoration(mut self, decoration: impl Into<TextFieldStyle>) -> Self {
        self.decoration = Some(decoration.into());
        self
    }

    #[must_use]
    pub fn input_decoration(mut self, decoration: InputDecoration) -> Self {
        self.input_decoration = Some(decoration);
        self
    }

    #[must_use]
    pub fn size(mut self, size: incular_core::Size) -> Self {
        self.size = size;
        self
    }

    #[must_use]
    pub fn max_lines(mut self, max_lines: Option<usize>) -> Self {
        self.max_lines = max_lines.map(|lines| lines.max(1));
        self
    }

    #[must_use]
    pub fn min_lines(mut self, lines: Option<usize>) -> Self {
        self.min_lines = lines.map(|value| value.max(1));
        self
    }

    #[must_use]
    pub fn expands(mut self, value: bool) -> Self {
        self.expands = value;
        self
    }

    #[must_use]
    pub fn text_align(mut self, value: TextAlign) -> Self {
        self.text_align = value;
        self
    }

    #[must_use]
    pub fn text_direction(mut self, value: TextDirection) -> Self {
        self.text_direction = Some(value);
        self
    }

    #[must_use]
    pub fn cursor_color(mut self, value: incular_core::Color) -> Self {
        self.cursor_color = Some(value);
        self
    }

    #[must_use]
    pub fn selection_color(mut self, value: incular_core::Color) -> Self {
        self.selection_color = Some(value);
        self
    }

    #[must_use]
    pub fn cursor_width(mut self, value: f32) -> Self {
        self.cursor_width = Some(value.max(0.0));
        self
    }

    #[must_use]
    pub fn cursor_height(mut self, value: Option<f32>) -> Self {
        self.cursor_height = value.map(|height| height.max(0.0));
        self
    }

    #[must_use]
    pub fn cursor_radius(mut self, value: f32) -> Self {
        self.cursor_radius = Some(value.max(0.0));
        self
    }

    #[must_use]
    pub fn show_cursor(mut self, value: bool) -> Self {
        self.show_cursor = Some(value);
        self
    }

    #[must_use]
    pub fn keyboard_type(mut self, value: TextInputType) -> Self {
        self.keyboard_type = value;
        self
    }

    #[must_use]
    pub fn text_input_action(mut self, value: TextInputAction) -> Self {
        self.text_input_action = value;
        self
    }

    #[must_use]
    pub fn input_formatter(mut self, value: impl TextInputFormatter + 'static) -> Self {
        self.input_formatters.push(Rc::new(value));
        self
    }

    #[must_use]
    pub fn input_formatters(
        mut self,
        values: impl IntoIterator<Item = Rc<dyn TextInputFormatter>>,
    ) -> Self {
        self.input_formatters.extend(values);
        self
    }

    #[must_use]
    pub fn focus_node(mut self, value: FocusNode) -> Self {
        self.focus_node = Some(value);
        self
    }

    #[must_use]
    pub fn can_request_focus(mut self, value: bool) -> Self {
        self.can_request_focus = value;
        self
    }

    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    #[must_use]
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    #[must_use]
    pub fn obscure_text(mut self, obscure_text: bool) -> Self {
        self.obscure_text = obscure_text;
        self
    }

    #[must_use]
    pub fn autofocus(mut self, autofocus: bool) -> Self {
        self.autofocus = autofocus;
        self
    }

    #[must_use]
    pub fn max_length(mut self, max_length: Option<usize>) -> Self {
        self.max_length = max_length;
        self
    }
}

impl From<TextFormField> for Widget {
    fn from(value: TextFormField) -> Self {
        let controller = value.controller.unwrap_or_default();
        let validation_error = if matches!(
            value.autovalidate_mode,
            AutovalidateMode::Always | AutovalidateMode::OnUserInteraction
        ) {
            value
                .validator
                .as_ref()
                .and_then(|validator| validator(&controller.text()))
        } else {
            None
        };
        let mut field = TextField::new(controller)
            .placeholder(value.placeholder)
            .size(value.size)
            .max_lines(value.max_lines)
            .min_lines(value.min_lines)
            .expands(value.expands)
            .text_align(value.text_align)
            .autofocus(value.autofocus)
            .keyboard_type(value.keyboard_type)
            .text_input_action(value.text_input_action)
            .can_request_focus(value.can_request_focus);
        if let Some(width) = value.cursor_width {
            field = field.cursor_width(width);
        }
        if let Some(height) = value.cursor_height {
            field = field.cursor_height(Some(height));
        }
        if let Some(radius) = value.cursor_radius {
            field = field.cursor_radius(radius);
        }
        if let Some(show_cursor) = value.show_cursor {
            field = field.show_cursor(show_cursor);
        }
        for formatter in value.input_formatters {
            field = field.input_formatters([formatter]);
        }
        if let Some(direction) = value.text_direction {
            field = field.text_direction(direction);
        }
        if let Some(color) = value.cursor_color {
            field = field.cursor_color(color);
        }
        if let Some(color) = value.selection_color {
            field = field.selection_color(color);
        }
        if let Some(node) = value.focus_node {
            field = field.focus_node(node);
        }
        if let Some(style) = value.style {
            field = field.style(style);
        }
        if let Some(decoration) = value.input_decoration {
            let decoration = validation_error
                .as_ref()
                .map_or(decoration.clone(), |error| decoration.error(error.clone()));
            field = field.input_decoration(decoration);
        } else if let Some(decoration) = value.decoration {
            field = field.decoration(decoration);
        }
        if let Some(on_submit) = value.on_submit {
            field = field.on_submit(move |text| on_submit(text));
        }
        if let Some(on_editing_complete) = value.on_editing_complete {
            field = field.on_editing_complete(move || on_editing_complete());
        }
        if let Some(on_changed) = value.on_changed {
            field = field.on_changed(move |text| on_changed(text));
        }
        if let Some(on_tap) = value.on_tap {
            field = field.on_tap(move || on_tap());
        }
        field = field
            .enabled(value.enabled)
            .read_only(value.read_only)
            .obscure_text(value.obscure_text)
            .max_length(value.max_length);
        field.into()
    }
}

/// Material prelude for application-facing imports.
pub mod prelude {
    pub use super::style_from;
    pub use super::{
        ActionChip, ActionIconThemeData, AlertDialog, AnimatedTheme, AppBar, AppBarThemeData,
        Autocomplete, BackButton, BackButtonIcon, Badge, BadgeThemeData, BottomAppBar,
        BottomAppBarThemeData, BottomNavigationBar, BottomNavigationBarItem,
        BottomNavigationBarLandscapeLayout, BottomNavigationBarThemeData, BottomNavigationBarType,
        ButtonBarLayoutBehavior, ButtonStyle, ButtonStyleConfig, ButtonTextTheme, Card,
        CardThemeData, Checkbox, CheckboxListTile, CheckboxThemeData, CheckedPopupMenuItem, Chip,
        ChoiceChip, CircleAvatar, CircularProgressIndicator, CloseButton, CloseButtonIcon,
        CollapseMode, ColorScheme, Colors, ComponentThemeData, DayPeriod, Dialog, DialogHandle,
        DialogResultHandle, DialogRoute, DialogThemeData, Divider, DividerThemeData, Drawer,
        DrawerAlignment, DrawerButton, DrawerButtonIcon, DrawerHeader, DrawerThemeData,
        DropdownButton, DropdownButtonBuilder, DropdownButtonFormField,
        DropdownButtonHideUnderline, DropdownMenu, DropdownMenuCloseBehavior,
        DropdownMenuDecorationBuilder, DropdownMenuEntry, DropdownMenuFormField, DropdownMenuItem,
        DropdownMenuThemeData, Durations, DynamicSchemeVariant, Easing, EasingCurve,
        ElevatedButton, ElevatedButtonThemeData, EndDrawerButton, EndDrawerButtonIcon,
        FilledButton, FilledButtonThemeData, FilterCallback, FilterChip, FloatingActionButton,
        FloatingActionButtonThemeData, FloatingLabelAlignment, FloatingLabelBehavior, HourFormat,
        IconButton, IconButtonThemeData, IconTheme, IconThemeData, Icons, Ink, InkResponse,
        InkWell, InputBorder, InputChip, InputDecoration, InputDecorationTheme,
        InputDecorationThemeData, InputDecorator, LinearProgressIndicator, ListTile,
        ListTileControlAffinity, ListTileStyle, ListTileThemeData, ListTileTitleAlignment,
        Material, MaterialApp, MaterialButton, MaterialColor, MaterialPointerDevice,
        MaterialScrollBehavior, MaterialState, MaterialStateProperty, MaterialStatePropertyAll,
        MaterialStates, MaterialStatesController, MaterialTapTargetSize, MaterialType, MenuAnchor,
        MenuBar, MenuBarThemeData, MenuButtonThemeData, MenuController, MenuItemButton, MenuStyle,
        MenuThemeData, NavigationBar, NavigationBarThemeData, NavigationDestination,
        NavigationDestinationLabelBehavior, NavigationDrawer, NavigationDrawerDestination,
        NavigationDrawerThemeData, NavigationRail, NavigationRailDestination,
        NavigationRailLabelType, NavigationRailThemeData, OutlineInputBorder, OutlinedButton,
        OutlinedButtonThemeData, PageTransitionsTheme, PopupMenuButton, PopupMenuDivider,
        PopupMenuEntry, PopupMenuItem, PopupMenuPosition, PopupMenuThemeData,
        ProgressIndicatorStrokeCap, ProgressIndicatorTheme, ProgressIndicatorThemeData, Radio,
        RadioGroup, RadioListTile, RadioThemeData, RangeLabels, RangeSlider, RangeValues, RawChip,
        RawMaterialButton, RefreshIndicatorStatus, RefreshIndicatorTriggerMode, Scaffold,
        ScaffoldMessenger, ScaffoldMessengerController, ScriptCategory, SearchCallback,
        SelectableText, SelectionArea, ShapedInputBorder, ShowValueIndicator, SimpleDialog,
        SimpleDialogOption, Slider, SliderInteraction, SliderThemeData, SliverAppBar, SnackBar,
        SnackBarAction, SnackBarBehavior, SnackBarClosedReason, SnackBarThemeData, StateProperty,
        StepState, StepperType, StretchMode, StyleFrom, SubmenuButton, Surface, Switch,
        SwitchListTile, SwitchThemeData, Tab, TabAlignment, TabBar, TabBarIndicatorSize,
        TabBarThemeData, TabBarView, TabController, TabIndicatorAnimation, TargetPlatform,
        TextButton, TextButtonThemeData, TextField, TextFieldStyle, TextFormField, TextInputAction,
        TextInputType, TextSelectionThemeData, TextTheme, Theme, ThemeData, ThemeDataPatch,
        ThemeExtension, ThemeExtensionValue, ThemeMode, TimeOfDayFormat, Tooltip,
        TooltipController, TooltipThemeData, TooltipTriggerMode, Typography, UnderlineInputBorder,
        VerticalDivider, VisualDensity, WidgetState, WidgetStateProperty, WidgetStatePropertyAll,
        WidgetStates, WidgetStatesController, show_dialog, show_dialog_result,
    };
    // Compatibility aliases retained for Incular applications; Flutter's
    // canonical button entry points are the four concrete variants above.
    pub use super::{GhostButton, PrimaryButton};
}

/// Complete Material component vocabulary. This explicit module is useful for
/// applications that want every component without importing the facade's
/// compact prelude (which intentionally avoids collisions with other design
/// systems).
pub mod components {
    pub use super::style_from;
    pub use super::{
        ActionChip, ActionIconThemeData, AlertDialog, AnimatedTheme, AppBar, AppBarThemeData,
        Autocomplete, Avatar, BackButton, BackButtonIcon, Badge, BadgeThemeData, BottomAppBar,
        BottomAppBarThemeData, BottomNavigationBar, BottomNavigationBarItem,
        BottomNavigationBarLandscapeLayout, BottomNavigationBarThemeData, BottomNavigationBarType,
        ButtonBarLayoutBehavior, ButtonTextTheme, Card, CardThemeData, Checkbox, CheckboxGroup,
        CheckboxListTile, CheckboxThemeData, CheckedPopupMenuItem, Chip, ChoiceChip, CircleAvatar,
        CircularProgressIndicator, CloseButton, CloseButtonIcon, CollapseMode, Collapsible,
        ColorScheme, Colors, ComboBox, ContextMenu, DayPeriod, DialogHandle, DialogResultHandle,
        DialogRoute, DialogThemeData, DividerThemeData, Drawer, DrawerAlignment, DrawerButton,
        DrawerButtonIcon, DrawerHeader, DrawerThemeData, DropdownButton, DropdownButtonBuilder,
        DropdownButtonFormField, DropdownButtonHideUnderline, DropdownMenu,
        DropdownMenuCloseBehavior, DropdownMenuDecorationBuilder, DropdownMenuEntry,
        DropdownMenuFormField, DropdownMenuItem, DropdownMenuThemeData, Durations,
        DynamicSchemeVariant, Easing, EasingCurve, ElevatedButton, ElevatedButtonThemeData,
        EndDrawerButton, EndDrawerButtonIcon, Field, FieldSet, FilledButton, FilledButtonThemeData,
        FilterCallback, FilterChip, FloatingActionButton, FloatingActionButtonThemeData,
        FloatingLabelAlignment, FloatingLabelBehavior, GhostButton, HourFormat, IconButton,
        IconButtonThemeData, IconTheme, IconThemeData, Icons, Ink, InkResponse, InkWell, InputChip,
        InputDecoration, InputDecorationTheme, InputDecorationThemeData, InputDecorator,
        LinearProgressIndicator, ListTile, ListTileControlAffinity, ListTileStyle,
        ListTileThemeData, ListTileTitleAlignment, Material, MaterialApp, MaterialButton,
        MaterialColor, MaterialPointerDevice, MaterialScrollBehavior, MaterialState,
        MaterialStateProperty, MaterialStatePropertyAll, MaterialStates, MaterialStatesController,
        MaterialTapTargetSize, MaterialType, Menu, MenuAnchor, MenuBar, MenuBarThemeData,
        MenuButtonThemeData, MenuController, MenuItemButton, MenuStyle, MenuThemeData, Meter,
        NavigationBar, NavigationBarThemeData, NavigationDestination,
        NavigationDestinationLabelBehavior, NavigationDrawer, NavigationDrawerDestination,
        NavigationDrawerThemeData, NavigationMenu, NavigationRail, NavigationRailDestination,
        NavigationRailLabelType, NavigationRailThemeData, NumberField, OtpField,
        OutlineInputBorder, OutlinedButton, OutlinedButtonThemeData, Popover, Popup,
        PopupMenuButton, PopupMenuDivider, PopupMenuEntry, PopupMenuItem, PopupMenuPosition,
        PopupMenuThemeData, PreviewCard, PrimaryButton, Progress, ProgressIndicator,
        ProgressIndicatorStrokeCap, ProgressIndicatorTheme, ProgressIndicatorThemeData, Radio,
        RadioGroup, RadioListTile, RadioThemeData, RangeLabels, RangeSlider, RangeValues, RawChip,
        RawMaterialButton, RefreshIndicatorStatus, RefreshIndicatorTriggerMode, Scaffold,
        ScaffoldMessenger, ScaffoldMessengerController, ScriptCategory, ScrollArea, Scrollbar,
        SearchCallback, Select, SelectableText, SelectionArea, Separator, ShapedInputBorder,
        ShowValueIndicator, SimpleDialog, SimpleDialogOption, Slider, SliderInteraction,
        SliderThemeData, SliverAppBar, SnackBar, SnackBarAction, SnackBarBehavior,
        SnackBarClosedReason, SnackBarThemeData, StateProperty, StepState, StepperType,
        StretchMode, StyleFrom, SubmenuButton, Surface, Switch, SwitchListTile, SwitchThemeData,
        Tab, TabAlignment, TabBar, TabBarIndicatorSize, TabBarThemeData, TabBarView, TabController,
        TabIndicatorAnimation, TargetPlatform, TextButton, TextButtonThemeData, TextField,
        TextFormField, TextInputAction, TextInputType, TextSelectionThemeData, TextTheme, Theme,
        ThemeData, ThemeExtension, ThemeExtensionValue, ThemeMode, TimeOfDayFormat,
        TimePickerEntryMode, Toast, ToastProvider, Toggle, ToggleGroup, Toolbar, Tooltip,
        TooltipController, TooltipProvider, TooltipThemeData, TooltipTriggerMode, Typography,
        UnderlineInputBorder, VerticalDivider, VisualDensity, WidgetState, WidgetStateProperty,
        WidgetStatePropertyAll, WidgetStates, WidgetStatesController, show_dialog,
        show_dialog_result,
    };
    pub use super::{
        AlertDialogTheme, AutocompleteTheme, AvatarTheme, ButtonStyle, ButtonTheme, CardStyle,
        CheckboxGroupTheme, CheckboxTheme, ContextMenuTheme, ControlTheme, ControlThemeScope,
        ControlTypography, Divider, DividerStyle, DrawerTheme, InputBorder, InputTheme, MenuTheme,
        MenubarTheme, MeterTheme, NavigationMenuTheme, PopupTheme, ProgressTheme, RadioTheme,
        ScrollAreaTheme, ScrollbarTheme, SliderTheme, SwitchTheme, TabsTheme, TextFieldStyle,
        ToastTheme, ToggleGroupTheme, TooltipTheme,
    };
}
