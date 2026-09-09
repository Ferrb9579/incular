//! Retained contracts for clipping and opacity: what each public option
//! records, what it invalidates, and what it deliberately leaves alone.
//!
//! Every case mounts, lays out, and paints first (warming the picture
//! cache), then updates and re-runs the phases. Raster assertions fold
//! the full compositor transforms in the flattened display list.
//! Diagnostics separate layout work (`layouts`), picture recording
//! (`paints`), and compositor-only updates (`compositor_only_updates`,
//! plus the compositor's own `opacity_updates`).

mod common;

use common::*;
use incular_config::{Clip, Constraints};
use incular_core::{Color, Offset, Rect, Size, Transform as CoreTransform};
use incular_rendering::{CornerRadii, DisplayList, FillRule, PaintCommand, Path};
use incular_widgets::internal::*;
use incular_widgets::*;
use std::sync::Arc;

fn labeled_action(label: &str) -> Widget {
    action(Size::new(40., 40.), Color::WHITE, ActionId(1)).accessibility_label(label)
}

/// World-space clip entries in record order: plain rects, rounded rects,
/// ovals, and paths each paired with their matching pop.
#[derive(Debug)]
enum WorldClip {
    Rect(Rect),
    RRect(incular_rendering::RRect),
    Oval(Rect),
    Path,
}

fn world_clips(list: &DisplayList) -> Vec<WorldClip> {
    let mut transforms = vec![CoreTransform::IDENTITY];
    let mut out = Vec::new();
    for command in list.commands() {
        match command {
            PaintCommand::PushTransform { transform } => {
                let top = *transforms.last().unwrap();
                transforms.push(top.then(*transform));
            }
            PaintCommand::PopTransform => {
                transforms.pop();
            }
            PaintCommand::PushClip { rect } => out.push(WorldClip::Rect(
                transforms.last().unwrap().transform_rect_bbox(*rect),
            )),
            PaintCommand::PushClipRRect { rrect } => {
                let world = transforms.last().unwrap().transform_rect_bbox(rrect.rect);
                out.push(WorldClip::RRect(incular_rendering::RRect::new(
                    world,
                    rrect.radii,
                )));
            }
            PaintCommand::PushClipOval { rect } => out.push(WorldClip::Oval(
                transforms.last().unwrap().transform_rect_bbox(*rect),
            )),
            PaintCommand::PushClipPath { .. } => out.push(WorldClip::Path),
            _ => {}
        }
    }
    out
}

fn clip_path_shape(list: &DisplayList) -> Option<(Arc<Path>, FillRule)> {
    list.commands().iter().find_map(|command| match command {
        PaintCommand::PushClipPath { path, fill_rule } => Some((path.clone(), *fill_rule)),
        _ => None,
    })
}

fn opacity_alphas(list: &DisplayList) -> Vec<f32> {
    list.commands()
        .iter()
        .filter_map(|command| match command {
            PaintCommand::PushOpacity { alpha, .. } => Some(*alpha),
            _ => None,
        })
        .collect()
}

fn approx_eq(actual: f32, expected: f32, what: &str) {
    assert!(
        (actual - expected).abs() < 0.01,
        "{what}: expected {expected}, got {actual}"
    );
}

fn assert_rect_eq(actual: Rect, expected: Rect, what: &str) {
    approx_eq(actual.origin.x, expected.origin.x, &format!("{what} x"));
    approx_eq(actual.origin.y, expected.origin.y, &format!("{what} y"));
    approx_eq(actual.size.width, expected.size.width, &format!("{what} w"));
    approx_eq(
        actual.size.height,
        expected.size.height,
        &format!("{what} h"),
    );
}

/// Centered single-child column so fixtures measure at deterministic
/// offsets: a 40-wide child sits at x 80, a 60-wide one at x 70.
fn centered(child: Widget) -> Widget {
    Column::new([child]).into()
}

fn triangle_path() -> Arc<Path> {
    let mut builder = Path::builder();
    builder
        .move_to(Offset::new(0., 0.))
        .line_to(Offset::new(40., 0.))
        .line_to(Offset::new(0., 40.))
        .close();
    Arc::new(builder.build())
}

