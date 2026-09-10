//! Selection-area endpoint lifetime and affinity identity.
//!
//! Logical identity is the (element, byte) location; visual affinity
//! selects handle edges without creating or suppressing content.
//! Stale endpoints from removal or replacement resolve to nothing
//! through generational IDs, and recovery needs no extra registry.

use incular_config::Constraints;
use incular_core::{Color, Offset, Size};
use incular_rendering::PaintCommand;
use incular_text::{TextAlign, TextStyle};
use incular_widgets::internal::*;
use incular_widgets::{Column, SelectionAreaController, Visibility};

const HIGHLIGHT: Color = Color::rgba(72, 120, 220, 150);

fn mount_area(
    controller: SelectionAreaController,
    children: Vec<Widget>,
) -> (WidgetTree, ElementId, Vec<ElementId>) {
    let mut tree = WidgetTree::new();
    let area = tree
        .mount(Widget::selection_area(
            controller,
            Column::new(children).into(),
        ))
        .expect("mount selection area");
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let column = tree.children(area).expect("area child")[0];
    let labels = tree.children(column).expect("column kids").to_vec();
    (tree, area, labels)
}

fn text_label(text: &str) -> Widget {
    Widget::selectable_text_styled(text, TextStyle::default(), TextAlign::Start)
}

fn mid_y(tree: &WidgetTree, id: ElementId) -> f32 {
    tree.element_bounds(id).expect("bounds").origin.y + 10.
}

fn drag(tree: &mut WidgetTree, id: ElementId, from: Offset, to: Offset) {
    assert!(tree.selectable_text_set_selection(id, from, false));
    assert!(tree.selectable_text_set_selection(id, to, true));
}

fn drag_across(
    tree: &mut WidgetTree,
    down_id: ElementId,
    from: Offset,
    up_id: ElementId,
    to: Offset,
) {
    assert!(tree.selectable_text_set_selection(down_id, from, false));
    assert!(tree.selectable_text_set_selection(up_id, to, true));
}

#[test]
fn shared_byte_keeps_affinity_without_creating_content() {
    // Byte 3 of "hi שלום bye" owns stops on both visual sides of its
    // run boundary (upstream left, downstream right). Dragging between
    // those two points stays on one byte: affinity differs, content
    // stays empty, and the collapsed handle follows the anchor side.
    let controller = SelectionAreaController::new();
    let (mut tree, _, labels) = mount_area(controller.clone(), vec![text_label("hi שלום bye")]);
    assert_eq!(labels.len(), 1);
    let id = labels[0];
    let y = mid_y(&tree, id);
    let origin_x = tree.element_bounds(id).expect("bounds").origin.x;
    drag(
        &mut tree,
        id,
        Offset::new(origin_x + 17.3, y),
        Offset::new(origin_x + 54.07, y),
    );
    assert_eq!(controller.selected_text(), "");
    let forward = controller.selection_geometry();
    let forward_x = forward
        .start_selection_point
        .expect("collapsed handle")
        .local_position
        .x;
    assert!(forward.end_selection_point.is_none());

    drag(
        &mut tree,
        id,
        Offset::new(origin_x + 54.07, y),
        Offset::new(origin_x + 17.3, y),
    );
    assert_eq!(controller.selected_text(), "");
    let backward_x = controller
        .selection_geometry()
        .start_selection_point
        .expect("collapsed handle")
        .local_position
        .x;
    assert!(
        forward_x < backward_x - 5.,
        "same byte, preserved affinities: {forward_x} vs {backward_x}"
    );
}

#[test]
fn cross_element_drags_normalize_copy_order() {
    let controller = SelectionAreaController::new();
    let (mut tree, _, labels) = mount_area(
        controller.clone(),
        vec![text_label("alpha"), text_label("beta")],
    );
    assert_eq!(labels.len(), 2);
    let top = tree.element_bounds(labels[0]).expect("bounds").origin.y;
    let bottom = tree.element_bounds(labels[1]).expect("bounds").origin.y;
    // Forward and backward drags produce identical document-ordered
    // output while handles track their own endpoints.
    drag_across(
        &mut tree,
        labels[0],
        Offset::new(2., top + 8.),
        labels[1],
        Offset::new(10_000., bottom + 8.),
    );
    assert_eq!(controller.selected_text(), "alpha\nbeta");
    let forward_geometry = controller.selection_geometry();
    drag_across(
        &mut tree,
        labels[1],
        Offset::new(10_000., bottom + 8.),
        labels[0],
        Offset::new(2., top + 8.),
    );
    assert_eq!(controller.selected_text(), "alpha\nbeta");
    let backward_geometry = controller.selection_geometry();
    assert_eq!(
        forward_geometry.start_selection_point, backward_geometry.start_selection_point,
        "handles follow document order, not drag direction"
    );
}

