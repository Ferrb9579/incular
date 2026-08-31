use incular_config::Constraints;
use incular_core::{Color, Offset, PointerPhase, Size};
use incular_gestures::PointerEvent;
use incular_semantics::Role;
use incular_widgets::internal::{RenderKind, Widget, WidgetKind, WidgetTree};
use incular_widgets::{
    AnimatedList, AnimatedModalBarrier, AutomaticKeepAlive, BackButtonListener,
    BackCallbackSubscription, BasicRouterDelegate, Container, CustomScrollView, KeepAliveHandle,
    KeepAliveNotification, KeepAliveRegistry, NavigationBackButtonDispatcher, PageStorage,
    PageStorageBucket, PageStorageIdentifier, PageStorageKey, PopScope, PopScopeController,
    ReorderableList, RootBackButtonDispatcher, RootRestorationScope, Router, SizedBox, Sliver,
    SliverAnimatedGrid, SliverGridDelegate, TreeRowAnimation, TreeSliver, TreeSliverNode,
};
use serde_json::json;
use std::{cell::Cell, rc::Rc, time::Instant};

fn colored_row(index: usize) -> Widget {
    Container::new()
        .width(100.)
        .height(24.)
        .color(Color::rgba(index as u8, 32, 96, 255))
        .into()
}

fn assert_retained_viewport(widget: Widget, logical_count: usize) {
    assert!(matches!(
        widget.kind().clone(),
        WidgetKind::SliverViewport { .. }
    ));
    assert!(format!("{widget:?}").contains("SliverViewport"));
    assert_eq!(widget, widget.clone());

    let mut tree = WidgetTree::new();
    let root = tree.mount(widget).expect("mount completed sliver widget");
    assert!(matches!(
        tree.render_object_kind(tree.render_id(root).expect("viewport render")),
        Some(RenderKind::SliverViewport { .. })
    ));

    tree.layout(Constraints::tight(Size::new(100., 80.)))
        .expect("layout");
    let diagnostics = tree
        .sliver_viewport_diagnostics()
        .expect("viewport diagnostics");
    assert_eq!(diagnostics.logical_item_count, logical_count);
    assert!(diagnostics.materialized_item_count > 0);
    assert!(!tree.paint().is_empty());

    tree.update_semantics();
    assert!(
        tree.semantics()
            .iter()
            .any(|(_, node)| node.role == Role::ScrollView)
    );
}

#[test]
fn completed_sliver_facade_exports_use_the_retained_viewport_protocol() {
    assert_retained_viewport(AnimatedList::new(3, colored_row).into(), 3);

    let grid = SliverAnimatedGrid::new(
        3,
        SliverGridDelegate::fixed_cross_axis_count(2),
        colored_row,
    );
    let grid_view: Widget = CustomScrollView::new([Box::new(grid) as Box<dyn Sliver>]).into();
    assert_retained_viewport(grid_view, 3);

    let child = TreeSliverNode::new(1_u8);
    let root = TreeSliverNode::new(0_u8)
        .with_children([child])
        .expanded(true);
    let tree_sliver = TreeSliver::new(vec![root], |_node, _animation: TreeRowAnimation| {
        colored_row(0)
    });
    let tree_view: Widget =
        CustomScrollView::new([Box::new(tree_sliver) as Box<dyn Sliver>]).into();
    assert_retained_viewport(tree_view, 2);

    let reorderable: Widget = ReorderableList::new(3, colored_row).item_extent(24.).into();
    assert_retained_viewport(reorderable, 3);
}