#[test]
fn clip_behavior_none_records_no_clip() {
    // Clip::None must not clip: no clip commands at all.
    let mut tree = WidgetTree::new();
    tree.mount(centered(Widget::from(
        ClipRect::new(labeled_action("plain child")).clip_behavior(Clip::None),
    )))
    .unwrap();
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    assert!(
        world_clips(&tree.paint()).is_empty(),
        "Clip::None must record no clip commands"
    );
}

#[test]
fn clip_rect_records_world_clip() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(centered(Widget::from(ClipRect::new(labeled_action(
            "plain child",
        )))))
        .unwrap();
    let clip = tree.children(root).unwrap()[0];
    let moved = tree.children(clip).unwrap()[0];
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    // The 40x40 child centers the clip at (80,0) in the 200-wide column.
    let expected = Rect::from_origin_size(Offset::new(80., 0.), Size::new(40., 40.));
    let clips = world_clips(&tree.paint());
    assert_eq!(clips.len(), 1, "one clip entry, got {clips:?}");
    match &clips[0] {
        WorldClip::Rect(rect) => assert_rect_eq(*rect, expected, "clip rect"),
        other => panic!("expected plain rect clip, got {other:?}"),
    }
    // Clipping is raster-only: the child stays hittable and keeps its
    // full semantic bounds.
    let hit = tree
        .hit_test(Offset::new(90., 10.))
        .and_then(|render| tree.element_for_render(render));
    assert_eq!(hit, Some(moved));
    tree.update_semantics();
    let semantic = tree
        .semantic_node_for_element(moved)
        .and_then(|id| tree.semantics().node(id))
        .expect("clipped semantics");
    assert_rect_eq(semantic.bounds, expected, "clipped semantic bounds");
}

#[test]
fn clip_behavior_toggle_needs_neither_layout_nor_paint() {
    // The clip lives in a compositor layer, not in any picture: toggling
    // the behavior rebuilds the layer binding and rewrites the shape, but
    // measures and records nothing.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(centered(Widget::from(ClipRect::new(labeled_action(
            "plain child",
        )))))
        .unwrap();
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    assert_eq!(world_clips(&tree.paint()).len(), 1);
    let moved = tree.children(tree.children(root).unwrap()[0]).unwrap()[0];
    let before = tree.diagnostics();
    tree.update(
        root,
        centered(Widget::from(
            ClipRect::new(labeled_action("plain child")).clip_behavior(Clip::None),
        )),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    assert_eq!(
        tree.diagnostics().layouts,
        before.layouts,
        "clip toggle must not relayout"
    );
    assert_eq!(
        tree.diagnostics().paints,
        before.paints,
        "clip toggle must not re-record pictures"
    );
    assert!(
        world_clips(&tree.paint()).is_empty(),
        "disabled clip records nothing"
    );
    // The layer binding rebuilds around a stable child element.
    assert_eq!(
        tree.children(tree.children(root).unwrap()[0]).unwrap()[0],
        moved
    );
}

#[test]
fn clip_rrect_records_rounded_clip() {
    let radius = CornerRadii::uniform(8.);
    let mut tree = WidgetTree::new();
    tree.mount(centered(Widget::from(ClipRRect::new(
        radius,
        labeled_action("round child"),
    ))))
    .unwrap();
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let expected = Rect::from_origin_size(Offset::new(80., 0.), Size::new(40., 40.));
    let clips = world_clips(&tree.paint());
    assert_eq!(clips.len(), 1, "one clip entry, got {clips:?}");
    match &clips[0] {
        WorldClip::RRect(rrect) => {
            assert_rect_eq(rrect.rect, expected, "rounded clip rect");
            assert_eq!(rrect.radii, radius, "rounded clip radii");
        }
        other => panic!("expected rounded-rect clip, got {other:?}"),
    }
}

