use incular_config::{Axis, Constraints};
use incular_core::{Color, Size};
use incular_rendering::PaintCommand;
use incular_text::{LineHeight, TextStyle};
use incular_widgets::{
    internal::{ActionSurface, SplitView, WidgetTree},
    *,
};

#[test]
fn container_empty_expands_when_bounded_shrinks_when_unbounded_and_paints_nothing() {
    // 1. Bounded parent: Container expands to fill available constraints
    let mut tree = WidgetTree::new();
    let root = tree.mount(Container::new().into()).unwrap();
    tree.layout(Constraints::loose(Size::new(500.0, 300.0)));
    let render_id = tree.render_id(root).unwrap();
    let size = tree.render_size(render_id).unwrap();
    assert_eq!(
        size,
        Size::new(500.0, 300.0),
        "Empty Container must expand to fill bounded constraints"
    );

    let display_list = tree.paint();
    let has_visual_draw = display_list.commands().iter().any(|cmd| {
        matches!(
            cmd,
            PaintCommand::Rect { .. }
                | PaintCommand::RRect { .. }
                | PaintCommand::GlyphRun { .. }
                | PaintCommand::Image { .. }
        )
    });
    assert!(
        !has_visual_draw,
        "Container::new() must produce 0 visual draw commands: {:?}",
        display_list.commands()
    );

    // 2. Unbounded parent: Container shrinks to zero
    let mut tree_unbounded = WidgetTree::new();
    let root_unbounded = tree_unbounded.mount(Container::new().into()).unwrap();
    tree_unbounded.layout(Constraints::unbounded());
    let render_id_unbounded = tree_unbounded.render_id(root_unbounded).unwrap();
    let size_unbounded = tree_unbounded.render_size(render_id_unbounded).unwrap();
    assert_eq!(
        size_unbounded,
        Size::ZERO,
        "Empty Container must shrink to zero in unbounded constraints"
    );
}

#[test]
fn button_with_child_sizes_strictly_to_child_and_is_unpainted() {
    let mut tree = WidgetTree::new();
    let child = SizedBox::new().width(50.0).height(25.0);
    let button = ActionSurface::with_child(child);
    let root = tree.mount(button.into()).unwrap();

    tree.layout(Constraints::loose(Size::new(500.0, 500.0)));
    let render_id = tree.render_id(root).unwrap();
    let size = tree.render_size(render_id).unwrap();
    assert_eq!(size, Size::new(50.0, 25.0));

    let display_list = tree.paint();
    let has_rrect = display_list
        .commands()
        .iter()
        .any(|cmd| matches!(cmd, PaintCommand::RRect { .. }));
    assert!(
        !has_rrect,
        "Unstyled Button must not paint background RRect"
    );
}

#[test]
fn button_with_color_paints_background_rrect() {
    let mut tree = WidgetTree::new();
    let child = SizedBox::new().width(60.0).height(30.0);
    let button = ActionSurface::with_child(child).color(Color::rgba(100, 150, 200, 255));
    let _root = tree.mount(button.into()).unwrap();

    tree.layout(Constraints::loose(Size::new(500.0, 500.0)));
    let display_list = tree.paint();
    let has_rrect = display_list
        .commands()
        .iter()
        .any(|cmd| matches!(cmd, PaintCommand::RRect { .. }));
    assert!(
        has_rrect,
        "Styled Button with explicit color must paint background RRect"
    );
}

#[test]
fn split_view_first_and_second_extents_are_unambiguous() {
    let mut tree_first = WidgetTree::new();
    let split_first = SplitView::horizontal(
        SizedBox::new().child(Container::new()),
        SizedBox::new().child(Container::new()),
    )
    .first_extent(160.0);
    let root_first = tree_first.mount(split_first.into()).unwrap();
    tree_first.layout(Constraints::tight(Size::new(800.0, 600.0)));
    let render_id = tree_first.render_id(root_first).unwrap();
    let total_size = tree_first.render_size(render_id).unwrap();
    assert_eq!(total_size, Size::new(800.0, 600.0));

    let mut tree_second = WidgetTree::new();
    let split_second = SplitView::horizontal(
        SizedBox::new().child(Container::new()),
        SizedBox::new().child(Container::new()),
    )
    .second_extent(240.0);
    let root_second = tree_second.mount(split_second.into()).unwrap();
    tree_second.layout(Constraints::tight(Size::new(800.0, 600.0)));
    let render_id_second = tree_second.render_id(root_second).unwrap();
    let total_size_second = tree_second.render_size(render_id_second).unwrap();
    assert_eq!(total_size_second, Size::new(800.0, 600.0));
}

#[test]
fn text_style_line_height_multiplier_and_inheritance() {
    let style = TextStyle::new().font_size(14.0).line_height(Some(1.5));
    assert_eq!(style.line_height, Some(LineHeight::Multiplier(1.5)));

    let base = TextStyle::new()
        .font_size(12.0)
        .color(Color::rgba(200, 100, 50, 255));
    let child_style = TextStyle::new().font_size(16.0);
    let merged = base.merge(&child_style);

    assert_eq!(merged.size, 16.0);
    assert_eq!(
        merged.color,
        Color::rgba(200, 100, 50, 255),
        "Child style must not overwrite ambient color when color was unspecified"
    );
}

#[test]
fn scrollable_widget_defaults_match_flutter_axes() {
    assert_eq!(
        SingleChildScrollView::new(SizedBox::new()).get_scroll_direction(),
        Axis::Vertical
    );
    assert_eq!(
        ListView::new(Vec::<Widget>::new()).get_scroll_direction(),
        Axis::Vertical
    );
    assert_eq!(
        ListView::builder(10, |_| SizedBox::new()).get_scroll_direction(),
        Axis::Vertical
    );
    assert_eq!(
        ListView::separated(10, |_| SizedBox::new(), |_| SizedBox::new()).get_scroll_direction(),
        Axis::Vertical
    );
    assert_eq!(
        PageView::new(Vec::<Widget>::new()).get_scroll_direction(),
        Axis::Horizontal
    );
    assert_eq!(
        PageView::builder(3, |_| SizedBox::new()).get_scroll_direction(),
        Axis::Horizontal
    );
}
