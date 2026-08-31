//! Structural performance contracts for retained-tree reconciliation.

use incular_config::Constraints;
use incular_core::{Color, Size};
use incular_widgets::Text;
use incular_widgets::internal::{ElementId, Key, RenderKind, Widget, WidgetTree};

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
        incular_widgets::Column::new(children).into()
    }

    fn prepared(root: Widget) -> (WidgetTree, ElementId) {
        let mut tree = WidgetTree::default();
        let root_id = tree.mount(root).expect("mount");
        tree.layout(frame()).expect("layout");
        (tree, root_id)
    }

    #[test]
    fn ten_thousand_unchanged_children_perform_no_mutation_work() {
        // The PARENT differs (spacing), so its child list is rescanned; every
        // child is byte-identical and must cost nothing but a comparison.
        let build = |spacing: f32| {
            incular_widgets::Wrap::new(
                (0..10_000)
                    .map(|index| {
                        Widget::from(Text::new(format!("row {index}")))
                            .with_key(Key::Value(index as u64))
                    })
                    .collect::<Vec<_>>(),
            )
            .direction(incular_config::Axis::Vertical)
            .spacing(spacing)
            .run_spacing(spacing)
            .into()
        };
        let (mut tree, root) = prepared(build(8.));
        let before = tree.diagnostics();

        tree.update(root, build(9.)).expect("update");
        tree.layout(frame()).expect("layout");
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
        tree.layout(frame()).expect("layout");
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
            incular_widgets::Column::new(
                order
                    .iter()
                    .map(|&index| {
                        Widget::from(Text::new(format!("state {index}")))
                            .with_key(Key::Value(index as u64))
                    })
                    .collect::<Vec<_>>(),
            )
            .into()
        };
        let (mut tree, root) = prepared(build(&[0, 1, 2, 3, 4]));
        let before_children = tree.children(root).expect("children").to_vec();
        let before = tree.diagnostics();

        // Pure reorder: [4,3,2,1,0].
        tree.update(root, build(&[4, 3, 2, 1, 0])).expect("reorder");
        tree.layout(frame()).expect("layout");

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
        assert_eq!(
            tree.children(root).expect("children"),
            &[
                before_children[4],
                before_children[3],
                before_children[2],
                before_children[1],
                before_children[0]
            ]
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
            incular_widgets::Column::new(vec![child.with_key(Key::Value(7))]).into()
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
        // The keyed Text was replaced by a fresh Box descriptor. The key is
        // intentionally retained, so identity is checked through the
        // retained render kind rather than by comparing the key lookup.
        let child = tree.children(root).expect("children")[0];
        let render = tree.render_id(child).expect("replacement render");
        assert!(matches!(
            tree.render_object_kind(render),
            Some(RenderKind::Box { .. })
        ));
    }
}
