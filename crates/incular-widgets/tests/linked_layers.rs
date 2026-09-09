//! Geometry and lifecycle contract for linked layers: leaders publish,
//! followers resolve, and hidden followers stay hidden.
//!
//! Every case mounts, lays out, and paints first, then updates and
//! re-runs the phases. Painted geometry comes from folding the full
//! compositor transforms in the flattened display list; hits must land
//! inside the new location and miss the old one; semantic bounds must
//! follow; element/render identity must hold; phase counters must show
//! the documented invalidation (follower/link updates are
//! compositor-only). Unequal 60x30 leader and 20x10 follower fixtures
//! keep anchor mistakes observable. Independently derived coordinates use
//! the documented relationship (leader origin + leader anchor + offset -
//! follower anchor, with anchors and offsets transformed in leader space
//! under scale/rotation) computed here with plain arithmetic, never by
//! calling the resolver under test.

mod common;

use common::*;
use incular_config::{Alignment, Constraints};
use incular_core::{Color, Offset, Rect, Size, Transform as CoreTransform};
use incular_rendering::{DisplayList, LayerLink, PaintCommand};
use incular_widgets::internal::*;
use incular_widgets::*;

const LEADER: (f32, f32) = (60., 30.);
const FOLLOWER: (f32, f32) = (20., 10.);

fn leader_box(link: &LayerLink) -> Widget {
    Widget::from(CompositedTransformTarget::new(
        link.clone(),
        Widget::box_(Size::new(LEADER.0, LEADER.1), Color::WHITE),
    ))
}

fn follower_button(link: &LayerLink) -> Widget {
    Widget::from(
        CompositedTransformFollower::new(
            link.clone(),
            action(Size::new(FOLLOWER.0, FOLLOWER.1), Color::WHITE, ActionId(1))
                .accessibility_label("linked child"),
        )
        .offset(Offset::new(5., 7.)),
    )
}

/// World-space painted rects: fold full compositor transforms (not just
/// translations) so scaled and rotated placements assert exactly.
fn painted_rects(list: &DisplayList) -> Vec<Rect> {
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
            PaintCommand::Rect { rect, .. } => {
                out.push(transforms.last().unwrap().transform_rect_bbox(*rect));
            }
            PaintCommand::RRect { rrect, .. } => {
                out.push(transforms.last().unwrap().transform_rect_bbox(rrect.rect));
            }
            _ => {}
        }
    }
    out
}

fn approx_eq(actual: f32, expected: f32, what: &str) {
    assert!(
        (actual - expected).abs() < 0.5,
        "{what}: expected {expected}, got {actual}"
    );
}

fn assert_rect_eq(actual: Rect, expected: Rect, what: &str) {
    approx_eq(actual.origin.x, expected.origin.x, &format!("{what} x"));
    approx_eq(actual.origin.y, expected.origin.y, &format!("{what} y"));
    approx_eq(
        actual.size.width,
        expected.size.width,
        &format!("{what} width"),
    );
    approx_eq(
        actual.size.height,
        expected.size.height,
        &format!("{what} height"),
    );
}

#[test]
fn linked_translation_places_hits_and_semantics_together() {
    let link = LayerLink::new();
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Column::new([leader_box(&link), follower_button(&link)]).into())
        .unwrap();
    let follower = tree.children(root).unwrap()[1];
    let moved = tree.children(follower).unwrap()[0];
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let list = tree.paint();
    let rects = painted_rects(&list);
    assert_rect_eq(
        rects[0],
        Rect::from_origin_size(Offset::new(70., 0.), Size::new(60., 30.)),
        "leader paint",
    );
    // Leader origin (70,0) + top-left anchors + offset (5,7).
    let expected = Rect::from_origin_size(Offset::new(75., 7.), Size::new(20., 10.));
    assert_rect_eq(rects[1], expected, "follower paint");
    let hit = tree
        .hit_test(Offset::new(80., 10.))
        .and_then(|render| tree.element_for_render(render));
    assert_eq!(hit, Some(moved));
    assert!(
        tree.hit_test(Offset::new(10., 42.)).is_none()
            || tree
                .hit_test(Offset::new(10., 42.))
                .and_then(|render| tree.element_for_render(render))
                != Some(moved)
    );
    tree.update_semantics();
    let semantic = tree
        .semantic_node_for_element(moved)
        .and_then(|id| tree.semantics().node(id))
        .expect("linked semantics");
    assert_rect_eq(semantic.bounds, expected, "follower semantics");
}

