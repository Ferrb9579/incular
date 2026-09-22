# Application guide

Start with the [standalone application](../README.md#first-application), then
explore these checkout examples. Commands run from the repository root. The
public entry point is `incular`; backend crate use requires following its
lifetime and thread contracts.

## State and lifecycle

Widgets describe the next tree. The runtime retains elements, layout, paint,
focus and semantics. Mutating a descriptor after mounting does not change the
mounted tree. Store persistent state in signals/controllers outside the build
closure. A build reads that state and returns the next description.

The `counter` example creates `Signal::new(0_u32)` outside `Application::new`,
clones it into the build closure, reads with `get()`, and calls `update` from
the button callback:

```text
cargo run -p incular --example counter
cargo run -p incular --example reactive
```

`incular::reactive::{Memo, Effect, Action}` provides derived/async behavior.
Qualify Effect and Action because similarly named rendering/widget types exist.
UI state is thread-affine unless documented otherwise. Keep registration tokens
for their intended lifetime. Callbacks may reenter application code; avoid
retaining mutable borrows across them. See [API ownership](../system-design/API_DESIGN.md).

## Layout and composition

Neutral `Container`, `Row`, `Column`, `Text`, and other widgets compose into
`Widget` through `Into<Widget>`. Required arguments use constructors; optional
configuration uses fluent methods or builders. Constraints, padding, alignment,
and logical dimensions are explicit. Prefer responsive constraints over one
assumed window size/DPI; keep controllers outside repeated build calls.

```text
cargo run -p incular --example widget_basics
cargo run -p incular --example layout_gallery
cargo run -p incular --example responsive_layout
```

## Controls, Material, and themes

`incular::prelude::*` contains neutral types. Add
`incular::controls_prelude::*` with `controls`, or
`incular::material_prelude::*` with `material`. Material builds on Controls and
neutral widgets; custom styling must preserve focus, keyboard, semantic, and
disabled behavior rather than implement another interaction mechanism.

```text
cargo run -p incular --example controls_gallery
cargo run -p incular --example material_gallery
cargo run -p incular --example material_workbench
```

## Input, text, localization, and accessibility

Use gesture/focus widgets for input and editing controllers for text state.
Text is UTF-8; byte offsets are not grapheme-aware selection positions. IME
preedit and committed text have different lifetimes. Label interactive controls
and meaningful images; mark decorative content accordingly. Check keyboard
traversal and a native screen reader in addition to pointer input.

```text
cargo run -p incular --example gesture_gallery
cargo run -p incular --example text_field
cargo run -p incular --example text_selection
cargo run -p incular --example localization
cargo run -p incular --example semantics_gallery
```

## Async work and native services

Use runtime task scopes and async facilities to keep work off the UI thread.
Scope ownership controls cancellation. Do not assume a queued native request has
completed: handle typed errors, unsupported capabilities, stale windows, and
shutdown. File dialogs, clipboard, activation and notifications vary by OS.

```text
cargo run -p incular --example async_runtime
cargo run -p incular --example multi_window
```

The async example demonstrates timers, blocking work, cancellation, and UI-state
completion. Check admission and eventual results separately for native requests.

## Navigation and restoration

Use navigation stacks for routes and runtime route outlets for mounted
composition. Route/task owners control cleanup on pop or replacement.
Restoration is opt-in and versioned; validate input and tolerate old, missing,
or invalid data. Choose an application-owned path and avoid storing secrets.

```text
cargo run -p incular --example navigation_showcase
cargo run -p incular --example restoration
```

## Scrolling, assets, and rendering

Use retained scroll controllers and lazy/sliver widgets for large collections.
Renderer-neutral canvas/path APIs live in `incular::painting`; WGPU resources
belong to the backend. Image decode limits and cache residency limits differ:
an accepted image can exceed a cache's admission budget. External owners may
keep data alive after cache eviction.

```text
cargo run -p incular --example scrolling_complete
cargo run -p incular --example images
cargo run -p incular --example canvas_layers
cargo run -p incular --example performance_gallery
```

See [DevTools](devtools.md) for instrumentation/privacy and [local validation](testing.md)
for simulations and native scenarios. Include a minimal reproduction, complete
native error and platform details in reports; review diagnostics before sharing.
