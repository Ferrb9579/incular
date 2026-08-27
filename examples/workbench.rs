//! Small, base-Widgets reference application for Task 23.
//!
//! This intentionally avoids Controls and Material. It exercises the pieces
//! an ordinary desktop application needs together: Signals, a Tokio task,
//! retained editing, semantics, navigation, a virtualized list, painting,
//! image loading, and compositor-safe transition widgets.

use std::{
    cell::Cell,
    rc::Rc,
    time::{Duration, Instant},
};

use incular::prelude::*;

const EMBEDDED_PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 2, 0, 0, 0, 2, 8, 6, 0,
    0, 0, 114, 182, 13, 36, 0, 0, 0, 24, 73, 68, 65, 84, 120, 156, 5, 193, 129, 1, 0, 0, 4, 192,
    160, 248, 220, 229, 83, 34, 105, 71, 226, 30, 63, 110, 6, 127, 180, 47, 0, 167, 0, 0, 0, 0, 73,
    69, 78, 68, 174, 66, 96, 130,
];

fn text(value: impl Into<String>, size: f32, color: Color) -> Widget {
    Text::new(value)
        .style(TextStyle::new().font_size(size).color(color))
        .into()
}

fn card(child: impl Into<Widget>) -> Widget {
    DecoratedBox::new(Padding::all(14., child))
        .background(Color::rgba(30, 39, 58, 255))
        .radius(12.)
        .into()
}

fn action(label: &str, callback: impl Fn() + 'static) -> Widget {
    GestureDetector::new(
        DecoratedBox::new(Padding::symmetric(
            14.,
            8.,
            Text::new(label).color(Color::WHITE),
        ))
        .background(Color::rgba(56, 112, 205, 255))
        .radius(8.),
    )
    .on_tap(callback)
    .into()
}

#[derive(Clone, Copy)]
struct WorkbenchCatalog;

impl LocalizationCatalog for WorkbenchCatalog {
    fn supported_locales(&self) -> &[Locale] {
        // The workbench intentionally keeps this catalog tiny; it is enough
        // to prove typed locale replacement and ICU4X fallback without
        // pulling a localization framework into the base example.
        static LOCALES: std::sync::OnceLock<Vec<Locale>> = std::sync::OnceLock::new();
        LOCALES
            .get_or_init(|| {
                vec![
                    "en".parse().expect("valid locale"),
                    "fr".parse().expect("valid locale"),
                ]
            })
            .as_slice()
    }

    fn message(&self, locale: &Locale, key: &str) -> Option<&str> {
        match (locale.to_string().as_str(), key) {
            (value, "greeting") if value.starts_with("fr") => Some("Bonjour"),
            (_, "greeting") => Some("Hello"),
            _ => None,
        }
    }
}

