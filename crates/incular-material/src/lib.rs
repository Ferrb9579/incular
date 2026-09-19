//! Material-layer controls for Incular.
//!
//! Flutter's Material library owns button families, `TextField`,
//! `TextFormField`, `SelectableText`, `SelectionArea`, and `Autocomplete`.
//! They are deliberately not part of the renderer-neutral
//! [`incular_widgets`] surface.

mod app_shell;
mod buttons;
mod feedback;
mod inputs;
mod lists;
#[path = "icons.rs"]
mod material_icons;
#[path = "selection.rs"]
mod material_selection;
#[path = "theme.rs"]
mod material_theme;
mod menus;
mod navigation;
mod surfaces;

pub use app_shell::{
    BackButton, BackButtonIcon, BottomAppBar, CloseButton, CloseButtonIcon, CollapseMode, Drawer,
    DrawerAlignment, DrawerButton, DrawerButtonIcon, DrawerHeader, EndDrawerButton,
    EndDrawerButtonIcon, K_TOOLBAR_HEIGHT, MaterialApp, MaterialPointerDevice,
    MaterialScrollBehavior, NavigationDrawer, NavigationDrawerDestination, NavigationRail,
    NavigationRailDestination, ScaffoldMessenger, ScaffoldMessengerController, StretchMode,
    k_toolbar_height,
};
pub use feedback::{
    AlertDialog, Dialog, DialogHandle, DialogResultHandle, DialogRoute, ProgressIndicatorStrokeCap,
    ProgressIndicatorTheme, ProgressIndicatorThemeData, RefreshIndicatorStatus,
    RefreshIndicatorTriggerMode, SimpleDialog, SimpleDialogOption, SnackBar, SnackBarAction,
    SnackBarBehavior, SnackBarClosedReason, StepState, StepperType, Tooltip, TooltipController,
    TooltipTriggerMode, current_progress_indicator_theme, show_dialog, show_dialog_result,
};
pub use material_icons::Icons;
pub use material_selection::{
    Checkbox, Radio, RadioGroup, RangeLabels, RangeSlider, RangeValues, ShowValueIndicator, Slider,
    SliderInteraction, SliderThemeData, Switch,
};
pub use menus::{
    CheckedPopupMenuItem, DropdownButton, DropdownButtonBuilder, DropdownButtonFormField,
    DropdownButtonHideUnderline, DropdownMenu, DropdownMenuCloseBehavior,
    DropdownMenuDecorationBuilder, DropdownMenuEntry, DropdownMenuFormField, DropdownMenuItem,
    DropdownMenuThemeData, FilterCallback, MenuAnchor, MenuBar, MenuController, MenuItemButton,
    MenuStyle, MenuThemeData, PopupMenuButton, PopupMenuDivider, PopupMenuEntry, PopupMenuItem,
    PopupMenuPosition, PopupMenuThemeData, SearchCallback, SubmenuButton,
};
pub use navigation::{Tab, TabBar, TabBarThemeData, TabBarView, TabController};

pub use app_shell::{AppBar, Scaffold, SliverAppBar};
pub use buttons::{
    ButtonBarLayoutBehavior, ButtonStyleConfig, ButtonTextTheme, ElevatedButton, FilledButton,
    FloatingActionButton, IconButton, K_FLOATING_ACTION_BUTTON_MARGIN, OutlinedButton, StyleFrom,
    TextButton, style_from,
};
pub use feedback::{CircularProgressIndicator, LinearProgressIndicator};
pub use inputs::*;
pub use lists::{
    ActionChip, Badge, CheckboxListTile, Chip, ChoiceChip, FilterChip, InputChip, ListTile,
    ListTileControlAffinity, ListTileStyle, ListTileTitleAlignment, RadioListTile, RawChip,
    SwitchListTile,
};
pub use material_theme::*;
pub use navigation::{
    BottomNavigationBar, BottomNavigationBarItem, BottomNavigationBarLandscapeLayout,
    BottomNavigationBarType, K_BOTTOM_NAVIGATION_BAR_HEIGHT, K_TAB_SCROLL_DURATION, NavigationBar,
    NavigationDestination, NavigationDestinationLabelBehavior, NavigationRailLabelType,
    TabAlignment, TabBarIndicatorSize, TabIndicatorAnimation, k_bottom_navigation_bar_height,
};
pub use surfaces::{Card, CircleAvatar, Divider, Surface, VerticalDivider};
pub use surfaces::{
    Ink, InkResponse, InkWell, K_MIN_INTERACTIVE_DIMENSION, K_RADIAL_REACTION_ALPHA,
    K_RADIAL_REACTION_RADIUS, Material, MaterialTapTargetSize, MaterialType, VisualDensity,
    k_min_interactive_dimension,
};

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
