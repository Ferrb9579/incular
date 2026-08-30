//! Opt-in, declarative session restoration. Close and relaunch to verify the
//! counter, editor selection, scroll offset, navigator stack, and inspector
//! window are reconstructed without serializing the retained widget tree.
use std::{cell::RefCell, rc::Rc};

use incular::material::{RawMaterialButton, TextField};
use incular::prelude::*;
use incular::widgets::internal::{ScrollView, TextEditingController};
use serde_json::json;

fn key(value: &str) -> RestorationKey {
    RestorationKey::new(value).expect("example restoration keys are stable")
}

fn route_page(title: &str, color: Color) -> Page {
    Page::new(
        title,
        DecoratedBox::new(Padding::all(
            14.,
            Text::new(title).style(TextStyle {
                size: 22.,
                color: Color::WHITE,
                ..TextStyle::default()
            }),
        ))
        .size(Size::new(680., 64.))
        .background(color)
        .radius(12.),
    )
}

fn inspector_options() -> WindowOptions {
    WindowOptions {
        title: "Restoration Inspector".into(),
        initial_logical_size: Size::new(420., 260.),
        ..WindowOptions::default()
    }
}

fn inspector_builder(
    shared_count: Restorable<u32>,
) -> impl Fn(&mut BuildContext) -> Widget + 'static {
    let inspector_count = Rc::new(RefCell::new(None::<Restorable<u32>>));
    move |cx| {
        let local_count = {
            let mut slot = inspector_count.borrow_mut();
            if slot.is_none() {
                *slot = cx.restored_signal(key("inspector-count"), 0_u32);
            }
            slot.as_ref()
                .expect("restoration is enabled for this example")
                .clone()
        };
        let shared_value = shared_count.get();
        let local_value = local_count.get();
        Padding::all(
            22.,
            Widget::column(vec![
                Text::new("Restorable inspector")
                    .style(TextStyle {
                        size: 24.,
                        color: Color::rgba(196, 224, 255, 255),
                        ..TextStyle::default()
                    })
                    .into(),
                Text::new(format!("Shared counter: {shared_value}")).into(),
                Text::new(format!("Inspector-local counter: {local_value}")).into(),
                RawMaterialButton::new("Increment shared counter")
                    .on_press({
                        let shared_count = shared_count.clone();
                        move || shared_count.update(|value| *value += 1)
                    })
                    .into(),
                RawMaterialButton::new("Increment inspector state")
                    .on_press(move || local_count.update(|value| *value += 1))
                    .into(),
                Text::new(
                    "Close this window to omit it next launch; leave it open and relaunch to restore it.",
                )
                .into(),
            ]),
        )
        .into()
    }
}

struct MainState {
    count: Restorable<u32>,
    editor: TextEditingController,
    scroll: ScrollController,
    navigator: Navigator,
    navigation: Restorable<NavigatorSnapshot>,
    restoration: RestorationHandle,
}

#[path = "../support/mod.rs"]
mod example_support;
#[cfg(test)]
#[path = "tests.rs"]
mod example_tests;
mod simulations;

