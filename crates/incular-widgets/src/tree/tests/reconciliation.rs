//! Child identity and reconciliation tests.

use super::support::*;
use super::*;

#[test]
fn keyed_reorder_reuses_elements() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Widget::row(vec![box_(1), box_(2), box_(3)]))
        .unwrap();
    let before = tree.children(root).unwrap().to_vec();
    tree.update(root, Widget::row(vec![box_(3), box_(1), box_(2)]))
        .unwrap();
    let after = tree.children(root).unwrap();
    assert_eq!(after, &[before[2], before[0], before[1]]);
}

#[test]
fn removed_ids_are_stale_and_unmounted_once() {
    let mut tree = WidgetTree::new();
    let root = tree.mount(Widget::row(vec![box_(1), box_(2)])).unwrap();
    let removed = tree.children(root).unwrap()[1];
    tree.update(root, Widget::row(vec![box_(1)])).unwrap();
    assert!(!tree.element_exists(removed));
    assert_eq!(tree.diagnostics().unmounts, 1);
}

#[test]
fn duplicate_local_keys_are_rejected() {
    let mut tree = WidgetTree::new();
    assert_eq!(
        tree.mount(Widget::row(vec![box_(1), box_(1)])).unwrap_err(),
        TreeError::DuplicateKey(Key::Value(1))
    );
}

/// Property tests for retained child reconciliation: random operation
/// sequences applied to both the real reconciliation path and a trivial
/// reference model must agree on logical order, key identity, and retained
/// state after every step.
#[cfg(test)]
mod reconciliation_property {
    use super::*;
    use proptest::prelude::*;

    #[derive(Clone, Debug, PartialEq)]
    struct ModelChild {
        key: Option<u64>,
        payload: String,
    }
    type Model = Vec<ModelChild>;

    fn frame() -> Constraints {
        Constraints::tight(Size::new(400., 4000.))
    }

    fn widget_for(child: &ModelChild) -> Widget {
        let mut widget = Widget::from(Text::new(format!(
            "{}|{}",
            child.payload,
            child.key.unwrap_or(u64::MAX)
        )));
        if let Some(key) = child.key {
            widget = widget.with_key(Key::Value(key));
        }
        widget
    }

    fn observed_keys(tree: &WidgetTree, parent: ElementId) -> Vec<Option<Key>> {
        tree.children(parent)
            .expect("children")
            .iter()
            .map(|&child| {
                tree.elements
                    .get(child.0)
                    .map(|e| e.widget.key.clone())
                    .unwrap_or_default()
            })
            .collect()
    }

    fn model_keys(model: &Model) -> Vec<Option<Key>> {
        model
            .iter()
            .map(|child| child.key.map(Key::Value))
            .collect()
    }

    /// Retained-state surrogate: payload travels with identity across moves.
    fn observed_payloads(tree: &WidgetTree, parent: ElementId) -> Vec<String> {
        tree.children(parent)
            .expect("children")
            .iter()
            .map(
                |&child| match &tree.elements.get(child.0).expect("live").widget.kind {
                    WidgetKind::Text { text, .. } => text.clone(),
                    _ => String::new(),
                },
            )
            .collect()
    }

    fn model_payloads(model: &Model) -> Vec<String> {
        model
            .iter()
            .map(|child| match widget_for(child).kind {
                WidgetKind::Text { text, .. } => text,
                _ => String::new(),
            })
            .collect()
    }

    #[derive(Clone, Debug)]
    enum TestOp {
        Insert(usize, Option<u64>, u64),
        Remove(usize),
        Move(usize, usize),
        Replace(usize, u64),
        ChangeKey(usize, Option<u64>),
    }

    fn apply(model: &mut Model, op: &TestOp) {
        match op {
            TestOp::Insert(position, key, payload) => {
                if key.is_some_and(|key| model.iter().any(|c| c.key == Some(key))) {
                    return;
                }
                model.insert(
                    (*position).min(model.len()),
                    ModelChild {
                        key: *key,
                        payload: format!("p{payload}"),
                    },
                );
            }
            TestOp::Remove(position) => {
                if !model.is_empty() {
                    model.remove((*position).min(model.len() - 1));
                }
            }
            TestOp::Move(from, to) => {
                if !model.is_empty() && from != to {
                    let from = *from % model.len();
                    let child = model.remove(from);
                    model.insert((*to).min(model.len()), child);
                }
            }
            TestOp::Replace(position, payload) => {
                if !model.is_empty() {
                    let position = (*position).min(model.len() - 1);
                    model[position].payload = format!("r{payload}");
                }
            }
            TestOp::ChangeKey(position, key) => {
                if !model.is_empty() {
                    let position = (*position).min(model.len() - 1);
                    let free = key.is_none_or(|key| {
                        !model
                            .iter()
                            .enumerate()
                            .any(|(index, c)| index != position && c.key == Some(key))
                    });
                    if free {
                        model[position].key = *key;
                    }
                }
            }
        }
    }

    fn reconcile(tree: &mut WidgetTree, parent: ElementId, model: &Model) {
        let desired: Vec<Widget> = model.iter().map(widget_for).collect();
        let mut parent_widget = tree.elements.get(parent.0).expect("parent").widget.clone();
        match &mut parent_widget.kind {
            WidgetKind::Flex { children, .. } => *children = desired,
            other => panic!("test parent is a column, found {other:?}"),
        }
        tree.update(parent, parent_widget).expect("reconcile");
        tree.layout(frame());
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        #[test]
        fn reconciliation_matches_reference_model(
            ops in proptest::collection::vec(
                prop_oneof![
                    (0usize..8, proptest::option::of(0u64..4u64), 0u64..1_000u64)
                        .prop_map(|(position, key, payload)| TestOp::Insert(position, key, payload)),
                    (0usize..8usize).prop_map(TestOp::Remove),
                    (0usize..8usize, 0usize..8usize).prop_map(|(from, to)| TestOp::Move(from, to)),
                    (0usize..8usize, 0u64..1_000u64)
                        .prop_map(|(position, payload)| TestOp::Replace(position, payload)),
                    (0usize..8usize, proptest::option::of(0u64..4u64))
                        .prop_map(|(position, key)| TestOp::ChangeKey(position, key)),
                ],
                0..40,
            )
        ) {
            let mut model: Model = Vec::new();
            let mut tree = WidgetTree::default();
            let parent = tree.mount(Widget::column(Vec::new())).expect("mount");
            tree.layout(frame());

            for op in &ops {
                apply(&mut model, op);
                reconcile(&mut tree, parent, &model);

                prop_assert_eq!(observed_keys(&tree, parent), model_keys(&model));
                prop_assert_eq!(observed_payloads(&tree, parent), model_payloads(&model));
                prop_assert_eq!(tree.children(parent).unwrap().len(), model.len());
            }
        }
    }
}
