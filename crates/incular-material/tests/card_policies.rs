use incular_config::{Constraints, EdgeInsets};
use incular_core::{Color, Size};
use incular_material::Card;
use incular_rendering::PaintCommand;
use incular_semantics::Role;
use incular_widgets::{
    Border, Widget,
    internal::{ActionSurface, WidgetTree},
};

const CHILD: Color = Color::rgba(1, 2, 3, 255);
const OUTLINE: Color = Color::rgba(4, 5, 6, 255);
const SURFACE: Color = Color::rgba(7, 8, 9, 255);

fn card(foreground: bool, group: bool) -> Widget {
    Card::new(
        ActionSurface::new("")
            .size(Size::new(80., 40.))
            .color(CHILD)
            .on_click(|| {}),
    )
    .color(SURFACE)
    .radius(12.)
    .elevation(0.)
    .padding(EdgeInsets::all(8.))
    .margin(EdgeInsets::all(5.))
    .border(Border::new(2., OUTLINE))
    .border_on_foreground(foreground)
    .semantic_container(group)
    .into()
}

fn mounted(widget: Widget) -> WidgetTree {
    let mut tree = WidgetTree::new();
    tree.mount(widget).expect("mount");
    tree.layout(Constraints::loose(Size::new(300., 200.)))
        .expect("layout");
    tree.update_semantics();
    tree
}

#[test]
fn border_order_changes_without_changing_surface_geometry() {
    let mut border_bounds = Vec::new();
    for foreground in [false, true] {
        let mut tree = mounted(card(foreground, false));
        let commands = tree.paint();
        let (border_index, bounds) = commands
            .commands()
            .iter()
            .enumerate()
            .find_map(|(i, command)| match command {
                PaintCommand::Border { rrect, border } if border.color == OUTLINE => {
                    assert_eq!(rrect.radii, incular_rendering::CornerRadii::uniform(12.));
                    Some((i, rrect.rect))
                }
                _ => None,
            })
            .expect("border paint");
        let child_index = commands.commands().iter().position(|command| {
            matches!(command, PaintCommand::Rect { color, .. } if *color == CHILD)
                || matches!(command, PaintCommand::RRect { brush, .. } if *brush == CHILD.into())
        }).expect("child paint");
        assert_eq!(border_index > child_index, foreground);
        let surface_index = commands.commands().iter().position(|command| {
            matches!(command, PaintCommand::RRect { brush, .. } if *brush == SURFACE.into())
        }).expect("surface paint");
        assert!(
            surface_index < border_index,
            "the surface must not cover its outline"
        );
        border_bounds.push(bounds);
    }
    assert_eq!(border_bounds[0], border_bounds[1]);
    assert_eq!(border_bounds[0].size, Size::new(96., 56.));
}

#[test]
fn foreground_border_preserves_pointer_and_semantic_activation() {
    use incular_core::{InputEvent, Offset, PointerPhase};
    use incular_runtime::Runtime;
    use std::{cell::Cell, rc::Rc};

    for grouped in [false, true] {
        let hits = Rc::new(Cell::new(0));
        let observed = hits.clone();
        let child = ActionSurface::new("Activate")
            .size(Size::new(80., 40.))
            .on_click(move || observed.set(observed.get() + 1));
        let mut runtime = Runtime::new(
            Card::new(child)
                .elevation(0.)
                .padding(EdgeInsets::all(8.))
                .border(Border::new(2., OUTLINE))
                .border_on_foreground(true)
                .semantic_container(grouped)
                .into(),
        )
        .expect("mount");
        runtime
            .run_frame(Constraints::loose(Size::new(300., 200.)))
            .expect("frame");
        for phase in [PointerPhase::Down, PointerPhase::Up] {
            let _ = runtime.handle_input(InputEvent::Pointer {
                phase,
                position: Offset::new(20., 20.),
            });
        }
        assert_eq!(hits.get(), 1);
        let button = runtime
            .tree()
            .semantics()
            .iter()
            .find_map(|(id, node)| (node.role == Role::Button).then_some(id))
            .expect("button");
        assert!(
            runtime.dispatch_semantic_action(button, incular_semantics::SemanticAction::Activate)
        );
        assert_eq!(hits.get(), 2);
    }
}

#[test]
fn semantic_container_adds_a_group_without_replacing_child_actions() {
    for grouped in [false, true] {
        let tree = mounted(card(true, grouped));
        let groups: Vec<_> = tree
            .semantics()
            .iter()
            .filter(|(_, node)| node.role == Role::Group)
            .collect();
        assert_eq!(groups.len(), usize::from(grouped));
        let buttons: Vec<_> = tree
            .semantics()
            .iter()
            .filter(|(_, node)| node.role == Role::Button)
            .collect();
        assert_eq!(buttons.len(), 1);
        assert!(
            buttons[0]
                .1
                .actions
                .contains(&incular_semantics::SemanticActionKind::Activate)
        );
        if grouped {
            assert!(groups[0].1.children.contains(&buttons[0].0));
            assert_eq!(groups[0].1.bounds.size, Size::new(96., 56.));
        }
    }
}

#[test]
fn changing_border_order_preserves_the_retained_child() {
    let mut tree = mounted(card(false, true));
    let button = tree
        .semantics()
        .iter()
        .find_map(|(id, node)| (node.role == Role::Button).then_some(id))
        .expect("button");
    let root = tree.root().expect("root");
    tree.update(root, card(true, true)).expect("update");
    tree.layout(Constraints::loose(Size::new(300., 200.)))
        .expect("layout");
    tree.update_semantics();
    let updated = tree
        .semantics()
        .iter()
        .find_map(|(id, node)| (node.role == Role::Button).then_some(id))
        .expect("updated button");
    assert_eq!(updated, button);
}
