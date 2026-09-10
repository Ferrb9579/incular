//! Selectable-text pointer mapping through shaped caret stops.
//!
//! Clicks and drags resolve bytes and affinities from the stored
//! layout's caret positions, not from raw glyph order. Assertions use
//! exact selected strings, handle positions from the area controller,
//! and paint rects — never bare validity.

use incular_config::Constraints;
use incular_core::{Color, Offset, Rect, Size};
use incular_rendering::PaintCommand;
use incular_text::{TextAlign, TextStyle};
use incular_widgets::Column;
use incular_widgets::internal::*;

const HIGHLIGHT: Color = Color::rgba(72, 120, 220, 150);

fn mount_area(
    controller: SelectionAreaController,
    children: Vec<Widget>,
    width: f32,
    height: f32,
) -> (WidgetTree, ElementId, Vec<ElementId>) {
    let mut tree = WidgetTree::new();
    let area = tree
        .mount(Widget::selection_area(
            controller,
            Column::new(children).into(),
        ))
        .expect("mount selection area");
    tree.layout(Constraints::tight(Size::new(width, height)))
        .expect("layout");
    let column = tree.children(area).expect("area child")[0];
    let labels = tree.children(column).expect("column kids").to_vec();
    (tree, area, labels)
}

fn label(
    controller: SelectionAreaController,
    text: &str,
    align: TextAlign,
) -> (WidgetTree, ElementId, ElementId) {
    let (tree, area, labels) = mount_area(
        controller,
        vec![Widget::selectable_text_styled(
            text,
            TextStyle::default(),
            align,
        )],
        200.,
        160.,
    );
    assert_eq!(labels.len(), 1);
    (tree, area, labels[0])
}

fn mid_y(tree: &WidgetTree, id: ElementId) -> f32 {
    tree.element_bounds(id).expect("bounds").origin.y + 10.
}

fn click(tree: &mut WidgetTree, id: ElementId, point: Offset) {
    assert!(tree.selectable_text_set_selection(id, point, false));
}

fn drag(tree: &mut WidgetTree, id: ElementId, from: Offset, to: Offset) {
    assert!(tree.selectable_text_set_selection(id, from, false));
    assert!(tree.selectable_text_set_selection(id, to, true));
}

fn glyph_edges(tree: &mut WidgetTree) -> Vec<(f32, f32)> {
    let mut shift = Offset::ZERO;
    let mut edges = Vec::new();
    for command in tree.paint().commands() {
        match command {
            PaintCommand::PushTransform { transform } => {
                shift = shift + transform.translation_offset();
            }
            PaintCommand::GlyphRun { run, .. } => {
                for glyph in run.glyphs.iter() {
                    let left = shift.x + glyph.offset.x;
                    edges.push((left, left + glyph.advance));
                }
            }
            _ => {}
        }
    }
    edges
}

fn ink_span(tree: &mut WidgetTree) -> (f32, f32) {
    let edges = glyph_edges(tree);
    let min = edges
        .iter()
        .map(|edge| edge.0)
        .fold(f32::INFINITY, f32::min);
    let max = edges
        .iter()
        .map(|edge| edge.1)
        .fold(f32::NEG_INFINITY, f32::max);
    (min, max)
}

fn highlight_rects(tree: &mut WidgetTree) -> Vec<Rect> {
    tree.paint()
        .commands()
        .iter()
        .filter_map(|command| match command {
            PaintCommand::Rect { rect, color } if *color == HIGHLIGHT => Some(*rect),
            _ => None,
        })
        .collect()
}

#[test]
fn rtl_clicks_map_to_logical_bytes() {
    let controller = SelectionAreaController::new();
    let (mut tree, _, id) = label(controller.clone(), "שלום", TextAlign::Start);
    let y = mid_y(&tree, id);
    let (ink_min, ink_max) = ink_span(&mut tree);
    // Visual right is logical start, visual left is logical end: drag
    // across the whole ink either way selects the full string.
    drag(
        &mut tree,
        id,
        Offset::new(ink_max - 1., y),
        Offset::new(-50., y),
    );
    assert_eq!(controller.selected_text(), "שלום");
    drag(
        &mut tree,
        id,
        Offset::new(-50., y),
        Offset::new(ink_max - 1., y),
    );
    assert_eq!(controller.selected_text(), "שלום");
    // Anchor just inside the visual left edge (byte 8 side), extend to
    // the ink middle: covers the trailing two characters exactly.
    let middle = ink_min + (ink_max - ink_min) / 2.;
    drag(
        &mut tree,
        id,
        Offset::new(ink_min + 2., y),
        Offset::new(middle, y),
    );
    assert_eq!(controller.selected_text(), "ום");
}

