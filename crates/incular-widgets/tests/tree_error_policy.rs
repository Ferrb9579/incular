use std::{cell::Cell, rc::Rc};

use incular_config::{Axis, Constraints};
use incular_core::{Color, Size};
use incular_scroll::{ScrollController, SliverConstraints, SliverGeometry};
use incular_widgets::internal::{
    GeneratedChildIdentity, RenderSliver, SliverChildId, SliverChildLayout, SliverLayout,
    TreeError, Widget, WidgetTree,
};
use incular_widgets::{
    CustomScrollView, LayoutBuilder, ListWheelScrollView, Sliver, SliverList,
    TwoDimensionalChildDelegate, TwoDimensionalViewport, WheelChildDelegate,
};

#[derive(Clone)]
struct DuplicateKeySliver {
    duplicate: Rc<Cell<bool>>,
}

impl Sliver for DuplicateKeySliver {
    fn build(&self, _: &ScrollController) -> Widget {
        Widget::box_(Size::new(10., 10.), Color::WHITE)
    }

    fn create_render_sliver(
        &self,
        _: &ScrollController,
        _: Axis,
        _: bool,
    ) -> Box<dyn RenderSliver> {
        Box::new(DuplicateKeyRenderSliver {
            duplicate: self.duplicate.clone(),
        })
    }
}

struct DuplicateKeyRenderSliver {
    duplicate: Rc<Cell<bool>>,
}

impl RenderSliver for DuplicateKeyRenderSliver {
    fn perform_layout(&mut self, constraints: SliverConstraints) -> SliverLayout {
        let child_constraints = Constraints::tight(Size::new(20., 20.));
        let second_id = if self.duplicate.get() { 1 } else { 2 };
        let child = |id, offset| SliverChildLayout {
            id: SliverChildId(id),
            widget: Widget::box_(Size::new(20., 20.), Color::WHITE),
            semantic_index: None,
            offset,
            cross_offset: 0.,
            constraints: child_constraints,
            extent: 20.,
            pinned: false,
        };
        SliverLayout {
            geometry: SliverGeometry::from_scroll_extent(constraints, 40.),
            children: vec![child(1, 0.), child(second_id, 20.)],
            absorbed_overlap: 0.,
        }
    }

    fn revision(&self) -> u64 {
        u64::from(self.duplicate.get())
    }
}

fn invalid_generated_widget() -> Widget {
    incular_widgets::Column::new(vec![
        Widget::box_(Size::new(10., 10.), Color::WHITE).with_key(7_u64),
        Widget::box_(Size::new(10., 10.), Color::WHITE).with_key(7_u64),
    ])
    .into()
}

fn assert_duplicate_generated(error: TreeError, expected: GeneratedChildIdentity) {
    match error {
        TreeError::InvalidGeneratedChild { child, source, .. } => {
            assert_eq!(child, expected);
            assert!(matches!(*source, TreeError::DuplicateKey { .. }));
        }
        other => panic!("expected generated-child error, got {other:?}"),
    }
}

#[test]
fn eager_mount_rejects_duplicate_keys_before_allocating_retained_state() {
    let mut tree = WidgetTree::new();
    let before_layers = tree.compositor_diagnostics().layers;

    let error = tree
        .mount(invalid_generated_widget())
        .expect_err("invalid mount");

    assert!(matches!(error, TreeError::DuplicateKey { .. }));
    assert_eq!(tree.element_count(), 0);
    assert_eq!(tree.render_object_count(), 0);
    assert_eq!(tree.compositor_diagnostics().layers, before_layers);
}

#[test]
fn layout_builder_returns_contextual_error_without_leaking_child_state() {
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(LayoutBuilder::new(|_, _| Widget::box_(Size::new(20., 20.), Color::WHITE)).into())
        .expect("layout builder mount");
    let constraints = Constraints::tight(Size::new(100., 100.));
    tree.layout(constraints).expect("initial valid layout");
    let before_elements = tree.element_count();
    let before_renders = tree.render_object_count();
    let before_layers = tree.compositor_diagnostics().layers;

    tree.update(
        root,
        LayoutBuilder::new(|_, _| invalid_generated_widget()).into(),
    )
    .expect("descriptor update is valid before generated output is evaluated");
    let error = tree
        .layout(constraints)
        .expect_err("builder output must be rejected");

    assert_duplicate_generated(error, GeneratedChildIdentity::LayoutBuilder);
    assert_eq!(tree.element_count(), before_elements);
    assert_eq!(tree.render_object_count(), before_renders);
    assert_eq!(tree.compositor_diagnostics().layers, before_layers);

    tree.update(
        root,
        LayoutBuilder::new(|_, _| Widget::box_(Size::new(30., 30.), Color::WHITE)).into(),
    )
    .expect("corrected descriptor update");
    tree.layout(constraints)
        .expect("layout must recover after a rejected generated subtree");
    assert_eq!(tree.element_count(), before_elements);
    assert_eq!(tree.render_object_count(), before_renders);
    assert_eq!(tree.compositor_diagnostics().layers, before_layers);
}

