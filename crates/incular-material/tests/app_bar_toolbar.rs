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

    // Rebuilding with a new spacing must move the retained title without
    // remounting: the option is mutable through the descriptor.
    let root = runtime.tree().root().expect("root");
    runtime
        .schedule_update(
            root,
            AppBar::new(marker("Title", Size::new(40., 20.)))
                .toolbar_height(40.)
                .leading(marker("Lead", Size::new(10., 20.)))
                .title_spacing(32.)
                .into(),
        )
        .expect("update");
    runtime
        .run_frame(Constraints::tight(Size::new(200., 40.)))
        .expect("frame");
    let title = bounds(&runtime, "Title");
    assert_eq!(title.origin.x, 58.);
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
    // Centered composition is preserved exactly: title centered in the
    // content width with the existing inter-item gaps.
    let runtime = mount_bar(
        AppBar::new(marker("Title", Size::new(40., 20.)))
            .toolbar_height(40.)
            .center_title(true),
    );
    assert_eq!(bounds(&runtime, "Title").origin.x, 88.);
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
