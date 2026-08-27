//! Material 3 component gallery.
//!
//! This example is intentionally built from the public Material facade.  It
//! exercises the theme scope, button families, stateful chips/list tiles,
//! progress indicators, input decoration, and retained callbacks in one
//! small application that is useful for visual and interaction smoke tests.

use incular::material::{
    ActionChip, AppBar, Badge, Card, CheckboxListTile, ChoiceChip, CircularProgressIndicator,
    ColorScheme, Dialog, DialogHandle, Divider, DropdownButton, DropdownMenuItem, ElevatedButton,
    FilledButton, FilterChip, FloatingActionButton, InputChip, InputDecoration,
    LinearProgressIndicator, ListTile, Material, MaterialApp, MenuAnchor, MenuItemButton,
    NavigationBar, NavigationDestination, NavigationDrawer, NavigationDrawerDestination,
    OutlinedButton, PopupMenuButton, PopupMenuItem, RadioListTile, RangeSlider, RangeValues,
    Scaffold, ScaffoldMessenger, ScaffoldMessengerController, Slider, SnackBar, SnackBarAction,
    SwitchListTile, Tab, TabBar, TabBarView, TabController, TextButton, TextField, ThemeData,
    ThemeMode, Tooltip, TooltipController,
};
use incular::prelude::{
    Application, Column, Container, CrossAxisAlignment, EdgeInsets, MainAxisSize, Row, Signal,
    SingleChildScrollView, Size, SizedBox, Text, Widget, WindowOptions,
};
use incular::widgets::internal::TextEditingController;

fn heading(text: impl Into<String>, theme: &ThemeData) -> Widget {
    Text::new(text)
        .style(theme.text_theme.title_large.clone())
        .into()
}

fn section(title: impl Into<String>, children: impl IntoIterator<Item = Widget>) -> Widget {
    Card::new(
        Column::new(
            std::iter::once(Text::new(title).into())
                .chain(children)
                .collect::<Vec<_>>(),
        )
        .spacing(12.0)
        .cross_axis_alignment(CrossAxisAlignment::Start),
    )
    .padding(EdgeInsets::all(20.0))
    .into()
}

