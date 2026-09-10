//! EditableText configuration and measurement contracts.
//!
//! Covers the descriptor setters (order effects, clamping floors,
//! conflicting ranges), sizing under bounded and unbounded constraints,
//! paint output for text/placeholder/caret/selection, alignment,
//! obscuring, semantic exposure, and invalidation on content, style, and
//! identical updates. All assertions use public tree APIs only.

mod common;

use common::*;
use incular_config::Constraints;
use incular_core::{Color, Offset, Rect, Size};
use incular_rendering::{Brush, PaintCommand};
use incular_semantics::{SemanticActionKind, SemanticRole};
use incular_text::{TextAlign, TextEditingController, TextSelection, TextStyle};
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

fn layout_loose(tree: &mut WidgetTree) {
    tree.layout(Constraints::loose(Size::new(300., 300.)))
        .expect("layout");
}

fn field_size(tree: &WidgetTree, id: ElementId) -> Size {
    tree.element_bounds(id).expect("field bounds").size
}

fn semantic_id(tree: &mut WidgetTree, id: ElementId) -> incular_semantics::SemanticNodeId {
    tree.update_semantics();
    tree.semantic_node_for_element(id).expect("semantic node")
}

#[test]
fn multiline_tracks_max_lines_with_last_setter_winning() {
    let controller = TextEditingController::new();
    let (mut tree, root) = mount_field(controller.clone(), |field| {
        field.multiline(true).max_lines(Some(3))
    });
    layout_loose(&mut tree);
    assert!(tree.is_multiline_text_field(root));

    tree.update(
        root,
        Widget::from(EditableText::new(controller.clone()).max_lines(Some(1))),
    )
    .expect("update");
    layout_loose(&mut tree);
    assert!(!tree.is_multiline_text_field(root));

    tree.update(
        root,
        Widget::from(EditableText::new(controller).multiline(true)),
    )
    .expect("update");
    layout_loose(&mut tree);
    assert!(tree.is_multiline_text_field(root));
    let sid = semantic_id(&mut tree, root);
    assert_eq!(
        tree.semantics().node(sid).expect("node").role,
        SemanticRole::TextArea,
        "multiline fields expose the text-area role"
    );
}

#[test]
fn line_count_floor_is_one() {
    let controller = TextEditingController::with_text("hi");
    let (mut tree, root) = mount_field(controller.clone(), |field| {
        field.max_lines(Some(0)).min_lines(Some(0))
    });
    layout_loose(&mut tree);
    assert!(
        !tree.is_multiline_text_field(root),
        "a zero max-lines clamp must stay single-line"
    );
    let floored = field_size(&tree, root);

    let (mut tree_2, root_2) = mount_field(controller, |field| {
        field.max_lines(Some(1)).min_lines(Some(1))
    });
    layout_loose(&mut tree_2);
    assert_eq!(field_size(&tree_2, root_2), floored);
}

#[test]
fn conflicting_min_max_lines_stay_deterministic() {
    // min_lines above max_lines is not renormalized: the minimum height
    // wins over the capped intrinsic height, deterministically.
    let controller = TextEditingController::with_text("hi");
    let (mut tree, root) = mount_field(controller.clone(), |field| {
        field.min_lines(Some(5)).max_lines(Some(2))
    });
    layout_loose(&mut tree);
    let conflicted = field_size(&tree, root);

    let (mut plain, plain_root) = mount_field(controller.clone(), |field| {
        field.min_lines(Some(5)).max_lines(None)
    });
    layout_loose(&mut plain);
    assert_eq!(
        field_size(&plain, plain_root),
        conflicted,
        "a conflicting cap must not move the minimum-dominated height"
    );

    let (mut single, single_root) = mount_field(controller, |field| field.min_lines(Some(1)));
    layout_loose(&mut single);
    assert!(
        conflicted.height > field_size(&single, single_root).height,
        "five minimum lines must exceed one"
    );
}

