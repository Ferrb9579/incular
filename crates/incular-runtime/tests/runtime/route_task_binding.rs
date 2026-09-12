use super::*;
use incular_navigation::{Navigator, Page, PageKey};
use std::panic::{AssertUnwindSafe, catch_unwind};

/// Pumps runtime work until `done` or a timeout: wake counters may already
/// be satisfied by earlier frames, so waiting on them alone can return
/// before the task under test settles.
fn pump_until(runtime: &mut Runtime, done: &AtomicBool) {
    let start = Instant::now();
    while !done.load(Ordering::Acquire) {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "runtime work never settled"
        );
        runtime.process_runtime_work();
        std::thread::yield_now();
    }
}

fn route_page(name: &str) -> Page {
    Page::new(name, Widget::box_(Size::new(1., 1.), Color::WHITE))
}

fn mounted_lifetime(navigator: &Navigator) -> incular_navigation::RouteLifetime {
    let id = navigator.current().expect("mounted route").id;
    navigator.lifetime_of(id).expect("mounted lifetime")
}

#[test]
fn route_removal_cancels_bound_scope_and_discards_late_completion() {
    let mut runtime = Runtime::new(Widget::box_(Size::new(1., 1.), Color::WHITE)).unwrap();
    let wake = Arc::new(TestWake::default());
    runtime.set_wake_handler(wake.clone());
    let navigator = Navigator::new();
    navigator.push_page(route_page("task"));
    let lifetime = mounted_lifetime(&navigator);
    let parent = runtime.spawner().scope();
    let binding = RouteTaskBinding::bind(&parent, &lifetime);
    assert!(!binding.scope().is_cancelled());
    let completed = Arc::new(AtomicBool::new(false));
    let completed_for_completion = completed.clone();
    runtime.spawner().spawn_into_in(
        binding.scope(),
        async move {
            std::future::pending::<()>().await;
        },
        move |_, _| {
            completed_for_completion.store(true, Ordering::Release);
        },
    );
    // Permanent removal cancels before the task can complete; the late
    // completion is discarded by the existing cancelled-scope machinery
    // instead of updating the removed route.
    navigator.pop();
    assert!(binding.scope().is_cancelled());
    let start = Instant::now();
    while runtime.runtime_diagnostics().tasks_cancelled < 1 {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "cancelled task never discarded"
        );
        runtime.process_runtime_work();
        std::thread::yield_now();
    }
    assert!(!completed.load(Ordering::Acquire));
    assert_eq!(runtime.runtime_diagnostics().tasks_cancelled, 1);
}

#[test]
fn reorder_and_deactivation_do_not_cancel_bound_tasks() {
    let mut runtime = Runtime::new(Widget::box_(Size::new(1., 1.), Color::WHITE)).unwrap();
    let wake = Arc::new(TestWake::default());
    runtime.set_wake_handler(wake.clone());
    let key = |name: &str| PageKey::new(name).unwrap();
    let navigator = Navigator::new();
    navigator
        .set_pages([
            Page::new("task", Widget::box_(Size::new(1., 1.), Color::WHITE)).key(key("task")),
            Page::new("other", Widget::box_(Size::new(1., 1.), Color::WHITE)).key(key("other")),
        ])
        .unwrap();
    let task_id = navigator.routes()[0].id;
    let lifetime = navigator.lifetime_of(task_id).expect("mounted lifetime");
    let parent = runtime.spawner().scope();
    let binding = RouteTaskBinding::bind(&parent, &lifetime);
    let completed = Arc::new(AtomicBool::new(false));
    let completed_for_completion = completed.clone();
    runtime
        .spawner()
        .spawn_into_in(binding.scope(), async { 7_u8 }, move |result, _| {
            assert_eq!(result.expect("task completes"), 7_u8);
            completed_for_completion.store(true, Ordering::Release);
        });
    // Keyed reorder preserves the lifetime, and covering the route with a
    // new top deactivates without removing: neither cancels the scope.
    navigator
        .set_pages([
            Page::new("other", Widget::box_(Size::new(1., 1.), Color::WHITE)).key(key("other")),
            Page::new("task", Widget::box_(Size::new(1., 1.), Color::WHITE)).key(key("task")),
        ])
        .unwrap();
    assert!(lifetime.is_live());
    navigator.push_page(route_page("cover"));
    assert!(lifetime.is_live());
    pump_until(&mut runtime, &completed);
    assert!(!binding.scope().is_cancelled());
}

#[test]
fn window_scope_cancellation_cascades_to_bound_scope() {
    // `close_window` cancels the window scope; the binding adds no engine,
    // so the pre-existing parent-to-child cascade must reach the bound
    // scope. This exercises the real window scope object.
    let runtime = Runtime::new(Widget::box_(Size::new(1., 1.), Color::WHITE)).unwrap();
    let window_scope = runtime.window_task_scope();
    let navigator = Navigator::new();
    navigator.push_page(route_page("task"));
    let lifetime = mounted_lifetime(&navigator);
    let binding = RouteTaskBinding::bind(&window_scope, &lifetime);
    window_scope.cancel();
    assert!(binding.scope().is_cancelled());
    // The route itself is untouched: parent cancellation is not removal.
    assert!(lifetime.is_live());
}

