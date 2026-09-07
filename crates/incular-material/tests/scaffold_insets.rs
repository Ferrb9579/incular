use incular_config::{Constraints, EdgeInsets};
use incular_core::{Color, Rect, Size};
use incular_material::{AppBar, Scaffold};
use incular_runtime::Runtime;
use incular_semantics::SemanticNodeId;
use incular_widgets::{Semantics, Text, Widget};

fn labeled(size: Size, label: &str) -> Widget {
    Semantics::new(Widget::box_(size, Color::WHITE))
        .role(incular_semantics::Role::Group)
        .label(label)
        .into()
}

fn bounds(runtime: &Runtime, label: &str) -> (SemanticNodeId, Rect) {
    runtime
        .tree()
        .semantics()
        .iter()
        .find_map(|(id, node)| (node.label.as_deref() == Some(label)).then_some((id, node.bounds)))
        .expect("labeled slot")
}

fn frame(runtime: &mut Runtime, inset: f32) {
    let mut environment = runtime.environment();
    environment.view_insets = EdgeInsets::only(11., 12., 13., inset);
    runtime.set_environment(environment);
    runtime
        .run_frame(Constraints::tight(Size::new(300., 600.)))
        .expect("frame");
}

#[test]
fn keyboard_inset_reflows_slots_and_preserves_the_body() {
    for resize in [true, false] {
        let scaffold = Scaffold::new(labeled(Size::new(300., 600.), "Body"))
            .app_bar(AppBar::new(Text::new("Title")).toolbar_height(40.))
            .bottom_navigation_bar(labeled(Size::new(300., 30.), "Footer"))
            .floating_action_button(labeled(Size::new(20., 20.), "Fab"))
            .resize_to_avoid_bottom_inset(resize);
        let mut runtime = Runtime::new(scaffold.into()).expect("mount");
        frame(&mut runtime, 0.);
        let initial = bounds(&runtime, "Body");
        let fab = bounds(&runtime, "Fab").1;
        let footer = bounds(&runtime, "Footer").1;
        assert_eq!(initial.1.size, Size::new(300., 530.));
        frame(&mut runtime, 180.);
        let resized = bounds(&runtime, "Body");
        assert_eq!(resized.0, initial.0);
        assert_eq!(resized.1.origin, initial.1.origin);
        assert_eq!(
            resized.1.size,
            Size::new(300., if resize { 350. } else { 530. })
        );
        assert_eq!(
            bounds(&runtime, "Fab").1.origin.y,
            fab.origin.y - if resize { 180. } else { 0. }
        );
        assert_eq!(
            bounds(&runtime, "Footer").1.origin.y,
            footer.origin.y - if resize { 180. } else { 0. }
        );
        frame(&mut runtime, 0.);
        assert_eq!(bounds(&runtime, "Body"), initial);
    }
}

#[test]
fn nested_scaffold_can_delegate_inset_avoidance_to_its_parent() {
    let inner =
        Scaffold::new(labeled(Size::new(300., 600.), "Body")).resize_to_avoid_bottom_inset(false);
    let mut runtime = Runtime::new(Scaffold::new(inner).into()).expect("mount");
    frame(&mut runtime, 180.);
    assert_eq!(bounds(&runtime, "Body").1.size, Size::new(300., 420.));
}

#[test]
fn keyboard_covering_the_view_produces_zero_available_body_height() {
    let mut runtime =
        Runtime::new(Scaffold::new(labeled(Size::new(300., 600.), "Body")).into()).expect("mount");
    frame(&mut runtime, 800.);
    assert_eq!(bounds(&runtime, "Body").1.size, Size::new(300., 0.));
}