#[test]
fn leader_translation_moves_follower_without_layout_or_paint() {
    let link = LayerLink::new();
    let make = |offset: Offset| incular_widgets::Transform::translation(offset, leader_box(&link));
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Column::new([make(Offset::ZERO).into(), follower_button(&link)]).into())
        .unwrap();
    let follower = tree.children(root).unwrap()[1];
    let moved = tree.children(follower).unwrap()[0];
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let _ = tree.paint();
    let before = tree.diagnostics();
    tree.update(
        root,
        Column::new([make(Offset::new(11., 13.)).into(), follower_button(&link)]).into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    assert_eq!(tree.children(root).unwrap()[1], follower);
    assert_eq!(tree.children(follower).unwrap()[0], moved);
    let after = tree.diagnostics();
    assert_eq!(
        after.layouts, before.layouts,
        "leader move must not relayout"
    );
    assert_eq!(after.paints, before.paints, "leader move must not repaint");
    // Leader origin moved (70,0) -> (81,13): follower follows by the same
    // delta to (86,20).
    let expected = Rect::from_origin_size(Offset::new(86., 20.), Size::new(20., 10.));
    assert_rect_eq(
        *painted_rects(&tree.paint()).last().expect("follower rect"),
        expected,
        "moved follower paint",
    );
    let hit = tree
        .hit_test(Offset::new(90., 22.))
        .and_then(|render| tree.element_for_render(render));
    assert_eq!(hit, Some(moved));
    tree.update_semantics();
    let semantic = tree
        .semantic_node_for_element(moved)
        .and_then(|id| tree.semantics().node(id))
        .expect("moved semantics");
    assert_rect_eq(semantic.bounds, expected, "moved semantics");
}

#[test]
fn leader_scale_resolves_anchors_in_leader_space() {
    let link = LayerLink::new();
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            Column::new([
                incular_widgets::Transform::scale(2., leader_box(&link)).into(),
                follower_button(&link),
            ])
            .into(),
        )
        .unwrap();
    let follower = tree.children(root).unwrap()[1];
    let moved = tree.children(follower).unwrap()[0];
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    // Leader box lays out at (70,0) size 60x30; scale 2 about its center
    // (100,15) paints it at (40,-15) size 120x60. Anchor and offset live
    // in leader space: target (40,-15) + ((100,15)+2*((5,7)-(100,15)) -
    // ((100,15)+2*((0,0)-(100,15)))) = (40,-15)+(10,14) = (50,-1).
    let expected = Rect::from_origin_size(Offset::new(50., -1.), Size::new(20., 10.));
    assert_rect_eq(
        *painted_rects(&tree.paint()).last().expect("follower rect"),
        expected,
        "scaled follower paint",
    );
    let hit = tree
        .hit_test(Offset::new(55., 2.))
        .and_then(|render| tree.element_for_render(render));
    assert_eq!(hit, Some(moved));
    tree.update_semantics();
    let semantic = tree
        .semantic_node_for_element(moved)
        .and_then(|id| tree.semantics().node(id))
        .expect("scaled semantics");
    assert_rect_eq(semantic.bounds, expected, "scaled semantics");
}

#[test]
fn follower_offset_and_center_anchors_reposition() {
    let link = LayerLink::new();
    let make = |offset: Offset| {
        CompositedTransformFollower::new(
            link.clone(),
            action(Size::new(FOLLOWER.0, FOLLOWER.1), Color::WHITE, ActionId(1))
                .accessibility_label("linked child"),
        )
        .offset(offset)
        .target_anchor(Alignment::CENTER)
        .follower_anchor(Alignment::CENTER)
    };
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Column::new([leader_box(&link), Widget::from(make(Offset::ZERO))]).into())
        .unwrap();
    let follower = tree.children(root).unwrap()[1];
    let moved = tree.children(follower).unwrap()[0];
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    // Leader center (100,15); follower center lands there, so its
    // 20x10 box starts at (90,10).
    let expected = Rect::from_origin_size(Offset::new(90., 10.), Size::new(20., 10.));
    assert_rect_eq(
        *painted_rects(&tree.paint()).last().expect("follower rect"),
        expected,
        "centered follower paint",
    );
    let before = tree.diagnostics();
    tree.update(
        root,
        Column::new([leader_box(&link), Widget::from(make(Offset::new(4., -6.)))]).into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    assert_eq!(tree.children(root).unwrap()[1], follower);
    assert_eq!(tree.children(follower).unwrap()[0], moved);
    let after = tree.diagnostics();
    assert_eq!(
        after.layouts, before.layouts,
        "anchor update must not relayout"
    );
    assert_eq!(
        after.paints, before.paints,
        "anchor update must not repaint"
    );
    let expected = Rect::from_origin_size(Offset::new(94., 4.), Size::new(20., 10.));
    assert_rect_eq(
        *painted_rects(&tree.paint()).last().expect("follower rect"),
        expected,
        "offset follower paint",
    );
    let hit = tree
        .hit_test(Offset::new(100., 8.))
        .and_then(|render| tree.element_for_render(render));
    assert_eq!(hit, Some(moved));
    tree.update_semantics();
    let semantic = tree
        .semantic_node_for_element(moved)
        .and_then(|id| tree.semantics().node(id))
        .expect("offset semantics");
    assert_rect_eq(semantic.bounds, expected, "offset semantics");
}

