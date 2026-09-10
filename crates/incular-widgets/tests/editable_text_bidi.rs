//! Right-to-left and mixed-direction caret/selection geometry.
//!
//! The shaped caret stops are the authoritative source for direction
//! geometry; cluster spans project selections to visual segments.
//! Glyph x positions, caret edges, and pointer mapping must agree in
//! text-area coordinates for pure RTL and mixed-direction text under
//! start/center/end alignment.

use incular_config::Constraints;
use incular_core::{Color, Offset, Rect, Size};
use incular_rendering::PaintCommand;
use incular_text::{TextAffinity, TextAlign, TextEditingController};
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

fn colored_rects(list: &incular_rendering::DisplayList, color: Color) -> Vec<Rect> {
    list.commands()
        .iter()
        .filter_map(|command| match command {
            PaintCommand::Rect { rect, color: paint } if *paint == color => Some(*rect),
            _ => None,
        })
        .collect()
}

fn select(controller: &TextEditingController, base: usize, extent: usize) {
    controller.set_selection(TextSelection { base, extent });
    controller.reset_caret(Instant::now());
}

// Controller edits invalidate through the layout preamble, so every
// select-then-paint step re-runs layout exactly like a real frame;
// painting twice without layout would read the cached list.
fn relayout(tree: &mut WidgetTree, width: f32, height: f32) {
    layout_tight(tree, width, height);
}

#[test]
fn rtl_logical_edges_map_to_visual_edges() {
    // Pure RTL: logical start sits at the visual right, logical end at
    // the visual left, under every alignment.
    let width = 300.;
    let avail = width - 16.;
    for align in [TextAlign::Start, TextAlign::Center, TextAlign::End] {
        let controller = TextEditingController::with_text("שלום");
        let (mut tree, root) = mount_field(controller.clone(), |field| {
            field.text_align(align).size(Size::new(width, 32.))
        });
        layout_tight(&mut tree, width, 32.);
        tree.set_focused(root, true, Instant::now())
            .expect("focus field");
        let (ink_min, ink_max) = ink_bounds(&glyph_edges(&tree.paint()));
        let advance = ink_max - ink_min;
        let (expect_min, expect_max) = match align {
            TextAlign::Start => (8. + avail - advance, 8. + avail),
            TextAlign::Center => (8. + (avail - advance) / 2., 8. + (avail + advance) / 2.),
            TextAlign::End => (8., 8. + advance),
            TextAlign::Justify => (8., 8. + advance),
        };
        assert!(
            (ink_min - expect_min).abs() < 2.5,
            "{align:?}: ink starts at {ink_min}, want {expect_min}"
        );
        assert!(
            (ink_max - expect_max).abs() < 2.5,
            "{align:?}: ink ends at {ink_max}, want {expect_max}"
        );

        // Caret at logical start meets the visual right edge; caret at
        // logical end meets the visual left edge.
        let cursor = Color::rgba(255, 0, 0, 255);
        let (mut tree, root) = mount_field(controller.clone(), |field| {
            field
                .text_align(align)
                .cursor_color(cursor)
                .size(Size::new(width, 32.))
        });
        layout_tight(&mut tree, width, 32.);
        tree.set_focused(root, true, Instant::now())
            .expect("focus field");
        select(&controller, 0, 0);
        relayout(&mut tree, width, 32.);
        let at_start = colored_rects(&tree.paint(), cursor)[0].origin.x;
        select(&controller, 8, 8);
        relayout(&mut tree, width, 32.);
        let at_end = colored_rects(&tree.paint(), cursor)[0].origin.x;
        assert!(
            (at_start - expect_max).abs() < 2.5,
            "{align:?}: logical-start caret at {at_start}, want {expect_max}"
        );
        assert!(
            (at_end - expect_min).abs() < 2.5,
            "{align:?}: logical-end caret at {at_end}, want {expect_min}"
        );
    }
}

