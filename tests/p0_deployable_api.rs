//! Public-facade smoke coverage for the Task 24 P0 deployable core.
//!
//! The test deliberately stays on the base Widgets/runtime/domain surface;
//! Material and controls are not needed to construct any of these scenarios.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;

use incular::prelude::*;

#[derive(Default)]
struct TestRestorationBackend(RefCell<BTreeMap<Vec<RestorationKey>, serde_json::Value>>);

impl incular::core::RestorationBackend for TestRestorationBackend {
    fn read_value(&self, path: &[RestorationKey]) -> Option<serde_json::Value> {
        self.0.borrow().get(path).cloned()
    }

    fn write_value(&self, path: &[RestorationKey], value: serde_json::Value) {
        self.0.borrow_mut().insert(path.to_vec(), value);
    }

    fn remove_value(&self, path: &[RestorationKey]) {
        self.0.borrow_mut().remove(path);
    }
}

#[test]
fn typed_commands_keyboard_and_focus_are_rust_native() {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    enum CommandName {
        Save,
    }

    let saves = Rc::new(Cell::new(0));
    let mut actions = Actions::<CommandName>::typed();
    let observed = saves.clone();
    actions.register(CommandName::Save, move || observed.set(observed.get() + 1));

    let mut shortcuts = Shortcuts::<CommandName>::typed();
    shortcuts.bind(
        ShortcutKey::new(Code::KeyS, Modifiers::CONTROL),
        CommandName::Save,
    );
    let mut event = KeyboardEvent::key_down(KeyboardKey::Named(NamedKey::Unidentified), Code::KeyS);
    event.modifiers = Modifiers::CONTROL;
    assert!(shortcuts.handle_actions(event, &actions));
    assert_eq!(saves.get(), 1);

    let first = FocusNode::new();
    let second = FocusNode::new();
    let scope = FocusScopeNode::new();
    scope.register(&first);
    scope.register(&second);
    assert!(scope.request_focus(&first));
    assert_eq!(scope.focus_next(false), Some(second.clone()));
    scope.clear_focus();
    assert!(scope.restore_focus());

    let key_events = Rc::new(Cell::new(0));
    let listener = KeyboardListener::new(SizedBox::shrink())
        .focus_node(first)
        .autofocus(true)
        .include_semantics(false)
        .on_key_down({
            let key_events = key_events.clone();
            move |_| key_events.set(key_events.get() + 1)
        });
    assert!(listener.is_autofocus());
    assert!(!listener.includes_semantics());
    assert!(listener.handle(KeyboardEvent::key_down(
        KeyboardKey::Named(NamedKey::Unidentified),
        Code::Enter,
    )));
    assert_eq!(key_events.get(), 1);
}

#[test]
fn composable_scroll_physics_and_paging_preserve_core_semantics() {
    let physics = ScrollPhysics::clamping()
        .bouncing()
        .always_scrollable()
        .range_maintaining();
    let overscroll = physics.apply_delta(0., -40., 0., 100.);
    assert!(overscroll.overscroll < 0.);

    let never = ScrollPhysics::clamping().never_scrollable();
    let blocked = never.apply_delta(10., 20., 0., 100.);
    assert_eq!(blocked.position, 10.);
    assert_eq!(blocked.unconsumed, 20.);

    let page = ScrollController::new();
    page.update_extents_with_physics(1_000., 250., ScrollPhysics::clamping().page());
    page.jump_to(270.);
    assert!(page.settle_physics(ScrollPhysics::clamping().page(), 0.));
    assert_eq!(page.offset(), 250.);
    page.adjust_for_content_change(40., physics);
    assert_eq!(page.offset(), 290.);
}

#[test]
fn domain_values_image_paint_and_icon_share_authoritative_crates() {
    let icon = IconData::new('A' as u32)
        .font_family("System UI")
        .match_text_direction(true);
    assert_eq!(icon.glyph(), Some('A'));
    assert!(icon.match_text_direction_value());

    let configuration = ImageConfiguration::new()
        .size(Size::new(32., 32.))
        .device_pixel_ratio(2.)
        .text_direction(TextDirection::Ltr)
        .locale("en-US".parse().expect("locale"))
        .normalized();
    assert_eq!(configuration.device_pixel_ratio, 2.);

    let paint = Paint::new()
        .shader(Shader::Solid(Color::WHITE))
        .stroke()
        .stroke_width(2.);
    assert_eq!(paint.style_value(), PaintStyle::Stroke);
    assert_eq!(FilterQuality::None.sampling(), ImageSampling::Nearest);
    assert_eq!(Shadow::new(Color::BLACK, Offset::ZERO, 4.).blur_radius, 4.);
}