#[test]
fn completed_navigation_facade_scopes_reconcile_and_route_retained_interaction() {
    let dispatcher = NavigationBackButtonDispatcher::new();
    let calls = Rc::new(Cell::new(0));
    let bucket = PageStorageBucket::default();
    let key = PageStorageKey::new("completed-module");
    let identifier = PageStorageIdentifier::from(&key);
    assert!(bucket.write_state(identifier.clone(), json!({ "offset": 12 })));
    assert_eq!(bucket.read_state(identifier), Some(json!({ "offset": 12 })));

    let child = PopScope::with_controller(
        PopScopeController::new(),
        PageStorage::with_bucket(bucket, SizedBox::shrink()),
    )
    .dispatcher(dispatcher.clone());
    let listener = BackButtonListener::new(RootRestorationScope::new("completed", child))
        .dispatcher(dispatcher.clone())
        .on_back_button_pressed({
            let calls = calls.clone();
            move || {
                calls.set(calls.get() + 1);
                true
            }
        });

    let mut tree = WidgetTree::new();
    let root = tree.mount(listener.into()).expect("mount navigation scope");
    tree.layout(Constraints::tight(Size::new(100., 40.)))
        .expect("layout");
    assert_eq!(dispatcher.handler_count(), 1);
    assert_eq!(dispatcher.pop_scope_count(), 1);
    assert!(dispatcher.dispatch_back().handled);
    assert_eq!(calls.get(), 1);

    tree.update(root, BackButtonListener::new(SizedBox::shrink()).into())
        .expect("reconcile navigation scope");
    tree.layout(Constraints::tight(Size::new(100., 40.)))
        .expect("layout");
    assert_eq!(dispatcher.handler_count(), 0);
    assert_eq!(dispatcher.pop_scope_count(), 0);

    let barrier_calls = Rc::new(Cell::new(0));
    let barrier: Widget = AnimatedModalBarrier::new()
        .semantics_label("Dismiss")
        .on_dismiss({
            let barrier_calls = barrier_calls.clone();
            move || barrier_calls.set(barrier_calls.get() + 1)
        })
        .into();
    let mut barrier_tree = WidgetTree::new();
    barrier_tree.mount(barrier).expect("mount modal barrier");
    barrier_tree
        .layout(Constraints::tight(Size::new(100., 40.)))
        .expect("layout");
    barrier_tree.update_semantics();
    assert!(
        barrier_tree
            .semantics()
            .iter()
            .any(|(_, node)| node.label.as_deref() == Some("Dismiss"))
    );
    assert!(!barrier_tree.paint().is_empty());
    let now = Instant::now();
    let _ = barrier_tree.dispatch_gesture(PointerEvent {
        pointer: 1,
        position: Offset::new(10., 10.),
        phase: PointerPhase::Down,
        time: now,
    });
    let _ = barrier_tree.dispatch_gesture(PointerEvent {
        pointer: 1,
        position: Offset::new(10., 10.),
        phase: PointerPhase::Up,
        time: now,
    });
    assert_eq!(barrier_calls.get(), 1);
}

#[test]
fn app_shell_router_and_navigation_scope_dispatchers_keep_distinct_public_identities() {
    let app_dispatcher = RootBackButtonDispatcher::new();
    let _subscription: BackCallbackSubscription = app_dispatcher.add_callback(|| true);
    assert!(app_dispatcher.dispatch_back());

    let navigation_dispatcher = NavigationBackButtonDispatcher::new();
    let _registration = navigation_dispatcher.add_callback(|| true);
    assert!(navigation_dispatcher.dispatch_back().handled);

    let delegate = BasicRouterDelegate::<String>::new(|| SizedBox::shrink().into());
    let _router_widget: Widget = Router::new(delegate).into_widget();
}

#[test]
fn keep_alive_facade_preserves_handle_registry_lifecycle() {
    let handle = KeepAliveHandle::new();
    let registry = KeepAliveRegistry::new();
    assert!(!registry.dispatch(KeepAliveNotification::new(handle.clone())));
    assert_eq!(registry.active_count(), 1);
    assert!(registry.keeping_alive());
    handle.release();
    assert_eq!(registry.active_count(), 0);
    assert!(!registry.keeping_alive());

    let _transparent: Widget = AutomaticKeepAlive::new(SizedBox::shrink()).into();
}