#[test]
fn keyed_reorder_keeps_selection_on_elements() {
    let controller = SelectionAreaController::new();
    let build = |first: Widget, second: Widget| {
        Widget::selection_area(controller.clone(), Column::new(vec![first, second]).into())
    };
    let mut tree = WidgetTree::new();
    let area = tree
        .mount(build(
            text_label("alpha").with_key(1u64),
            text_label("beta").with_key(2u64),
        ))
        .expect("mount");
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let before: Vec<ElementId> = tree
        .children(tree.children(area).expect("column")[0])
        .expect("labels")
        .to_vec();
    assert_eq!(before.len(), 2);
    drag_across(
        &mut tree,
        before[0],
        Offset::new(2., 8.),
        before[1],
        Offset::new(10_000., 60.),
    );
    assert_eq!(controller.selected_text(), "alpha\nbeta");

    // Reordering by key permutes element identities instead of
    // replacing them. Endpoints stick to their elements while coverage
    // follows document order, so the same (alpha-start, beta-end)
    // endpoints span an empty backward range here; swapping back
    // restores the full copy, proving the endpoints survived.
    tree.update(
        area,
        build(
            text_label("beta").with_key(2u64),
            text_label("alpha").with_key(1u64),
        ),
    )
    .expect("reorder");
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let after: Vec<ElementId> = tree
        .children(tree.children(area).expect("column")[0])
        .expect("labels")
        .to_vec();
    assert_eq!(after, vec![before[1], before[0]]);
    assert_eq!(controller.selected_text(), "");
    tree.update(
        area,
        build(
            text_label("alpha").with_key(1u64),
            text_label("beta").with_key(2u64),
        ),
    )
    .expect("reorder back");
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    assert_eq!(controller.selected_text(), "alpha\nbeta");
}

#[test]
fn removed_endpoint_degrades_without_panic_and_recovers() {
    let controller = SelectionAreaController::new();
    let build = |children: Vec<Widget>| {
        Widget::selection_area(controller.clone(), Column::new(children).into())
    };
    let mut tree = WidgetTree::new();
    let area = tree
        .mount(build(vec![text_label("alpha"), text_label("beta")]))
        .expect("mount");
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let labels: Vec<ElementId> = tree
        .children(tree.children(area).expect("column")[0])
        .expect("labels")
        .to_vec();
    let top = tree.element_bounds(labels[0]).expect("bounds").origin.y;
    drag_across(
        &mut tree,
        labels[0],
        Offset::new(2., top + 8.),
        labels[1],
        Offset::new(10_000., top + 40.),
    );
    assert_eq!(controller.selected_text(), "alpha\nbeta");

    // Dropping the extent element leaves a stale endpoint behind. The
    // generational ID resolves to nothing, so tree queries degrade to
    // empty instead of panicking, with no extra registry involved.
    tree.update(area, build(vec![text_label("alpha")]))
        .expect("remove extent element");
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    assert_eq!(controller.selected_text(), "");
    let highlights = tree
        .paint()
        .commands()
        .iter()
        .filter(
            |command| matches!(command, PaintCommand::Rect { color, .. } if *color == HIGHLIGHT),
        )
        .count();
    assert_eq!(highlights, 0, "unresolvable selection paints nothing");
    // A fresh click recovers immediately on the surviving content.
    let labels: Vec<ElementId> = tree
        .children(tree.children(area).expect("column")[0])
        .expect("labels")
        .to_vec();
    assert_eq!(labels.len(), 1);
    drag(
        &mut tree,
        labels[0],
        Offset::new(2., top + 8.),
        Offset::new(10_000., top + 8.),
    );
    assert_eq!(controller.selected_text(), "alpha");
}

#[test]
fn shortened_text_clamps_the_selection() {
    let controller = SelectionAreaController::new();
    let build = |text: &str| {
        Widget::selection_area(
            controller.clone(),
            Column::new(vec![text_label(text)]).into(),
        )
    };
    let mut tree = WidgetTree::new();
    let area = tree.mount(build("hello world")).expect("mount");
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    assert!(
        tree.selectable_text_select_all(
            tree.children(tree.children(area).expect("column")[0])
                .expect("labels")[0]
        )
    );
    assert_eq!(controller.selected_text(), "hello world");

    // Replacing the text shortens the selected element in place: the
    // stored endpoint clamps to the new length instead of slicing out
    // of bounds.
    tree.update(area, build("hi")).expect("shorten text");
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    assert_eq!(controller.selected_text(), "hi");
}

#[test]
fn hidden_and_nested_content_follow_ownership() {
    let controller = SelectionAreaController::new();
    let (mut tree, area, labels) = mount_area(
        controller.clone(),
        vec![
            text_label("shown"),
            Visibility::new(text_label("concealed"))
                .visible(false)
                .maintain_state(true)
                .into(),
            Widget::selection_area(
                SelectionAreaController::new(),
                Column::new(vec![text_label("nested")]).into(),
            ),
        ],
    );
    assert_eq!(labels.len(), 3);
    // Offstage-but-maintained content still copies; nested boundaries
    // own their leaves and stay out of the ancestor selection.
    assert!(tree.selectable_text_select_all(labels[0]));
    assert_eq!(controller.selected_text(), "shown\nconcealed");
    // Plain hiding without maintain flags replaces the child at
    // conversion, so it never enters selection either.
    let _ = area;
}
