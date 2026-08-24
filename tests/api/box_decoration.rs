use incular::prelude::*;

#[test]
fn test_box_decoration_composition_and_validity() {
    let side = BorderSide::new(Color::rgba(100, 100, 100, 255), 1.5, BorderStyle::Solid);
    let border = BoxBorder::all(side);
    let radius = BorderRadius::circular(12.0);

    // Flutter permits BOTH color and gradient in BoxDecoration (painted according to composition semantics)
    let dec_with_both = BoxDecoration::new()
        .color(Color::rgba(255, 255, 255, 255))
        .border(border)
        .border_radius(radius)
        .box_shadow(vec![BoxShadow::new(
            Color::rgba(0, 0, 0, 40),
            Offset::new(0.0, 4.0),
            8.0,
            0.0,
        )]);

    assert!(dec_with_both.is_valid());

    // Circle shape with border radius is invalid in Flutter and Incular
    let invalid_circle = BoxDecoration::new()
        .shape(BoxShape::Circle)
        .border_radius(radius);
    assert!(!invalid_circle.is_valid());

    // Circle shape without border radius is valid
    let valid_circle = BoxDecoration::new()
        .shape(BoxShape::Circle)
        .color(Color::rgba(0, 120, 240, 255));
    assert!(valid_circle.is_valid());

    // DecoratedBox widget wrapping a child
    let decorated_box: Widget = DecoratedBox::new(SizedBox::shrink())
        .decoration(dec_with_both)
        .into();

    let _ = decorated_box;
}
