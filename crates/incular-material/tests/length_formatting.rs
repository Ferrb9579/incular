use incular_material::TextField;
use incular_text::{TextEditingController, TextEditingValue, TextRange};
use incular_widgets::{LengthLimitingTextInputFormatter, MaxLengthEnforcement, Widget};

#[test]
fn material_max_length_uses_grapheme_safe_formatter() {
    let controller = TextEditingController::new();
    let _widget: Widget = TextField::new(controller.clone())
        .max_length(Some(1))
        .into();
    controller.set_text("👩‍👩‍👧‍👦x");
    assert_eq!(controller.text(), "👩‍👩‍👧‍👦");
    assert_eq!(controller.value().selection.extent, controller.text().len());
}

#[test]
fn material_formatter_defers_until_controller_composition_commits() {
    let controller = TextEditingController::new();
    let _widget: Widget = TextField::new(controller.clone())
        .input_formatter(LengthLimitingTextInputFormatter::with_enforcement(
            1,
            MaxLengthEnforcement::TruncateAfterCompositionEnds,
        ))
        .into();
    controller.set_value(TextEditingValue::new("éx").with_composing(Some(TextRange::new(0, 3))));
    assert_eq!(controller.text(), "éx");
    controller.set_value(controller.value().with_composing(None));
    assert_eq!(controller.text(), "é");
    assert!(controller.value().composing.is_none());
    assert_eq!(controller.value().selection.extent, 2);
}
