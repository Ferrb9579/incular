//! Public-API-only Material workbench.
//!
//! This is intentionally a small deployable application rather than a test
//! fixture.  It keeps the common controls visible at once so keyboard,
//! pointer, popup, tab, dialog, snackbar, and theme changes can be exercised
//! manually on a desktop backend.

use incular::prelude::*;
use incular_material::prelude::*;
use incular_material::{
    Dialog as MaterialDialog, DialogHandle, NavigationDrawer, NavigationDrawerDestination,
    NavigationRail, NavigationRailDestination, ScaffoldMessenger, ScaffoldMessengerController,
};
use std::time::Duration;

fn panel(title: &str, children: impl IntoIterator<Item = Widget>) -> Widget {
    Card::new(
        Column::new(
            std::iter::once(Text::new(title).into())
                .chain(children)
                .collect::<Vec<_>>(),
        )
        .spacing(10.0)
        .cross_axis_alignment(CrossAxisAlignment::Start),
    )
    .padding(EdgeInsets::all(18.0))
    .into()
}

fn main() {
    let mut options = WindowOptions::new("Incular Material workbench");
    options.initial_logical_size = Size::new(1280.0, 860.0);
    options.minimum_logical_size = Some(Size::new(900.0, 620.0));

    let dark = Signal::new(false);
    let checked = Signal::new(false);
    let slider_value = Signal::new(0.35_f32);
    let selected_tab = TabController::new(3);
    let messenger = ScaffoldMessengerController::new();
    let dialog = DialogHandle::new(
        MaterialDialog::new(
            Column::new([
                Widget::from(Text::new("A retained Material dialog")),
                Widget::from(Text::new("Escape or press the action to close it.")),
            ])
            .spacing(12.0),
        )
        .semantic_label("Workbench dialog"),
    );

    let menu_controller = MenuController::new();
    let app = Application::new_with_options(options, move |_cx| {
        let is_dark = dark.get();
        let checked_now = checked.get();
        let slider_now = slider_value.get();
        let theme = if is_dark {
            ThemeData::dark()
        } else {
            ThemeData::from_seed(Color::rgba(103, 80, 164, 255))
        };
        let theme_toggle = FilledButton::new(if is_dark {
            "Use light theme"
        } else {
            "Use dark theme"
        })
        .on_click({
            let dark = dark.clone();
            move || dark.update(|value| *value = !*value)
        });
        let snackbar_button = OutlinedButton::new("Show snackbar").on_click({
            let messenger = messenger.clone();
            move || {
                messenger.show_snack_bar(
                    SnackBar::text("Saved from the Material workbench")
                        .duration(Duration::from_secs(3))
                        .action(SnackBarAction::new("Dismiss", {
                            let messenger = messenger.clone();
                            move || {
                                let _ = messenger.hide_current_snack_bar();
                            }
                        })),
                )
            }
        });
        let dialog_button = ElevatedButton::new("Open dialog").on_click({
            let dialog = dialog.clone();
            move || dialog.open()
        });
        let menu = MenuAnchor::new(vec![
            Widget::from(
                MenuItemButton::label("New document").on_pressed(|| println!("new document")),
            ),
            Widget::from(MenuItemButton::label("Disabled item").enabled(false)),
            Widget::from(SubmenuButton::new(
                Text::new("More"),
                vec![Widget::from(MenuItemButton::label("Preferences"))],
            )),
        ])
        .controller(menu_controller.clone())
        .child(FilledButton::tonal("Open menu"));
        let tabs = TabBar::new([
            Tab::text("Overview"),
            Tab::text("Activity"),
            Tab::text("Settings"),
        ])
        .controller(selected_tab.clone())
        .on_tap({
            let selected_tab = selected_tab.clone();
            move |index| selected_tab.animate_to(index)
        });
        let tab_view = TabBarView::new([
            Text::new("Overview content"),
            Text::new("Activity content"),
            Text::new("Settings content"),
        ])
        .controller(selected_tab.clone());
        let body = SingleChildScrollView::new(
            Column::new([
                panel(
                    "Theme and feedback",
                    [
                        Text::new("Material 3 deployability workbench").into(),
                        theme_toggle.into(),
                        snackbar_button.into(),
                        dialog_button.into(),
                    ],
                ),
                panel(
                    "Selection and slider",
                    [
                        Checkbox::new(checked_now)
                            .on_changed_bool({
                                let checked = checked.clone();
                                move |value| {
                                    checked.set(value);
                                }
                            })
                            .into(),
                        Switch::new(checked_now)
                            .on_changed({
                                let checked = checked.clone();
                                move |value| {
                                    checked.set(value);
                                }
                            })
                            .into(),
                        Slider::new(slider_now)
                            .on_changed({
                                let slider_value = slider_value.clone();
                                move |value| {
                                    slider_value.set(value);
                                }
                            })
                            .into(),
                        RangeSlider::new(RangeValues::new(0.2, 0.8))
                            .range(0.0, 1.0)
                            .into(),
                    ],
                ),
                panel(
                    "Menus and tabs",
                    [menu.into(), tabs.into(), tab_view.into()],
                ),
                panel(
                    "Forms and navigation",
                    [
                        TextField::new(TextEditingController::with_text("Project"))
                            .input_decoration(
                                InputDecoration::new()
                                    .label("Project name")
                                    .helper("Material InputDecorator"),
                            )
                            .into(),
                        NavigationBar::new([
                            NavigationDestination::new(Text::new(Icons::MENU), "Home"),
                            NavigationDestination::new(Text::new(Icons::SETTINGS), "Settings"),
                        ])
                        .selected_index(0)
                        .into(),
                        NavigationRail::new([
                            NavigationRailDestination::new(Text::new(Icons::MENU), "Home"),
                            NavigationRailDestination::new(Text::new(Icons::SEARCH), "Search"),
                        ])
                        .extended(true)
                        .into(),
                    ],
                ),
            ])
            .spacing(16.0)
            .cross_axis_alignment(CrossAxisAlignment::Stretch),
        );
        let scaffold = Scaffold::new(
            Container::new()
                .color(theme.scaffold_background_color)
                .padding(EdgeInsets::all(24.0))
                .child(body),
        )
        .app_bar(
            AppBar::new(Text::new("Material Workbench"))
                .actions([IconButton::icon(Text::new(Icons::MORE_VERT)).tooltip("More")]),
        )
        .drawer(
            NavigationDrawer::new([
                NavigationDrawerDestination::new(Text::new(Icons::MENU), "Home"),
                NavigationDrawerDestination::new(Text::new(Icons::SETTINGS), "Settings"),
            ])
            .header(Text::new("Workbench")),
        )
        .floating_action_button(FloatingActionButton::extended("Create"));
        let shell = ScaffoldMessenger::with_controller(messenger.clone(), scaffold);
        let root: Widget = MaterialApp::new(shell)
            .theme(theme.clone())
            .dark_theme(ThemeData::dark())
            .theme_mode(if is_dark {
                ThemeMode::Dark
            } else {
                ThemeMode::Light
            })
            .title("Incular Material workbench")
            .into();
        dialog.present(root)
    })
    .expect("valid Material workbench application");

    incular::run(app).expect("native Material workbench application");
}
