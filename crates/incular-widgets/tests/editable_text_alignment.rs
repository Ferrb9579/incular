//! EditableText alignment contracts across entry points.
//!
//! Covers left/center/right and direction-sensitive start/end over empty,
//! short, overflowing, and multiline text, mounted width/alignment
//! changes, and the shared caret/selection/pointer mapping built on the
//! stored line offset. Glyph x positions, caret edges, and pointer
//! mapping must all observe the alignment translation exactly once.

use incular_config::Constraints;
use incular_core::{Color, Offset, Rect, Size};
use incular_rendering::PaintCommand;
use incular_text::{TextAlign, TextEditingController};
use incular_widgets::internal::*;
use std::time::Instant;

fn mount_field(
    controller: TextEditingController,
    build: impl FnOnce(EditableText) -> EditableText,
) -> (WidgetTree, ElementId) {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Widget::from(build(EditableText::new(controller))))
        .expect("mount field");
    (tree, root)
}

fn layout_tight(tree: &mut WidgetTree, width: f32, height: f32) {
    tree.layout(Constraints::tight(Size::new(width, height)))
        .expect("layout");
}

/// Left/right ink edges per glyph in field coordinates, following the
/// same transform accumulation paint itself applies.
fn glyph_edges(list: &incular_rendering::DisplayList) -> Vec<(f32, f32)> {
    let mut shift = Offset::ZERO;
    let mut edges = Vec::new();
    for command in list.commands() {
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

fn ink_bounds(edges: &[(f32, f32)]) -> (f32, f32) {
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

fn selection_rects(list: &incular_rendering::DisplayList, color: Color) -> Vec<Rect> {
    list.commands()
        .iter()
        .filter_map(|command| match command {
            PaintCommand::Rect { rect, color: paint } if *paint == color => Some(*rect),
            _ => None,
        })
        .collect()
}

fn field_edges(controller: TextEditingController, width: f32, align: TextAlign) -> Vec<(f32, f32)> {
    let (mut tree, _) = mount_field(controller, |field| {
        field.text_align(align).size(Size::new(width, 32.))
    });
    layout_tight(&mut tree, width, 32.);
    glyph_edges(&tree.paint())
}

#[test]
fn alignment_modes_space_short_single_line() {
    let width = 200.;
    let avail = width - 16.;
    let edges = field_edges(
        TextEditingController::with_text("hi"),
        width,
        TextAlign::Start,
    );
    let (start_min, start_max) = ink_bounds(&edges);
    let advance = start_max - start_min;
    assert!(
        (start_min - 8.).abs() < 1.0,
        "start aligns to the text area edge"
    );

    let centered = field_edges(
        TextEditingController::with_text("hi"),
        width,
        TextAlign::Center,
    );
    let (center_min, center_max) = ink_bounds(&centered);
    let expected = 8. + (avail - advance) / 2.;
    assert!(
        (center_min - expected).abs() < 1.5,
        "center splits the leftover space, got {center_min} want {expected}"
    );
    assert!(
        (center_max - (expected + advance)).abs() < 1.5,
        "centered advance is preserved, no double translation"
    );

    let end = field_edges(
        TextEditingController::with_text("hi"),
        width,
        TextAlign::End,
    );
    let (end_min, end_max) = ink_bounds(&end);
    assert!((end_min - (8. + avail - advance)).abs() < 1.5);
    assert!((end_max - (8. + avail)).abs() < 1.5);

    let justified = field_edges(
        TextEditingController::with_text("hi"),
        width,
        TextAlign::Justify,
    );
    let (justify_min, _) = ink_bounds(&justified);
    assert!(
        (justify_min - start_min).abs() < 1.5,
        "a single unjustifiable line behaves as start"
    );
}

#[test]
fn alignment_start_end_follow_text_direction() {
    let width = 200.;
    let avail = width - 16.;
    // Left-to-right: start is left, end is right.
    let ltr_start = field_edges(
        TextEditingController::with_text("hi"),
        width,
        TextAlign::Start,
    );
    let ltr_end = field_edges(
        TextEditingController::with_text("hi"),
        width,
        TextAlign::End,
    );
    let (ltr_start_min, _) = ink_bounds(&ltr_start);
    let (ltr_end_min, _) = ink_bounds(&ltr_end);
    assert!((ltr_start_min - 8.).abs() < 1.0);
    assert!(ltr_end_min > ltr_start_min + 5.);

    // Right-to-left: start is right, end is left. Bounds use ink edges
    // rather than storage order so visual direction cannot confuse them.
    let rtl = "שלום";
    let rtl_start = field_edges(
        TextEditingController::with_text(rtl),
        width,
        TextAlign::Start,
    );
    let rtl_end = field_edges(TextEditingController::with_text(rtl), width, TextAlign::End);
    let (rtl_start_min, rtl_start_max) = ink_bounds(&rtl_start);
    let (rtl_end_min, _) = ink_bounds(&rtl_end);
    let rtl_advance = rtl_start_max - rtl_start_min;
    assert!(
        (rtl_start_min - (8. + avail - rtl_advance)).abs() < 2.5,
        "rtl start aligns right, got {rtl_start_min}"
    );
    assert!(
        (rtl_end_min - 8.).abs() < 2.5,
        "rtl end aligns left, got {rtl_end_min}"
    );
}

#[test]
fn alignment_empty_text_and_placeholder() {
    let cursor = Color::rgba(255, 0, 0, 255);
    // Empty with no placeholder: the empty line still centers, so the
    // caret sits mid-area where the first glyph would be.
    let controller = TextEditingController::new();
    let (mut tree, root) = mount_field(controller.clone(), |field| {
        field
            .text_align(TextAlign::Center)
            .cursor_color(cursor)
            .size(Size::new(200., 32.))
    });
    layout_tight(&mut tree, 200., 32.);
    tree.set_focused(root, true, Instant::now())
        .expect("focus field");
    controller.reset_caret(Instant::now());
    let carets = selection_rects(&tree.paint(), cursor);
    assert_eq!(carets.len(), 1);
    assert!(
        (carets[0].origin.x - (8. + (200. - 16.) / 2.)).abs() < 1.5,
        "empty centered caret sits mid-area, got {}",
        carets[0].origin.x
    );

    // A centered placeholder shapes through the same aligned path, and the
    // caret for the empty value sits at the placeholder line start.
    let controller = TextEditingController::new();
    let (mut tree, root) = mount_field(controller.clone(), |field| {
        field
            .text_align(TextAlign::Center)
            .placeholder("type")
            .cursor_color(cursor)
            .size(Size::new(200., 32.))
    });
    layout_tight(&mut tree, 200., 32.);
    tree.set_focused(root, true, Instant::now())
        .expect("focus field");
    controller.reset_caret(Instant::now());
    let painted = tree.paint();
    let (placeholder_min, _) = ink_bounds(&glyph_edges(&painted));
    assert!(
        placeholder_min > 8. + 5.,
        "centered placeholder must shift right, got {placeholder_min}"
    );
    let carets = selection_rects(&painted, cursor);
    assert_eq!(carets.len(), 1);
    assert!(
        (carets[0].origin.x - placeholder_min).abs() < 1.5,
        "empty caret sits at the aligned placeholder start"
    );
}

#[test]
fn alignment_translation_applies_exactly_once_on_overflow() {
    // Overflowing centered text: the caret at the buffer end (positions
    // path) must agree with the last ink edge (glyph path). Any double
    // translation separates them by the alignment offset.
    let text: String = "0123456789".repeat(8);
    let len = text.len();
    let cursor = Color::rgba(255, 0, 0, 255);
    let controller = TextEditingController::with_text(text);
    controller.set_selection(incular_widgets::internal::TextSelection::collapsed(len));
    let (mut tree, root) = mount_field(controller.clone(), |field| {
        field
            .text_align(TextAlign::Center)
            .cursor_color(cursor)
            .size(Size::new(200., 32.))
    });
    layout_tight(&mut tree, 200., 32.);
    tree.set_focused(root, true, Instant::now())
        .expect("focus field");
    controller.reset_caret(Instant::now());
    let painted = tree.paint();
    let (_, ink_max) = ink_bounds(&glyph_edges(&painted));
    let carets = selection_rects(&painted, cursor);
    assert_eq!(carets.len(), 1);
    assert!(
        (carets[0].origin.x - ink_max).abs() < 1.5,
        "end caret must meet the last ink edge, caret {} ink {ink_max}",
        carets[0].origin.x
    );
}

#[test]
fn alignment_multiline_offsets_follow_line_lengths() {
    let cursor = Color::rgba(255, 0, 0, 255);
    let controller = TextEditingController::with_text("a\nlonger line here");
    let (mut tree, root) = mount_field(controller.clone(), |field| {
        field
            .text_align(TextAlign::Center)
            .multiline(true)
            .cursor_color(cursor)
            .size(Size::new(200., 64.))
    });
    layout_tight(&mut tree, 200., 64.);
    tree.set_focused(root, true, Instant::now())
        .expect("focus field");
    // Selection edits consume revisions in the layout preamble, so each
    // caret read follows a layout like a real frame would.
    controller.set_selection(incular_widgets::internal::TextSelection::collapsed(0));
    controller.reset_caret(Instant::now());
    layout_tight(&mut tree, 200., 64.);
    let first_line = selection_rects(&tree.paint(), cursor)[0].origin.x;
    controller.set_selection(incular_widgets::internal::TextSelection::collapsed(2));
    controller.reset_caret(Instant::now());
    layout_tight(&mut tree, 200., 64.);
    let second_line = selection_rects(&tree.paint(), cursor)[0].origin.x;
    assert!(
        first_line > second_line + 5.,
        "the short centered line starts right of the long one: {first_line} vs {second_line}"
    );
}

#[test]
fn alignment_and_width_change_while_mounted() {
    let width = 200.;
    let controller = TextEditingController::with_text("hi");
    let (mut tree, root) = mount_field(controller.clone(), |field| {
        field
            .text_align(TextAlign::Start)
            .size(Size::new(width, 32.))
    });
    layout_tight(&mut tree, width, 32.);
    let first = ink_bounds(&glyph_edges(&tree.paint())).0;

    tree.update(
        root,
        Widget::from(
            EditableText::new(controller.clone())
                .text_align(TextAlign::Center)
                .size(Size::new(width, 32.)),
        ),
    )
    .expect("center while mounted");
    layout_tight(&mut tree, width, 32.);
    let second = ink_bounds(&glyph_edges(&tree.paint())).0;

    tree.update(
        root,
        Widget::from(
            EditableText::new(controller.clone())
                .text_align(TextAlign::End)
                .size(Size::new(300., 32.)),
        ),
    )
    .expect("end with new width");
    layout_tight(&mut tree, 300., 32.);
    let third = ink_bounds(&glyph_edges(&tree.paint())).0;
    assert!(first < second && second < third);

    // Re-layout is deterministic: a fresh mount agrees exactly.
    let fresh = field_edges(TextEditingController::with_text("hi"), 300., TextAlign::End);
    assert!((ink_bounds(&fresh).0 - third).abs() < f32::EPSILON);
}

#[test]
fn selection_rect_covers_the_aligned_line_end() {
    for (name, text) in [
        ("short", "hi".to_owned()),
        ("overflowing", "0123456789".repeat(8)),
    ] {
        let highlight = Color::rgba(0, 255, 0, 255);
        let controller = TextEditingController::with_text(text.clone());
        controller.set_selection(incular_widgets::internal::TextSelection {
            base: 0,
            extent: text.len(),
        });
        let (mut tree, root) = mount_field(controller.clone(), |field| {
            field
                .text_align(TextAlign::Center)
                .selection_color(highlight)
                .size(Size::new(200., 32.))
        });
        layout_tight(&mut tree, 200., 32.);
        tree.set_focused(root, true, Instant::now())
            .expect("focus field");
        let painted = tree.paint();
        let (ink_min, ink_max) = ink_bounds(&glyph_edges(&painted));
        let rects = selection_rects(&painted, highlight);
        assert_eq!(rects.len(), 1, "{name}: one rect for the single line");
        // The pre-offset advance alone ends the rect early on aligned
        // text; both edges must ride the translation exactly once.
        assert!(
            (rects[0].origin.x - ink_min).abs() < 1.5,
            "{name}: rect starts at the ink, got {} want {ink_min}",
            rects[0].origin.x
        );
        assert!(
            ((rects[0].origin.x + rects[0].size.width) - ink_max).abs() < 1.5,
            "{name}: rect ends at the ink, got {} want {ink_max}",
            rects[0].origin.x + rects[0].size.width
        );
    }
}

#[test]
fn pointer_maps_through_centered_text() {
    let width = 200.;
    let controller = TextEditingController::with_text("hi");
    let (mut tree, root) = mount_field(controller.clone(), |field| {
        field
            .text_align(TextAlign::Center)
            .size(Size::new(width, 32.))
    });
    layout_tight(&mut tree, width, 32.);
    let (ink_min, ink_max) = ink_bounds(&glyph_edges(&tree.paint()));
    let now = Instant::now();
    // Padding before the centered ink still belongs to the line start.
    assert!(tree.text_field_set_caret(root, Offset::new(ink_min - 2., 16.), false, now));
    assert_eq!(controller.selection().extent, 0);
    // Padding after the ink maps to the buffer end, not back inside.
    assert!(tree.text_field_set_caret(root, Offset::new(ink_max + 2., 16.), false, now));
    assert_eq!(controller.selection().extent, 2);
}