#[test]
fn desired_size_expands_and_unbounded_width() {
    let controller = TextEditingController::with_text("hi");
    let (mut tree, root) =
        mount_field(controller.clone(), |field| field.size(Size::new(120., 40.)));
    layout_loose(&mut tree);
    assert_eq!(field_size(&tree, root), Size::new(120., 40.));

    tree.update(
        root,
        Widget::from(
            EditableText::new(controller.clone())
                .size(Size::new(120., 40.))
                .expands(true),
        ),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    assert_eq!(field_size(&tree, root), Size::new(200., 200.));

    let (mut unbounded, unbounded_root) = mount_field(controller, |field| field);
    unbounded.layout(Constraints::unbounded()).expect("layout");
    assert_eq!(field_size(&unbounded, unbounded_root).width, 260.);

    // height() preserves the current width while replacing the height.
    let (mut sized, sized_root) = mount_field(TextEditingController::with_text("hi"), |field| {
        field.size(Size::new(120., 40.)).height(55.)
    });
    layout_loose(&mut sized);
    assert_eq!(field_size(&sized, sized_root), Size::new(120., 55.));
}

fn placeholder_gray_runs(list: &DisplayList) -> usize {
    list.commands()
        .iter()
        .filter(|command| {
            matches!(
                command,
                PaintCommand::GlyphRun { color, .. }
                    if *color == Color::rgba(150, 154, 170, 255)
            )
        })
        .count()
}

#[test]
fn empty_field_paints_placeholder_then_text() {
    let controller = TextEditingController::new();
    let (mut tree, _) = mount_field(controller.clone(), |field| {
        field.placeholder("Name").size(Size::new(180., 32.))
    });
    tree.layout(Constraints::tight(Size::new(180., 32.)))
        .expect("layout");
    assert_eq!(
        placeholder_gray_runs(&tree.paint()),
        1,
        "empty field paints one gray placeholder run"
    );

    controller.set_text("Ada");
    tree.layout(Constraints::tight(Size::new(180., 32.)))
        .expect("layout");
    let list = tree.paint();
    assert_eq!(
        placeholder_gray_runs(&list),
        0,
        "committed text replaces the placeholder"
    );
    assert!(
        list.commands().iter().any(|command| matches!(
            command,
            PaintCommand::GlyphRun { color, .. }
                if *color == TextStyle::default().color
        )),
        "committed text paints in the style color"
    );
}

#[test]
fn cursor_paints_geometry_and_color_when_focused() {
    let controller = TextEditingController::with_text("hi");
    let caret = Color::rgba(255, 0, 0, 255);
    let (mut tree, root) = mount_field(controller.clone(), |field| {
        field
            .cursor_width(3.)
            .cursor_color(caret)
            .size(Size::new(180., 32.))
    });
    tree.layout(Constraints::tight(Size::new(180., 32.)))
        .expect("layout");
    tree.set_focused(root, true, Instant::now())
        .expect("focus field");
    controller.reset_caret(Instant::now());
    let caret_rects: Vec<Rect> = tree
        .paint()
        .commands()
        .iter()
        .filter_map(|command| match command {
            PaintCommand::Rect { rect, color } if *color == caret => Some(*rect),
            _ => None,
        })
        .collect();
    assert_eq!(caret_rects.len(), 1, "one caret rect while focused");
    assert!((caret_rects[0].size.width - 3.).abs() < f32::EPSILON);
    assert!(caret_rects[0].size.height > 0.);

    // Zero width, hidden cursor, and lost focus all remove the caret.
    for build in [
        |field: EditableText| field.cursor_width(0.),
        |field: EditableText| field.show_cursor(false),
    ] {
        tree.update(
            root,
            Widget::from(build(EditableText::new(controller.clone())).size(Size::new(180., 32.))),
        )
        .expect("update");
        tree.layout(Constraints::tight(Size::new(180., 32.)))
            .expect("layout");
        tree.set_focused(root, true, Instant::now())
            .expect("focus field");
        controller.reset_caret(Instant::now());
        assert!(
            !tree.paint().commands().iter().any(|command| matches!(
                command,
                PaintCommand::Rect { color, .. } if *color == caret
            )),
            "caret must disappear"
        );
    }

    tree.set_focused(root, false, Instant::now())
        .expect("unfocus field");
    assert!(
        !tree.paint().commands().iter().any(|command| matches!(
            command,
            PaintCommand::Rect { color, .. } if *color == caret
        )),
        "unfocused field paints no caret"
    );
}

#[test]
fn cursor_height_and_radius_shape() {
    let controller = TextEditingController::with_text("hi");
    let caret = Color::rgba(255, 0, 0, 255);
    let (mut tree, root) = mount_field(controller.clone(), |field| {
        field
            .cursor_height(Some(10.))
            .cursor_radius(2.)
            .cursor_color(caret)
            .size(Size::new(180., 32.))
    });
    tree.layout(Constraints::tight(Size::new(180., 32.)))
        .expect("layout");
    tree.set_focused(root, true, Instant::now())
        .expect("focus field");
    controller.reset_caret(Instant::now());
    let list = tree.paint();
    assert!(
        list.commands().iter().any(|command| matches!(
            command,
            PaintCommand::RRect { brush, .. } if *brush == Brush::Solid(caret)
        )),
        "rounded caret paints as a rounded rect"
    );
    let heights: Vec<f32> = list
        .commands()
        .iter()
        .filter_map(|command| match command {
            PaintCommand::RRect { rrect, .. } => Some(rrect.rect.size.height),
            _ => None,
        })
        .collect();
    assert_eq!(heights, vec![10.]);
}

#[test]
fn selection_highlight_follows_focus_and_color() {
    let controller = TextEditingController::with_text("hello");
    controller.set_selection(TextSelection { base: 0, extent: 2 });
    let highlight = Color::rgba(0, 255, 0, 255);
    let (mut tree, root) = mount_field(controller.clone(), |field| {
        field.selection_color(highlight).size(Size::new(180., 32.))
    });
    tree.layout(Constraints::tight(Size::new(180., 32.)))
        .expect("layout");
    tree.set_focused(root, true, Instant::now())
        .expect("focus field");
    assert!(
        tree.paint().commands().iter().any(|command| matches!(
            command,
            PaintCommand::Rect { color, .. } if *color == highlight
        )),
        "focused selection paints the highlight"
    );

    tree.set_focused(root, false, Instant::now())
        .expect("unfocus field");
    assert!(
        !tree.paint().commands().iter().any(|command| matches!(
            command,
            PaintCommand::Rect { color, .. } if *color == highlight
        )),
        "unfocused selection paints no highlight"
    );
}

#[test]
fn center_alignment_shifts_glyphs() {
    // Glyph-run origins stay zero; alignment lives in per-glyph offsets.
    fn first_glyph_x(list: &DisplayList) -> f32 {
        let mut shift = Offset::ZERO;
        for command in list.commands() {
            match command {
                PaintCommand::PushTransform { transform } => {
                    shift = shift + transform.translation_offset();
                }
                PaintCommand::GlyphRun { run, .. } => {
                    if let Some(glyph) = run.glyphs.first() {
                        return shift.x + glyph.offset.x;
                    }
                }
                _ => {}
            }
        }
        panic!("expected a glyph run");
    }
    let controller = TextEditingController::with_text("hi");
    let origin_x = |align: TextAlign| {
        let (mut tree, _) = mount_field(controller.clone(), |field| {
            field.text_align(align).size(Size::new(200., 32.))
        });
        tree.layout(Constraints::tight(Size::new(200., 32.)))
            .expect("layout");
        first_glyph_x(&tree.paint())
    };
    let start = origin_x(TextAlign::Start);
    let centered = origin_x(TextAlign::Center);
    assert!(
        centered > start + 5.,
        "centered text must start right of start-aligned text"
    );
}

#[test]
fn obscure_masks_paint_keeps_semantics() {
    let controller = TextEditingController::with_text("é");
    let (mut tree, root) = mount_field(controller.clone(), |field| {
        field.obscure_text(true).size(Size::new(180., 32.))
    });
    tree.layout(Constraints::tight(Size::new(180., 32.)))
        .expect("layout");
    // One mask glyph per source byte: two bytes in, two glyphs out.
    assert_eq!(glyph_count(&tree.paint()), 2);

    let (mut plain, _) = mount_field(controller.clone(), |field| field.size(Size::new(180., 32.)));
    plain
        .layout(Constraints::tight(Size::new(180., 32.)))
        .expect("layout");
    assert_eq!(glyph_count(&plain.paint()), 1);

    let sid = semantic_id(&mut tree, root);
    let node = tree.semantics().node(sid).expect("node");
    assert_eq!(node.value.as_deref(), Some("é"));
    assert!(node.state.obscured);
}

#[test]
fn identical_reapplication_schedules_nothing() {
    let controller = TextEditingController::with_text("hi");
    let (mut tree, root) =
        mount_field(controller.clone(), |field| field.size(Size::new(180., 32.)));
    tree.layout(Constraints::tight(Size::new(180., 32.)))
        .expect("layout");
    let _ = tree.paint();
    let before = tree.diagnostics();
    tree.update(
        root,
        Widget::from(EditableText::new(controller.clone()).size(Size::new(180., 32.))),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(180., 32.)))
        .expect("layout");
    let _ = tree.paint();
    let after = tree.diagnostics();
    assert_eq!(after.layouts, before.layouts);
    assert_eq!(after.paints, before.paints);
    assert_eq!(after.rebuilds, before.rebuilds);
    assert!(
        tree.text_controller(root)
            .is_some_and(|current| current == controller)
    );
}

