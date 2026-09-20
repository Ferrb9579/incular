use incular::prelude::*;
use incular::scroll::{BoundaryPhysics, Scrollability};

#[test]
fn test_scrolling_widgets_and_physics_contract() {
    let physics = ScrollPhysics::default()
        .bouncing()
        .then(ScrollPhysics::default().always_scrollable());

    assert!(matches!(physics.boundary, BoundaryPhysics::Bouncing { .. }));
    assert_eq!(physics.scrollability, Scrollability::Always);

    let list_view: Widget = ListView::builder(500, |index| Text::new(format!("Item #{index}")))
        .scroll_direction(Axis::Vertical)
        .physics(physics)
        .cache_extent(150.0)
        .shrink_wrap(false)
        .into();

    let grid_view: Widget =
        GridView::count(3, (0..12).map(|i| Text::new(format!("Grid cell {i}"))))
            .padding(EdgeInsets::all(8.0))
            .into();

    let page_view: Widget =
        PageView::builder(5, |page| Center::new(Text::new(format!("Page {page}")))).into();

    let single_child: Widget = SingleChildScrollView::new(Column::new([
        Text::new("Heading"),
        Text::new("Body content paragraph"),
    ]))
    .padding(EdgeInsets::all(16.0))
    .into();

    let _ = (list_view, grid_view, page_view, single_child);
}