fn main() {
    let count = Signal::new(0_u32);
    let async_state = Signal::new(AsyncState::<String, String>::Idle);
    let started = Rc::new(Cell::new(false));
    let navigator = Navigator::new();
    navigator.push(Route::new("home", text("Home route", 22., Color::WHITE)));
    let navigation_revision = Signal::new(0_u64);
    let save_count = Signal::new(0_u32);
    let locale = Signal::new("en".parse::<Locale>().expect("valid locale"));
    let form = FormController::new();
    let form_editor = TextEditingController::with_text("required field");
    let form_field = form
        .register(form_editor.clone())
        .validator(|value| value.trim().is_empty().then_some("Required".to_owned()));
    let undo = UndoHistoryController::new(form_editor.clone());
    let focus_scope = FocusScopeNode::new();
    let first_focus = FocusNode::new();
    let second_focus = FocusNode::new();
    focus_scope.register(&first_focus);
    focus_scope.register(&second_focus);
    let mut actions = Actions::<Command>::typed();
    let mut shortcuts = Shortcuts::<Command>::typed();
    let save_action = save_count.clone();
    actions.register(Command::new("save"), move || {
        save_action.update(|count| *count += 1);
    });
    shortcuts.bind(
        ShortcutKey::new(Code::KeyS, Modifiers::CONTROL),
        Command::new("save"),
    );
    let actions = Rc::new(actions);
    let shortcuts = Rc::new(shortcuts);

    let image = ImageHandle::embedded(EMBEDDED_PNG).expect("embedded reference image");
    let editor = TextEditingController::with_text("Type here — IME and grapheme-safe editing");
    let app_count = count.clone();
    let app_async = async_state.clone();
    let app_started = started.clone();
    let app_navigator = navigator.clone();
    let app_image = image.clone();
    let app_editor = editor.clone();
    let app_navigation_revision = navigation_revision.clone();
    let app_save_count = save_count.clone();
    let app_locale = locale.clone();
    let app_form = form.clone();
    let app_form_editor = form_editor.clone();
    let app_form_field = form_field;
    let app_undo = undo.clone();
    let app_focus_scope = focus_scope.clone();
    let app_first_focus = first_focus.clone();
    let app_second_focus = second_focus.clone();
    let app_actions = actions.clone();
    let app_shortcuts = shortcuts.clone();
    let app = Application::new(move |cx| {
        let viewport = cx.viewport();
        let scale_factor = cx.scale_factor();
        let brightness = cx.brightness();
        if !app_started.replace(true) {
            app_async.set(AsyncState::Loading);
            if let Some(scope) = cx.restoration_scope()
                && let Ok(key) = RestorationKey::new("workbench-editor")
            {
                app_editor.bind_restoration(scope, key);
            }
            let result = app_async.clone();
            cx.spawn_into(
                async {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    "Tokio task completed".to_owned()
                },
                move |value, _| {
                    match value {
                        Ok(value) => result.set(AsyncState::Ready(value)),
                        Err(error) => result.set(AsyncState::Error(format!("{error:?}"))),
                    };
                },
            );
        }

        let value = app_count.get();
        let loading = match app_async.get() {
            AsyncState::Idle => "idle".to_owned(),
            AsyncState::Loading => "loading…".to_owned(),
            AsyncState::Ready(value) => value,
            AsyncState::Error(error) => format!("error: {error}"),
        };
        let locale_value = app_locale.get();
        let localized = Localizations::new(
            locale_value.clone(),
            WorkbenchCatalog,
            text("localized content", 13., Color::WHITE),
        );
        let nav_revision = app_navigation_revision.get();
        let save_count_value = app_save_count.get();
        let form_valid = app_form.validate();
        let form_error = app_form_field.error().unwrap_or_else(|| "none".to_owned());
        let focus_status = if app_focus_scope.focused() == Some(app_first_focus.clone()) {
            "first"
        } else if app_focus_scope.focused() == Some(app_second_focus.clone()) {
            "second"
        } else {
            "none"
        };
        let icon = IconData::new('✓' as u32).font_family("System UI");
        let increment = app_count.clone();
        let route = app_navigator.current().expect("home route");

        let home = Widget::column(vec![
            text("Incular Workbench", 28., Color::rgba(220, 232, 255, 255)),
            text(
                "A deployable base-Widgets smoke test — no Material or controls.",
                14.,
                Color::rgba(170, 185, 210, 255),
            ),
            card(Widget::column(vec![
                text(format!("Signal counter: {value}"), 18., Color::WHITE),
                action("Increment", move || increment.update(|value| *value += 1)),
                text(format!("Async state: {loading}"), 14., Color::WHITE),
            ])),
            card(Widget::column(vec![
                text("Typed commands and focus", 18., Color::WHITE),
                text(
                    format!("Ctrl+S saves: {save_count_value} · focus: {focus_status}"),
                    14.,
                    Color::WHITE,
                ),
                KeyboardListener::new(Text::new("KeyboardListener (focusable)"))
                    .focus_node(app_first_focus.clone())
                    .autofocus(true)
                    .with_shortcuts(app_shortcuts.clone(), app_actions.clone())
                    .into(),
                Widget::row(vec![
                    action("Focus first", {
                        let scope = app_focus_scope.clone();
                        let node = app_first_focus.clone();
                        move || {
                            let _ = scope.request_focus(&node);
                        }
                    }),
                    action("Focus next", {
                        let scope = app_focus_scope.clone();
                        move || {
                            let _ = scope.focus_next(false);
                        }
                    }),
                    action("Simulate Ctrl+S", {
                        let actions = app_actions.clone();
                        let shortcuts = app_shortcuts.clone();
                        move || {
                            let mut event = KeyboardEvent::key_down(
                                KeyboardKey::Named(NamedKey::Unidentified),
                                Code::KeyS,
                            );
                            event.modifiers = Modifiers::CONTROL;
                            let _ = shortcuts.handle_actions(event, &actions);
                        }
                    }),
                ]),
            ])),
            card(Widget::column(vec![
                text("Retained editing", 18., Color::WHITE),
                EditableText::new(app_editor.clone())
                    .size(Size::new(620., 42.))
                    .placeholder("Text")
                    .into(),
                Widget::row(vec![
                    action("Undo", {
                        let undo = app_undo.clone();
                        move || {
                            let _ = undo.undo();
                        }
                    }),
                    action("Redo", {
                        let undo = app_undo.clone();
                        move || {
                            let _ = undo.redo();
                        }
                    }),
                    text(
                        format!("undo={} redo={}", app_undo.can_undo(), app_undo.can_redo()),
                        13.,
                        Color::WHITE,
                    ),
                ]),
            ])),
            card(Widget::column(vec![
                text("Form validate / reset", 18., Color::WHITE),
                EditableText::new(app_form_editor.clone())
                    .size(Size::new(620., 42.))
                    .placeholder("Required")
                    .into(),
                text(
                    format!("valid={form_valid} error={form_error}"),
                    13.,
                    Color::WHITE,
                ),
                Widget::row(vec![
                    action("Validate", {
                        let form = app_form.clone();
                        move || {
                            let _ = form.validate();
                        }
                    }),
                    action("Reset", {
                        let form = app_form.clone();
                        move || {
                            form.reset();
                        }
                    }),
                ]),
            ])),
            card(Widget::column(vec![
                text("Virtualized list (100,000 rows)", 18., Color::WHITE),
                SizedBox::new()
                    .width(620.)
                    .height(220.)
                    .child(ListView::builder(100_000, |index| {
                        Text::new(format!("row {index:06}"))
                    }))
                    .into(),
            ])),
            card(Widget::row(vec![
                Image::new(app_image.clone()).width(96.).height(96.).into(),
                text(
                    format!("IconData glyph: {}", icon.glyph_text().unwrap_or_default()),
                    18.,
                    Color::WHITE,
                ),
                Semantics::new(text("Accessible image", 14., Color::WHITE))
                    .label("Reference image")
                    .into(),
                action("Push route", {
                    let nav = app_navigator.clone();
                    let revision = app_navigation_revision.clone();
                    move || {
                        nav.push(
                            Route::new("details", text("Details route", 22., Color::WHITE))
                                .transition(RouteTransition::fade_in(
                                    Duration::from_millis(220),
                                    Instant::now(),
                                )),
                        );
                        revision.update(|value| *value = value.saturating_add(1));
                    }
                }),
                action("Modal route", {
                    let nav = app_navigator.clone();
                    let revision = app_navigation_revision.clone();
                    move || {
                        nav.push(
                            Route::new("modal", text("Neutral modal route", 22., Color::WHITE))
                                .dialog(),
                        );
                        revision.update(|value| *value = value.saturating_add(1));
                    }
                }),
                action("Popup route", {
                    let nav = app_navigator.clone();
                    let revision = app_navigation_revision.clone();
                    move || {
                        nav.push(
                            Route::new("popup", text("Neutral popup route", 22., Color::WHITE))
                                .popup(None),
                        );
                        revision.update(|value| *value = value.saturating_add(1));
                    }
                }),
                action("Pop route", {
                    let nav = app_navigator.clone();
                    let revision = app_navigation_revision.clone();
                    move || {
                        let _ = nav.pop();
                        revision.update(|value| *value = value.saturating_add(1));
                    }
                }),
            ])),
            card(Widget::column(vec![
                text("PageView and pinned header", 18., Color::WHITE),
                CustomScrollView::new(vec![
                    Box::new(PinnedHeaderSliver::new(Text::new("Pinned header")))
                        as Box<dyn Sliver>,
                    Box::new(SliverList::builder(40, 28., |index| {
                        Text::new(format!("sliver row {index}"))
                    })) as Box<dyn Sliver>,
                ])
                .physics(ScrollPhysics::clamping().bouncing().always_scrollable())
                .into(),
                PageView::builder(3, 220., |index| {
                    DecoratedBox::new(Padding::all(16., Text::new(format!("Page {index}"))))
                        .background(Color::rgba(45, 67, 100, 255))
                })
                .into(),
                RotationTransition::from_turns(0.25, Text::new("RotationTransition (compositor)"))
                    .alignment(Alignment::CENTER)
                    .into(),
            ])),
            card(Widget::column(vec![
                text("Typed environment and locale", 18., Color::WHITE),
                text(
                    format!(
                        "locale={} · viewport={}×{} · scale={scale_factor:.2} · brightness={brightness:?} · nav revision={nav_revision}",
                        locale_value,
                        viewport.width,
                        viewport.height,
                    ),
                    14.,
                    Color::WHITE,
                ),
                localized.into(),
                Widget::row(vec![
                    action("English", {
                        let locale = app_locale.clone();
                        move || {
                            locale.set("en".parse().expect("locale"));
                        }
                    }),
                    action("French", {
                        let locale = app_locale.clone();
                        move || {
                            locale.set("fr".parse().expect("locale"));
                        }
                    }),
                ]),
            ])),
            Opacity::new(
                0.92,
                text("Animated compositor property", 14., Color::WHITE),
            )
            .into(),
        ]);

        SafeArea::new(Widget::column(vec![
            text(format!("active route: {}", route.name), 13., Color::WHITE),
            route.presented_child(),
            home,
        ]))
        .into()
    })
    .expect("valid workbench application");

    incular::run(app).expect("native workbench application");
}