#[test]
fn rtl_partial_selection_covers_its_visual_span() {
    // Selecting the middle two RTL characters must highlight their
    // visual span, not a sliver at the line edge.
    let highlight = Color::rgba(0, 255, 0, 255);
    let controller = TextEditingController::with_text("שלום");
    let (mut tree, root) = mount_field(controller.clone(), |field| {
        field
            .text_align(TextAlign::Start)
            .selection_color(highlight)
            .size(Size::new(300., 32.))
    });
    layout_tight(&mut tree, 300., 32.);
    tree.set_focused(root, true, Instant::now())
        .expect("focus field");
    select(&controller, 2, 6);
    let rects = colored_rects(&tree.paint(), highlight);
    assert_eq!(rects.len(), 1, "one visual span for one run slice");
    assert!(
        rects[0].size.width > 5.,
        "partial RTL selection must span ink, got width {}",
        rects[0].size.width
    );
}

#[test]
fn mixed_direction_partial_selection_matches_cluster_spans() {
    // "hi שלום bye": selecting the Hebrew run plus its surrounding
    // spaces must cover exactly their visual span.
    let highlight = Color::rgba(0, 255, 0, 255);
    let controller = TextEditingController::with_text("hi שלום bye");
    let (mut tree, root) = mount_field(controller.clone(), |field| {
        field
            .text_align(TextAlign::Start)
            .selection_color(highlight)
            .size(Size::new(300., 32.))
    });
    layout_tight(&mut tree, 300., 32.);
    tree.set_focused(root, true, Instant::now())
        .expect("focus field");
    relayout(&mut tree, 300., 32.);
    select(&controller, 2, 12);
    relayout(&mut tree, 300., 32.);
    let painted = tree.paint();
    let rects = colored_rects(&painted, highlight);
    // Bytes 2..12 tile the space (ink 12.93..17.31), the Hebrew run,
    // and the next space (ink 54.07..58.45) with no visual gap: one
    // merged span, in field coordinates (+8 area offset).
    assert_eq!(
        rects.len(),
        1,
        "contiguous visual coverage merges: {rects:?}"
    );
    assert!(
        (rects[0].origin.x - 20.93).abs() < 2.0,
        "span starts at the first space ink, got {}",
        rects[0].origin.x
    );
    assert!(
        ((rects[0].origin.x + rects[0].size.width) - 66.45).abs() < 2.0,
        "span ends at the second space ink: {rects:?}"
    );
}

#[test]
fn mixed_affinities_at_run_edges_select_visual_sides() {
    // Byte 11 (the space between the Hebrew run and "bye") owns a stop
    // on each visual side of the boundary.
    let cursor = Color::rgba(255, 0, 0, 255);
    let controller = TextEditingController::with_text("hi שלום bye");
    let (mut tree, root) = mount_field(controller.clone(), |field| {
        field
            .text_align(TextAlign::Start)
            .cursor_color(cursor)
            .size(Size::new(300., 32.))
    });
    layout_tight(&mut tree, 300., 32.);
    tree.set_focused(root, true, Instant::now())
        .expect("focus field");
    controller.set_selection_with_affinity(TextSelection::collapsed(11), TextAffinity::Upstream);
    controller.reset_caret(Instant::now());
    relayout(&mut tree, 300., 32.);
    let upstream = colored_rects(&tree.paint(), cursor)[0].origin.x;
    controller.set_selection_with_affinity(TextSelection::collapsed(11), TextAffinity::Downstream);
    controller.reset_caret(Instant::now());
    relayout(&mut tree, 300., 32.);
    let downstream = colored_rects(&tree.paint(), cursor)[0].origin.x;
    assert!(
        (upstream - downstream).abs() > 5.,
        "affinities must resolve to opposite visual sides: {upstream} vs {downstream}"
    );
    // Upstream hugs the Hebrew run's left edge, downstream the run's
    // right edge where "bye" continues.
    assert!(upstream < downstream, "upstream sits left of downstream");
}