#[test]
fn clip_rrect_radius_change_needs_neither_layout_nor_paint() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(centered(Widget::from(ClipRRect::new(
            CornerRadii::uniform(4.),
            labeled_action("round child"),
        ))))
        .unwrap();
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let _ = tree.paint();
    let before = tree.diagnostics();
    tree.update(
        root,
        centered(Widget::from(ClipRRect::new(
            CornerRadii::uniform(12.),
            labeled_action("round child"),
        ))),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    assert_eq!(
        tree.diagnostics().layouts,
        before.layouts,
        "radius change must not relayout"
    );
    assert_eq!(
        tree.diagnostics().paints,
        before.paints,
        "radius change must not re-record pictures"
    );
    match world_clips(&tree.paint()).as_slice() {
        [WorldClip::RRect(rrect)] => assert_eq!(rrect.radii, CornerRadii::uniform(12.)),
        other => panic!("expected one rounded clip, got {other:?}"),
    }
}

#[test]
fn clip_oval_records_oval_clip() {
    let mut tree = WidgetTree::new();
    tree.mount(centered(Widget::from(ClipOval::new(labeled_action(
        "oval child",
    )))))
    .unwrap();
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let expected = Rect::from_origin_size(Offset::new(80., 0.), Size::new(40., 40.));
    let clips = world_clips(&tree.paint());
    assert_eq!(clips.len(), 1, "one clip entry, got {clips:?}");
    match &clips[0] {
        WorldClip::Oval(rect) => assert_rect_eq(*rect, expected, "oval clip rect"),
        other => panic!("expected oval clip, got {other:?}"),
    }
}

#[test]
fn clip_path_records_path_clip() {
    let path = triangle_path();
    let mut tree = WidgetTree::new();
    tree.mount(centered(Widget::from(ClipPath::new(
        path.clone(),
        labeled_action("path child"),
    ))))
    .unwrap();
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let list = tree.paint();
    let clips = world_clips(&list);
    assert_eq!(clips.len(), 1, "one clip entry, got {clips:?}");
    assert!(
        matches!(clips[0], WorldClip::Path),
        "expected path clip, got {:?}",
        clips[0]
    );
    let (recorded, rule) = clip_path_shape(&list).expect("path clip shape");
    // The compositor resolves the local path to world space: the triangle
    // rides the column offset (80,0) with its shape intact.
    assert_eq!(
        recorded.bounds(),
        path.bounds().map(|bounds| Rect::from_origin_size(
            bounds.origin + Offset::new(80., 0.),
            bounds.size
        ))
    );
    assert_eq!(rule, FillRule::NonZero);
}

#[test]
fn nested_clips_balance() {
    let mut tree = WidgetTree::new();
    tree.mount(Widget::from(ClipRect::new(ClipRect::new(labeled_action(
        "nested",
    )))))
    .unwrap();
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let list = tree.paint();
    let clips = world_clips(&list);
    assert_eq!(clips.len(), 2, "nested clips both record, got {clips:?}");
    let pops = list
        .commands()
        .iter()
        .filter(|command| matches!(command, PaintCommand::PopClip))
        .count();
    assert_eq!(pops, 2, "every push balances with a pop");
}