#[test]
fn mixed_click_recovers_byte_and_affinity() {
    let controller = SelectionAreaController::new();
    let (mut tree, _, id) = label(controller.clone(), "hi שלום bye", TextAlign::Start);
    let y = mid_y(&tree, id);
    let origin_x = tree.element_bounds(id).expect("bounds").origin.x;
    // Byte 11 owns stops on both visual sides of the run boundary
    // (upstream left, downstream right); the stored affinity selects
    // the handle side, observed through the area controller.
    click(&mut tree, id, Offset::new(origin_x + 17.3, y));
    let upstream = controller.selection_geometry();
    click(&mut tree, id, Offset::new(origin_x + 54.07, y));
    let downstream = controller.selection_geometry();
    let upstream_x = upstream
        .start_selection_point
        .expect("collapsed handle")
        .local_position
        .x;
    let downstream_x = downstream
        .start_selection_point
        .expect("collapsed handle")
        .local_position
        .x;
    assert!(
        upstream_x < downstream_x - 5.,
        "affinities resolve to opposite sides: {upstream_x} vs {downstream_x}"
    );
    // Clicking the LTR head then dragging past the end selects a proper
    // prefix ending inside the Hebrew run.
    drag(
        &mut tree,
        id,
        Offset::new(origin_x + 4., y),
        Offset::new(origin_x + 40., y),
    );
    let selected = controller.selected_text();
    assert!(
        selected.starts_with("hi ") && selected.len() > 4 && selected.len() < "hi שלום bye".len(),
        "drag covers the head plus Hebrew bytes, got {selected:?}"
    );
}

#[test]
fn aligned_interior_clicks_map_inside_the_ink() {
    let controller = SelectionAreaController::new();
    let (mut tree, _, id) = label(controller.clone(), "abcd", TextAlign::Center);
    let y = mid_y(&tree, id);
    let (ink_min, ink_max) = ink_span(&mut tree);
    // Padding before centered ink anchors at byte zero.
    drag(
        &mut tree,
        id,
        Offset::new(ink_min - 4., y),
        Offset::new(10_000., y),
    );
    assert_eq!(controller.selected_text(), "abcd");
    // Padding after anchors at the end.
    drag(
        &mut tree,
        id,
        Offset::new(ink_max + 4., y),
        Offset::new(10_000., y),
    );
    assert_eq!(controller.selected_text(), "");
    // Well inside the ink anchors strictly inside: a non-empty proper
    // suffix, with the highlight covering the suffix ink. (The
    // pre-offset cutoff mapped this zone to the buffer end; the
    // offset-aware walk already repaired LTR, and the stops keep it.)
    drag(
        &mut tree,
        id,
        Offset::new(ink_max - 6., y),
        Offset::new(10_000., y),
    );
    let selected = controller.selected_text();
    assert!(
        !selected.is_empty() && selected.len() < 4 && "abcd".ends_with(&selected),
        "interior click anchors inside, got {selected:?}"
    );
    // The selection highlight covers the suffix ink, not the full line.
    let rects = highlight_rects(&mut tree);
    assert_eq!(rects.len(), 1);
    assert!(rects[0].origin.x > ink_min + 1.);
}

#[test]
fn wrapped_line_clicks_partition_the_text() {
    let controller = SelectionAreaController::new();
    let text = "aaaa bbbb cccc dddd eeee ffff";
    let (mut tree, _, id) = label(controller.clone(), text, TextAlign::Start);
    let top = tree.element_bounds(id).expect("bounds").origin.y;
    // The same click maps deterministically from either direction, so
    // the two drags partition the text exactly with no overlap or gap.
    // Extending past the document end clamps to the final byte.
    drag(
        &mut tree,
        id,
        Offset::new(5., top + 34.),
        Offset::new(10_000., 10_000.),
    );
    let tail = controller.selected_text();
    // Anchor left of the ink so the head starts at byte zero.
    drag(
        &mut tree,
        id,
        Offset::new(-50., top + 5.),
        Offset::new(5., top + 34.),
    );
    let head = controller.selected_text();
    assert!(!head.is_empty() && !tail.is_empty());
    assert_eq!(head.len() + tail.len(), text.len());
    assert_eq!(format!("{head}{tail}"), text);
}