#[test]
fn forms_restoration_undo_environment_and_routes_are_deployable() {
    let form = FormController::new();
    let form_changes = Rc::new(Cell::new(0));
    let form_changes_observer = form_changes.clone();
    let form_listener = form.add_listener(move || {
        form_changes_observer.set(form_changes_observer.get() + 1);
    });
    let editor = TextEditingController::with_text("initial");
    let field = form
        .register(editor.clone())
        .validator(|value| value.is_empty().then_some("required".to_owned()));
    editor.clear();
    assert!(!form.validate());
    assert_eq!(field.error().as_deref(), Some("required"));
    assert!(form_changes.get() > 0);
    editor.set_text("saved");
    assert!(form.save());
    form.reset();
    assert_eq!(editor.text(), "initial");
    assert!(form.remove_listener(form_listener));

    let undo = UndoHistoryController::new(editor.clone());
    editor.set_text("next");
    assert!(undo.can_undo());
    assert!(undo.undo());
    assert_eq!(editor.text(), "initial");
    assert!(undo.can_redo());

    let restoration = RestorationScope::root(Rc::new(TestRestorationBackend::default()));
    let child = restoration.child(RestorationKey::new("settings").expect("key"));
    assert!(!child.is_root());
    let environment = RuntimeEnvironment {
        viewport: Size::new(900., 600.),
        ..RuntimeEnvironment::default()
    };
    let _environment_widget: Widget = MediaQuery::new(environment, Text::new("settings")).into();

    let route = Route::new("settings", Text::new("Settings"))
        .settings(
            RouteSettings::new("settings")
                .restoration_scope(RouteScopeKey::new("settings").expect("scope key")),
        )
        .modal(ModalBarrier::default());
    assert!(route.presentation.blocks_background_input());
    let popup = Route::new("menu", Text::new("Menu")).popup(None);
    assert!(!popup.presentation.is_opaque());
    let built = PageRouteBuilder::new("home", Text::new("Home"))
        .transition(RouteTransition::None)
        .build();
    assert_eq!(built.settings.name(), "home");
    let result = RouteResult::<bool>::new();
    assert!(result.complete(true).is_ok());
    assert_eq!(result.take(), Some(true));
}

#[test]
fn base_widgets_cover_page_pinned_header_rotation_and_plain_composition() {
    let physics = ScrollPhysics::clamping().page();
    let scroll = CustomScrollView::new(vec![
        Box::new(PinnedHeaderSliver::new(Text::new("Pinned"))) as Box<dyn Sliver>,
        Box::new(SliverToBoxAdapter::new(Text::new("content"))) as Box<dyn Sliver>,
    ])
    .physics(physics);
    let pages =
        PageView::builder(3, 240., |index| Text::new(format!("page {index}"))).reverse(true);
    let rotated: Widget = RotationTransition::from_turns(0.25, Text::new("rotate"))
        .alignment(Alignment::TOP_LEFT)
        .into();
    let _root: Widget = Column::new([Widget::from(scroll), Widget::from(pages), rotated]).into();
}

#[test]
fn route_and_overlay_lifecycle_stress_stays_bounded() {
    let navigator = Navigator::new();
    for iteration in 0..1_000 {
        navigator.push(Route::new(format!("page-{iteration}"), Text::new("page")));
        assert!(navigator.pop().is_some());

        navigator.push(Route::new("modal", Text::new("modal")).modal(ModalBarrier::default()));
        assert!(navigator.pop().is_some());

        navigator.push(Route::new("popup", Text::new("popup")).popup(None));
        assert!(navigator.pop().is_some());
        assert!(navigator.routes().is_empty());
    }

    let overlay = Overlay::new();
    for _ in 0..1_000 {
        overlay.insert(OverlayEntry::new(Text::new("popup")));
        overlay.insert(OverlayEntry::new(Text::new("modal")).modal(ModalBarrier::default()));
        assert!(overlay.dismiss_top().is_some());
        assert!(overlay.remove_top().is_some());
        assert!(overlay.entries().is_empty());
        assert!(!overlay.blocks_background_input());
    }
}
