//! AppBar toolbar horizontal composition honesty.
//!
//! The title gap, leading placeholder, centering, flexible background, and
//! elevation shadow must all reach retained geometry, paint, hit testing, or
//! semantics. Clocks are unnecessary here; layout plus semantic snapshots
//! decide everything deterministically.

use incular_config::Constraints;
use incular_core::{Color, Size};
use incular_material::AppBar;
use incular_runtime::Runtime;
use incular_semantics::Role;
use incular_widgets::{Semantics, Widget};

fn marker(label: &str, size: Size) -> Widget {
    Semantics::new(Widget::box_(size, Color::WHITE))
        .role(Role::Group)
        .label(label)
        .into()
}

fn mount_bar(bar: AppBar) -> Runtime {
    let mut runtime = Runtime::new(bar.into()).expect("mount");
    runtime
        .run_frame(Constraints::tight(Size::new(200., 40.)))
        .expect("frame");
    runtime
}

fn bounds(runtime: &Runtime, label: &str) -> incular_core::Rect {
    runtime
        .tree()
        .semantics()
        .iter()
        .find_map(|(_, node)| (node.label.as_deref() == Some(label)).then_some(node.bounds))
        .expect("labeled slot")
}

#[test]
fn title_spacing_separates_title_from_leading() {
    // Toolbar content starts after the 16px outer padding. The title must
    // begin one leading width plus the configured gap later.
    for (spacing, title_x) in [(None, 42.), (Some(0.), 26.), (Some(32.), 58.)] {
        let mut bar = AppBar::new(marker("Title", Size::new(40., 20.))).toolbar_height(40.);
        if let Some(spacing) = spacing {
            bar = bar.title_spacing(spacing);
        }
        let runtime = mount_bar(bar.leading(marker("Lead", Size::new(10., 20.))));
        let title = bounds(&runtime, "Title");
        let lead = bounds(&runtime, "Lead");
        assert_eq!(lead.origin.x, 16.);
        assert_eq!(
            title.origin.x, title_x,
            "title_spacing {spacing:?} must offset the title from the leading edge"
        );
        assert_eq!(title.size, Size::new(40., 20.));
    }
}

#[test]
fn title_spacing_updates_after_rebuild() {
    use incular_core::Offset;

    let bar = || {
        AppBar::new(marker("Title", Size::new(40., 20.)))
            .toolbar_height(40.)
            .leading(marker("Lead", Size::new(10., 20.)))
            .title_spacing(0.)
    };
    let mut runtime = Runtime::new(bar().into()).expect("mount");
    runtime
        .run_frame(Constraints::tight(Size::new(200., 40.)))
        .expect("frame");
    assert_eq!(bounds(&runtime, "Title").origin.x, 26.);

    // Rebuilding spacing, leading width, and title content together must
    // move the retained slots without remounting: every option is mutable
    // through the descriptor.
    let root = runtime.tree().root().expect("root");
    runtime
        .schedule_update(
            root,
            AppBar::new(marker("Title2", Size::new(60., 20.)))
                .toolbar_height(40.)
                .leading(marker("Lead", Size::new(10., 20.)))
                .leading_width(24.)
                .title_spacing(32.)
                .into(),
        )
        .expect("update");
    runtime
        .run_frame(Constraints::tight(Size::new(200., 40.)))
        .expect("frame");
    assert_eq!(bounds(&runtime, "Lead").size.width, 24.);
    let title = bounds(&runtime, "Title2");
    assert_eq!(title.origin.x, 16. + 24. + 32.);
    assert_eq!(title.size.width, 60.);
    assert!(
        runtime
            .tree()
            .semantics()
            .iter()
            .all(|(_, node)| node.label.as_deref() != Some("Title")),
        "the replaced title must be gone"
    );
    // The moved title must hit-test at its new center.
    let center = Offset::new(
        title.origin.x + title.size.width / 2.,
        title.origin.y + title.size.height / 2.,
    );
    assert!(
        runtime.tree().hit_test(center).is_some(),
        "rebuilt title must hit at its new position"
    );
}

#[test]
fn leading_width_and_imply_place_title_deterministically() {
    // No explicit leading: the implied placeholder reserves leading_width,
    // then the default title gap applies.
    let runtime = mount_bar(
        AppBar::new(marker("Title", Size::new(40., 20.)))
            .toolbar_height(40.)
            .leading_width(50.),
    );
    assert_eq!(bounds(&runtime, "Title").origin.x, 82.);

    // Opting out removes the placeholder: only outer padding plus gap remain.
    let runtime = mount_bar(
        AppBar::new(marker("Title", Size::new(40., 20.)))
            .toolbar_height(40.)
            .automatically_imply_leading(false),
    );
    assert_eq!(bounds(&runtime, "Title").origin.x, 32.);
}

