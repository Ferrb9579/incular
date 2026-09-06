# Reactive state and ownership

Application code uses `incular::Signal` (the runtime implementation), `Memo`,
`Effect`, and `Action`. Construct persistent handles outside rebuilding closures
and capture clones. There is no separate core signal or runtime binding argument.

## Reads and writes

`Signal::get` and `with` subscribe the current builder, memo, effect, or core
context consumer. `get_untracked` and `with_untracked` read without creating an
edge. Each evaluation replaces its previous dependencies: switching a branch
unsubscribes the branch that is no longer read. Clones share identity and state.

`set` compares with `PartialEq`; `update_if_changed` compares before and after.
Equal values do not invalidate readers. `update` always invalidates after the
mutation. Writes are synchronous, but runtime work is queued and deduplicated:
several writes before a frame produce one pending rebuild per consumer. This is
frame coalescing, not a transaction or rollback mechanism.

Signal mutation during builds or memo evaluation panics before modifying state.
Use event callbacks, actions, or effects for writes. Memo computations must be
pure. Controllers also serve retained layout/input backends, so their domain
mutations are not universally forbidden during a frame.

## Derived values and effects

Memos evaluate lazily and keep their own source subscriptions. Runtime-bound
memos recompute when dirty and propagate changed results using `PartialEq`.
Nested reads ensure each dirty input is fresh before its consumer computes;
diamond graphs observe consistent values. Recursive evaluation panics with a
memo dependency-cycle error. Before a runtime owns a memo's subscriptions,
standalone reads evaluate directly instead of caching without an invalidation
owner. Untracked memo reads still maintain the memo's runtime dependencies.

`Effect::mount` schedules its first run in the runtime reactive phase, outside
the builder. Keep a stable handle; mounting it again in the same root is
idempotent. Each run replaces its read dependencies. `dispose`, mounting-owner
unmount, window disposal, and runtime teardown remove its subscription edges.
An external effect handle still owns its callback and captured resources after
disposal; drop that handle to release those resources. Effects that write their
own inputs must converge; they are not a substitute for a pure memo.

`Action::dispatch` starts work explicitly. Reading action state is tracked.
A newer dispatch cancels the previous task and rejects stale completions using
the dispatch generation. Owner cancellation prevents later delivery. Cancellation
drops asynchronous work and suppresses results; it cannot undo external work
already performed or guarantee interruption of a blocking service.

## Controllers and listeners

Tracked text value/text/selection/composing reads, scroll geometry reads, focus
state reads, form errors/revisions, Material widget states, and animation value
reads use the same dependency source as signals. They can be read directly in
builders and reactive computations without forwarding their changes into a
second signal or calling a manual refresh method.

Text, form, animation, and focus `observe` methods return owned subscription
tokens. Keep the token for the listener's lifetime; dropping it unsubscribes
immediately. Dispatch snapshots observers outside registry borrows, skips tokens
dropped during dispatch, and defers newly registered observers until the next
notification. Existing numeric registration methods remain for low-level
integrations that explicitly pair registration and removal. Scroll's consumable
event notifications retain their domain-specific subscription API.

## Windows, contexts, and diagnostics

Signals are UI-thread state (`Rc`, not `Send`/`Sync`). Sharing clones explicitly
across windows or independent applications on the same thread shares the value;
each retained root has its own subscriptions and scheduling hook. Closing one
root removes only its edges. Keyed element identity remains owned by the widget
tree. Core context clones share a consumer lease; its last owner drops the
consumer's edges.

The backend module `incular_core::reactivity` provides `DependencySource`,
`Subscription`, collection guards, and value storage. Hosts supply stable
consumer metadata and invalidation callbacks; core knows no element, window,
executor, or rendering phase. This module is infrastructure, not another
application state API.

DevTools registration is tied to the signal's last owner. Renaming updates the
existing ID and preserves editing configuration. Dropping the signal removes
its row. Registry visitors receive a snapshot and may rename, edit, or drop
signals safely. UI subscriber diagnostics enumerate element edges;
`Signal::dependent_count` includes all source edges, including graph and context
consumers.

## Examples and migration

- `examples/counter`: equality-aware application signals.
- `examples/text_field`: editable form validation and a tracked greeting.
- `examples/reactive`: memo, effect, and explicit asynchronous topic search.
- `examples/multi_window`: shared state across independently scheduled windows.

Replace `incular_core::Signal` with the facade/runtime signal. Replace
`Signal::with_runtime(value, runtime)` with `Signal::new(value)`. Backend code
creating `SignalRegistration` supplies shared `Rc` callbacks so snapshots can be
visited outside the registry borrow.
