use incular_config::{Constraints, EdgeInsets};
use incular_core::{Color, Rect, Size};
use incular_material::{AppBar, Scaffold};
use incular_runtime::Runtime;
use incular_semantics::{Role, SemanticNodeId};
use incular_widgets::{Semantics, Widget};

fn slot(label: &str, height: f32) -> Widget {
    Semantics::new(Widget::box_(Size::new(300., height), Color::WHITE))
        .role(Role::Group)
        .label(label)
        .into()
}

fn scaffold(top: bool, bottom: bool) -> Widget {
    Scaffold::new(slot("Body", 600.))
        .app_bar(
            AppBar::new(slot("Title", 20.))
                .toolbar_height(40.)
                .bottom(slot("App bottom", 15.)),
        )
        .bottom_sheet(slot("Sheet", 25.))
        .bottom_navigation_bar(slot("Navigation", 30.))
        .floating_action_button(slot("Fab", 20.))
        .extend_body_behind_app_bar(top)
        .extend_body(bottom)
        .into()
}

fn bounds(runtime: &Runtime, label: &str) -> (SemanticNodeId, Rect) {
    runtime
        .tree()
        .semantics()
        .iter()
        .find_map(|(id, node)| (node.label.as_deref() == Some(label)).then_some((id, node.bounds)))
        .expect("slot")
}

fn frame(runtime: &mut Runtime) {
    runtime
        .run_frame(Constraints::tight(Size::new(300., 600.)))
        .expect("frame");
}

#[test]
fn extension_policies_use_measured_regions_and_preserve_body_identity() {
    let mut runtime = Runtime::new(scaffold(false, false)).expect("mount");
    frame(&mut runtime);
    let body_id = bounds(&runtime, "Body").0;
    let fab_id = bounds(&runtime, "Fab").0;
    let root = runtime.tree().root().expect("root");
    for (top, bottom) in [
        (false, false),
        (true, false),
        (false, true),
        (true, true),
        (false, false),
    ] {
        runtime
            .schedule_update(root, scaffold(top, bottom))
            .expect("update");
        frame(&mut runtime);
        let body = bounds(&runtime, "Body");
        assert_eq!(body.0, body_id);
        assert_eq!(bounds(&runtime, "Fab").0, fab_id);
        assert_eq!(body.1.origin.y, if top { 0. } else { 55. });
        assert_eq!(
            body.1.size.height,
            600. - if top { 0. } else { 55. } - if bottom { 0. } else { 55. }
        );
        assert_eq!(bounds(&runtime, "App bottom").1.origin.y, 40.);
        assert_eq!(bounds(&runtime, "Sheet").1.origin.y, 545.);
        assert_eq!(bounds(&runtime, "Navigation").1.origin.y, 570.);
    }
}

#[test]
fn overlaid_bars_receive_clicks_above_the_extended_body() {
    use incular_core::{InputEvent, Offset, PointerPhase};
    use incular_widgets::internal::ActionSurface;
    use std::{cell::Cell, rc::Rc};

    let hits = Rc::new(Cell::new(0));
    let button = |label, height, bit| {
        let observed = hits.clone();
        ActionSurface::new(label)
            .size(Size::new(100., height))
            .on_click(move || observed.set(observed.get() | bit))
    };
    let mut runtime = Runtime::new(
        Scaffold::new(button("Body", 600., 1))
            .app_bar(AppBar::new(button("Top", 20., 2)).toolbar_height(40.))
            .bottom_navigation_bar(button("Bottom", 30., 4))
            .extend_body(true)
            .extend_body_behind_app_bar(true)
            .into(),
    )
    .expect("mount");
    frame(&mut runtime);
    for label in ["Top", "Bottom", "Body"] {
        let rect = bounds(&runtime, label).1;
        let position = Offset::new(
            rect.origin.x + rect.size.width / 2.,
            rect.origin.y + rect.size.height / 2.,
        );
        for phase in [PointerPhase::Down, PointerPhase::Up] {
            let _ = runtime.handle_input(InputEvent::Pointer { phase, position });
        }
    }
    assert_eq!(hits.get(), 7);
}

#[test]
fn extended_regions_remain_above_keyboard_inset() {
    let mut runtime = Runtime::new(scaffold(true, true)).expect("mount");
    let mut environment = runtime.environment();
    environment.view_insets = EdgeInsets::only(0., 0., 0., 180.);
    runtime.set_environment(environment);
    frame(&mut runtime);
    assert_eq!(bounds(&runtime, "Body").1.size.height, 420.);
    assert_eq!(bounds(&runtime, "Sheet").1.origin.y, 365.);
    assert_eq!(bounds(&runtime, "Navigation").1.origin.y, 390.);
}

#[test]
fn drawers_bottom_app_bar_and_background_execute() {
    use incular_core::{InputEvent, Offset, PointerPhase};
    use incular_rendering::{Brush, PaintCommand};
    use incular_widgets::internal::ActionSurface;
    use std::{cell::Cell, rc::Rc};

    let background = Color::rgba(9, 9, 9, 255);
    let hits = Rc::new(Cell::new(0));
    let button = |label: &str, size: Size, bit: i32| {
        let observed = hits.clone();
        ActionSurface::new(label)
            .size(size)
            .on_click(move || observed.set(observed.get() | bit))
    };
    let widget: Widget = Scaffold::new(button("Body", Size::new(300., 600.), 1))
        .drawer(button("Drawer", Size::new(80., 600.), 2))
        .end_drawer(button("EndDrawer", Size::new(80., 600.), 4))
        .bottom_app_bar(button("BottomAppBar", Size::new(300., 30.), 8))
        .background_color(background)
        .into();
    let mut runtime = Runtime::new(widget).expect("mount");
    frame(&mut runtime);
    // Drawers dock at the leading and trailing edges across the full height;
    // the bottom app bar fills the bottom region on its own.
    assert_eq!(
        bounds(&runtime, "Drawer").1,
        Rect::from_origin_size(Offset::new(0., 0.), Size::new(80., 600.))
    );
    assert_eq!(
        bounds(&runtime, "EndDrawer").1,
        Rect::from_origin_size(Offset::new(220., 0.), Size::new(80., 600.))
    );
    assert_eq!(
        bounds(&runtime, "BottomAppBar").1,
        Rect::from_origin_size(Offset::new(0., 570.), Size::new(300., 30.))
    );
    // The background color reaches the body paint.
    let painted = runtime.tree_mut().paint();
    assert!(
        painted.commands().iter().any(|command| {
            matches!(command,
                PaintCommand::Rect { color, .. }
                | PaintCommand::RRect { brush: Brush::Solid(color), .. } if *color == background)
        }),
        "scaffold background must paint"
    );
    // Clicks land on the topmost slot: drawers win over the body beneath.
    let mut expected = 0;
    for (label, bit) in [("Drawer", 2), ("EndDrawer", 4), ("BottomAppBar", 8)] {
        let rect = bounds(&runtime, label).1;
        let position = Offset::new(
            rect.origin.x + rect.size.width / 2.,
            rect.origin.y + rect.size.height / 2.,
        );
        for phase in [PointerPhase::Down, PointerPhase::Up] {
            let _ = runtime.handle_input(InputEvent::Pointer { phase, position });
        }
        expected |= bit;
        assert_eq!(hits.get(), expected, "{label} must receive its click");
    }
}
