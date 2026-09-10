//! Multi-root semantic diagnostics through real collection.
//!
//! Sibling semantic roots share no parent, so the debug dump must visit
//! every root deterministically instead of following only the designated
//! one. These tests assert dump text because the dump itself is the
//! contract under test.

use incular_config::Constraints;
use incular_core::Size;
use incular_semantics::SemanticRole;
use incular_widgets::{Semantics, Stack, Text, Widget, internal::WidgetTree};

fn collect(tree: &mut WidgetTree) -> String {
    tree.layout(Constraints::loose(Size::new(200., 200.)))
        .expect("layout");
    tree.update_semantics();
    tree.semantics_debug_dump()
}

fn mount(tree: &mut WidgetTree, root: Widget) -> incular_widgets::internal::ElementId {
    tree.mount(root).expect("mount")
}

#[test]
fn dump_visits_two_independent_roots() {
    let mut tree = WidgetTree::new();
    mount(
        &mut tree,
        Stack::new([
            Widget::from(Text::new("alpha")),
            Widget::from(Text::new("beta")),
        ])
        .into(),
    );
    let dump = collect(&mut tree);
    assert!(dump.contains("alpha"), "first root present:\n{dump}");
    assert!(dump.contains("beta"), "second root present:\n{dump}");
    assert_eq!(
        dump.lines().count(),
        2,
        "exactly the two roots, no more:\n{dump}"
    );
}

#[test]
fn dump_nests_descendants_without_duplication() {
    let mut tree = WidgetTree::new();
    mount(
        &mut tree,
        Stack::new([
            Widget::from(
                Semantics::new(Stack::new([Widget::from(Text::new("inner"))]))
                    .role(SemanticRole::Group)
                    .label("group"),
            ),
            Widget::from(Text::new("solo")),
        ])
        .into(),
    );
    let dump = collect(&mut tree);
    assert!(dump.contains("group"), "group root present:\n{dump}");
    assert_eq!(
        dump.matches("inner").count(),
        1,
        "descendant prints once:\n{dump}"
    );
    let solo = dump
        .lines()
        .find(|line| line.contains("solo"))
        .expect("sibling root present");
    assert!(
        solo.starts_with("Text#"),
        "sibling root prints at depth zero: {solo}"
    );
    let inner = dump
        .lines()
        .find(|line| line.contains("inner"))
        .expect("descendant present");
    assert!(inner.starts_with("  "), "descendant prints nested: {inner}");
}

#[test]
fn dump_drops_a_removed_root() {
    let mut tree = WidgetTree::new();
    let root = mount(
        &mut tree,
        Stack::new([
            Widget::from(Text::new("kept")),
            Widget::from(Text::new("gone")),
        ])
        .into(),
    );
    assert!(collect(&mut tree).contains("gone"));
    tree.update(root, Stack::new([Widget::from(Text::new("kept"))]).into())
        .expect("remove sibling");
    let dump = collect(&mut tree);
    assert!(dump.contains("kept"), "survivor present:\n{dump}");
    assert!(!dump.contains("gone"), "removed root gone:\n{dump}");
    assert_eq!(dump.lines().count(), 1);
}

#[test]
fn dump_is_deterministic_across_collections() {
    let mut tree = WidgetTree::new();
    mount(
        &mut tree,
        Stack::new([
            Widget::from(
                Semantics::new(Stack::new([Widget::from(Text::new("inner"))]))
                    .role(SemanticRole::Group)
                    .label("group"),
            ),
            Widget::from(Text::new("solo")),
            Widget::from(Text::new("third")),
        ])
        .into(),
    );
    let first = collect(&mut tree);
    // Recollect without any tree change: byte-identical output.
    tree.update_semantics();
    let second = tree.semantics_debug_dump();
    assert_eq!(first, second);
    assert!(second.contains("group"));
    assert!(second.contains("solo"));
    assert!(second.contains("third"));
}

#[test]
fn dump_omits_hidden_and_excluded_content() {
    let mut tree = WidgetTree::new();
    mount(
        &mut tree,
        Stack::new([
            Widget::from(Text::new("shown")),
            Widget::from(Text::new("sibling")),
            Widget::from(Text::new("concealed")).exclude_semantics(),
        ])
        .into(),
    );
    let dump = collect(&mut tree);
    assert!(dump.contains("shown"), "visible root present:\n{dump}");
    assert!(dump.contains("sibling"), "sibling root present:\n{dump}");
    assert!(
        !dump.contains("concealed"),
        "excluded subtree stays absent:\n{dump}"
    );
    assert_eq!(dump.lines().count(), 2);
}