#[test]
fn sliver_builder_returns_contextual_error_without_leaking_child_state() {
    let sliver = SliverList::builder(4, |_| invalid_generated_widget());
    let root: Widget = CustomScrollView::new(vec![Box::new(sliver) as Box<dyn Sliver>]).into();
    let mut tree = WidgetTree::new();
    tree.mount(root).expect("sliver root mount");
    let before_elements = tree.element_count();
    let before_renders = tree.render_object_count();
    let before_layers = tree.compositor_diagnostics().layers;

    let error = tree
        .layout(Constraints::tight(Size::new(100., 100.)))
        .expect_err("sliver output must be rejected");

    assert!(matches!(
        error,
        TreeError::InvalidGeneratedChild {
            child: GeneratedChildIdentity::Sliver(_),
            ..
        }
    ));
    assert_eq!(tree.element_count(), before_elements);
    assert_eq!(tree.render_object_count(), before_renders);
    assert_eq!(tree.compositor_diagnostics().layers, before_layers);
}

#[test]
fn wheel_builder_returns_contextual_error_without_leaking_child_state() {
    let delegate = WheelChildDelegate::builder(Some(4), |_| Some(invalid_generated_widget()));
    let root: Widget = ListWheelScrollView::new(ScrollController::new(), 24., delegate).into();
    let mut tree = WidgetTree::new();
    tree.mount(root).expect("wheel root mount");
    let before_elements = tree.element_count();
    let before_renders = tree.render_object_count();
    let before_layers = tree.compositor_diagnostics().layers;

    let error = tree
        .layout(Constraints::tight(Size::new(120., 120.)))
        .expect_err("wheel output must be rejected");

    assert!(matches!(
        error,
        TreeError::InvalidGeneratedChild {
            child: GeneratedChildIdentity::Advanced(_),
            ..
        }
    ));
    assert_eq!(tree.element_count(), before_elements);
    assert_eq!(tree.render_object_count(), before_renders);
    assert_eq!(tree.compositor_diagnostics().layers, before_layers);
}

#[test]
fn two_dimensional_builder_returns_contextual_error_without_leaking_child_state() {
    let delegate = TwoDimensionalChildDelegate::new(4, 4, |_| Some(invalid_generated_widget()));
    let viewport = TwoDimensionalViewport::new(
        delegate,
        ScrollController::new(),
        ScrollController::new(),
        24.,
        24.,
    );
    let mut tree = WidgetTree::new();
    tree.mount(Widget::from(viewport))
        .expect("two-dimensional root mount");
    let before_elements = tree.element_count();
    let before_renders = tree.render_object_count();
    let before_layers = tree.compositor_diagnostics().layers;

    let error = tree
        .layout(Constraints::tight(Size::new(120., 120.)))
        .expect_err("two-dimensional output must be rejected");

    assert!(matches!(
        error,
        TreeError::InvalidGeneratedChild {
            child: GeneratedChildIdentity::Advanced(_),
            ..
        }
    ));
    assert_eq!(tree.element_count(), before_elements);
    assert_eq!(tree.render_object_count(), before_renders);
    assert_eq!(tree.compositor_diagnostics().layers, before_layers);
}

#[test]
fn duplicate_dynamic_key_rejects_the_new_set_without_destroying_the_previous_commit() {
    let duplicate = Rc::new(Cell::new(false));
    let root: Widget = CustomScrollView::new(vec![Box::new(DuplicateKeySliver {
        duplicate: duplicate.clone(),
    }) as Box<dyn Sliver>])
    .into();
    let mut tree = WidgetTree::new();
    tree.mount(root).expect("custom sliver root mount");
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("initial unique child set");
    let before_elements = tree.element_count();
    let before_renders = tree.render_object_count();
    let before_layers = tree.compositor_diagnostics().layers;

    duplicate.set(true);
    let error = tree
        .layout(Constraints::tight(Size::new(100., 100.)))
        .expect_err("duplicate generated key must be rejected");

    assert!(matches!(
        error,
        TreeError::InvalidGeneratedChild {
            child: GeneratedChildIdentity::Sliver(_),
            source,
            ..
        } if matches!(*source, TreeError::InvalidWidgetConfiguration { widget: "dynamic child set", .. })
    ));
    assert_eq!(tree.element_count(), before_elements);
    assert_eq!(tree.render_object_count(), before_renders);
    assert_eq!(tree.compositor_diagnostics().layers, before_layers);

    duplicate.set(false);
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("corrected generated child set must recover");
    assert_eq!(tree.element_count(), before_elements);
    assert_eq!(tree.render_object_count(), before_renders);
    assert_eq!(tree.compositor_diagnostics().layers, before_layers);
}

#[test]
fn canonical_layout_returns_recoverable_error_directly_and_can_retry() {
    let invalid = Rc::new(Cell::new(true));
    let invalid_for_builder = invalid.clone();
    let mut tree = WidgetTree::new();
    tree.mount(
        LayoutBuilder::new(move |_, _| {
            if invalid_for_builder.get() {
                invalid_generated_widget()
            } else {
                Widget::box_(Size::new(20., 20.), Color::WHITE)
            }
        })
        .into(),
    )
    .expect("layout builder mount");

    let error = tree
        .layout(Constraints::tight(Size::new(100., 100.)))
        .expect_err("invalid generated child must return directly");
    assert!(matches!(
        error,
        TreeError::InvalidGeneratedChild {
            child: GeneratedChildIdentity::LayoutBuilder,
            ..
        }
    ));

    invalid.set(false);
    tree.layout(Constraints::tight(Size::new(100., 100.)))
        .expect("corrected builder must retry successfully");
}