#[test]
fn layer_anchor_setters_match_alignment_setters() {
    use incular_rendering::LayerAnchor;
    let link = LayerLink::new();
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            Column::new([
                leader_box(&link),
                Widget::from(
                    CompositedTransformFollower::new(
                        link.clone(),
                        action(Size::new(FOLLOWER.0, FOLLOWER.1), Color::WHITE, ActionId(1))
                            .accessibility_label("linked child"),
                    )
                    .target_layer_anchor(LayerAnchor::new(1., 1.))
                    .follower_layer_anchor(LayerAnchor::new(1., 1.)),
                ),
            ])
            .into(),
        )
        .unwrap();
    let follower = tree.children(root).unwrap()[1];
    let moved = tree.children(follower).unwrap()[0];
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    // Bottom-right to bottom-right: leader corner (130,30), follower
    // corner lands there, so its 20x10 box starts at (110,20).
    let expected = Rect::from_origin_size(Offset::new(110., 20.), Size::new(20., 10.));
    assert_rect_eq(
        *painted_rects(&tree.paint()).last().expect("follower rect"),
        expected,
        "layer-anchor follower paint",
    );
    let hit = tree
        .hit_test(Offset::new(115., 22.))
        .and_then(|render| tree.element_for_render(render));
    assert_eq!(hit, Some(moved));
    tree.update_semantics();
    let semantic = tree
        .semantic_node_for_element(moved)
        .and_then(|id| tree.semantics().node(id))
        .expect("layer-anchor semantics");
    assert_rect_eq(semantic.bounds, expected, "layer-anchor semantics");
}

#[test]
fn follower_beneath_transformed_ancestor_follows_paint() {
    let link = LayerLink::new();
    let mut tree = WidgetTree::new();
    // Short leader so the wrapper layout box overlaps the painted
    // content: hits must pass the ancestor bounds check before the
    // follower delta applies, exactly like every transformed ancestor.
    let short_leader = Widget::from(CompositedTransformTarget::new(
        link.clone(),
        Widget::box_(Size::new(60., 10.), Color::WHITE),
    ));
    let root = tree
        .mount(
            Column::new([
                short_leader,
                incular_widgets::Transform::scale(2., follower_button(&link)).into(),
            ])
            .into(),
        )
        .unwrap();
    let transform = tree.children(root).unwrap()[1];
    let follower = tree.children(transform).unwrap()[0];
    let moved = tree.children(follower).unwrap()[0];
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    // Leader lays out at (70,0); follower resolves (70,0)+(5,7) in its
    // own space, then its scale-2 ancestor paints the 20x10 box doubled.
    // Wrapper layout (90,10,20,10) overlaps the painted content, so the
    // button stays hittable through both transforms.
    let painted = painted_rects(&tree.paint());
    let expected = Rect::from_origin_size(Offset::new(75., 7.), Size::new(40., 20.));
    assert_rect_eq(
        *painted.last().expect("follower rect"),
        expected,
        "ancestor-scaled follower paint",
    );
    let hit = tree
        .hit_test(Offset::new(100., 20.))
        .and_then(|render| tree.element_for_render(render));
    assert_eq!(hit, Some(moved));
    assert!(
        tree.hit_test(Offset::new(10., 100.)).is_none()
            || tree
                .hit_test(Offset::new(10., 100.))
                .and_then(|render| tree.element_for_render(render))
                != Some(moved)
    );
    tree.update_semantics();
    let semantic = tree
        .semantic_node_for_element(moved)
        .and_then(|id| tree.semantics().node(id))
        .expect("ancestor-scaled semantics");
    assert_rect_eq(semantic.bounds, expected, "ancestor-scaled semantics");
}

