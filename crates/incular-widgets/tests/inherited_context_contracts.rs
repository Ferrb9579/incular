//! Retained inherited-context and dependency invalidation contracts.

use incular_config::Constraints;
use incular_core::Size;
use incular_widgets::internal::WidgetTree;
use incular_widgets::{SizedBox, Widget};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

fn constraints() -> Constraints {
    Constraints::tight(Size::new(80.0, 40.0))
}

#[test]
fn nearest_scope_shadows_outer_value_and_keeps_other_types_visible() {
    let seen = Rc::new(RefCell::new(None));
    let observed = seen.clone();
    let child = incular_widgets::LayoutBuilder::new(move |context, _| {
        *observed.borrow_mut() = Some((context.depend_on::<u32>(), context.depend_on::<String>()));
        SizedBox::shrink().into()
    })
    .into();
    let widget = Widget::environment_scope(
        1_u32,
        Widget::environment_scope(
            String::from("outer"),
            Widget::environment_scope(2_u32, child),
        ),
    );

    let mut tree = WidgetTree::new();
    tree.mount(widget).expect("mount inherited scopes");
    tree.layout(constraints()).expect("layout");

    assert_eq!(*seen.borrow(), Some((Some(2), Some(String::from("outer")))));
}

#[test]
fn lookup_boundary_stops_inherited_lookup() {
    let seen = Rc::new(RefCell::new(Some(0_u32)));
    let observed = seen.clone();
    let child = incular_widgets::LayoutBuilder::new(move |context, _| {
        *observed.borrow_mut() = context.depend_on::<u32>();
        SizedBox::shrink().into()
    })
    .into();
    let widget = Widget::environment_scope(7_u32, Widget::environment_boundary(child));

    let mut tree = WidgetTree::new();
    tree.mount(widget).expect("mount lookup boundary");
    tree.layout(constraints()).expect("layout");

    assert_eq!(*seen.borrow(), None);
}

#[test]
fn inherited_value_change_rebuilds_only_subscribers() {
    let dependent_builds = Rc::new(Cell::new(0));
    let independent_builds = Rc::new(Cell::new(0));
    let dependent: Widget = {
        let builds = dependent_builds.clone();
        incular_widgets::LayoutBuilder::new(move |context, _| {
            builds.set(builds.get() + 1);
            let _ = context.depend_on::<u32>();
            SizedBox::shrink().into()
        })
        .into()
    };
    let independent: Widget = {
        let builds = independent_builds.clone();
        incular_widgets::LayoutBuilder::new(move |_, _| {
            builds.set(builds.get() + 1);
            SizedBox::shrink().into()
        })
        .into()
    };
    let stable_child: Widget = incular_widgets::Row::new([dependent, independent]).into();
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Widget::environment_scope(1_u32, stable_child.clone()))
        .expect("mount inherited scope");
    tree.layout(constraints()).expect("layout");
    assert_eq!((dependent_builds.get(), independent_builds.get()), (1, 1));

    tree.update(root, Widget::environment_scope(2_u32, stable_child))
        .expect("update inherited value");
    tree.layout(constraints()).expect("layout");

    assert_eq!(dependent_builds.get(), 2);
    assert_eq!(independent_builds.get(), 1);
}

#[test]
fn non_subscribing_find_does_not_schedule_rebuilds() {
    let builds = Rc::new(Cell::new(0));
    let observed = Rc::new(Cell::new(0_u32));
    let child: Widget = {
        let builds = builds.clone();
        let observed = observed.clone();
        incular_widgets::LayoutBuilder::new(move |context, _| {
            builds.set(builds.get() + 1);
            observed.set(context.find::<u32>().unwrap_or_default());
            SizedBox::shrink().into()
        })
        .into()
    };
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Widget::environment_scope(1_u32, child.clone()))
        .expect("mount inherited scope");
    tree.layout(constraints()).expect("layout");
    assert_eq!((builds.get(), observed.get()), (1, 1));

    tree.update(root, Widget::environment_scope(2_u32, child))
        .expect("update inherited value");
    tree.layout(constraints()).expect("layout");

    assert_eq!(builds.get(), 1);
    assert_eq!(observed.get(), 1);
}

#[test]
fn rebuild_replaces_stale_inherited_dependencies() {
    let builds = Rc::new(Cell::new(0));
    let should_depend = Rc::new(Cell::new(true));
    let revision = Rc::new(Cell::new(0_u64));
    let child = {
        let builds = builds.clone();
        let should_depend = should_depend.clone();
        Widget::stateful_layout_builder(revision.clone(), move |context, _| {
            builds.set(builds.get() + 1);
            if should_depend.get() {
                let _ = context.depend_on::<u32>();
            }
            SizedBox::shrink().into()
        })
    };
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Widget::environment_scope(1_u32, child.clone()))
        .expect("mount inherited scope");
    tree.layout(constraints()).expect("layout");
    assert_eq!(builds.get(), 1);

    should_depend.set(false);
    revision.set(1);
    tree.layout(constraints()).expect("layout");
    assert_eq!(builds.get(), 2);

    tree.update(root, Widget::environment_scope(2_u32, child))
        .expect("update inherited value after dependency removal");
    tree.layout(constraints()).expect("layout");
    assert_eq!(builds.get(), 2);
}

#[test]
fn unmounted_consumers_leave_no_stale_invalidation_edge() {
    let builds = Rc::new(Cell::new(0));
    let dependent = {
        let builds = builds.clone();
        incular_widgets::LayoutBuilder::new(move |context, _| {
            builds.set(builds.get() + 1);
            let _ = context.depend_on::<u32>();
            SizedBox::shrink().into()
        })
        .into()
    };
    let mut tree = WidgetTree::new();
    let root = tree
        .mount(Widget::environment_scope(1_u32, dependent))
        .expect("mount inherited scope");
    tree.layout(constraints()).expect("layout");
    assert_eq!(builds.get(), 1);

    let replacement: Widget = SizedBox::shrink().into();
    tree.update(root, Widget::environment_scope(1_u32, replacement.clone()))
        .expect("unmount inherited consumer");
    tree.layout(constraints()).expect("layout");

    tree.update(root, Widget::environment_scope(2_u32, replacement))
        .expect("update scope after consumer unmount");
    tree.layout(constraints()).expect("layout");
    assert_eq!(builds.get(), 1);
}
