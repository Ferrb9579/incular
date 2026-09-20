use incular::prelude::*;

#[test]
fn test_invalidation_bitflags_and_properties_contract() {
    let base = TextStyle::new()
        .font_size(14.0)
        .color(Color::rgba(0, 0, 0, 255))
        .font_family("Inter");

    // Changing color causes only Paint damage
    let paint_only = base.clone().color(Color::rgba(255, 0, 0, 255));
    assert_eq!(base.invalidation(&paint_only), Invalidation::PAINT);
    assert_eq!(base.change_impact(&paint_only), ChangeImpact::Paint);

    // Changing font size causes Layout and Paint damage
    let layout_and_paint = base.clone().font_size(18.0);
    assert_eq!(
        base.invalidation(&layout_and_paint),
        Invalidation::LAYOUT | Invalidation::PAINT
    );
    assert_eq!(base.change_impact(&layout_and_paint), ChangeImpact::Layout);

    // Invalidation bitflag operations
    let inv = Invalidation::LAYOUT | Invalidation::SEMANTICS;
    assert!(inv.contains(Invalidation::LAYOUT));
    assert!(inv.contains(Invalidation::SEMANTICS));
    assert!(!inv.contains(Invalidation::PAINT));
    assert!(inv.intersects(Invalidation::LAYOUT | Invalidation::PAINT));

    let composite_hit = Invalidation::COMPOSITE | Invalidation::HIT_TEST;
    assert!(composite_hit.contains(Invalidation::COMPOSITE));
    assert!(composite_hit.contains(Invalidation::HIT_TEST));
    assert!(!composite_hit.contains(Invalidation::BUILD));
}
