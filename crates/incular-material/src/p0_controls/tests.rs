use super::*;
use incular_widgets::{Text, Widget};

#[test]
fn controlled_material_controls_lower_to_widgets() {
    let _: Widget = Checkbox::new(false).tristate(true).value(None).into();
    let _: Widget = Radio::new("a").group_value(Some("b")).into();
    let _: Widget = Switch::new(true).enabled(false).into();
    let _: Widget = Slider::new(0.5).range(0.0, 10.0).divisions(Some(10)).into();
    let _: Widget = RangeSlider::new(RangeValues::new(0.2, 0.8)).into();
}

#[test]
fn tabs_share_a_retained_controller_and_page_view() {
    let controller = TabController::new(2);
    controller.animate_to(1);
    assert_eq!(controller.index(), 1);
    let _: Widget = TabBar::new([Tab::text("One"), Tab::text("Two")])
        .controller(controller.clone())
        .into();
    let _: Widget = TabBarView::new([Text::new("One"), Text::new("Two")])
        .controller(controller)
        .into();
}

#[test]
fn typed_builders_keep_material_defaults_and_accept_widgets() {
    let checkbox = Checkbox::builder().build();
    assert_eq!(checkbox.value, None);
    assert!(checkbox.enabled);

    let switch = Switch::builder().value(true).build();
    assert!(switch.value);
    let _: Widget = switch.into();

    let radio = Radio::builder()
        .value("option")
        .group_value("option")
        .build();
    let _: Widget = radio.into();

    let range_values = RangeValues::builder().start(0.8_f32).end(0.2_f32).build();
    let range = RangeSlider::builder()
        .values(range_values)
        .minimum_separation(-1.0)
        .build();
    assert_eq!(range.values, RangeValues::new(0.2, 0.8));
    assert_eq!(range.minimum_separation, Some(0.0));

    let slider_theme = SliderThemeData::builder()
        .track_height(-1.0)
        .thumb_size(0.0)
        .build();
    assert_eq!(slider_theme.track_height, Some(0.0));
    assert_eq!(slider_theme.thumb_size, Some(1.0));

    let tab = Tab::builder().child(Text::new("First")).build();
    let _: Widget = TabBar::builder().tabs([tab]).build().into();
    let _: Widget = TabBarView::builder()
        .children([Text::new("First"), Text::new("Second")])
        .build()
        .into();
}