#[test]
fn content_and_style_changes_repaint_with_layout() {
    let controller = TextEditingController::with_text("hi");
    let (mut tree, root) =
        mount_field(controller.clone(), |field| field.size(Size::new(180., 32.)));
    tree.layout(Constraints::tight(Size::new(180., 32.)))
        .expect("layout");
    let _ = tree.paint();
    let before = tree.diagnostics();
    controller.set_text("hello, world");
    tree.layout(Constraints::tight(Size::new(180., 32.)))
        .expect("layout");
    let _ = tree.paint();
    let after_content = tree.diagnostics();
    assert!(after_content.layouts > before.layouts);
    assert!(after_content.paints > before.paints);

    let style = TextStyle {
        color: Color::rgba(10, 20, 30, 255),
        ..TextStyle::default()
    };
    tree.update(
        root,
        Widget::from(
            EditableText::new(controller.clone())
                .style(style)
                .size(Size::new(180., 32.)),
        ),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(180., 32.)))
        .expect("layout");
    let _ = tree.paint();
    let after_style = tree.diagnostics();
    assert!(after_style.layouts > after_content.layouts);
    assert!(after_style.paints > after_content.paints);
}

#[test]
fn submit_invokes_current_callback_with_text() {
    use std::{cell::Cell, rc::Rc};
    let controller = TextEditingController::with_text("go");
    let first = Rc::new(Cell::new(0));
    let observed_first = first.clone();
    let (mut tree, root) = mount_field(controller.clone(), |field| {
        field
            .on_submit(move |text| {
                assert_eq!(text, "go");
                observed_first.set(observed_first.get() + 1);
            })
            .size(Size::new(180., 32.))
    });
    layout_loose(&mut tree);
    assert!(tree.submit_text_field(root));
    assert_eq!(first.get(), 1);
}

