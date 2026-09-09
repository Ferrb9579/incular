//! Retained contracts for `ShaderMask` and `BackdropFilter`: when the
//! shader callback runs, which bounds it receives, what the compositor
//! stages record, and what stays compositor-only.
//!
//! Every case mounts, lays out, and paints first (warming all caches),
//! then updates and re-runs the phases. Raster assertions read the
//! flattened renderer-neutral commands; diagnostics separate layout
//! work (`layouts`), picture recording (`paints`), and compositor-only
//! updates. A counting callback pins exactly when shader resolution
//! runs: never per frame, only at the documented resolve points.

mod common;

use common::*;
use incular_config::Constraints;
use incular_core::{Color, Offset, Rect, Size, Transform as CoreTransform};
use incular_rendering::{BlendMode, Brush, DisplayList, GaussianBlur, PaintCommand, Shader};
use incular_widgets::internal::*;
use incular_widgets::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// A shader callback that records every invocation's bounds and counts
/// calls, returning a constant color so output equality is separable
/// from callback identity.
/// A shader callback with a stable allocation identity: every clone
/// shares one `Rc`, so callback identity survives rebuilds exactly like
/// production callbacks held by their owners.
#[derive(Clone)]
struct Probe {
    calls: Rc<Cell<u32>>,
    seen: Rc<RefCell<Vec<Rect>>>,
    callback: ShaderCallback,
}

impl Probe {
    fn new(color: Color) -> Self {
        let calls = Rc::new(Cell::new(0));
        let seen = Rc::new(RefCell::new(Vec::new()));
        let recorded_calls = calls.clone();
        let recorded_seen = seen.clone();
        let callback = ShaderCallback::new(move |bounds| {
            recorded_calls.set(recorded_calls.get() + 1);
            recorded_seen.borrow_mut().push(bounds);
            Brush::Solid(color)
        });
        Self {
            calls,
            seen,
            callback,
        }
    }

    fn callback(&self) -> ShaderCallback {
        self.callback.clone()
    }
}

fn centered(child: Widget) -> Widget {
    Column::new([child]).into()
}

#[derive(Debug)]
struct MaskStage {
    shader: Shader,
    blend_mode: BlendMode,
    mask_size: Size,
    mask_origin: Offset,
    bounds: Rect,
}

fn mask_stages(list: &DisplayList) -> Vec<MaskStage> {
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
            PaintCommand::PushShaderMask {
                shader,
                blend_mode,
                mask_size,
                mask_transform,
                bounds,
                ..
            } => out.push(MaskStage {
                shader: shader.clone(),
                blend_mode: *blend_mode,
                mask_size: *mask_size,
                mask_origin: mask_transform.translation_offset(),
                bounds: *bounds,
            }),
            _ => {}
        }
    }
    out
}

#[derive(Debug)]
struct BackdropStage {
    sigma: (f32, f32),
    blend_mode: BlendMode,
    enabled: bool,
    bounds: Rect,
}

