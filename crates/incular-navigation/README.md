# incular-navigation

Stack navigation, deep-link routing, route transitions, and overlay state for
Incular.

This crate depends one-way on `incular-widgets` for widget descriptions and
retained transition layers. Widgets do not depend on navigation: applications
can use regular widget trees without a route stack, while the public facade
reexports the navigation API for the conventional application-facing surface.

## Optional route restoration

`Navigator` can persist an opt-in, declarative stack through a
`RouteRegistry`. Register every route that is safe to restore with a stable
application ID and a builder which validates JSON arguments:

```rust,ignore
let home = routes.register_restorable("/home", |arguments| {
    let tab = arguments.get("tab").and_then(Value::as_str)
        .ok_or_else(|| RestorableRouteBuildError::invalid_arguments("missing tab"))?;
    Ok(Page::new(format!("home-{tab}"), build_home(tab)))
})?;

routes.navigate_restorable(
    &navigator,
    RestorableRoute::new(home, json!({ "tab": "recent" })),
)?;
```

Store `navigator.restoration_snapshot()` as one JSON value in an application
restoration scope, then load it before mounting UI and call
`RouteRegistry::restore_navigator_or`. The snapshot contains only the
versioned stack order, stable route IDs, JSON arguments/state, optional stable
route-scope keys, and active-route index. It never contains route builders,
widgets, runtime `RouteId`s, transitions, or overlays.

An unregistered route is transient. A snapshot stops at the first transient
entry so dialogs, context menus, tooltips, and arbitrary ordinary routes do
not reappear after restart. During restore, an unknown route or invalid
arguments truncates the saved stack to its longest valid prefix. If its root
cannot be rebuilt, `restore_navigator_or` installs the explicit fallback page;
the Navigator never receives an empty invalid restored stack.

Dynamic restorable route instances can carry a `RouteScopeKey`, a validated
single restoration-path segment supplied by the application. For routes
registered with `register_restorable_with_scope_cleanup`, a deliberate
`Navigator::pop` invokes the optional `set_route_scope_cleanup` bridge with
that stable key. Connect that bridge to the runtime restoration manager to
remove the route's own scope. Declarative `set_pages` reconciliation does not
trigger cleanup, so a temporary unmount or rebuild cannot erase persisted
state.

Route snapshots are small declarative session state, not secure storage. Do
not put passwords, tokens, files, images, or other secrets in route arguments
or route state.

## Nested back dispatch and observers

`Navigator::observe` registers a lifetime-scoped callback for push, pop,
replace, and active-route changes. Keep the returned `NavigatorObserver` token
for as long as the callback is needed; dropping it unregisters the observer.

Independent nested stacks use `BackDispatcher`. Attach a child dispatcher to
its parent and mark the focused child active. `dispatch_back()` then tries the
deepest active navigator first and walks outward only when that child cannot
pop, so one platform back event can never pop two stacks. `set_pop_guard`
supports unsaved-work confirmation by returning `PopDecision::Deny` until an
application is ready to permit the route mutation.