#[test]
fn transformed_child_crossing_clip_boundary_keeps_picture() {
    // Clipping is a raster effect: the translated child's picture is still
    // recorded (no retained culling) while the clip bounds raster output.
    // Hit testing follows layout bounds through the clip node, and
    // semantic bounds stay whole.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(centered(Widget::from(ClipRect::new(Widget::from(
            Transform::translation(Offset::new(30., 0.), labeled_action("shifted child")),
        )))))
        .unwrap();
    let clip = tree.children(root).unwrap()[0];
    let shifted = tree.children(clip).unwrap()[0];
    let moved = tree.children(shifted).unwrap()[0];
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let list = tree.paint();
    let clips = world_clips(&list);
    assert_eq!(clips.len(), 1, "clip still records, got {clips:?}");
    // Child 40x40 at column x 80, shifted +30: paints at (110,0), hanging
    // 10px past the clip's right edge at x 120... the clip node spans
    // (80,0,40,40) while the picture spans (110,0,40,40).
    let expected_clip = Rect::from_origin_size(Offset::new(80., 0.), Size::new(40., 40.));
    match &clips[0] {
        WorldClip::Rect(rect) => assert_rect_eq(*rect, expected_clip, "clip rect"),
        other => panic!("expected plain rect clip, got {other:?}"),
    }
    let expected_child = Rect::from_origin_size(Offset::new(110., 0.), Size::new(40., 40.));
    // Positive target: inside both the translated child and the clip node.
    let hit = tree
        .hit_test(Offset::new(115., 10.))
        .and_then(|render| tree.element_for_render(render));
    assert_eq!(hit, Some(moved));
    // Negative target: inside the translated child but past the clip node's
    // right edge — hit testing follows layout bounds, so the child misses
    // and the hit falls through to the column root.
    let miss = tree
        .hit_test(Offset::new(130., 10.))
        .and_then(|render| tree.element_for_render(render));
    assert_eq!(miss, Some(root));
    tree.update_semantics();
    let semantic = tree
        .semantic_node_for_element(moved)
        .and_then(|id| tree.semantics().node(id))
        .expect("shifted semantics");
    assert_rect_eq(semantic.bounds, expected_child, "shifted semantic bounds");
}

#[test]
fn stack_clip_behavior_clips_overflow() {
    // Stack documents clipping for overflowing children; HardEdge (the
    // default) must bound raster output to the stack bounds.
    let mut tree = WidgetTree::new();
    tree.mount(centered(Widget::from(Stack::new([
        Widget::box_(Size::new(60., 30.), Color::WHITE),
        Widget::from(
            Positioned::new(labeled_action("overhang"))
                .left(-20.)
                .top(5.),
        ),
    ]))))
    .unwrap();
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let clips = world_clips(&tree.paint());
    assert_eq!(clips.len(), 1, "stack overflow clips once, got {clips:?}");
    match &clips[0] {
        WorldClip::Rect(rect) => assert_rect_eq(
            *rect,
            Rect::from_origin_size(Offset::new(70., 0.), Size::new(60., 30.)),
            "stack clip bounds",
        ),
        other => panic!("expected stack rect clip, got {other:?}"),
    }
}

#[test]
fn stack_clip_behavior_none_records_no_clip() {
    let mut tree = WidgetTree::new();
    tree.mount(centered(Widget::from(
        Stack::new([
            Widget::box_(Size::new(60., 30.), Color::WHITE),
            Widget::from(
                Positioned::new(labeled_action("overhang"))
                    .left(-20.)
                    .top(5.),
            ),
        ])
        .clip_behavior(Clip::None),
    )))
    .unwrap();
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    assert!(
        world_clips(&tree.paint()).is_empty(),
        "Clip::None stack records no clip"
    );
}

#[test]
fn opacity_zero_stays_hittable_and_exposed() {
    // Unlike Visibility, Opacity never gates input or semantics: even a
    // fully transparent child hits and stays exposed with whole bounds.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(centered(Widget::from(Opacity::new(
            0.,
            labeled_action("ghost"),
        ))))
        .unwrap();
    let faded = tree.children(root).unwrap()[0];
    let moved = tree.children(faded).unwrap()[0];
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let expected = Rect::from_origin_size(Offset::new(80., 0.), Size::new(40., 40.));
    assert_eq!(opacity_alphas(&tree.paint()), vec![0.]);
    let hit = tree
        .hit_test(Offset::new(90., 10.))
        .and_then(|render| tree.element_for_render(render));
    assert_eq!(hit, Some(moved), "zero alpha still hits");
    tree.update_semantics();
    let semantic = tree
        .semantic_node_for_element(moved)
        .and_then(|id| tree.semantics().node(id))
        .expect("zero-alpha semantics");
    assert_rect_eq(semantic.bounds, expected, "zero-alpha bounds stay whole");
}

