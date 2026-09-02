use incular_config::{TextDirection, TransientRole};
use incular_core::{Offset, Rect, Size};
use incular_widgets::{
    TransientAlignment, TransientDismissPolicy, TransientDismissReason, TransientPlacement,
    TransientPlacementInput, TransientSide, place_transient,
};

fn place(
    anchor: Rect,
    desired: Size,
    available: Rect,
    direction: TextDirection,
    placement: TransientPlacement,
) -> incular_widgets::TransientPlacementResult {
    place_transient(TransientPlacementInput {
        anchor_rect: anchor,
        desired_size: desired,
        available_rect: available,
        role: TransientRole::Menu,
        text_direction: direction,
        placement,
    })
}

#[test]
fn placement_flips_bottom_to_top_when_only_top_fits() {
    let result = place(
        Rect::from_origin_size(Offset::new(80.0, 170.0), Size::new(40.0, 20.0)),
        Size::new(100.0, 80.0),
        Rect::from_origin_size(Offset::ZERO, Size::new(300.0, 200.0)),
        TextDirection::Ltr,
        TransientPlacement::new().side(TransientSide::Bottom),
    );
    assert_eq!(result.side, TransientSide::Top);
    assert!(result.flipped);
    assert_eq!(result.rect.origin.y, 90.0);
    assert_eq!(result.rect.size, Size::new(100.0, 80.0));
}

#[test]
fn placement_flips_right_to_left_when_only_left_fits() {
    let result = place(
        Rect::from_origin_size(Offset::new(260.0, 70.0), Size::new(30.0, 30.0)),
        Size::new(120.0, 80.0),
        Rect::from_origin_size(Offset::ZERO, Size::new(300.0, 200.0)),
        TextDirection::Ltr,
        TransientPlacement::new().side(TransientSide::Right),
    );
    assert_eq!(result.side, TransientSide::Left);
    assert!(result.flipped);
    assert_eq!(result.rect.origin.x, 140.0);
}

#[test]
fn all_viewport_corners_shift_cross_axis_inside_safe_bounds() {
    let available = Rect::from_origin_size(Offset::ZERO, Size::new(200.0, 160.0));
    for anchor in [
        Offset::new(0.0, 0.0),
        Offset::new(190.0, 0.0),
        Offset::new(0.0, 150.0),
        Offset::new(190.0, 150.0),
    ] {
        let result = place(
            Rect::from_origin_size(anchor, Size::new(10.0, 10.0)),
            Size::new(90.0, 60.0),
            available,
            TextDirection::Ltr,
            TransientPlacement::default(),
        );
        assert!(result.rect.origin.x >= 8.0);
        assert!(result.rect.origin.y >= 8.0);
        assert!(result.rect.origin.x + result.rect.size.width <= 192.0);
        assert!(result.rect.origin.y + result.rect.size.height <= 152.0);
    }
}

#[test]
fn huge_popup_is_constrained_to_available_safe_area() {
    let result = place(
        Rect::from_origin_size(Offset::new(100.0, 80.0), Size::new(20.0, 20.0)),
        Size::new(800.0, 600.0),
        Rect::from_origin_size(Offset::ZERO, Size::new(320.0, 240.0)),
        TextDirection::Ltr,
        TransientPlacement::default(),
    );
    assert!(result.constrained);
    assert_eq!(result.rect.origin, Offset::new(8.0, 8.0));
    assert_eq!(result.rect.size, Size::new(304.0, 224.0));
}

#[test]
fn submenu_prefers_outward_text_direction_and_flips_at_edge() {
    let available = Rect::from_origin_size(Offset::ZERO, Size::new(400.0, 240.0));
    let submenu = TransientPlacement::new()
        .submenu(true)
        .alignment(TransientAlignment::Start);
    let ltr = place(
        Rect::from_origin_size(Offset::new(100.0, 50.0), Size::new(80.0, 30.0)),
        Size::new(120.0, 100.0),
        available,
        TextDirection::Ltr,
        submenu,
    );
    let rtl = place(
        Rect::from_origin_size(Offset::new(220.0, 50.0), Size::new(80.0, 30.0)),
        Size::new(120.0, 100.0),
        available,
        TextDirection::Rtl,
        submenu,
    );
    assert_eq!(ltr.side, TransientSide::Right);
    assert_eq!(rtl.side, TransientSide::Left);

    let edge = place(
        Rect::from_origin_size(Offset::new(350.0, 50.0), Size::new(40.0, 30.0)),
        Size::new(120.0, 100.0),
        available,
        TextDirection::Ltr,
        submenu,
    );
    assert_eq!(edge.side, TransientSide::Left);
    assert!(edge.flipped);
}

#[test]
fn identical_inputs_are_bitwise_deterministic_at_the_value_level() {
    let input = TransientPlacementInput {
        anchor_rect: Rect::from_origin_size(Offset::new(73.25, 91.5), Size::new(31.0, 17.0)),
        desired_size: Size::new(117.75, 88.25),
        available_rect: Rect::from_origin_size(Offset::new(-20.0, -10.0), Size::new(320.0, 240.0)),
        role: TransientRole::Popover,
        text_direction: TextDirection::Rtl,
        placement: TransientPlacement::default()
            .alignment(TransientAlignment::End)
            .alignment_offset(Offset::new(3.5, -2.25)),
    };
    let first = place_transient(input);
    for _ in 0..64 {
        assert_eq!(place_transient(input), first);
    }
}

#[test]
fn equal_primary_overflow_keeps_the_preferred_side() {
    let result = place(
        Rect::from_origin_size(Offset::new(90.0, 90.0), Size::new(20.0, 20.0)),
        Size::new(80.0, 200.0),
        Rect::from_origin_size(Offset::ZERO, Size::new(200.0, 200.0)),
        TextDirection::Ltr,
        TransientPlacement::new().side(TransientSide::Bottom),
    );
    assert_eq!(result.side, TransientSide::Bottom);
    assert!(!result.flipped);
}

#[test]
fn malformed_geometry_is_normalized_to_a_finite_bounded_result() {
    let result = place(
        Rect {
            origin: Offset::new(f32::NAN, f32::INFINITY),
            size: Size {
                width: -40.0,
                height: f32::NEG_INFINITY,
            },
        },
        Size {
            width: f32::NAN,
            height: -10.0,
        },
        Rect {
            origin: Offset::new(f32::NEG_INFINITY, 5.0),
            size: Size {
                width: f32::INFINITY,
                height: -20.0,
            },
        },
        TextDirection::Ltr,
        TransientPlacement::default().alignment_offset(Offset::new(f32::NAN, f32::INFINITY)),
    );
    assert!(result.rect.origin.x.is_finite());
    assert!(result.rect.origin.y.is_finite());
    assert!(result.rect.size.width.is_finite());
    assert!(result.rect.size.height.is_finite());
    assert!(result.rect.size.width >= 0.0);
    assert!(result.rect.size.height >= 0.0);
}

#[test]
fn tooltip_policy_never_dismisses_for_pointer_escape_or_logical_focus() {
    let policy = TransientDismissPolicy::for_role(TransientRole::Tooltip);
    assert!(!policy.allows(TransientDismissReason::OutsidePointer));
    assert!(!policy.allows(TransientDismissReason::Escape));
    assert!(!policy.allows(TransientDismissReason::FocusLost));
    assert!(policy.allows(TransientDismissReason::ParentDeactivated));
}