#[test]
fn application_shutdown_cancels_bound_scope() {
    let runtime = Runtime::new(Widget::box_(Size::new(1., 1.), Color::WHITE)).unwrap();
    let navigator = Navigator::new();
    navigator.push_page(route_page("task"));
    let lifetime = mounted_lifetime(&navigator);
    let parent = runtime.spawner().scope();
    let binding = RouteTaskBinding::bind(&parent, &lifetime);
    let scope = binding.scope().clone();
    let mut application = Application::from_runtime(runtime, |_| {});
    application.shutdown();
    assert!(scope.is_cancelled());
    assert!(lifetime.is_live());
}

#[test]
fn repeated_cleanup_is_harmless() {
    let runtime = Runtime::new(Widget::box_(Size::new(1., 1.), Color::WHITE)).unwrap();
    let navigator = Navigator::new();
    navigator.push_page(route_page("task"));
    let lifetime = mounted_lifetime(&navigator);
    let parent = runtime.spawner().scope();
    let binding = RouteTaskBinding::bind(&parent, &lifetime);
    navigator.pop();
    assert!(binding.scope().is_cancelled());
    // Disposal re-ends the already-ended lifetime (no-op), and an explicit
    // second cancel stays a no-op: exactly-once ending, idempotent scope.
    drop(navigator);
    binding.scope().cancel();
    assert!(binding.scope().is_cancelled());
    assert!(!lifetime.is_live());
}

#[test]
fn binding_disposal_detaches_later_removal() {
    // Documented disposal policy: dropping the binding unregisters its
    // removal callback, so a later removal no longer cancels through it.
    let runtime = Runtime::new(Widget::box_(Size::new(1., 1.), Color::WHITE)).unwrap();
    let navigator = Navigator::new();
    navigator.push_page(route_page("task"));
    let lifetime = mounted_lifetime(&navigator);
    let parent = runtime.spawner().scope();
    let binding = RouteTaskBinding::bind(&parent, &lifetime);
    let scope = binding.scope().clone();
    drop(binding);
    navigator.pop();
    assert!(!lifetime.is_live());
    assert!(!scope.is_cancelled());
}

#[test]
fn panicking_sibling_cannot_spare_bound_scope() {
    // Mandatory cancellation is terminal commitment, not callback order:
    // an earlier lifetime callback panicking must not spare a later
    // route-bound scope. Uses real scopes through the real scheduler path
    // (cancellation itself, without pumping a completion).
    let runtime = Runtime::new(Widget::box_(Size::new(1., 1.), Color::WHITE)).unwrap();
    let key = |name: &str| PageKey::new(name).unwrap();
    let navigator = Navigator::new();
    navigator
        .set_pages([
            Page::new("doomed", Widget::box_(Size::new(1., 1.), Color::WHITE)).key(key("doomed")),
            Page::new("task", Widget::box_(Size::new(1., 1.), Color::WHITE)).key(key("task")),
        ])
        .unwrap();
    let doomed_id = navigator.routes()[0].id;
    let task_id = navigator.routes()[1].id;
    let doomed = navigator.lifetime_of(doomed_id).expect("mounted");
    let task_lifetime = navigator.lifetime_of(task_id).expect("mounted");
    let parent = runtime.spawner().scope();
    let binding = RouteTaskBinding::bind(&parent, &task_lifetime);
    assert!(!binding.scope().is_cancelled());
    // Retired order follows previous stack order: the veto attempt belongs
    // to the first removed route, the binding to the second.
    let _doomed_subscription =
        doomed.on_ended(|| panic!("sibling veto attempt must not spare the scope"));
    let result = catch_unwind(AssertUnwindSafe(|| {
        navigator
            .set_pages([
                Page::new("gone", Widget::box_(Size::new(1., 1.), Color::WHITE)).key(key("gone")),
            ])
            .unwrap();
    }));
    assert!(result.is_err(), "the panic resumes after delivery");
    assert!(!doomed.is_live());
    assert!(!task_lifetime.is_live());
    assert!(
        binding.scope().is_cancelled(),
        "no promised route-bound task survives another listener's failure"
    );
}

#[test]
fn binding_on_ended_lifetime_is_immediately_cancelled() {
    let runtime = Runtime::new(Widget::box_(Size::new(1., 1.), Color::WHITE)).unwrap();
    let navigator = Navigator::new();
    navigator.push_page(route_page("task"));
    let lifetime = mounted_lifetime(&navigator);
    navigator.pop();
    let parent = runtime.spawner().scope();
    let binding = RouteTaskBinding::bind(&parent, &lifetime);
    assert!(binding.scope().is_cancelled());
}