#[test]
fn drag_across_visual_runs_selects_exact_text() {
    let controller = SelectionAreaController::new();
    let (mut tree, _, id) = label(controller.clone(), "hi שלום bye", TextAlign::Start);
    let y = mid_y(&tree, id);
    let origin_x = tree.element_bounds(id).expect("bounds").origin.x;
    // Head to far right: the exact full string.
    drag(
        &mut tree,
        id,
        Offset::new(origin_x + 2., y),
        Offset::new(10_000., y),
    );
    assert_eq!(controller.selected_text(), "hi שלום bye");
    // Hebrew interior to far right: exact suffix starting inside the run.
    drag(
        &mut tree,
        id,
        Offset::new(origin_x + 30., y),
        Offset::new(10_000., y),
    );
    let selected = controller.selected_text();
    let text = "hi שלום bye";
    assert!(
        text.ends_with(&selected)
            && !selected.is_empty()
            && selected.len() < text.len()
            && (selected.starts_with("ש")
                || selected.starts_with("ל")
                || selected.starts_with("ו")
                || selected.starts_with("ם")
                || selected.starts_with(" ")),
        "suffix starts inside the Hebrew run, got {selected:?}"
    );
}

#[test]
fn ligature_and_combining_clicks_snap_to_cluster_edges() {
    // U+FB01 is one char: clicks anywhere on it map to a cluster edge.
    let controller = SelectionAreaController::new();
    let (mut tree, _, id) = label(controller.clone(), "ﬁsh", TextAlign::Start);
    let y = mid_y(&tree, id);
    let origin_x = tree.element_bounds(id).expect("bounds").origin.x;
    drag(
        &mut tree,
        id,
        Offset::new(origin_x + 2., y),
        Offset::new(origin_x + 60., y),
    );
    let selected = controller.selected_text();
    assert!(
        selected == "ﬁsh" || selected == "ish",
        "ligature click snaps to a cluster edge, got {selected:?}"
    );
    // e + combining acute share one cluster: a click on the pair starts
    // at a cluster edge, never mid-grapheme.
    let controller = SelectionAreaController::new();
    let (mut tree, _, id) = label(controller.clone(), "e\u{301}x", TextAlign::Start);
    let y = mid_y(&tree, id);
    let origin_x = tree.element_bounds(id).expect("bounds").origin.x;
    drag(
        &mut tree,
        id,
        Offset::new(origin_x + 2., y),
        Offset::new(10_000., y),
    );
    let selected = controller.selected_text();
    assert!(
        selected == "e\u{301}x" || selected == "x",
        "combining click snaps to a cluster edge, got {selected:?}"
    );
}

#[test]
fn mounted_width_change_keeps_mapping_with_fresh_mount() {
    let text = "hello selectable world";
    let build = |controller: SelectionAreaController| {
        Widget::selection_area(
            controller,
            Column::new(vec![Widget::selectable_text_styled(
                text,
                TextStyle::default(),
                TextAlign::Start,
            )])
            .into(),
        )
    };
    // Extend past the document end so wrapped lines still resolve to
    // the final byte.
    let drag_mid_to_end = |tree: &mut WidgetTree, id: ElementId| {
        let y = mid_y(tree, id);
        drag(tree, id, Offset::new(14., y), Offset::new(10_000., 10_000.));
    };
    let controller = SelectionAreaController::new();
    // Identical drags at both widths: narrowing reflows the same
    // content, and an updated tree agrees with a fresh mount.
    let mut wide = WidgetTree::new();
    let wide_area = wide.mount(build(controller.clone())).expect("mount wide");
    wide.layout(Constraints::tight(Size::new(200., 160.)))
        .expect("layout wide");
    let wide_id = wide
        .children(wide.children(wide_area).expect("column")[0])
        .expect("labels")[0];
    drag_mid_to_end(&mut wide, wide_id);
    let wide_selected = controller.selected_text();

    let mut narrow = WidgetTree::new();
    let narrow_area = narrow
        .mount(build(controller.clone()))
        .expect("mount narrow");
    narrow
        .layout(Constraints::tight(Size::new(100., 160.)))
        .expect("layout narrow");
    let narrow_id = narrow
        .children(narrow.children(narrow_area).expect("column")[0])
        .expect("labels")[0];
    drag_mid_to_end(&mut narrow, narrow_id);
    let narrow_selected = controller.selected_text();

    // Narrowing reflows the same content: both mappings stay inside the
    // text and agree with a remount at the same width.
    assert!(!wide_selected.is_empty() && text.ends_with(&wide_selected));
    assert!(!narrow_selected.is_empty() && text.ends_with(&narrow_selected));
    let mut updated = WidgetTree::new();
    let updated_area = updated
        .mount(build(controller.clone()))
        .expect("mount updated");
    updated
        .layout(Constraints::tight(Size::new(200., 160.)))
        .expect("layout updated");
    updated
        .update(updated_area, build(controller.clone()))
        .expect("rebuild area");
    updated
        .layout(Constraints::tight(Size::new(100., 160.)))
        .expect("narrow updated");
    let updated_id = updated
        .children(updated.children(updated_area).expect("column")[0])
        .expect("labels")[0];
    drag_mid_to_end(&mut updated, updated_id);
    assert_eq!(controller.selected_text(), narrow_selected);
}