#[test]
fn center_title_keeps_centered_composition() {
    // Centered contract: the title is centered within the full content
    // width (200 - 2*16 outer padding = 168), with the configured spacing
    // kept as the minimum gap on each side. Symmetric gaps preserve the
    // center, so (168 - 40) / 2 + 16 = 80.
    let runtime = mount_bar(
        AppBar::new(marker("Title", Size::new(40., 20.)))
            .toolbar_height(40.)
            .center_title(true),
    );
    assert_eq!(bounds(&runtime, "Title").origin.x, 80.);
}

#[test]
fn long_title_does_not_displace_fitting_actions() {
    use incular_core::{InputEvent, Offset, PointerPhase};
    use incular_widgets::internal::ActionSurface;
    use std::{cell::Cell, rc::Rc};

    // Fixed slots are reserved before the title gets space: leading 10 plus
    // two 20px actions leave the title cell at 16 + 10 .. 16 + 10 + 118, so
    // actions sit at 144..164 and 164..184 whatever the title measures.
    let layout_with_title = |title_w: f32| {
        let hits = Rc::new(Cell::new(0));
        let action = |label: &str, bit: i32| {
            let observed = hits.clone();
            ActionSurface::new(label)
                .size(Size::new(20., 20.))
                .on_click(move || observed.set(observed.get() | bit))
        };
        let widget: Widget = AppBar::new(marker("Title", Size::new(title_w, 20.)))
            .toolbar_height(40.)
            .leading(marker("Lead", Size::new(10., 20.)))
            .actions([action("A1", 1), action("A2", 2)])
            .into();
        let mut runtime = Runtime::new(widget).expect("mount");
        runtime
            .run_frame(Constraints::tight(Size::new(200., 40.)))
            .expect("frame");
        (runtime, hits)
    };
    let (short, _) = layout_with_title(40.);
    let short_a1 = bounds(&short, "A1");
    let short_a2 = bounds(&short, "A2");
    assert_eq!(short_a1.origin.x, 144.);
    assert_eq!(short_a2.origin.x, 164.);
    assert_eq!(bounds(&short, "Title").origin.x, 42.);

    // A 200px title is constrained to the title cell (118px minus the
    // 16px gap) instead of pushing the actions out: action bounds must be
    // identical, and the title starts at the exact configured spacing.
    let (mut long, hits) = layout_with_title(200.);
    assert_eq!(
        bounds(&long, "A1"),
        short_a1,
        "a long title must not move fitting actions"
    );
    assert_eq!(
        bounds(&long, "A2"),
        short_a2,
        "a long title must not move fitting actions"
    );
    let title = bounds(&long, "Title");
    assert_eq!(title.origin.x, 42.);
    assert_eq!(title.size.width, 102.);
    // Targeted clicks must still reach the actual actions through the
    // overlapping title: a bare hit-test hit could be the background.
    for (label, bit) in [("A1", 1), ("A2", 2)] {
        let rect = bounds(&long, label);
        let position = Offset::new(
            rect.origin.x + rect.size.width / 2.,
            rect.origin.y + rect.size.height / 2.,
        );
        for phase in [PointerPhase::Down, PointerPhase::Up] {
            let _ = long.handle_input(InputEvent::Pointer { phase, position });
        }
        assert_eq!(hits.get() & bit, bit, "{label} must stay clickable");
    }
    assert_eq!(hits.get(), 3);
}

#[test]
fn leading_width_constrains_explicit_leading_content() {
    // The configured width sizes the leading slot even when explicit
    // content exists: content fills the slot, and the title starts one
    // slot plus the default gap later.
    for (width, lead_w, title_x) in [(24., 24., 56.), (56., 56., 88.)] {
        let runtime = mount_bar(
            AppBar::new(marker("Title", Size::new(40., 20.)))
                .toolbar_height(40.)
                .leading(marker("Lead", Size::new(10., 20.)))
                .leading_width(width),
        );
        let lead = bounds(&runtime, "Lead");
        assert_eq!(lead.origin.x, 16.);
        assert_eq!(
            lead.size.width, lead_w,
            "leading_width {width} must size the slot around explicit content"
        );
        assert_eq!(bounds(&runtime, "Title").origin.x, title_x);
    }
}

