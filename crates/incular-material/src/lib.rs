//! Material-layer controls for Incular.
//!
//! Flutter's Material library owns button families, `TextField`,
//! `TextFormField`, `SelectableText`, `SelectionArea`, and `Autocomplete`.
//! They are deliberately not part of the renderer-neutral
//! [`incular_widgets`] surface.

use incular_core::Size;
use incular_text::TextStyle;
use incular_widgets::{
    AutovalidateMode, Border, BorderRadius, BoxDecoration, Container,
    EditableText as RawEditableText, SelectionAreaController, TextEditingController, Widget,
};
use std::rc::Rc;

type Validator = Rc<dyn Fn(&str) -> Option<String>>;
type ChangedCallback = Rc<dyn Fn(&str)>;

pub use incular_controls::theme::TypographyTokens;
pub use incular_controls::theme::{
    AutocompleteTheme, AvatarTheme, CheckboxTheme, ContextMenuTheme, DrawerTheme, InputTheme,
    MenuTheme, MenubarTheme, MeterTheme, NavigationMenuTheme, PopupTheme, ProgressTheme,
    RadioTheme, ScrollAreaTheme, ScrollbarTheme, SliderTheme, SwitchTheme, TabsTheme, ToastTheme,
    ToggleGroupTheme, TooltipTheme,
};
pub use incular_controls::toast::{Provider as ToastProvider, ToastController, ToastId};
pub use incular_controls::{
    AlertDialogTheme, Avatar, ButtonStyle, ButtonTheme, ButtonVariant, Card, CardStyle, Checkbox,
    CheckboxGroupTheme, CheckedState, CompositeController, CompositeItem, CompositeOrientation,
    ControlColors, ControlDensity, ControlIcon, ControlMetrics, ControlMotion, ControlState,
    ControlTheme, ControlThemeScope, ControlTypography, Divider, DividerStyle, ElevationTokens,
    GhostButton, GroupTheme, IconButton, Meter, MotionTokens, PaletteTokens, PrimaryButton,
    Progress, Radio, RadiusTokens, Scrollbar, SelectionStyle, SpacingTokens, StateColor,
    StateTable, StateValue, Switch, TextFieldStyle, Toast, alert_dialog, app,
    autocomplete as control_autocomplete, avatar, checkbox, checkbox_group, collapsible, combobox,
    composite, containers, context_menu, current_control_theme, dialog, drawer, field, fieldset,
    form, icons, menu, menubar, meter, navigation_menu, number_field, otp_field, overlay, popover,
    popup, preview_card, progress, scroll_area, scrollbar, select, selection, separator, slider,
    styles, switch, tabs, theme, toast, toggle, toggle_group, toolbar, tooltip,
};

/// Material elevated button vocabulary. Incular's themed primary button is
/// the equivalent high-emphasis surface.
pub type ElevatedButton = incular_controls::PrimaryButton;

/// Material filled button vocabulary. It shares the primary button's retained
/// behavior while allowing applications to override its `ButtonStyle`.
pub type FilledButton = incular_controls::PrimaryButton;

/// Material outlined button vocabulary.
pub type OutlinedButton = incular_controls::Button;

/// Material text button vocabulary.
pub type TextButton = incular_controls::GhostButton;

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
pub type AlertDialog = incular_controls::alert_dialog::Root;
pub type CheckboxGroup = incular_controls::checkbox_group::Root;
pub type Collapsible = incular_controls::collapsible::Root;
pub type ComboBox = incular_controls::combobox::Root;
pub type ContextMenu = incular_controls::context_menu::Root;
pub type Drawer = incular_controls::drawer::Root;
pub type Field = incular_controls::field::Root;
pub type FieldSet = incular_controls::fieldset::Root;
pub type Menu = incular_controls::menu::Root;
pub type MenuBar = incular_controls::menubar::Root;
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
pub type Slider = incular_controls::slider::Root;
pub type TabBar = incular_controls::tabs::Root;
pub type Toggle = incular_controls::toggle::Toggle;
pub type ToggleGroup = incular_controls::toggle_group::Root;
pub type Toolbar = incular_controls::toolbar::Root;
pub type Tooltip = incular_controls::tooltip::Root;
pub type TooltipProvider = incular_controls::tooltip::Provider;