#[test]
fn opacity_transitions_carry_alpha_without_repaint_or_layout() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Widget::from(Opacity::new(1., labeled_action("fader"))))
        .unwrap();
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let _ = tree.paint();
    for alpha in [0., 0.5, 1.] {
        let before = tree.diagnostics();
        let before_compositor = tree.compositor_diagnostics().opacity_updates;
        tree.update(
            root,
            Widget::from(Opacity::new(alpha, labeled_action("fader"))),
        )
        .expect("update");
        tree.layout(Constraints::tight(Size::new(200., 200.)))
            .expect("layout");
        assert_eq!(
            tree.diagnostics().layouts,
            before.layouts,
            "alpha {alpha} must not relayout"
        );
        assert_eq!(
            tree.diagnostics().paints,
            before.paints,
            "alpha {alpha} must not re-record"
        );
        assert!(
            tree.compositor_diagnostics().opacity_updates > before_compositor,
            "alpha {alpha} must reach the compositor layer"
        );
        assert_eq!(opacity_alphas(&tree.paint()), vec![alpha]);
    }
}

#[test]
fn opacity_identical_reapply_and_identity_hold() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(centered(Widget::from(Opacity::new(
            0.5,
            labeled_action("fader"),
        ))))
        .unwrap();
    let faded = tree.children(root).unwrap()[0];
    let moved = tree.children(faded).unwrap()[0];
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let _ = tree.paint();
    let before = tree.diagnostics();
    let before_compositor = tree.compositor_diagnostics();
    tree.update(
        root,
        centered(Widget::from(Opacity::new(0.5, labeled_action("fader")))),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let _ = tree.paint();
    // Phase counters must not move (reconciliation bookkeeping may):
    // identical configuration schedules no layout, paint, or composite.
    let after = tree.diagnostics();
    assert_eq!(
        (after.layouts, after.paints, after.mounts, after.unmounts),
        (
            before.layouts,
            before.paints,
            before.mounts,
            before.unmounts
        )
    );
    assert_eq!(
        tree.compositor_diagnostics().opacity_updates,
        before_compositor.opacity_updates
    );
    assert_eq!(tree.children(root).unwrap()[0], faded);
    assert_eq!(tree.children(faded).unwrap()[0], moved);
}

#[test]
fn fade_transition_follows_its_controller() {
    // FadeTransition reads the live controller value: a controller-side
    // change with no widget update still reaches the compositor layer.
    let controller = OpacityController::new();
    let mut tree = WidgetTree::new();
    tree.mount(Widget::from(FadeTransition::new(
        controller.clone(),
        labeled_action("fader"),
    )))
    .unwrap();
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    assert_eq!(opacity_alphas(&tree.paint()), vec![1.]);
    assert!(controller.set_opacity(0.25));
    // Controller-side changes propagate through the compositor tick, not
    // through layout: an explicit change reaches the layer on demand.
    tree.update_compositor(std::time::Instant::now())
        .expect("compositor tick");
    assert_eq!(opacity_alphas(&tree.paint()), vec![0.25]);
}

#[test]
fn clip_builders_match_fluent_construction() {
    // Generated-builder parity for the TypedBuilder clip widgets: default
    // builders equal the constructors, and set builders equal fluent sets.
    let child = || labeled_action("parity");
    assert_eq!(
        ClipRect::builder().child(child()).build(),
        ClipRect::new(child())
    );
    assert_eq!(
        ClipRect::builder()
            .clip_behavior(Clip::AntiAlias)
            .child(child())
            .build(),
        ClipRect::new(child()).clip_behavior(Clip::AntiAlias)
    );
    let radius = CornerRadii::uniform(6.);
    assert_eq!(
        ClipRRect::builder().radius(radius).child(child()).build(),
        ClipRRect::new(radius, child())
    );
    assert_eq!(
        ClipOval::builder().child(child()).build(),
        ClipOval::new(child())
    );
    assert_eq!(
        ClipOval::builder()
            .clip_behavior(Clip::None)
            .child(child())
            .build(),
        ClipOval::new(child()).clip_behavior(Clip::None)
    );
    let path = triangle_path();
    assert_eq!(
        ClipPath::builder()
            .path(path.clone())
            .child(child())
            .build(),
        ClipPath::new(path.clone(), child())
    );
}
