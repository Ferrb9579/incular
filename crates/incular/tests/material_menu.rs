#![cfg(all(feature = "desktop", feature = "material"))]

use incular::material::{
    AppBar, Card, Drawer, DrawerButton, FilledButton, FloatingActionButton, IconButton,
    MaterialApp, MenuAnchor, MenuController, MenuItemButton, NavigationDrawer,
    NavigationDrawerDestination, Scaffold, ScaffoldMessenger, ScaffoldMessengerController,
    ThemeData, ThemeMode,
};
use incular::prelude::*;

#[test]
fn material_menu_opens_in_an_application_frame_without_recursing() {
    let controller = MenuController::new();
    let controller_for_build = controller.clone();
    let light_theme = ThemeData::from_seed_shared(Color::rgba(103, 80, 164, 255));
    let dark_theme = ThemeData::dark_shared();
    let app =
        Application::new_with_options(WindowOptions::new("Material menu regression"), move |_cx| {
            let body: Widget = Container::new()
                .color(light_theme.colors().scaffold_background_color)
                .padding(EdgeInsets::all(24.0))
                .child(SingleChildScrollView::new(
                    Column::new([Widget::from(
                        Card::new(
                            Column::new([
                                Widget::from(Text::new("Button families")),
                                MenuAnchor::new([MenuItemButton::label("New document")])
                                    .controller(controller_for_build.clone())
                                    .child(FilledButton::tonal("Menu"))
                                    .into(),
                            ])
                            .spacing(10.0)
                            .cross_axis_alignment(CrossAxisAlignment::Start),
                        )
                        .padding(EdgeInsets::all(18.0)),
                    )])
                    .spacing(16.0)
                    .cross_axis_alignment(CrossAxisAlignment::Stretch),
                ))
                .into();
            let messenger = ScaffoldMessengerController::new();
            let drawer = Drawer::new()
                .open(false)
                .panel(
                    NavigationDrawer::new([
                        NavigationDrawerDestination::new(Text::new("☰"), "Home"),
                        NavigationDrawerDestination::new(Text::new("⚙"), "Settings"),
                    ])
                    .header(Text::new("Workbench")),
                )
                .child(SizedBox::shrink());
            MaterialApp::new(ScaffoldMessenger::with_controller(
                messenger,
                Scaffold::new(body)
                    .app_bar(
                        AppBar::new(Text::new("Material Workbench"))
                            .leading(DrawerButton::new())
                            .actions([IconButton::icon(Text::new("⋮")).tooltip("More")]),
                    )
                    .drawer(drawer)
                    .floating_action_button(FloatingActionButton::extended("Create")),
            ))
            .theme_shared(light_theme.clone())
            .dark_theme_shared(dark_theme.clone())
            .theme_mode(ThemeMode::Light)
            .title("Incular Material workbench")
            .build()
        })
        .expect("create material menu application");
    let constraints = Constraints::tight(Size::new(1180.0, 820.0));
    let window = app.primary_window();
    let mut app = app;
    app.run_window_frame_at(window, constraints, std::time::Instant::now())
        .expect("initial frame");
    controller.open();
    app.run_window_frame_at(window, constraints, std::time::Instant::now())
        .expect("opened frame");
    app.run_window_frame_at(window, constraints, std::time::Instant::now())
        .expect("stable opened frame");
}