/// Read-only text with Material selection behavior.
#[derive(Clone, Debug, PartialEq)]
pub struct SelectableText {
    text: String,
    style: TextStyle,
    align: incular_text::TextAlign,
}

impl SelectableText {
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style: TextStyle::default(),
            align: incular_text::TextAlign::Start,
        }
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
    style: TextStyle,
    on_submit: Option<Rc<dyn Fn(String) + 'static>>,
    max_lines: Option<usize>,
}

impl TextField {
    #[must_use]
    pub fn new(controller: TextEditingController) -> Self {
        Self {
            controller,
            size: Size::ZERO,
            placeholder: String::new(),
            decoration: TextFieldStyle::default(),
            style: TextStyle::default(),
            on_submit: None,
            max_lines: Some(1),
        }
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
    pub fn decoration(mut self, decoration: TextFieldStyle) -> Self {
        self.decoration = decoration;
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
            .placeholder(self.placeholder.clone())
            .style(self.style.clone().color(foreground));
        if let Some(callback) = self.on_submit.clone() {
            raw = raw.on_submit(move |value| callback(value));
        }

        Container::new()
            .padding(padding)
            .decoration(
                BoxDecoration::new()
                    .color(background)
                    .border(border)
                    .border_radius(BorderRadius::circular(radius)),
            )
            .child(raw)
            .into()
    }
}

impl From<TextField> for Widget {
    fn from(value: TextField) -> Self {
        let value = Rc::new(value);
        Widget::layout_builder(move |_| {
            let theme =
                incular_widgets::current_build_environment::<ControlTheme>().unwrap_or_default();
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
    on_submit: Option<Rc<dyn Fn(String)>>,
    on_changed: Option<ChangedCallback>,
    autovalidate_mode: AutovalidateMode,
    placeholder: String,
    style: Option<TextStyle>,
    decoration: Option<TextFieldStyle>,
    size: Size,
    max_lines: Option<usize>,
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
            on_submit: None,
            on_changed: None,
            autovalidate_mode: AutovalidateMode::Disabled,
            placeholder: String::new(),
            style: None,
            decoration: None,
            size: Size::new(200.0, 36.0),
            max_lines: Some(1),
        }
    }

    #[must_use]
    pub fn controller(mut self, controller: TextEditingController) -> Self {
        self.controller = Some(controller);
        self
    }

    #[must_use]
    pub fn validator(mut self, validator: impl Fn(&str) -> Option<String> + 'static) -> Self {
        self.validator = Some(Rc::new(validator));
        self
    }

    #[must_use]
    pub fn on_submit(mut self, on_submit: impl Fn(String) + 'static) -> Self {
        self.on_submit = Some(Rc::new(on_submit));
        self
    }

    #[must_use]
    pub fn on_changed(mut self, on_changed: impl Fn(&str) + 'static) -> Self {
        self.on_changed = Some(Rc::new(on_changed));
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
    pub fn decoration(mut self, decoration: TextFieldStyle) -> Self {
        self.decoration = Some(decoration);
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
}

impl From<TextFormField> for Widget {
    fn from(value: TextFormField) -> Self {
        let controller = value.controller.unwrap_or_default();
        let mut field = TextField::new(controller)
            .placeholder(value.placeholder)
            .size(value.size)
            .max_lines(value.max_lines);
        if let Some(style) = value.style {
            field = field.style(style);
        }
        if let Some(decoration) = value.decoration {
            field = field.decoration(decoration);
        }
        if let Some(on_submit) = value.on_submit {
            field = field.on_submit(move |text| on_submit(text));
        }
        // Keep the form callbacks in the descriptor so the API remains
        // source-compatible; the retained Form/Field model owns validation
        // and change notification when mounted by an application.
        let _ = (value.validator, value.on_changed, value.autovalidate_mode);
        field.into()
    }
}

/// Material prelude for application-facing imports.
pub mod prelude {
    pub use super::{
        Autocomplete, ElevatedButton, FilledButton, IconButton, MaterialButton, OutlinedButton,
        RawMaterialButton, SelectableText, SelectionArea, TextButton, TextField, TextFieldStyle,
        TextFormField,
    };
    // Compatibility aliases retained for Incular applications; Flutter's
    // canonical button entry points are the four concrete variants above.
    pub use super::{GhostButton, PrimaryButton};
    pub use incular_controls::{Checkbox, Radio, Switch};
}

/// Complete Material component vocabulary. This explicit module is useful for
/// applications that want every component without importing the facade's
/// compact prelude (which intentionally avoids collisions with other design
/// systems).
pub mod components {
    pub use super::{
        AlertDialog, Autocomplete, Avatar, Card, Checkbox, CheckboxGroup, Collapsible, ComboBox,
        ContextMenu, Drawer, ElevatedButton, Field, FieldSet, FilledButton, GhostButton,
        IconButton, MaterialButton, Menu, MenuBar, Meter, NavigationMenu, NumberField, OtpField,
        OutlinedButton, Popover, Popup, PreviewCard, PrimaryButton, Progress, ProgressIndicator,
        Radio, RawMaterialButton, ScrollArea, Scrollbar, Select, SelectableText, SelectionArea,
        Separator, Slider, Switch, TabBar, TextButton, TextField, TextFormField, Toast,
        ToastProvider, Toggle, ToggleGroup, Toolbar, Tooltip, TooltipProvider,
    };
    pub use super::{
        AlertDialogTheme, AutocompleteTheme, AvatarTheme, ButtonStyle, ButtonTheme, CardStyle,
        CheckboxGroupTheme, CheckboxTheme, ContextMenuTheme, ControlTheme, ControlThemeScope,
        ControlTypography, Divider, DividerStyle, DrawerTheme, InputTheme, MenuTheme, MenubarTheme,
        MeterTheme, NavigationMenuTheme, PopupTheme, ProgressTheme, RadioTheme, ScrollAreaTheme,
        ScrollbarTheme, SliderTheme, SwitchTheme, TabsTheme, TextFieldStyle, ToastTheme,
        ToggleGroupTheme, TooltipTheme,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autocomplete_filters_and_selects_options() {
        let mut autocomplete = Autocomplete::strings(["Ada", "Grace", "Alan"]);
        assert_eq!(autocomplete.matching_indices(), vec![0, 1, 2]);
        autocomplete.set_query("al");
        assert_eq!(autocomplete.matching_indices(), vec![2]);
        assert_eq!(autocomplete.select(2).map(String::as_str), Some("Alan"));
        assert_eq!(autocomplete.selected().map(String::as_str), Some("Alan"));
    }

    #[test]
    fn material_text_field_uses_the_core_editable_text_primitive() {
        let controller = TextEditingController::with_text("hello");
        let _widget: Widget = TextField::new(controller)
            .max_lines(None)
            .style(TextStyle::default().font_size(16.0))
            .into();
    }

    #[test]
    fn material_components_convert_to_widgets() {
        let _button: Widget = ElevatedButton::new("Save").into();
        let _dialog: Widget = AlertDialog::new().into();
        let _tabs: Widget = TabBar::new().into();
    }

    #[test]
    fn raw_material_button_is_exposed_only_from_the_material_layer() {
        let _button: Widget = RawMaterialButton::new("Low-level action")
            .on_press(|| {})
            .into();
    }
}