#[test]
fn unlink_relink_follows_show_policy_and_new_leader() {
    let link = LayerLink::new();
    let make = |leader: bool| {
        let mut children = Vec::new();
        if leader {
            children.push(leader_box(&link));
        }
        children.push(follower_button(&link));
        Column::new(children).into()
    };
    let mut tree = WidgetTree::new();
    let root = tree.mount(make(true)).unwrap();
    let follower = tree.children(root).unwrap()[1];
    let moved = tree.children(follower).unwrap()[0];
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let _ = tree.paint();
    assert!(link.is_linked());
    // Removing the leader releases the link: the follower falls back to
    // its layout position instead of tracking a ghost.
    tree.update(root, make(false)).expect("update");
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    assert_eq!(tree.children(root).unwrap()[0], follower);
    assert!(!link.is_linked());
    let expected = Rect::from_origin_size(Offset::new(90., 0.), Size::new(20., 10.));
    assert_rect_eq(
        *painted_rects(&tree.paint()).last().expect("follower rect"),
        expected,
        "unlinked follower paint",
    );
    let hit = tree
        .hit_test(Offset::new(95., 5.))
        .and_then(|render| tree.element_for_render(render));
    assert_eq!(hit, Some(moved));
    // A replacement leader on the same link takes over: first publication
    // wins only while data is live, never across removal.
    tree.update(
        root,
        Column::new([
            Widget::box_(Size::new(10., 10.), Color::BLACK),
            leader_box(&link),
            follower_button(&link),
        ])
        .into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    // Publication happens at flatten time: paint first, then the link
    // must resolve to the replacement leader.
    let _ = tree.paint();
    assert!(link.is_linked());
    // New leader lays out below the 10px box: Column centers 60-wide in
    // 200 -> x=70; y=10. Follower: (70,10)+(5,7).
    let expected = Rect::from_origin_size(Offset::new(75., 17.), Size::new(20., 10.));
    assert_rect_eq(
        *painted_rects(&tree.paint()).last().expect("follower rect"),
        expected,
        "relinked follower paint",
    );
    let hit = tree
        .hit_test(Offset::new(80., 20.))
        .and_then(|render| tree.element_for_render(render));
    assert_eq!(hit, Some(moved));
}

#[test]
fn unlinked_hidden_follower_is_not_interactive_or_exposed() {
    let link = LayerLink::new();
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            Column::new([Widget::from(
                CompositedTransformFollower::new(
                    link.clone(),
                    action(Size::new(FOLLOWER.0, FOLLOWER.1), Color::WHITE, ActionId(1))
                        .accessibility_label("linked child"),
                )
                .show_when_unlinked(false),
            )])
            .into(),
        )
        .unwrap();
    let follower = tree.children(root).unwrap()[0];
    let _moved = tree.children(follower).unwrap()[0];
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    assert!(!link.is_linked());
    assert!(painted_rects(&tree.paint()).is_empty());
    assert!(
        tree.hit_test(Offset::new(95., 5.)).is_none()
            || tree
                .hit_test(Offset::new(95., 5.))
                .and_then(|render| tree.element_for_render(render))
                != Some(follower)
    );
    tree.update_semantics();
    assert!(
        !tree.semantics_debug_dump().contains("linked child"),
        "hidden follower must not be semantically exposed"
    );
}

#[test]
fn unlinked_shown_follower_keeps_layout_placement() {
    let link = LayerLink::new();
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Column::new([follower_button(&link)]).into())
        .unwrap();
    let follower = tree.children(root).unwrap()[0];
    let moved = tree.children(follower).unwrap()[0];
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    assert!(!link.is_linked());
    let expected = Rect::from_origin_size(Offset::new(90., 0.), Size::new(20., 10.));
    assert_rect_eq(
        *painted_rects(&tree.paint()).last().expect("follower rect"),
        expected,
        "unlinked shown follower paint",
    );
    let hit = tree
        .hit_test(Offset::new(95., 5.))
        .and_then(|render| tree.element_for_render(render));
    assert_eq!(hit, Some(moved));
    tree.update_semantics();
    assert!(
        tree.semantics_debug_dump().contains("linked child"),
        "shown follower keeps semantics"
    );
}

#[test]
fn two_followers_share_one_leader() {
    let link = LayerLink::new();
    let mut tree = WidgetTree::new();
    tree.mount(
        Column::new([
            leader_box(&link),
            follower_button(&link),
            follower_button(&link),
        ])
        .into(),
    )
    .unwrap();
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let rects = painted_rects(&tree.paint());
    assert_eq!(rects.len(), 3);
    assert_rect_eq(
        rects[0],
        Rect::from_origin_size(Offset::new(70., 0.), Size::new(60., 30.)),
        "leader",
    );
    assert_rect_eq(
        rects[1],
        Rect::from_origin_size(Offset::new(75., 7.), Size::new(20., 10.)),
        "first follower",
    );
    assert_rect_eq(
        rects[2],
        Rect::from_origin_size(Offset::new(75., 7.), Size::new(20., 10.)),
        "second follower",
    );
}

