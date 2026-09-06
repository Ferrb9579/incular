use incular_text::{TextEditingValue, TextRange, TextSelection};
use incular_widgets::{
    LengthLimitingTextInputFormatter as Limit, MaxLengthEnforcement as Mode, TextInputFormatter,
};

fn format(mode: Mode, limit: usize, value: &TextEditingValue) -> TextEditingValue {
    Limit::with_enforcement(limit, mode).format_edit_update(&TextEditingValue::empty(), value)
}

#[test]
fn limits_extended_graphemes_instead_of_scalars() {
    for cluster in ["e\u{301}", "👩‍👩‍👧‍👦", "🇮🇳", "👍🏽"] {
        let input = TextEditingValue::new(format!("{cluster}x"));
        let output = format(Mode::Enforced, 1, &input);
        assert_eq!(output.text, cluster);
        assert_eq!(output.selection, TextSelection::collapsed(cluster.len()));
    }
}

#[test]
fn enforcement_modes_preserve_or_defer_composition() {
    let input = TextEditingValue::new("abcd").with_composing(Some(TextRange::new(1, 4)));
    assert_eq!(format(Mode::None, 1, &input), input);
    assert_eq!(format(Mode::TruncateAfterCompositionEnds, 1, &input), input);
    let enforced = format(Mode::Enforced, 2, &input);
    assert_eq!(enforced.text, "ab");
    assert_eq!(enforced.composing, Some(TextRange::new(1, 2)));
    let committed = input.with_composing(None);
    assert_eq!(
        format(Mode::TruncateAfterCompositionEnds, 2, &committed).text,
        "ab"
    );
}

#[test]
fn truncation_preserves_new_directional_selection_and_clamps_ranges() {
    let input = TextEditingValue::new("éabc")
        .with_selection(TextSelection::new(5, 2))
        .with_composing(Some(TextRange::new(3, 5)));
    let result = format(Mode::Enforced, 2, &input);
    assert_eq!(result.text, "éa");
    assert_eq!(result.selection, TextSelection::new(3, 2));
    assert_eq!(result.composing, None);
}

#[test]
fn zero_limit_and_invalid_ranges_are_normalized() {
    let input = TextEditingValue {
        text: "éx".into(),
        selection: TextSelection::new(99, 1),
        composing: Some(TextRange::new(9, 2)),
    };
    let zero = format(Mode::Enforced, 0, &input);
    assert_eq!(zero, TextEditingValue::empty());
    let none = format(Mode::None, 0, &input);
    assert_eq!(none.text, input.text);
    assert_eq!(none.selection, TextSelection::new(3, 0));
    assert_eq!(none.composing, None);
}