fn backdrop_stages(list: &DisplayList) -> Vec<BackdropStage> {
    list.commands()
        .iter()
        .filter_map(|command| match command {
            PaintCommand::PushBackdropFilter {
                blur,
                blend_mode,
                enabled,
                bounds,
                ..
            } => Some(BackdropStage {
                sigma: (blur.sigma_x, blur.sigma_y),
                blend_mode: *blend_mode,
                enabled: *enabled,
                bounds: *bounds,
            }),
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

#[test]
fn shader_callback_runs_once_with_local_bounds() {
    // The callback runs at resolve time with local bounds (zero origin,
    // node size). The stage records the resolved shader, the node size,
    // and the world transform at resolve time.
    let probe = Probe::new(Color::WHITE);
    let mut tree = WidgetTree::new();
    tree.mount(centered(Widget::from(ShaderMask::new(
        probe.callback(),
        action(Size::new(40., 40.), Color::WHITE, ActionId(1)),
    ))))
    .unwrap();
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let stages = mask_stages(&tree.paint());
    assert_eq!(probe.calls.get(), 1, "one resolution per paint");
    assert_eq!(
        probe.seen.borrow().as_slice(),
        &[Rect::from_origin_size(Offset::ZERO, Size::new(40., 40.))],
        "callback receives local bounds"
    );
    assert_eq!(stages.len(), 1);
    assert_eq!(stages[0].shader, Brush::Solid(Color::WHITE));
    assert_eq!(stages[0].blend_mode, BlendMode::Modulate);
    assert_eq!(stages[0].mask_size, Size::new(40., 40.));
    assert_eq!(stages[0].mask_origin, Offset::new(80., 0.));
    assert_rect_eq(
        stages[0].bounds,
        Rect::from_origin_size(Offset::new(80., 0.), Size::new(40., 40.)),
        "mask bounds",
    );
}

#[test]
fn closure_converts_into_callback() {
    // A raw closure converts through From<F> into one identity-bearing
    // callback: mounting with it resolves exactly once with local bounds.
    let calls = Rc::new(Cell::new(0));
    let recorded = calls.clone();
    let mut tree = WidgetTree::new();
    tree.mount(centered(Widget::from(ShaderMask::new(
        move |_bounds: Rect| {
            recorded.set(recorded.get() + 1);
            Brush::Solid(Color::BLACK)
        },
        Widget::box_(Size::new(40., 40.), Color::WHITE),
    ))))
    .unwrap();
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let stages = mask_stages(&tree.paint());
    assert_eq!(calls.get(), 1);
    assert_eq!(stages.len(), 1);
    assert_eq!(stages[0].shader, Brush::Solid(Color::BLACK));
}

#[test]
fn identical_reapply_never_reruns_callback() {
    // Rebuilding the identical mask (same callback identity, same blend)
    // schedules nothing and never re-invokes the callback: resolution is
    // not per frame.
    let probe = Probe::new(Color::WHITE);
    let make = || {
        centered(Widget::from(ShaderMask::new(
            probe.callback(),
            action(Size::new(40., 40.), Color::WHITE, ActionId(1)),
        )))
    };
    let mut tree = WidgetTree::new();
    let root = tree.mount(make()).unwrap();
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let _ = tree.paint();
    assert_eq!(probe.calls.get(), 1);
    let before = tree.diagnostics();
    tree.update(root, make()).expect("update");
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let _ = tree.paint();
    assert_eq!(probe.calls.get(), 1, "identical reapply must not resolve");
    let after = tree.diagnostics();
    assert_eq!(
        (after.layouts, after.paints),
        (before.layouts, before.paints)
    );
}

#[test]
fn child_resize_reruns_callback_with_new_bounds() {
    // Bounds-dependent callbacks follow measurement: growing the child
    // relayouts the mask node, repaints it, and re-resolves the shader
    // with the new local bounds.
    let probe = Probe::new(Color::WHITE);
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(centered(Widget::from(ShaderMask::new(
            probe.callback(),
            Widget::box_(Size::new(40., 40.), Color::WHITE),
        ))))
        .unwrap();
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let _ = tree.paint();
    assert_eq!(probe.calls.get(), 1);
    tree.update(
        root,
        centered(Widget::from(ShaderMask::new(
            probe.callback(),
            Widget::box_(Size::new(60., 40.), Color::WHITE),
        ))),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let stages = mask_stages(&tree.paint());
    assert_eq!(probe.calls.get(), 2, "resize must re-resolve once");
    assert_eq!(
        probe.seen.borrow()[1],
        Rect::from_origin_size(Offset::ZERO, Size::new(60., 40.))
    );
    assert_eq!(stages.len(), 1);
    assert_eq!(stages[0].mask_size, Size::new(60., 40.));
}

#[test]
fn new_callback_identity_reruns_despite_equal_output() {
    // Callback equality is allocation identity, not output equality: a
    // distinct callback returning the same brush still re-resolves once,
    // while the recorded stage is unchanged.
    let first = Probe::new(Color::WHITE);
    let second = Probe::new(Color::WHITE);
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(centered(Widget::from(ShaderMask::new(
            first.callback(),
            Widget::box_(Size::new(40., 40.), Color::WHITE),
        ))))
        .unwrap();
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let _ = tree.paint();
    assert_eq!(first.calls.get(), 1);
    tree.update(
        root,
        centered(Widget::from(ShaderMask::new(
            second.callback(),
            Widget::box_(Size::new(40., 40.), Color::WHITE),
        ))),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let stages = mask_stages(&tree.paint());
    assert_eq!(first.calls.get(), 1, "old identity never reruns");
    assert_eq!(second.calls.get(), 1, "new identity resolves once");
    assert_eq!(stages.len(), 1);
    assert_eq!(stages[0].shader, Brush::Solid(Color::WHITE));
}

#[test]
fn blend_change_updates_layer_without_layout_or_paint() {
    let probe = Probe::new(Color::WHITE);
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(centered(Widget::from(ShaderMask::new(
            probe.callback(),
            Widget::box_(Size::new(40., 40.), Color::WHITE),
        ))))
        .unwrap();
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let _ = tree.paint();
    let before = tree.diagnostics();
    tree.update(
        root,
        centered(Widget::from(
            ShaderMask::new(
                probe.callback(),
                Widget::box_(Size::new(40., 40.), Color::WHITE),
            )
            .blend_mode(BlendMode::SrcOver),
        )),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let after = tree.diagnostics();
    assert_eq!(
        (after.layouts, after.paints),
        (before.layouts, before.paints)
    );
    let stages = mask_stages(&tree.paint());
    assert_eq!(stages.len(), 1);
    assert_eq!(stages[0].blend_mode, BlendMode::SrcOver);
}

#[test]
fn ancestor_move_keeps_resolve_time_transform_with_live_bounds() {
    // Placement follows the live transform stack every flatten, but the
    // recorded mask transform is a resolve-time snapshot: moving an
    // ancestor without repainting the mask leaves the snapshot behind
    // while bounds track the move. Both halves are pinned so the
    // boundary cannot drift silently.
    let probe = Probe::new(Color::WHITE);
    let make = |offset: Offset| {
        centered(Widget::from(Transform::translation(
            offset,
            ShaderMask::new(
                probe.callback(),
                Widget::box_(Size::new(40., 40.), Color::WHITE),
            ),
        )))
    };
    let mut tree = WidgetTree::new();
    let root = tree.mount(make(Offset::new(11., 13.))).unwrap();
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let stages = mask_stages(&tree.paint());
    assert_eq!(stages.len(), 1);
    assert_eq!(stages[0].mask_origin, Offset::new(91., 13.));
    assert_eq!(probe.calls.get(), 1);
    tree.update(root, make(Offset::new(20., 0.)))
        .expect("update");
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let stages = mask_stages(&tree.paint());
    assert_eq!(
        probe.calls.get(),
        1,
        "ancestor move must not re-resolve the mask"
    );
    assert_eq!(
        stages[0].mask_origin,
        Offset::new(91., 13.),
        "snapshot stays at resolve time"
    );
    assert_rect_eq(
        stages[0].bounds,
        Rect::from_origin_size(Offset::new(100., 0.), Size::new(40., 40.)),
        "bounds follow the live stack",
    );
}

#[test]
fn mask_under_clip_and_transform_positions_together() {
    // Nested clipping and transforms compose: the clip encloses the mask
    // stage, the mask bounds carry the ancestor transform, and the child
    // stays hittable with whole semantic bounds.
    let probe = Probe::new(Color::WHITE);
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(centered(Widget::from(ClipRect::new(
            Transform::translation(
                Offset::new(10., 5.),
                ShaderMask::new(
                    probe.callback(),
                    action(Size::new(40., 40.), Color::WHITE, ActionId(1))
                        .accessibility_label("masked child"),
                ),
            ),
        ))))
        .unwrap();
    let clip = tree.children(root).unwrap()[0];
    let shifted = tree.children(clip).unwrap()[0];
    let masked = tree.children(shifted).unwrap()[0];
    let moved = tree.children(masked).unwrap()[0];
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let list = tree.paint();
    let stages = mask_stages(&list);
    assert_eq!(stages.len(), 1);
    // Clip node (80,0,40,40); translated mask at (90,5,40,40).
    assert_rect_eq(
        stages[0].bounds,
        Rect::from_origin_size(Offset::new(90., 5.), Size::new(40., 40.)),
        "mask bounds under clip and transform",
    );
    let hit = tree
        .hit_test(Offset::new(95., 10.))
        .and_then(|render| tree.element_for_render(render));
    assert_eq!(hit, Some(moved));
    tree.update_semantics();
    let semantic = tree
        .semantic_node_for_element(moved)
        .and_then(|id| tree.semantics().node(id))
        .expect("masked semantics");
    assert_rect_eq(
        semantic.bounds,
        Rect::from_origin_size(Offset::new(90., 5.), Size::new(40., 40.)),
        "masked semantic bounds",
    );
}

#[test]
fn child_replacement_keeps_callback_and_layer() {
    // Swapping the child for a same-size sibling neither re-invokes the
    // callback nor rebuilds the mask stage: resolution keys on mask
    // configuration and bounds, not child identity.
    let probe = Probe::new(Color::WHITE);
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(centered(Widget::from(ShaderMask::new(
            probe.callback(),
            Widget::box_(Size::new(40., 40.), Color::WHITE),
        ))))
        .unwrap();
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let _ = tree.paint();
    assert_eq!(probe.calls.get(), 1);
    let before = mask_stages(&tree.paint());
    tree.update(
        root,
        centered(Widget::from(ShaderMask::new(
            probe.callback(),
            Widget::box_(Size::new(40., 40.), Color::BLACK),
        ))),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let after = mask_stages(&tree.paint());
    assert_eq!(probe.calls.get(), 1, "child swap must not re-resolve");
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].shader, before[0].shader);
    assert_eq!(after[0].mask_size, before[0].mask_size);
}

#[test]
fn zero_size_mask_culls_raster_but_keeps_semantics() {
    // An empty mask has no pixels to shade: flatten culls the stage and
    // its children, while semantics still exposes the child, mirroring
    // the clip contract.
    let probe = Probe::new(Color::WHITE);
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(centered(Widget::from(ShaderMask::new(
            probe.callback(),
            action(Size::new(0., 0.), Color::WHITE, ActionId(1))
                .accessibility_label("empty mask child"),
        ))))
        .unwrap();
    let masked = tree.children(root).unwrap()[0];
    let moved = tree.children(masked).unwrap()[0];
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    assert!(
        mask_stages(&tree.paint()).is_empty(),
        "empty mask records no stage"
    );
    tree.update_semantics();
    assert!(
        tree.semantic_node_for_element(moved).is_some(),
        "empty mask child stays exposed"
    );
}

#[test]
fn backdrop_filter_bounds_child_with_prior_backdrop() {
    // The backdrop is whatever painted before the stage in paint order;
    // the recorded bounds cover the child only, not the backdrop.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            Column::new([
                Widget::box_(Size::new(60., 30.), Color::WHITE),
                Widget::from(ClipRect::new(BackdropFilter::blur(
                    4.,
                    action(Size::new(40., 40.), Color::WHITE, ActionId(1))
                        .accessibility_label("filtered child"),
                ))),
            ])
            .into(),
        )
        .unwrap();
    let clip = tree.children(root).unwrap()[1];
    let filtered = tree.children(clip).unwrap()[0];
    let moved = tree.children(filtered).unwrap()[0];
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let list = tree.paint();
    let stages = backdrop_stages(&list);
    // The enclosing clip stays active across the filter stage.
    let clip_at = list
        .commands()
        .iter()
        .position(|command| matches!(command, PaintCommand::PushClip { .. }))
        .expect("enclosing clip");
    let filter_at = list
        .commands()
        .iter()
        .position(|command| matches!(command, PaintCommand::PushBackdropFilter { .. }))
        .expect("backdrop stage");
    assert!(clip_at < filter_at, "clip encloses the filter stage");
    assert_eq!(stages.len(), 1);
    assert_eq!(stages[0].sigma, (4., 4.));
    assert_eq!(stages[0].blend_mode, BlendMode::SrcOver);
    assert!(stages[0].enabled);
    // Backdrop box (70,0,60,30); filter node sizes to its 40x40 child at
    // (80,30).
    assert_rect_eq(
        stages[0].bounds,
        Rect::from_origin_size(Offset::new(80., 30.), Size::new(40., 40.)),
        "backdrop bounds cover the child only",
    );
    let backdrop_end = list
        .commands()
        .iter()
        .position(|command| matches!(command, PaintCommand::PushBackdropFilter { .. }))
        .expect("backdrop stage");
    assert!(
        list.commands()[..backdrop_end]
            .iter()
            .any(|command| matches!(
                command,
                PaintCommand::Rect { rect, .. }
                if rect.size == Size::new(60., 30.)
            )),
        "backdrop content paints before the stage"
    );
    let hit = tree
        .hit_test(Offset::new(90., 35.))
        .and_then(|render| tree.element_for_render(render));
    assert_eq!(hit, Some(moved));
    tree.update_semantics();
    let semantic = tree
        .semantic_node_for_element(moved)
        .and_then(|id| tree.semantics().node(id))
        .expect("filtered semantics");
    assert_rect_eq(
        semantic.bounds,
        Rect::from_origin_size(Offset::new(80., 30.), Size::new(40., 40.)),
        "filtered semantic bounds",
    );
}

#[test]
fn backdrop_disabled_and_zero_sigma_pass_through() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(centered(Widget::from(BackdropFilter::blur(
            4.,
            Widget::box_(Size::new(40., 40.), Color::WHITE),
        ))))
        .unwrap();
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    assert_eq!(backdrop_stages(&tree.paint()).len(), 1);
    // Disabling drops the stage while the child keeps painting, with no
    // layout or picture work.
    let before = tree.diagnostics();
    tree.update(
        root,
        centered(Widget::from(
            BackdropFilter::blur(4., Widget::box_(Size::new(40., 40.), Color::WHITE))
                .enabled(false),
        )),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let after = tree.diagnostics();
    assert_eq!(
        (after.layouts, after.paints),
        (before.layouts, before.paints),
        "disabling must not relayout or re-record"
    );
    let list = tree.paint();
    assert!(
        backdrop_stages(&list).is_empty(),
        "disabled filter records no stage"
    );
    assert!(
        list.commands().iter().any(|command| matches!(
            command,
            PaintCommand::Rect { rect, .. } if rect.size == Size::new(40., 40.)
        )),
        "disabled filter child still paints"
    );
    // Zero sigma is an equivalent passthrough at flatten time.
    tree.update(
        root,
        centered(Widget::from(BackdropFilter::blur(
            0.,
            Widget::box_(Size::new(40., 40.), Color::WHITE),
        ))),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    assert!(backdrop_stages(&tree.paint()).is_empty());
}

#[test]
fn backdrop_updates_are_compositor_only() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(centered(Widget::from(BackdropFilter::blur(
            4.,
            Widget::box_(Size::new(40., 40.), Color::WHITE),
        ))))
        .unwrap();
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let _ = tree.paint();
    let before = tree.diagnostics();
    tree.update(
        root,
        centered(Widget::from(
            BackdropFilter::blur(8., Widget::box_(Size::new(40., 40.), Color::WHITE))
                .blend_mode(BlendMode::SrcOver),
        )),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let after = tree.diagnostics();
    assert_eq!(
        (after.layouts, after.paints),
        (before.layouts, before.paints),
        "filter updates must not relayout or re-record"
    );
    let stages = backdrop_stages(&tree.paint());
    assert_eq!(stages.len(), 1);
    assert_eq!(stages[0].sigma, (8., 8.));
}

#[test]
fn backdrop_constructors_alias_split_sigmas() {
    // The asymmetric constructor and the filter setter retain split
    // sigmas exactly like direct construction, staying compositor-only.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(centered(Widget::from(BackdropFilter::asymmetric(
            3.,
            7.,
            Widget::box_(Size::new(40., 40.), Color::WHITE),
        ))))
        .unwrap();
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let stages = backdrop_stages(&tree.paint());
    assert_eq!(stages.len(), 1);
    assert_eq!(stages[0].sigma, (3., 7.));
    let before = tree.diagnostics();
    tree.update(
        root,
        centered(Widget::from(
            BackdropFilter::blur(1., Widget::box_(Size::new(40., 40.), Color::WHITE))
                .filter(GaussianBlur::new(5., 6.)),
        )),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let after = tree.diagnostics();
    assert_eq!(
        (after.layouts, after.paints),
        (before.layouts, before.paints)
    );
    let stages = backdrop_stages(&tree.paint());
    assert_eq!(stages.len(), 1);
    assert_eq!(stages[0].sigma, (5., 6.));
}

#[test]
fn backdrop_child_identity_and_identical_reapply() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(centered(Widget::from(BackdropFilter::blur(
            4.,
            Widget::box_(Size::new(40., 40.), Color::WHITE),
        ))))
        .unwrap();
    let filtered = tree.children(root).unwrap()[0];
    let moved = tree.children(filtered).unwrap()[0];
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let _ = tree.paint();
    let before = tree.diagnostics();
    tree.update(
        root,
        centered(Widget::from(BackdropFilter::blur(
            4.,
            Widget::box_(Size::new(40., 40.), Color::WHITE),
        ))),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let _ = tree.paint();
    let after = tree.diagnostics();
    assert_eq!(
        (after.layouts, after.paints),
        (before.layouts, before.paints)
    );
    assert_eq!(tree.children(root).unwrap()[0], filtered);
    assert_eq!(tree.children(filtered).unwrap()[0], moved);
}