#[test]
fn centered_title_with_asymmetric_slots_uses_remaining_space() {
    // Leading 10 plus two 20px actions: the title cell spans 26..144
    // (118px). The 72px padded content (16 gap + 40 title + 16 gap)
    // centers with a 23px offset, so the title starts at 26 + 23 + 16.
    let runtime = mount_bar(
        AppBar::new(marker("Title", Size::new(40., 20.)))
            .toolbar_height(40.)
            .leading(marker("Lead", Size::new(10., 20.)))
            .actions([
                marker("A1", Size::new(20., 20.)),
                marker("A2", Size::new(20., 20.)),
            ])
            .center_title(true),
    );
    let title = bounds(&runtime, "Title");
    assert_eq!(title.origin.x, 65.);
    // Centered within the title cell, not the full toolbar.
    assert_eq!(title.origin.x + title.size.width / 2., 26. + 118. / 2.);
    // Side slots keep their reserved geometry.
    assert_eq!(bounds(&runtime, "Lead").origin.x, 16.);
    assert_eq!(bounds(&runtime, "A1").origin.x, 144.);
    assert_eq!(bounds(&runtime, "A2").origin.x, 164.);
}

#[test]
fn centered_title_clamps_to_remaining_space_under_pressure() {
    // A 130px title would need 162px of padded content in a 118px cell, so
    // it is constrained to the cell minus the symmetric gaps (86px) and
    // stays centered: 26 + 0 + 16 with both side slots exactly where they
    // belong. Nothing is dropped and no slot moves.
    let runtime = mount_bar(
        AppBar::new(marker("Title", Size::new(130., 20.)))
            .toolbar_height(40.)
            .leading(marker("Lead", Size::new(10., 20.)))
            .actions([
                marker("A1", Size::new(20., 20.)),
                marker("A2", Size::new(20., 20.)),
            ])
            .center_title(true),
    );
    let title = bounds(&runtime, "Title");
    assert_eq!(title.origin.x, 42.);
    assert_eq!(title.size.width, 86.);
    assert_eq!(title.origin.x + title.size.width / 2., 26. + 118. / 2.);
    assert_eq!(bounds(&runtime, "Lead").origin.x, 16.);
    assert_eq!(bounds(&runtime, "A1").origin.x, 144.);
    assert_eq!(bounds(&runtime, "A2").origin.x, 164.);
}

#[test]
fn flexible_space_paints_behind_toolbar_row() {
    use incular_rendering::PaintCommand;

    let flex = Color::rgba(1, 2, 3, 255);
    let toolbar = Color::rgba(4, 5, 6, 255);
    let mut runtime = Runtime::new(
        AppBar::new(marker("Title", Size::new(40., 20.)))
            .toolbar_height(40.)
            .flexible_space(Widget::box_(Size::new(160., 40.), flex))
            .background_color(toolbar)
            .into(),
    )
    .expect("mount");
    runtime
        .run_frame(Constraints::tight(Size::new(200., 40.)))
        .expect("frame");
    let commands = runtime.tree_mut().paint();
    let commands = commands.commands();
    let flex_at = commands.iter().position(|command| {
        matches!(command, PaintCommand::Rect { rect, color }
            if *color == flex && rect.size == Size::new(160., 40.))
    });
    let title_at = commands.iter().position(|command| {
        matches!(command, PaintCommand::Rect { rect, color }
            if *color == Color::WHITE && rect.size == Size::new(40., 20.))
    });
    let (flex_at, title_at) = (flex_at.expect("flex paint"), title_at.expect("title paint"));
    assert!(
        flex_at < title_at,
        "flexible space must paint beneath the toolbar row"
    );
    assert!(
        commands.iter().any(|command| {
            matches!(command, PaintCommand::RRect { brush, .. }
            if *brush == incular_rendering::Brush::Solid(toolbar))
        }),
        "toolbar background must still paint with flexible space set"
    );
}

#[test]
fn elevation_paints_shadow_only_when_positive() {
    use incular_rendering::PaintCommand;

    for (elevation, shadowed) in [(0., false), (4., true)] {
        let mut runtime = Runtime::new(
            AppBar::new(marker("Title", Size::new(40., 20.)))
                .toolbar_height(40.)
                .elevation(elevation)
                .into(),
        )
        .expect("mount");
        runtime
            .run_frame(Constraints::tight(Size::new(200., 40.)))
            .expect("frame");
        let paint = runtime.tree_mut().paint();
        let has_shadow = paint.commands().iter().any(|command| {
            matches!(command, PaintCommand::PushDropShadow { .. })
                || format!("{command:?}").contains("DropShadow")
        });
        assert_eq!(
            has_shadow, shadowed,
            "elevation {elevation} must control shadow paint"
        );
    }
}
