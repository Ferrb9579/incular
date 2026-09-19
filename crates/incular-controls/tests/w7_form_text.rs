use incular_controls::{ControlTheme, TextField, TextFieldStyle, form::Form};
use incular_core::Color;
use incular_widgets::internal::TextEditingController;
use std::time::Instant;
use std::{cell::Cell, rc::Rc};

#[test]
fn form_submit_reaches_registered_validation_and_save_once() {
    let submitted = Rc::new(Cell::new(0_u32));
    let submitted_out = submitted.clone();
    let form = Form::new().on_submit(move || submitted_out.set(submitted_out.get() + 1));
    let controller = TextEditingController::with_text("Ada");
    let saved = Rc::new(Cell::new(0_u32));
    let saved_out = saved.clone();
    let field = form
        .register(controller.clone())
        .validator(|value| value.trim().is_empty().then(|| "Required".to_owned()))
        .on_saved(move |value| {
            assert_eq!(value, "Grace");
            saved_out.set(saved_out.get() + 1);
        });

    controller.set_text("Grace");
    assert!(form.submit());
    assert_eq!(saved.get(), 1);
    assert_eq!(submitted.get(), 1);

    controller.set_text("");
    assert!(!form.submit());
    assert_eq!(saved.get(), 1);
    assert_eq!(submitted.get(), 1);
    drop(field);
}

#[test]
fn disabled_form_rejects_submit_without_user_callbacks() {
    let calls = Rc::new(Cell::new(0_u32));
    let output = calls.clone();
    let form = Form::new()
        .disabled(true)
        .on_submit(move || output.set(output.get() + 1));
    let field = form.register(TextEditingController::with_text("Ada"));

    assert!(!form.submit());
    assert_eq!(calls.get(), 0);
    drop(field);
}

#[test]
fn text_field_style_placeholder_color_reaches_editable_text() {
    let placeholder = Color::rgba(12, 34, 56, 255);
    let field = TextField::new(TextEditingController::new())
        .placeholder("Hint")
        .style(TextFieldStyle {
            placeholder_color: Some(placeholder),
            ..TextFieldStyle::default()
        });

    let mut tree = incular_widgets::internal::WidgetTree::new();
    tree.mount(field.build(&ControlTheme::light()))
        .expect("mount text field");
    tree.layout(incular_config::Constraints::loose(incular_core::Size::new(
        240.0, 80.0,
    )))
    .expect("layout text field");
    let list = tree.paint();
    assert!(list.commands().iter().any(|command| matches!(
        command,
        incular_rendering::PaintCommand::GlyphRun { color, .. } if *color == placeholder
    )));
}

#[test]
fn focused_border_uses_the_editors_existing_focus_owner() {
    let focused = incular_widgets::Border::new(3.0, Color::rgba(220, 30, 40, 255));
    let field = TextField::new(TextEditingController::with_text("Ada")).style(TextFieldStyle {
        border_focused: Some(focused),
        ..TextFieldStyle::default()
    });
    let mut tree = incular_widgets::internal::WidgetTree::new();
    tree.mount(field.build(&ControlTheme::light()))
        .expect("mount text field");
    tree.layout(incular_config::Constraints::loose(incular_core::Size::new(
        240.0, 80.0,
    )))
    .expect("layout text field");
    let editor = tree
        .text_field_at(incular_core::Offset::new(20.0, 20.0))
        .expect("editor hit");
    tree.set_focused(editor, true, Instant::now())
        .expect("focus editor");
    let list = tree.paint();
    assert!(list.commands().iter().any(|command| matches!(
        command,
        incular_rendering::PaintCommand::Border { border, .. }
            if border.width == 3.0 && border.color == Color::rgba(220, 30, 40, 255)
    )));
}
