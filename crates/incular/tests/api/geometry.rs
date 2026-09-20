use incular::prelude::*;

#[test]
fn test_geometry_and_insets_compile_contract() {
    let insets = EdgeInsets::all(16.0);
    assert_eq!(insets.left, 16.0);
    assert_eq!(insets.top, 16.0);
    assert_eq!(insets.right, 16.0);
    assert_eq!(insets.bottom, 16.0);

    let symmetric = EdgeInsets::symmetric(24.0, 12.0);
    assert_eq!(symmetric.left, 24.0);
    assert_eq!(symmetric.top, 12.0);

    let only = EdgeInsets::only(1.0, 2.0, 3.0, 4.0);
    assert_eq!(only.left, 1.0);
    assert_eq!(only.top, 2.0);
    assert_eq!(only.right, 3.0);
    assert_eq!(only.bottom, 4.0);

    let directional = EdgeInsetsDirectional::from_ste_b(10.0, 5.0, 20.0, 15.0);
    let resolved_ltr = directional.resolve(TextDirection::Ltr);
    assert_eq!(resolved_ltr.left, 10.0);
    assert_eq!(resolved_ltr.right, 20.0);

    let resolved_rtl = directional.resolve(TextDirection::Rtl);
    assert_eq!(resolved_rtl.left, 20.0);
    assert_eq!(resolved_rtl.right, 10.0);

    let alignment = Alignment::TOP_CENTER;
    assert_eq!(alignment.x, 0.0);
    assert_eq!(alignment.y, -1.0);

    let dir_align = AlignmentDirectional::CENTER_START;
    assert_eq!(
        dir_align.resolve(TextDirection::Ltr),
        Alignment::CENTER_LEFT
    );
    assert_eq!(
        dir_align.resolve(TextDirection::Rtl),
        Alignment::CENTER_RIGHT
    );

    let frac = FractionalOffset::new(0.0, 1.0);
    assert_eq!(frac.to_alignment(), Alignment::BOTTOM_LEFT);
}
