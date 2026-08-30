//! Structural performance contracts for retained-tree reconciliation.

use super::*;

/// Task 15 structural reconciliation contracts: operation-count based, no
/// timing thresholds.
#[cfg(test)]
mod reconciliation_structural_contracts {

    use super::*;

    fn frame() -> Constraints {
        Constraints::tight(Size::new(600., 6000.))
    }

    fn keyed_row(items: usize, generation: u64) -> Widget {
        let children = (0..items)
            .map(|index| {
                let label = if index == items / 2 && generation > 0 {
                    format!("changed {generation}")
                } else {
                    format!("row {index}")
                };
                Widget::from(Text::new(label)).with_key(Key::Value(index as u64))
            })
            .collect::<Vec<_>>();
        Widget::column(children)
    }

    fn prepared(root: Widget) -> (WidgetTree, ElementId) {
        let mut tree = WidgetTree::default();
        let root_id = tree.mount(root).expect("mount");
        tree.layout(frame());
        (tree, root_id)
    }

    #[test]
    fn ten_thousand_unchanged_children_perform_no_mutation_work() {
        // The PARENT differs (spacing), so its child list is rescanned; every
        // child is byte-identical and must cost nothing but a comparison.
        let build = |spacing: f32| {
            Widget::wrap(
                incular_config::Axis::Vertical,
                spacing,
                spacing,
                (0..10_000)
                    .map(|index| {
                        Widget::from(Text::new(format!("row {index}")))
                            .with_key(Key::Value(index as u64))
                    })
                    .collect::<Vec<_>>(),
            )
        };
        let (mut tree, root) = prepared(build(8.));
        let before = tree.diagnostics();

        tree.update(root, build(9.)).expect("update");
        tree.layout(frame());
        let _ = tree.paint();

        let after = tree.diagnostics();
        assert_eq!(
            after.elements_created - before.elements_created,
            0,
            "created"
        );
        assert_eq!(
            after.elements_removed - before.elements_removed,
            0,
            "removed"
        );
        assert_eq!(
            after.rebuilds - before.rebuilds,
            1,
            "only the parent itself rebuilds"
        );
        assert_eq!(
            after.identical_child_bailouts - before.identical_child_bailouts,
            10_000,
            "every child must hit the identical-widget bailout"
        );
    }

    #[test]
    fn single_changed_child_touches_only_that_child() {
        let (mut tree, root) = prepared(keyed_row(10_000, 0));
        let before = tree.diagnostics();

        tree.update(root, keyed_row(10_000, 1)).expect("update");
        tree.layout(frame());
        let _ = tree.paint();

        let after = tree.diagnostics();
        assert_eq!(after.elements_created - before.elements_created, 0);
        assert_eq!(after.elements_removed - before.elements_removed, 0);
        // Root + the one changed descendant; nothing else rebuilds.
        assert_eq!(after.rebuilds - before.rebuilds, 2);
        assert_eq!(
            after.identical_child_bailouts - before.identical_child_bailouts,
            9_999
        );
        // Only the changed text and its column re-resolve layout.
        assert_eq!(after.layouts - before.layouts, 2);
        assert!(after.paints - before.paints >= 1, "changed text repaints");
    }

    #[test]
    fn key_reorder_preserves_state_and_counts_moves() {
        let build = |order: &[usize]| {
            Widget::column(
                order
                    .iter()
                    .map(|&index| {
                        Widget::from(Text::new(format!("state {index}")))
                            .with_key(Key::Value(index as u64))
                    })
                    .collect::<Vec<_>>(),
            )
        };
        let (mut tree, root) = prepared(build(&[0, 1, 2, 3, 4]));
        let before = tree.diagnostics();

        // Pure reorder: [4,3,2,1,0].
        tree.update(root, build(&[4, 3, 2, 1, 0])).expect("reorder");
        tree.layout(frame());

        let after = tree.diagnostics();
        assert_eq!(
            after.elements_created - before.elements_created,
            0,
            "no remounts"
        );
        assert_eq!(
            after.elements_removed - before.elements_removed,
            0,
            "no unmounts"
        );
        assert_eq!(
            after.elements_moved - before.elements_moved,
            4,
            "four positions moved"
        );

        // Retained state identity: each moved child keeps its payload.
        let payloads: Vec<String> = tree
            .children(root)
            .expect("children")
            .iter()
            .map(
                |&child| match &tree.elements.get(child.0).expect("live").widget.kind {
                    WidgetKind::Text { text, .. } => text.clone(),
                    _ => String::new(),
                },
            )
            .collect();
        assert_eq!(
            payloads,
            ["state 4", "state 3", "state 2", "state 1", "state 0"]
        );
    }

    #[test]
    fn incompatible_key_replacement_does_not_inherit_state() {
        let build = |kind: u8| {
            let child = if kind == 0 {
                Widget::from(Text::new("text 0"))
            } else {
                Widget::box_(Size::new(20., 20.), Color::WHITE)
            };
            Widget::column(vec![child.with_key(Key::Value(7))])
        };
        let (mut tree, root) = prepared(build(0));
        let before = tree.diagnostics();

        // Same key, different widget type: incompatible, must remount fresh.
        tree.update(root, build(1)).expect("replace");

        let after = tree.diagnostics();
        assert_eq!(
            after.mounts - before.mounts,
            1,
            "replacement mounts a new element"
        );
        assert_eq!(after.unmounts - before.unmounts, 1, "old element unmounts");
        // The retained element is genuinely new: a Box, not the old Text.
        for &child in tree.children(root).expect("children") {
            assert!(matches!(
                tree.elements.get(child.0).expect("live").widget.kind,
                WidgetKind::Box { .. }
            ));
        }
    }
}
