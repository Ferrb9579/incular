//! Compile/use coverage for public form, autocomplete, focus, and action APIs.

use incular::prelude::*;

#[test]
fn facade_exposes_form_autocomplete_focus_and_typed_actions() {
    let form = Form::new();
    let editor = TextEditingController::with_text("Ada");
    let field = form
        .register(editor.clone())
        .validator(|text| (!text.contains('@')).then_some("email".into()))
        .autovalidate(AutovalidateMode::OnUserInteraction);
    field.set_text("ada@example.test");
    assert!(form.validate());

    let mut autocomplete = Autocomplete::strings(["Ada", "Grace"]);
    autocomplete.set_query("gr");
    assert_eq!(autocomplete.matching_indices(), [1]);

    let first = FocusNode::new();
    let second = FocusNode::new();
    let mut focus = FocusManager::new();
    focus.register(&first);
    focus.register(&second);
    assert!(focus.request_focus(&first));
    assert_eq!(focus.focus_next(false), Some(second));

    let mut actions = Actions::new();
    actions.register(Command::new("save"), || {});
    let mut shortcuts = Shortcuts::new();
    shortcuts.bind(
        incular::gestures::ShortcutKey::new(Code::KeyA, Modifiers::default()),
        "save",
    );
    assert!(shortcuts.handle_actions(
        KeyboardEvent::key_down(KeyboardKey::Named(NamedKey::Unidentified), Code::KeyA),
        &actions,
    ));
}