fn main() {
    let mut options = WindowOptions::new("Incular Material 3.47.1 gallery");
    options.initial_logical_size = Size::new(1180.0, 820.0);
    options.minimum_logical_size = Some(Size::new(760.0, 560.0));

    let checked = Signal::new(true);
    let radio = Signal::new("one".to_owned());
    let switched = Signal::new(false);
    let choice = Signal::new(false);
    let filter = Signal::new(true);
    let progress = Signal::new(0.62_f32);
    let dark_mode = Signal::new(false);
    let slider_value = Signal::new(0.45_f32);
    let range_values = Signal::new(RangeValues::new(0.2, 0.8));
    let field = TextEditingController::with_text("Material defaults");
    let messenger = ScaffoldMessengerController::new();
    let dialog = DialogHandle::new(
        Dialog::new(
            Column::new([
                Widget::from(Text::new("Gallery dialog")),
                Widget::from(Text::new("Modal focus and Material elevation")),
            ])
            .spacing(10.0),
        )
        .semantic_label("Material gallery dialog"),
    );
    let tooltip = TooltipController::new();
    let tabs = TabController::new(2);
    let menu_controller = incular_material::MenuController::new();

    let app = Application::new_with_options(options, move |_cx| {
        let is_dark = dark_mode.get();
        let theme = if is_dark {
            ThemeData::dark()
        } else {
            ThemeData::from_color_scheme(ColorScheme::from_seed(incular::prelude::Color::rgba(
                0x67, 0x50, 0xa4, 255,
            )))
        };
        let checked_value = checked.get();
        let radio_value = radio.get();
        let switch_value = switched.get();
        let choice_value = choice.get();
        let filter_value = filter.get();
        let progress_value = progress.get();
        let slider_now = slider_value.get();
        let range_now = range_values.get();

        let buttons = section(
            "Button families",
            [
                Widget::from(
                    Row::new([
                        Widget::from(
                            ElevatedButton::new("Elevated")
                                .on_click(|| println!("elevated button pressed")),
                        ),
                        Widget::from(FilledButton::new("Filled")),
                        Widget::from(FilledButton::tonal("Tonal")),
                        Widget::from(OutlinedButton::new("Outlined")),
                        Widget::from(TextButton::new("Text")),
                        Widget::from(
                            FilledButton::tonal(if is_dark { "Light" } else { "Dark" }).on_click({
                                let dark_mode = dark_mode.clone();
                                move || dark_mode.update(|value| *value = !*value)
                            }),
                        ),
                    ])
                    .spacing(10.0)
                    .cross_axis_alignment(CrossAxisAlignment::Center),
                ),
                Widget::from(
                    Row::new([
                        Widget::from(FloatingActionButton::extended("Create")),
                        Widget::from(SizedBox::new().width(10.0)),
                        Widget::from(
                            FloatingActionButton::small(Text::new("+"))
                                .tooltip("Add item")
                                .on_click(|| println!("fab pressed")),
                        ),
                    ])
                    .spacing(8.0),
                ),
                Widget::from(
                    Row::new([
                        Widget::from(CheckboxListTile::new(
                            checked_value,
                            Text::new("Checkbox tile"),
                        )),
                        Widget::from(SwitchListTile::new(switch_value, Text::new("Switch tile"))),
                    ])
                    .spacing(12.0),
                ),
            ],
        );

        let selection = section(
            "Selection and toggles",
            [
                Widget::from(
                    CheckboxListTile::new(checked_value, Text::new("Remember settings"))
                        .on_changed({
                            let checked = checked.clone();
                            move |value| {
                                checked.set(value);
                            }
                        }),
                ),
                Widget::from(
                    RadioListTile::new(
                        "one".to_owned(),
                        Some(radio_value.clone()),
                        Text::new("First option"),
                    )
                    .on_changed({
                        let radio = radio.clone();
                        move |value| {
                            radio.set(value);
                        }
                    }),
                ),
                Widget::from(
                    RadioListTile::new(
                        "two".to_owned(),
                        Some(radio_value),
                        Text::new("Second option"),
                    )
                    .on_changed({
                        let radio = radio.clone();
                        move |value| {
                            radio.set(value);
                        }
                    }),
                ),
                Widget::from(
                    SwitchListTile::new(switch_value, Text::new("Enable notifications"))
                        .on_changed({
                            let switched = switched.clone();
                            move |value| {
                                switched.set(value);
                            }
                        }),
                ),
            ],
        );

        let chips = section(
            "Chip family",
            [
                Widget::from(
                    Row::new([
                        Widget::from(ActionChip::new("Action").on_pressed(|| println!("action"))),
                        Widget::from(ChoiceChip::new("Choice", choice_value).on_selected({
                            let choice = choice.clone();
                            move |value| {
                                choice.set(value);
                            }
                        })),
                        Widget::from(FilterChip::new("Filter", filter_value).on_selected({
                            let filter = filter.clone();
                            move |value| {
                                filter.set(value);
                            }
                        })),
                        Widget::from(
                            InputChip::new("Input")
                                .on_deleted(|| println!("chip deleted"))
                                .on_pressed(|| println!("chip pressed")),
                        ),
                    ])
                    .spacing(8.0),
                ),
                Widget::from(Badge::new("3").child(Text::new("Inbox"))),
            ],
        );

        let progress_section = section(
            "Progress and list content",
            [
                Widget::from(
                    LinearProgressIndicator::new()
                        .value(progress_value)
                        .label("Loading"),
                ),
                Widget::from(
                    Row::new([
                        Widget::from(CircularProgressIndicator::new().value(progress_value)),
                        Widget::from(SizedBox::new().width(16.0)),
                        Widget::from(Text::new(format!(
                            "{:.0}% complete",
                            progress_value * 100.0
                        ))),
                        Widget::from(SizedBox::new().width(16.0)),
                        Widget::from(FilledButton::new("Advance").on_click({
                            let progress = progress.clone();
                            move || progress.update(|value| *value = (*value + 0.1).min(1.0))
                        })),
                    ])
                    .cross_axis_alignment(CrossAxisAlignment::Center),
                ),
                Widget::from(Divider::new()),
                Widget::from(
                    ListTile::new(Text::new("Material list tile"))
                        .subtitle(Text::new("Leading, trailing, and semantic actions"))
                        .trailing(TextButton::new("Open"))
                        .on_tap(|| println!("tile tapped")),
                ),
            ],
        );

        let text_section = section(
            "Text input and defaults",
            [Widget::from(
                TextField::new(field.clone())
                    .placeholder("Type something")
                    .input_decoration(
                        InputDecoration::new()
                            .label("Project name")
                            .hint("A themed Material text field"),
                    ),
            )],
        );

        let slider_section = section(
            "Slider and range slider",
            [
                Widget::from(
                    Slider::new(slider_now)
                        .range(0.0, 1.0)
                        .divisions(Some(20))
                        .label("Opacity")
                        .on_changed({
                            let slider_value = slider_value.clone();
                            move |value| {
                                slider_value.set(value);
                            }
                        }),
                ),
                Widget::from(RangeSlider::new(range_now).range(0.0, 1.0).on_changed({
                    let range_values = range_values.clone();
                    move |value| {
                        range_values.set(value);
                    }
                })),
            ],
        );

        let menu_section = section(
            "Menus, dropdowns, tabs and feedback",
            [
                Widget::from(
                    MenuAnchor::new(vec![
                        Widget::from(MenuItemButton::label("New")),
                        Widget::from(MenuItemButton::label("Disabled").enabled(false)),
                    ])
                    .controller(menu_controller.clone())
                    .child(FilledButton::tonal("Menu")),
                ),
                Widget::from(
                    PopupMenuButton::from_items([
                        PopupMenuItem::label("One").value("one"),
                        PopupMenuItem::label("Two").value("two"),
                    ])
                    .child(TextButton::new("Popup menu")),
                ),
                Widget::from(
                    DropdownButton::new([
                        DropdownMenuItem::label("Alpha").value("a"),
                        DropdownMenuItem::label("Beta").value("b"),
                    ])
                    .hint(Text::new("Choose a value")),
                ),
                Widget::from(
                    Row::new([
                        Widget::from(
                            TabBar::new([Tab::text("Overview"), Tab::text("Details")])
                                .controller(tabs.clone()),
                        ),
                        Widget::from(
                            TabBarView::new([Text::new("Overview"), Text::new("Details")])
                                .controller(tabs.clone()),
                        ),
                    ])
                    .spacing(12.0),
                ),
                Widget::from(
                    Tooltip::new("Material tooltip", Text::new("Hover or focus me"))
                        .controller(tooltip.clone()),
                ),
                Widget::from(
                    Row::new([
                        Widget::from(ElevatedButton::new("Dialog").on_click({
                            let dialog = dialog.clone();
                            move || dialog.open()
                        })),
                        Widget::from(OutlinedButton::new("SnackBar").on_click({
                            let messenger = messenger.clone();
                            move || {
                                messenger.show_snack_bar(SnackBar::text("Saved").action(
                                    SnackBarAction::new("Dismiss", {
                                        let messenger = messenger.clone();
                                        move || {
                                            let _ = messenger.hide_current_snack_bar();
                                        }
                                    }),
                                ))
                            }
                        })),
                    ])
                    .spacing(12.0),
                ),
            ],
        );

        let navigation = section(
            "Material navigation",
            [
                Widget::from(
                    NavigationBar::new([
                        NavigationDestination::new(Text::new("⌂"), "Home"),
                        NavigationDestination::new(Text::new("⚙"), "Settings"),
                    ])
                    .selected_index(0),
                ),
                Widget::from(
                    NavigationDrawer::new([
                        NavigationDrawerDestination::new(Text::new("⌂"), "Home"),
                        NavigationDrawerDestination::new(Text::new("⚙"), "Settings"),
                    ])
                    .width(280.0),
                ),
            ],
        );

        let body = Container::new()
            .color(theme.scaffold_background_color)
            .padding(EdgeInsets::all(28.0))
            .child(
                SingleChildScrollView::new(
                    Column::new([
                        heading("Material 3.47.1", &theme),
                        Text::new("Retained widgets, stateful controls, and theme-driven defaults")
                            .style(theme.text_theme.body_large.clone())
                            .into(),
                        buttons,
                        selection,
                        chips,
                        progress_section,
                        text_section,
                        slider_section,
                        menu_section,
                        navigation,
                        Widget::from(Container::new().padding(EdgeInsets::all(18.0)).child(
                            Material::new(Text::new("Surface with elevation")).elevation(3.0),
                        )),
                    ])
                    .spacing(18.0)
                    .main_axis_size(MainAxisSize::Min)
                    .cross_axis_alignment(CrossAxisAlignment::Stretch),
                )
                .padding(EdgeInsets::all(2.0)),
            );

        let scaffold = Scaffold::new(body)
            .app_bar(AppBar::new(Text::new("Material Gallery")))
            .floating_action_button(FloatingActionButton::extended("Add"));
        let shell = ScaffoldMessenger::with_controller(messenger.clone(), scaffold);
        let root: Widget = MaterialApp::new(shell)
            .theme(theme.clone())
            .dark_theme(ThemeData::dark())
            .theme_mode(if is_dark {
                ThemeMode::Dark
            } else {
                ThemeMode::Light
            })
            .title("Incular Material gallery")
            .into();
        dialog.present(root)
    })
    .expect("valid Material gallery application");

    incular::run(app).expect("native Material gallery application");
}
