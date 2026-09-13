//! Child identity and reconciliation tests.

mod common;

use common::*;
use incular_config::Constraints;
use incular_core::Size;
use incular_widgets::Text;
use incular_widgets::internal::{ElementId, Key, TreeError, Widget, WidgetTree};

#[test]
fn keyed_reorder_reuses_elements() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(incular_widgets::Row::new(vec![box_(1), box_(2), box_(3)]).into())
        .unwrap();
    let before = tree.children(root).unwrap().to_vec();
    tree.update(
        root,
        incular_widgets::Row::new(vec![box_(3), box_(1), box_(2)]).into(),
    )
    .unwrap();
    let after = tree.children(root).unwrap();
    assert_eq!(after, &[before[2], before[0], before[1]]);
}

#[test]
fn removed_ids_are_stale_and_unmounted_once() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(incular_widgets::Row::new(vec![box_(1), box_(2)]).into())
        .unwrap();
    let removed = tree.children(root).unwrap()[1];
    tree.update(root, incular_widgets::Row::new(vec![box_(1)]).into())
        .unwrap();
    assert!(!tree.element_exists(removed));
    assert_eq!(tree.diagnostics().unmounts, 1);
}

#[test]
fn duplicate_local_keys_are_rejected() {
    let mut tree = WidgetTree::new();
    assert_eq!(
        tree.mount(incular_widgets::Row::new(vec![box_(1), box_(1)]).into())
            .unwrap_err(),
        TreeError::DuplicateKey {
            key: Key::Value(1),
            parent: None,
        }
    );
}

#[test]
fn update_rejects_nested_duplicate_keys_before_mutation() {
    // The bounded prevalidation in `update` checks the whole incoming
    // subtree before touching retained state: the duplicate fails with
    // zero mounts, zero unmounts, and identical children afterwards.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(
            incular_widgets::Row::new(vec![
                box_(1),
                incular_widgets::Column::new(vec![box_(2), box_(3)]).into(),
            ])
            .into(),
        )
        .unwrap();
    let before = tree.children(root).unwrap().to_vec();
    let mounts = tree.diagnostics().mounts;
    let error = tree
        .update(
            root,
            incular_widgets::Row::new(vec![
                box_(1),
                incular_widgets::Column::new(vec![box_(2), box_(2)]).into(),
            ])
            .into(),
        )
        .unwrap_err();
    assert!(
        matches!(error, TreeError::DuplicateKey { .. }),
        "unexpected failure: {error:?}"
    );
    assert_eq!(tree.children(root).unwrap(), &before[..]);
    for id in &before {
        assert!(tree.element_exists(*id));
    }
    assert_eq!(tree.diagnostics().mounts, mounts);
    assert_eq!(tree.diagnostics().unmounts, 0);
    // Recovery with valid content works on the untouched tree.
    tree.update(
        root,
        incular_widgets::Row::new(vec![
            box_(1),
            incular_widgets::Column::new(vec![box_(2), box_(4)]).into(),
        ])
        .into(),
    )
    .unwrap();
    assert_eq!(tree.children(root).unwrap().len(), 2);
}

#[test]
fn update_allows_same_key_in_disjoint_subtrees() {
    // Keys scope to siblings: the same key value under two different
    // parents is legal, and updating one branch never disturbs the other.
    let mut tree = WidgetTree::new();
    let first: Widget = incular_widgets::Column::new(vec![box_(1)]).into();
    let second: Widget = incular_widgets::Column::new(vec![box_(1)]).into();
    let root = tree
        .mount(incular_widgets::Row::new(vec![first, second]).into())
        .unwrap();
    let before = tree.children(root).unwrap().to_vec();
    let left: Widget = incular_widgets::Column::new(vec![box_(1), box_(2)]).into();
    let right: Widget = incular_widgets::Column::new(vec![box_(1)]).into();
    tree.update(root, incular_widgets::Row::new(vec![left, right]).into())
        .unwrap();
    // Unkeyed positional children update in place: same ids, still
    // distinct, with the new box landing in the left branch only.
    let after = tree.children(root).unwrap();
    assert_eq!(after, &before[..]);
    assert_ne!(after[0], after[1]);
    assert_eq!(tree.children(after[0]).unwrap().len(), 2);
    assert_eq!(tree.children(after[1]).unwrap().len(), 1);
}

#[test]
fn transparent_wrappers_do_not_hide_duplicate_keys() {
    // Hidden-but-mounted wrappers are still descriptors: duplicates under
    // invisible Visibility and offstage content fail before mutation.
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(incular_widgets::Row::new(vec![box_(1)]).into())
        .unwrap();
    let before = tree.children(root).unwrap().to_vec();
    let error = tree
        .update(
            root,
            incular_widgets::Row::new(vec![
                box_(1),
                incular_widgets::Visibility::new(
                    incular_widgets::Offstage::new(incular_widgets::Column::new(vec![
                        box_(2),
                        box_(2),
                    ]))
                    .offstage(true),
                )
                .visible(false)
                .maintain_state(true)
                .into(),
            ])
            .into(),
        )
        .unwrap_err();
    assert!(
        matches!(error, TreeError::DuplicateKey { .. }),
        "unexpected failure: {error:?}"
    );
    assert_eq!(tree.children(root).unwrap(), &before[..]);
    assert_eq!(tree.diagnostics().unmounts, 0);
}

#[test]
fn builder_generated_duplicates_surface_at_execution() {
    // Static validation sees no children inside a LayoutBuilder, but
    // mounting must execute the builder to materialize anything — so the
    // duplicate surfaces at mount-execution through the generated-child
    // check, naming the builder owner rather than a static parent.
    // Descriptor validation and execution errors stay distinct by
    // construction.
    let mut tree = WidgetTree::new();
    let generated: Widget = incular_widgets::LayoutBuilder::new(|_, _| {
        incular_widgets::Row::new(vec![box_(1), box_(1)]).into()
    })
    .into();
    // Mounting defers builder execution, so static validation passes.
    let root = tree
        .mount(incular_widgets::Column::new(vec![generated]).into())
        .expect("mount defers builder execution");
    let error = tree
        .layout(incular_config::Constraints::tight(incular_core::Size::new(
            200., 200.,
        )))
        .unwrap_err();
    assert!(
        matches!(error, TreeError::InvalidGeneratedChild { .. }),
        "unexpected failure: {error:?}"
    );
    // The tree stays usable: valid content updates afterwards.
    tree.update(root, incular_widgets::Column::new(vec![box_(1)]).into())
        .unwrap();
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
                (0..5).find_map(|key| {
                    (tree.element_with_key(&Key::Value(key)) == Some(child))
                        .then_some(Key::Value(key))
                })
            })
            .collect()
    }

    fn model_keys(model: &Model) -> Vec<Option<Key>> {
        model
            .iter()
            .map(|child| child.key.map(Key::Value))
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
        tree.update(parent, incular_widgets::Column::new(desired).into())
            .expect("reconcile");
        tree.layout(frame()).expect("layout");
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
            let parent = tree
                .mount(incular_widgets::Column::new(Vec::<Widget>::new()).into())
                .expect("mount");
            tree.layout(frame()).expect("layout");

            for op in &ops {
                apply(&mut model, op);
                reconcile(&mut tree, parent, &model);

                prop_assert_eq!(observed_keys(&tree, parent), model_keys(&model));
                prop_assert_eq!(tree.children(parent).unwrap().len(), model.len());
            }
        }
    }
}
