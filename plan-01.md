# Plan 01 - Platform Capability, Command, and Error Foundation

## Goal

Create one durable contract for desktop capabilities and native operation results before adding more platform features.

Today `WindowHandle` largely reports whether a command was queued, while native support/failure often cannot be observed by the caller. Several future features also differ materially by OS/compositor. Fix that boundary first instead of adding ad-hoc booleans and silent no-ops.

## Architecture

### Portable capability model

Add structured capability snapshots in `incular-platform`, separated by domain rather than one giant bitfield:

- window control;
- display/placement;
- transient surfaces;
- native menus;
- data transfer;
- application services;
- advanced input.

Capabilities describe what the active backend/session can actually do. They are runtime values where support can depend on session/backend, e.g. Wayland vs X11.

Do not expose a single `is_desktop` or `supports_everything` flag.

### Typed operation results

Replace ambiguous command-send booleans with typed enqueue errors.

For operations whose native result matters, add stable request identity and completion:

```text
WindowHandle / service handle
  -> portable request + request id
  -> UI/event-loop backend
  -> native API
  -> typed completion event/result
  -> runtime resolves request/future/callback
```

Do not make every trivial setter async. Distinguish:

- queue acceptance failure;
- unsupported capability;
- native execution failure;
- asynchronous OS acknowledgement where relevant.

Use one reusable request/completion primitive rather than inventing a bespoke future type for every service.

### Error taxonomy

Use domain errors with stable categories, for example:

- `Unsupported`;
- `Unavailable`/session capability missing;
- `RejectedByPlatform`;
- `InvalidState`;
- `StaleResource`;
- native failure with safe diagnostic context.

Do not expose raw `winit::error::ExternalError`, HRESULT, NSError, Wayland protocol objects, etc. through the portable API.

## Crate changes

- `incular-platform`: capabilities, portable operation/request IDs, result/error contracts.
- `incular-runtime`: request tracking/completion and stale-resource cleanup.
- `incular-desktop`: common capability discovery and native-result normalization.
- OS crates: extend capability snapshots for native-only facilities.
- `incular`: re-export only deliberate public surface.

Do not put capability discovery in widgets.

## Hard invariants

1. A successful enqueue is not represented as successful native execution.
2. Unsupported operations are observable without relying on logs.
3. Stale window/service/resource handles cannot affect a new resource that reused a slot.
4. Request completion occurs at most once.
5. Dropping a request handle cannot leak backend state.
6. Backend-specific errors are normalized at the boundary.
7. Capability queries never require constructing fake native resources.

## Implementation sequence

1. Inventory existing platform/runtime operations and classify whether they require native completion.
2. Introduce portable capability and error types with unit tests.
3. Replace `bool` command-send semantics with typed enqueue results where public.
4. Add request/completion machinery for result-bearing operations.
5. Integrate current desktop window commands without changing their behavior.
6. Expose application/window capability snapshots through stable runtime handles.
7. Add debug/DevTools visibility for capabilities and last native operation failure where useful.
8. Remove superseded legacy result paths rather than retaining compatibility wrappers.

## Acceptance tests

- Enqueue failure is distinguishable from native unsupported/failure.
- Unsupported operation resolves exactly once with typed error.
- Late completion for a closed/reused window is rejected as stale.
- Multiple windows can issue concurrent requests without cross-resolution.
- Headless/memory backend can deterministically declare capabilities and simulate completion.
- Existing window title/visibility/size/focus/redraw/close behavior remains correct.

## Validation

Run the campaign-wide validation from `plan-00.md`, plus focused platform/runtime tests.

## Completion condition

Do not start adding broad native service APIs until this contract is stable enough that later plans do not need to redesign error/capability handling.
