#![cfg(all(feature = "desktop", feature = "material"))]

use incular::material::{
    AppBar, Card, Drawer, DrawerButton, FilledButton, FloatingActionButton, IconButton,
    MaterialApp, MenuAnchor, MenuController, MenuItemButton, NavigationDrawer,
    NavigationDrawerDestination, Scaffold, ScaffoldMessenger, ScaffoldMessengerController,
    ThemeData, ThemeMode,
};
use incular::platform::PlatformEvent;
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
            .into()
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

#[test]
fn material_menu_does_not_resize_a_content_sized_undecorated_window() {
    let controller = MenuController::new();
    let controller_for_build = controller.clone();
    let options = WindowOptions {
        initial_logical_size: Size::new(100.0, 200.0),
        size_policy: WindowSizePolicy::Content,
        decorations: false,
        ..WindowOptions::new("Content-sized menu")
    };
    let mut app = Application::new_with_options(options, move |_cx| {
        MaterialApp::new(
            Container::new().width(100.0).height(200.0).child(
                MenuAnchor::new([
                    MenuItemButton::label("First"),
                    MenuItemButton::label("Second"),
                ])
                .controller(controller_for_build.clone())
                .alignment_offset(Offset::new(0.0, 200.0))
                .child(FilledButton::tonal("Menu")),
            ),
        )
        .into()
    })
    .expect("create content-sized material menu application");
    let window = app.primary_window();
    let _ = app.take_native_window_commands();
    let constraints = Constraints::tight(Size::new(100.0, 200.0));
    app.run_window_frame_at(window, constraints, std::time::Instant::now())
        .expect("initial content-sized frame");
    let _ = app.take_native_window_commands();

    controller.open();
    app.run_window_frame_at(window, constraints, std::time::Instant::now())
        .expect("opened frame");
    let transients = app.transient_surfaces(window);
    assert_eq!(transients.len(), 1);
    assert_eq!(transients[0].role, TransientRole::Menu);
    assert_eq!(transients[0].presentation, TransientPresentation::Auto);
    assert!(transients[0].anchor_rect.size.width > 0.0);
    assert!(transients[0].anchor_rect.size.height > 0.0);
    assert!(transients[0].content_rect.size.height > 0.0);
    let resize_requested = app
        .take_native_window_commands()
        .into_iter()
        .any(|command| {
            matches!(
                command,
                NativeWindowCommand::Operate(WindowCommand {
                    operation: WindowOperation::SetLogicalSize(_),
                    ..
                })
            )
        });
    assert!(
        !resize_requested,
        "transient menu presentation must not change top-level content size"
    );
}

#[test]
fn open_material_menu_anchor_remains_pointer_interactive_above_its_barrier() {
    let controller = MenuController::new();
    let controller_for_build = controller.clone();
    let mut app = Application::new(move |_cx| {
        MenuAnchor::new([MenuItemButton::label("Item")])
            .controller(controller_for_build.clone())
            .child(FilledButton::tonal("Menu"))
            .into()
    })
    .expect("create material menu application");
    let window = app.primary_window();
    let constraints = Constraints::tight(Size::new(320.0, 240.0));
    app.run_window_frame_at(window, constraints, std::time::Instant::now())
        .expect("initial frame");

    controller.open();
    app.run_window_frame_at(window, constraints, std::time::Instant::now())
        .expect("open frame");
    let anchor = app.transient_surfaces(window)[0].anchor_rect;
    let point = anchor.origin + Offset::new(anchor.size.width * 0.5, anchor.size.height * 0.5);
    for phase in [PointerPhase::Down, PointerPhase::Up] {
        app.handle_window_event(WindowEvent::platform(
            window,
            PlatformEvent::Input(InputEvent::Pointer {
                phase,
                position: point,
            }),
        ));
    }

    assert!(
        !controller.is_open(),
        "the anchor must stay above the overlay barrier so it can toggle closed"
    );
}
