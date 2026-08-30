use incular_config::WidgetDefaults;
use incular_core::{ChangeImpact, Color, Lerp};
use incular_text::{FontFeature, FontVariation, FontWeight, TextScaler, TextStyle};

#[test]
fn scaler_is_deterministic_and_defensive() {
    assert_eq!(TextScaler::linear(1.5).scale(16.0), 24.0);
    assert_eq!(TextScaler::no_scaling().scale(16.0), 16.0);
    assert_eq!(TextScaler::linear(-1.0).scale(16.0), 16.0);
}

#[test]
fn text_style_uses_the_shared_widget_default_size() {
    assert_eq!(TextStyle::default().size, WidgetDefaults::DEFAULT.text_size);
}

#[test]
fn style_lerp_interpolates_metrics_and_color() {
    let first = TextStyle::default().font_size(10.0).color(Color::BLACK);
    let second = TextStyle::default().font_size(20.0).color(Color::WHITE);
    let middle = first.lerp(&second, 0.5);
    assert_eq!(middle.size, 15.0);
    assert_eq!(middle.color, Color::rgba(128, 128, 128, 255));
}

#[test]
fn change_impact_distinguishes_layout_from_paint() {
    let base = TextStyle::default().font_size(16.0).color(Color::WHITE);
    let paint_change = base.clone().color(Color::BLACK);
    assert_eq!(base.change_impact(&paint_change), ChangeImpact::Paint);
    let layout_change = base.clone().font_size(18.0);
    assert_eq!(base.change_impact(&layout_change), ChangeImpact::Layout);
}

#[test]
fn font_weight_maps_to_text_stack() {
    let style = TextStyle::default().font_weight(FontWeight::W700);
    assert_eq!(style.weight, FontWeight::W700);
    assert_eq!(style.weight.value(), 700);
}

#[test]
fn font_feature_tag_round_trip() {
    let feature = FontFeature::new(*b"liga", 0);
    assert_eq!(feature.tag, *b"liga");
    assert_eq!(feature.value, 0);
    assert_eq!(feature.to_css_setting(), "\"liga\" 0");
}

#[test]
fn font_variation_axis_round_trip() {
    let variation = FontVariation::new(*b"wght", 650.);
    assert_eq!(variation.axis, *b"wght");
    assert_eq!(variation.value, 650.);
    assert_eq!(variation.to_css_setting(), "\"wght\" 650.0");
}