#[test]
fn input_hints_reach_the_native_snapshot() {
    use incular_widgets::{TextInputActionHint, TextInputTypeHint};
    let controller = TextEditingController::new();
    let (mut tree, root) = mount_field(controller, |field| {
        field
            .input_type(TextInputTypeHint::Email)
            .input_action(TextInputActionHint::Send)
            .size(Size::new(180., 32.))
    });
    layout_loose(&mut tree);
    let snapshot = tree
        .text_field_input_snapshot(root)
        .expect("input snapshot");
    assert_eq!(snapshot.input_type, TextInputTypeHint::Email);
    assert_eq!(snapshot.input_action, TextInputActionHint::Send);
}

#[test]
fn semantic_text_selection_and_actions() {
    let controller = TextEditingController::with_text("hello");
    controller.set_selection(TextSelection { base: 1, extent: 3 });
    let (mut tree, root) =
        mount_field(controller.clone(), |field| field.size(Size::new(180., 32.)));
    tree.layout(Constraints::tight(Size::new(180., 32.)))
        .expect("layout");
    let sid = semantic_id(&mut tree, root);
    let node = tree.semantics().node(sid).expect("node");
    assert_eq!(node.role, SemanticRole::TextField);
    assert_eq!(node.value.as_deref(), Some("hello"));
    assert_eq!(
        node.state
            .selection
            .map(|selection| (selection.base, selection.extent)),
        Some((1, 3))
    );
    for action in [
        SemanticActionKind::Focus,
        SemanticActionKind::SetSelection,
        SemanticActionKind::SetText,
    ] {
        assert!(node.actions.contains(&action), "missing {action:?}");
    }
    assert!(node.state.editable);

    let snapshot = tree
        .text_field_input_snapshot(root)
        .expect("input snapshot");
    assert_eq!(snapshot.text, "hello");
    assert!(snapshot.enabled && !snapshot.read_only && !snapshot.obscure_text);

    tree.update(
        root,
        Widget::from(
            EditableText::new(controller.clone())
                .enabled(false)
                .size(Size::new(180., 32.)),
        ),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(180., 32.)))
        .expect("layout");
    let sid = semantic_id(&mut tree, root);
    let node = tree.semantics().node(sid).expect("node");
    assert!(!node.actions.contains(&SemanticActionKind::SetText));
    assert!(!node.state.editable);

    tree.update(
        root,
        Widget::from(
            EditableText::new(controller)
                .enabled(true)
                .read_only(true)
                .size(Size::new(180., 32.)),
        ),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(180., 32.)))
        .expect("layout");
    assert!(!tree.text_field_is_editable(root));
    assert!(tree.text_field_is_read_only(root));
    let sid = semantic_id(&mut tree, root);
    let node = tree.semantics().node(sid).expect("node");
    assert!(!node.actions.contains(&SemanticActionKind::SetText));
    assert!(node.state.read_only);
    assert!(!node.state.editable);
}