#[test]
fn replacing_link_moves_follower_to_new_leader() {
    let first = LayerLink::new();
    let second = LayerLink::new();
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            Column::new([
                leader_box(&first),
                leader_box(&second),
                follower_button(&first),
            ])
            .into(),
        )
        .unwrap();
    let follower = tree.children(root).unwrap()[2];
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let _ = tree.paint();
    tree.update(
        root,
        Column::new([
            leader_box(&first),
            leader_box(&second),
            follower_button(&second),
        ])
        .into(),
    )
    .expect("update");
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    assert_eq!(tree.children(root).unwrap()[2], follower);
    // Second leader sits below the first: y=30, x centered at 70.
    let expected = Rect::from_origin_size(Offset::new(75., 37.), Size::new(20., 10.));
    assert_rect_eq(
        *painted_rects(&tree.paint()).last().expect("follower rect"),
        expected,
        "relinked follower paint",
    );
}

#[test]
fn identical_reapply_schedules_no_phases() {
    let link = LayerLink::new();
    let make = || Column::new([leader_box(&link), follower_button(&link)]).into();
    let mut tree = WidgetTree::new();
    let root = tree.mount(make()).unwrap();
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let _ = tree.paint();
    let before = tree.diagnostics();
    tree.update(root, make()).expect("update");
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let _ = tree.paint();
    let after = tree.diagnostics();
    assert_eq!(after.layouts, before.layouts);
    assert_eq!(after.paints, before.paints);
    assert_eq!(after.composites, before.composites);
    assert!(after.identical_child_bailouts > before.identical_child_bailouts);
}

#[test]
fn follower_before_leader_sees_unlinked() {
    // Paint order decides linkage: a follower flattened before any leader
    // publication observes the unlinked state, even with a live leader
    // later in the same tree.
    let link = LayerLink::new();
    let mut tree = WidgetTree::new();
    tree.mount(
        Column::new([
            Widget::from(
                CompositedTransformFollower::new(
                    link.clone(),
                    Widget::box_(Size::new(20., 10.), Color::WHITE),
                )
                .show_when_unlinked(false),
            ),
            leader_box(&link),
        ])
        .into(),
    )
    .unwrap();
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    assert!(painted_rects(&tree.paint()).iter().all(|rect| {
        !(rect.size.width == 20.
            && rect.size.height == 10.
            && rect.origin.x == 75.
            && rect.origin.y == 7.)
    }));
}

#[test]
fn multiple_leaders_first_painted_wins() {
    // The compositor keeps the first publication per flatten; a later
    // leader on the same link never moves an already-painted follower.
    let link = LayerLink::new();
    let mut tree = WidgetTree::new();
    tree.mount(Column::new([leader_box(&link), leader_box(&link), follower_button(&link)]).into())
        .unwrap();
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    assert_rect_eq(
        *painted_rects(&tree.paint()).last().expect("follower rect"),
        Rect::from_origin_size(Offset::new(75., 7.), Size::new(20., 10.)),
        "follower tracks the first leader",
    );
}

#[test]
fn singular_ancestor_culls_everywhere() {
    // A singular ancestor chain admits no inverse: the compositor culls,
    // and hit testing plus semantics agree that nothing is observable.
    let link = LayerLink::new();
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Widget::from(incular_widgets::Transform::scale(
            0.,
            Column::new([leader_box(&link), follower_button(&link)]),
        )))
        .unwrap();
    let follower = tree.children(root).unwrap()[0];
    tree.layout(Constraints::tight(Size::new(200., 200.)))
        .expect("layout");
    let list = tree.paint();
    assert!(
        !painted_rects(&list).iter().any(|rect| {
            rect.size.width == 20.
                && rect.size.height == 10.
                && (rect.origin.x - 75.).abs() < 0.5
                && (rect.origin.y - 7.).abs() < 0.5
        }),
        "singular follower must not paint at its resolved location"
    );
    assert!(
        tree.hit_test(Offset::new(75., 7.)).is_none()
            || tree
                .hit_test(Offset::new(75., 7.))
                .and_then(|render| tree.element_for_render(render))
                != Some(follower)
    );
    tree.update_semantics();
    assert!(
        !tree.semantics_debug_dump().contains("linked child"),
        "singular follower must not be semantically exposed"
    );
}
