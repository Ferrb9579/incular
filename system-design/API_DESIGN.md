# Incular API design

Incular exposes Flutter-familiar concepts through idiomatic Rust and a retained
runtime. API design must follow the ownership rules in
[ARCHITECTURE.md](ARCHITECTURE.md).

## Core model

- Widgets are lightweight declarative descriptions.
- Elements, render state, display lists, compositor layers, semantic nodes and
  controller state are retained across frames.
- Rebuilds reconcile descriptors against retained identity; they do not recreate
  the whole UI.
- State that must survive rebuilds belongs in controllers, signals, runtime/domain
  owners or retained objects—not widget descriptors.

## Constructors and configuration

- Constructors take required inputs only.
- Optional configuration uses fluent builders.
- Builders and generated builders must produce the same validated descriptor.
- Use `From<T> for Widget` for composition where conversion is lossless and clear.
- Public configuration must not silently accept an option that has no effect.
  Implement it, reject it, or remove it.

## Mutation and results

Choose the result type that matches the operation:

- synchronous local mutation: direct return value;
- optional/blocked operation: `Option` or a small outcome enum;
- fallible operation: `Result<T, E>`;
- asynchronous/native work: `Future<Output = Result<T, E>>`;
- genuinely binary observation/policy: `bool`.

Do not use one boolean for queued/applied/unsupported/failed states.

User-controlled invalid input should have a checked path with a typed error.
Panics are for programmer contract violations and must name the violated invariant.

## Invalidation

Properties invalidate only the phases they affect:

- **BUILD**: structure/reconciliation.
- **LAYOUT**: measurement or positioning.
- **PAINT**: display-list recording.
- **COMPOSITE**: retained layer/compositor properties.
- **SEMANTICS**: accessibility state/tree.
- **HIT_TEST**: pointer target geometry/routing.

Do not trigger layout or paint merely because a broader change would be easier to
implement.

## State, controllers and callbacks

- Controllers such as scroll, text, focus and page controllers are retained state
  owners.
- UI state is thread-affine unless an API explicitly states otherwise.
- Callback registration has explicit lifetime ownership and cleanup.
- Never invoke arbitrary user callbacks while holding mutable domain borrows.
- Reentrant callbacks are legal unless explicitly prohibited; outer operations
  must verify that their assumptions still hold afterward.

## Async and native APIs

- Admission is not completion.
- Dropping a future must have documented cancellation semantics.
- Window/application generations protect against stale native callbacks.
- Unsupported backend behavior returns an explicit typed outcome.
- Shutdown resolves or cancels owned pending work; it must not silently orphan it.

## Text and Unicode

- Incular strings are UTF-8.
- Editing/cursor/selection operations must respect UTF-8 and grapheme boundaries.
- Shaping and bidi behavior come from the text engine rather than byte/character
  guesses.
- IME preedit remains separate from committed text until the platform commits it.

## Visual and component APIs

- Neutral widgets provide layout, input, semantics and explicit paint/composition
  mechanisms without design-system defaults.
- Controls provide Incular visual policy over neutral behavior.
- Material adds Material styling/composition without duplicating behavior owners.
- Custom visual slots must preserve the same focus, activation and semantics path
  as default visuals.
- Conflicting visual representations should be structurally prevented or resolved
  deterministically by the owning type.

## Animation and interpolation

- Long-lived animation state belongs to retained controllers/runtime clocks.
- Interpolation uses type-owned policies; continuous numeric/geometry values
  interpolate, discrete values switch according to an explicit threshold/policy.
- Reduced-motion policy changes timing/presentation, not ownership.

## Public API quality

Every public API should make these clear:

1. who owns the state;
2. what a successful call means;
3. what invalidation/work it schedules;
4. how failure is represented;
5. what happens on drop/unmount/shutdown;
6. whether it is application, backend or bridge API.

Prefer a smaller truthful API over compatibility surface that compiles but does
nothing. Flutter naming may be used when the concept maps cleanly; Rust ownership,
types, futures, errors and lifecycle semantics take precedence.