#[test]
fn pointer_maps_rtl_and_mixed_text() {
    let width = 300.;
    let controller = TextEditingController::with_text("שלום");
    let (mut tree, root) = mount_field(controller.clone(), |field| {
        field
            .text_align(TextAlign::Start)
            .size(Size::new(width, 32.))
    });
    layout_tight(&mut tree, width, 32.);
    let (ink_min, ink_max) = ink_bounds(&glyph_edges(&tree.paint()));
    let now = Instant::now();
    // Visual right is logical start; visual left is logical end.
    assert!(tree.text_field_set_caret(root, Offset::new(ink_max - 1., 16.), false, now));
    assert_eq!(controller.selection().extent, 0);
    assert!(tree.text_field_set_caret(root, Offset::new(ink_min + 1., 16.), false, now));
    assert_eq!(controller.selection().extent, 8);

    let controller = TextEditingController::with_text("hi שלום bye");
    let (mut tree, root) = mount_field(controller.clone(), |field| {
        field
            .text_align(TextAlign::Start)
            .size(Size::new(width, 32.))
    });
    layout_tight(&mut tree, width, 32.);
    // Inside the LTR head maps to low bytes.
    assert!(tree.text_field_set_caret(root, Offset::new(8. + 4., 16.), false, now));
    assert!(
        controller.selection().extent <= 2,
        "LTR head maps low, got {}",
        controller.selection().extent
    );
    // Inside the Hebrew run maps to Hebrew bytes.
    assert!(tree.text_field_set_caret(root, Offset::new(8. + 30., 16.), false, now));
    let hebrew = controller.selection().extent;
    assert!(
        (3..=11).contains(&hebrew),
        "Hebrew area maps to Hebrew bytes, got {hebrew}"
    );
}

#[test]
fn rtl_wrapping_and_width_change_stay_consistent() {
    let text = "שלום עולם hello world ".repeat(4);
    let controller = TextEditingController::with_text(text.clone());
    let (mut tree, root) = mount_field(controller.clone(), |field| {
        field
            .text_align(TextAlign::Start)
            .multiline(true)
            .size(Size::new(300., 200.))
    });
    layout_tight(&mut tree, 300., 200.);
    // Whole-text selection covers every line's ink and nothing else wild.
    controller.set_selection(TextSelection {
        base: 0,
        extent: text.len(),
    });
    tree.set_focused(root, true, Instant::now())
        .expect("focus field");
    let highlight = Color::rgba(0, 255, 0, 255);
    let (mut tree, root) = mount_field(controller.clone(), |field| {
        field
            .text_align(TextAlign::Start)
            .multiline(true)
            .selection_color(highlight)
            .size(Size::new(300., 200.))
    });
    layout_tight(&mut tree, 300., 200.);
    tree.set_focused(root, true, Instant::now())
        .expect("focus field");
    let painted = tree.paint();
    let (ink_min, ink_max) = ink_bounds(&glyph_edges(&painted));
    for rect in colored_rects(&painted, highlight) {
        assert!(
            rect.origin.x >= ink_min - 1.5 && rect.origin.x + rect.size.width <= ink_max + 1.5,
            "wrapped rect {rect:?} escapes ink [{ink_min}, {ink_max}]"
        );
    }
    // Narrowing while mounted re-resolves exactly like a fresh mount.
    tree.update(
        root,
        Widget::from(
            EditableText::new(controller.clone())
                .text_align(TextAlign::Start)
                .multiline(true)
                .selection_color(highlight)
                .size(Size::new(150., 200.)),
        ),
    )
    .expect("narrow while mounted");
    layout_tight(&mut tree, 150., 200.);
    let narrow: Vec<Rect> = colored_rects(&tree.paint(), highlight);
    let fresh_controller = TextEditingController::with_text(text);
    fresh_controller.set_selection(TextSelection {
        base: 0,
        extent: fresh_controller.text().len(),
    });
    let (mut fresh, fresh_root) = mount_field(fresh_controller, |field| {
        field
            .text_align(TextAlign::Start)
            .multiline(true)
            .selection_color(highlight)
            .size(Size::new(150., 200.))
    });
    layout_tight(&mut fresh, 150., 200.);
    fresh
        .set_focused(fresh_root, true, Instant::now())
        .expect("focus fresh field");
    assert_eq!(narrow, colored_rects(&fresh.paint(), highlight));
}

#[test]
fn collapsed_selection_paints_no_highlight() {
    let highlight = Color::rgba(0, 255, 0, 255);
    let controller = TextEditingController::with_text("hello");
    let (mut tree, root) = mount_field(controller.clone(), |field| {
        field.selection_color(highlight).size(Size::new(200., 32.))
    });
    layout_tight(&mut tree, 200., 32.);
    tree.set_focused(root, true, Instant::now())
        .expect("focus field");
    select(&controller, 2, 2);
    relayout(&mut tree, 200., 32.);
    assert!(
        colored_rects(&tree.paint(), highlight).is_empty(),
        "a collapsed selection highlights nothing"
    );
}

