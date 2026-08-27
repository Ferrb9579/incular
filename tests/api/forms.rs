use incular::prelude::*;
use incular::widgets::internal::{
    FilteringTextInputFormatter, GenericFormField, LengthLimitingTextInputFormatter,
    TextEditingController as RetainedTextEditingController,
    TextEditingValue as RetainedTextEditingValue, TextInputFormatter,
    TextSelection as RetainedTextSelection,
};
use std::cell::Cell;
use std::rc::Rc;

#[test]
fn test_forms_and_input_formatters_contract() {
    let form = Form::new();
    let controller = RetainedTextEditingController::with_text("123");
    let field = form.register(controller.clone()).validator(|text| {
        if text.len() < 3 {
            Some("Length must be at least 3".into())
        } else {
            None
        }
    });

    assert!(field.validate());
    assert!(form.validate());

    let digits = FilteringTextInputFormatter::digits_only();
    let old_val = RetainedTextEditingValue {
        text: "123".into(),
        selection: RetainedTextSelection::collapsed(3),
        composing: None,
    };
    let new_val = RetainedTextEditingValue {
        text: "123abc45".into(),
        selection: RetainedTextSelection::collapsed(8),
        composing: None,
    };
    let filtered = digits.format_edit_update(&old_val, &new_val);
    assert_eq!(filtered.text, "12345");

    let limiter = LengthLimitingTextInputFormatter::new(4);
    let limited = limiter.format_edit_update(&old_val, &filtered);
    assert_eq!(limited.text, "1234");

    let saved_val = Rc::new(Cell::new(0));
    let saved_clone = saved_val.clone();

    let generic_field: Widget = GenericFormField::new(|val: Option<&i32>| {
        let text = format!("Current val: {:?}", val);
        Text::new(text)
    })
    .initial_value(42)
    .on_saved(move |val| saved_clone.set(val))
    .into();

    let _ = generic_field;
}