fn main() {
    let registry = RouteRegistry::new();
    let home_route = registry
        .register_restorable("/home", |_| {
            Ok(route_page("Home route", Color::rgba(41, 92, 169, 255)))
        })
        .expect("unique example route");
    let details_route = registry
        .register_restorable("/details", |_| {
            Ok(route_page("Details route", Color::rgba(111, 72, 174, 255)))
        })
        .expect("unique example route");

    let state = Rc::new(RefCell::new(None::<MainState>));
    let state_for_build = state.clone();
    let registry_for_build = registry.clone();
    let home_for_build = home_route.clone();
    let details_for_build = details_route.clone();
    let config = RestorationConfig::file_backed("dev.incular.examples.restoration", 1)
        .expect("a platform application-state directory");
    let mut app = Application::new_restorable(
        WindowRestorationId::new("main").expect("stable main window ID"),
        "main",
        WindowOptions {
            title: "Incular Restoration".into(),
            initial_logical_size: Size::new(780., 720.),
            ..WindowOptions::default()
        },
        config,
        move |cx| {
            let (count, editor, scroll, navigator, navigation, restoration) = {
                let mut slot = state_for_build.borrow_mut();
                if slot.is_none() {
                    let scope = cx
                        .restoration_scope()
                        .expect("restoration is enabled for this example");
                    let count = cx
                        .restored_signal(key("counter"), 0_u32)
                        .expect("restoration is enabled for this example");
                    let editor = TextEditingController::with_text(
                        "This committed text and its selection survive relaunch.",
                    );
                    editor.bind_restoration(scope.child_unchecked(key("editor")), key("document"));
                    let scroll = ScrollController::restored(
                        scope.child_unchecked(key("scroll")),
                        key("items"),
                    );
                    let navigation = cx
                        .restorable(key("navigation"), NavigatorSnapshot::default())
                        .expect("restoration is enabled for this example");
                    let navigator = Navigator::new();
                    let report =
                        registry_for_build.restore_navigator(&navigator, &navigation.get());
                    if report.restored_routes == 0 {
                        registry_for_build
                            .navigate_restorable(
                                &navigator,
                                RestorableRoute::new(home_for_build.clone(), json!({})),
                            )
                            .expect("registered home route");
                        let _ = navigation.set(navigator.restoration_snapshot());
                    }
                    *slot = Some(MainState {
                        count,
                        editor,
                        scroll,
                        navigator,
                        navigation,
                        restoration: cx
                            .restoration()
                            .expect("restoration is enabled for this example"),
                    });
                }
                let state = slot.as_ref().expect("main restoration state");
                (
                    state.count.clone(),
                    state.editor.clone(),
                    state.scroll.clone(),
                    state.navigator.clone(),
                    state.navigation.clone(),
                    state.restoration.clone(),
                )
            };

            let count_value = count.get();
            let current = navigator.current().expect("home route is always present");
            let route_depth = navigator.routes().len();
            let opener = cx
                .window_opener()
                .expect("application owns a window opener");
            let snapshot_path = restoration
                .path()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "custom/in-memory restoration store".into());
            let items = (1..=48)
                .map(|index| {
                    RawMaterialButton::new(format!("Persistent scroll item {index:02}"))
                        .color(Color::rgba(42, 70 + (index % 5) as u8 * 20, 116, 255))
                        .into()
                })
                .collect::<Vec<Widget>>();

            Padding::all(
                20.,
                Widget::column(vec![
                    Text::new("Incular state restoration")
                        .style(TextStyle {
                            size: 30.,
                            color: Color::rgba(219, 231, 255, 255),
                            ..TextStyle::default()
                        })
                        .into(),
                    Text::new(format!("Snapshot: {snapshot_path}"))
                        .color(Color::rgba(174, 196, 226, 255))
                        .into(),
                    Text::new(format!(
                        "Counter: {count_value}   •   route stack: {route_depth}"
                    ))
                    .into(),
                    Widget::row(vec![
                        RawMaterialButton::new("Increment persistent counter")
                            .on_press({
                                let count = count.clone();
                                move || count.update(|value| *value += 1)
                            })
                            .into(),
                        RawMaterialButton::new("Open restorable inspector")
                            .on_press({
                                let opener = opener.clone();
                                let count = count.clone();
                                move || {
                                    let _ = opener.open_restorable_window_with(
                                        WindowRestorationId::new("inspector")
                                            .expect("stable inspector window ID"),
                                        "inspector",
                                        inspector_options(),
                                        inspector_builder(count.clone()),
                                    );
                                }
                            })
                            .into(),
                        RawMaterialButton::new("Reset restoration for next launch")
                            .on_press({
                                let restoration = restoration.clone();
                                move || restoration.reset()
                            })
                            .into(),
                    ]),
                    TextField::new(editor)
                        .multiline(true)
                        .placeholder("Persistent text")
                        .size(Size::new(720., 92.))
                        .into(),
                    current.presented_child(),
                    Widget::row(vec![
                        RawMaterialButton::new("Push restorable details")
                            .on_press({
                                let registry = registry_for_build.clone();
                                let navigator = navigator.clone();
                                let navigation = navigation.clone();
                                let details = details_for_build.clone();
                                move || {
                                    if registry
                                        .navigate_restorable(
                                            &navigator,
                                            RestorableRoute::new(
                                                details.clone(),
                                                json!({ "source": "restoration example" }),
                                            ),
                                        )
                                        .is_ok()
                                    {
                                        let _ = navigation.set(navigator.restoration_snapshot());
                                    }
                                }
                            })
                            .into(),
                        RawMaterialButton::new("Pop route")
                            .on_press({
                                let navigator = navigator.clone();
                                let navigation = navigation.clone();
                                move || {
                                    if navigator.can_pop() {
                                        let _ = navigator.pop();
                                        let _ = navigation.set(navigator.restoration_snapshot());
                                    }
                                }
                            })
                            .into(),
                        RawMaterialButton::new("Flush snapshot")
                            .on_press(move || restoration.flush())
                            .into(),
                    ]),
                    SizedBox::from_size(Size::new(720., 220.))
                        .child(ScrollView::vertical(scroll, Widget::column(items)))
                        .into(),
                ]),
            )
            .into()
        },
    )
    .expect("valid restoration application");

    let inspector_count = state
        .borrow()
        .as_ref()
        .expect("main window mounts before restoration factories run")
        .count
        .clone();
    let inspector_factory = inspector_builder(inspector_count);
    app.register_restorable_window_factory("inspector", inspector_options(), move |cx| {
        inspector_factory(cx)
    })
    .expect("valid inspector factory");
    let restored = app
        .restore_restorable_windows()
        .expect("valid restored inspector descriptor");
    if let Some(path) = app.restoration_path() {
        eprintln!("Restoration snapshot: {}", path.display());
    }
    eprintln!("Restored {restored} auxiliary window(s) before first presentation.");

    example_support::spawn_if_requested(app.simulation(), simulations::run);
    incular::run(app).expect("native restoration application");
}