#[test]
fn newline_only_selection_paints_no_highlight() {
    // Selecting just the line break covers no visible cluster on
    // either line; the caret still lands at the next line start.
    let highlight = Color::rgba(0, 255, 0, 255);
    let cursor = Color::rgba(255, 0, 0, 255);
    let controller = TextEditingController::with_text("a\nb");
    let (mut tree, root) = mount_field(controller.clone(), |field| {
        field
            .multiline(true)
            .selection_color(highlight)
            .cursor_color(cursor)
            .size(Size::new(200., 64.))
    });
    layout_tight(&mut tree, 200., 64.);
    tree.set_focused(root, true, Instant::now())
        .expect("focus field");
    select(&controller, 1, 2);
    relayout(&mut tree, 200., 64.);
    let painted = tree.paint();
    assert!(
        colored_rects(&painted, highlight).is_empty(),
        "the invisible break highlights nothing"
    );
    assert_eq!(
        colored_rects(&painted, cursor).len(),
        1,
        "the caret still shows"
    );
}

#[test]
fn joiner_only_selection_paints_no_highlight() {
    // U+200D shapes no cluster of its own: selecting only the joiner
    // covers nothing visible, while surrounding selections still span it.
    let highlight = Color::rgba(0, 255, 0, 255);
    let controller = TextEditingController::with_text("a\u{200D}b");
    let (mut tree, root) = mount_field(controller.clone(), |field| {
        field.selection_color(highlight).size(Size::new(200., 32.))
    });
    layout_tight(&mut tree, 200., 32.);
    tree.set_focused(root, true, Instant::now())
        .expect("focus field");
    select(&controller, 1, 4);
    relayout(&mut tree, 200., 32.);
    assert!(
        colored_rects(&tree.paint(), highlight).is_empty(),
        "the invisible joiner highlights nothing"
    );
    select(&controller, 0, 5);
    relayout(&mut tree, 200., 32.);
    let rects = colored_rects(&tree.paint(), highlight);
    assert_eq!(rects.len(), 1);
    assert!(rects[0].size.width > 5.);
}

#[test]
fn ligature_and_combining_carets_snap_to_cluster_edges() {
    // U+FB01 is one char in three bytes: intra-char bytes never reach
    // caret mapping because selection clamps down to char boundaries.
    let controller = TextEditingController::with_text("\u{FB01}sh");
    controller.set_selection(TextSelection { base: 1, extent: 1 });
    assert_eq!(controller.selection().extent, 0);
    controller.set_selection(TextSelection { base: 2, extent: 2 });
    assert_eq!(controller.selection().extent, 0);

    // e + combining acute share one cluster across a char boundary
    // (byte 1): the mapping has stops only at cluster edges, so the
    // mid-grapheme caret resolves to the cluster edge.
    let cursor = Color::rgba(255, 0, 0, 255);
    let controller = TextEditingController::with_text("e\u{301}x");
    let (mut tree, root) = mount_field(controller.clone(), |field| {
        field
            .text_align(TextAlign::Start)
            .cursor_color(cursor)
            .size(Size::new(200., 32.))
    });
    layout_tight(&mut tree, 200., 32.);
    tree.set_focused(root, true, Instant::now())
        .expect("focus field");
    let caret_at = |tree: &mut WidgetTree, byte: usize| {
        select(&controller, byte, byte);
        relayout(tree, 200., 32.);
        colored_rects(&tree.paint(), cursor)[0].origin.x
    };
    let base = caret_at(&mut tree, 0);
    let mid_grapheme = caret_at(&mut tree, 1);
    let after = caret_at(&mut tree, 3);
    let tail = caret_at(&mut tree, 4);
    assert_eq!(
        mid_grapheme, after,
        "mid-grapheme caret resolves to the cluster edge"
    );
    assert!(
        base < after && after < tail,
        "cluster edges order left to right"
    );
}
