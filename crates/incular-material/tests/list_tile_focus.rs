use std::{cell::Cell, rc::Rc};

use incular_config::Constraints;
use incular_core::{Code, InputEvent, KeyboardEvent, KeyboardKey, NamedKey, Size};
use incular_material::ListTile;
use incular_runtime::Runtime;
use incular_widgets::{Column, Text, Widget, internal::WidgetTree};

fn frame(runtime: &mut Runtime) {
    runtime
        .run_frame(Constraints::tight(Size::new(300., 200.)))
        .expect("frame");
}

fn key(runtime: &mut Runtime, name: NamedKey, code: Code) {
    let _ = runtime.handle_input(InputEvent::Key(KeyboardEvent::key_down(
        KeyboardKey::Named(name),
        code,
    )));
}

#[test]
fn autofocus_selects_the_existing_tile_action_and_enter_activates_once() {
    let first_hits = Rc::new(Cell::new(0));
    let second_hits = Rc::new(Cell::new(0));
    let first = first_hits.clone();
    let second = second_hits.clone();
    let root: Widget = Column::new([
        ListTile::new(Text::new("First")).on_tap(move || first.set(first.get() + 1)),
        ListTile::new(Text::new("Second"))
            .autofocus(true)
            .on_long_press(|| {})
            .on_tap(move || second.set(second.get() + 1)),
    ])
    .into();
    let mut runtime = Runtime::new(root).expect("mount");
    frame(&mut runtime);
    let candidates = runtime.tree().focusable_elements();
    assert_eq!(candidates.len(), 2, "one traversal target per tile");
    assert_eq!(runtime.focused_element(), Some(candidates[1]));
    assert!(runtime.tree().action_for_element(candidates[1]).is_some());
    key(&mut runtime, NamedKey::Enter, Code::Enter);
    assert_eq!(first_hits.get(), 0);
    assert_eq!(second_hits.get(), 1);

    key(&mut runtime, NamedKey::Tab, Code::Tab);
    assert_eq!(runtime.focused_element(), Some(candidates[0]));
    frame(&mut runtime);
    assert_eq!(
        runtime.focused_element(),
        Some(candidates[0]),
        "autofocus must not reclaim focus"
    );
}

#[test]
fn disabled_and_opted_out_tiles_do_not_request_autofocus() {
    for (enabled, autofocus) in [(false, true), (true, false)] {
        let mut tree = WidgetTree::new();
        tree.mount(
            ListTile::new(Text::new("Tile"))
                .enabled(enabled)
                .autofocus(autofocus)
                .on_tap(|| {})
                .into(),
        )
        .expect("mount");
        tree.layout(Constraints::tight(Size::new(300., 100.)))
            .expect("layout");
        assert_eq!(tree.autofocus_element(), None);
        assert_eq!(tree.focusable_elements().len(), usize::from(enabled));
    }
}
