use incular::prelude::*;

#[test]
fn test_text_and_style_compile_contract() {
    let style = TextStyle::new()
        .font_size(16.0)
        .font_weight(FontWeight::W500)
        .font_family("Inter")
        .letter_spacing(0.5)
        .line_height(Some(1.4))
        .color(Color::rgba(20, 20, 20, 255))
        .font_feature_settings(&[
            FontFeature::tabular_figures(),
            FontFeature::contextual_alternates(),
        ])
        .font_variation_settings(&[
            FontVariation::weight(500.0),
            FontVariation::optical_size(16.0),
        ])
        .decoration(TextDecoration::UNDERLINE | TextDecoration::LINE_THROUGH)
        .decoration_color(Color::rgba(255, 0, 0, 255))
        .decoration_style(TextDecorationStyle::Wavy)
        .overflow(TextOverflow::Ellipsis);

    assert_eq!(style.size, 16.0);
    assert_eq!(style.weight, FontWeight::W500);
    assert_eq!(style.color, Color::rgba(20, 20, 20, 255));
    assert!(style.foreground.is_none());

    // Exclusivity: setting foreground clears color
    let fg_style = style.clone().foreground(Color::rgba(0, 128, 255, 255));
    assert_eq!(fg_style.foreground, Some(Color::rgba(0, 128, 255, 255)));

    // Exclusivity: setting background_color clears background
    let bg_style = style
        .clone()
        .background_color(Color::rgba(240, 240, 240, 255));
    assert_eq!(
        bg_style.background_color,
        Some(Color::rgba(240, 240, 240, 255))
    );
    assert!(bg_style.background.is_none());

    let text_widget: Widget = Text::new("Hello Incular")
        .style(style.clone())
        .max_lines(Some(2))
        .align(TextAlign::Center)
        .overflow(TextOverflow::Ellipsis)
        .into();

    let _ = text_widget;

    let rich_text: Widget =
        RichText::new(TextSpan::new("Root ").style(style).child(
            TextSpan::new("Child bold").style(TextStyle::new().font_weight(FontWeight::BOLD)),
        ))
        .into();

    let _ = rich_text;
}
