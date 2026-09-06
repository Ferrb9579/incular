# API migration inventory

Stage B decisions, based on the Stage A tree. API classification is exhaustive
by namespace inheritance in `specs/architecture.json`; this inventory records
the exceptions and migration families, not a second list of every method.
“Keep” means one implementation with multiple useful names. “Migrate” means
current code is still callable but must change in the named stage.

## Aliases and duplicate entry paths

| Surface | Decision and owner | Consumers / validation |
| --- | --- | --- |
| `incular_painting::*`, facade `painting` | Keep pure import shim until J; rendering is canonical (B08). No implementation may be added. | Existing external imports; no current example imports painting. Compatibility type-identity test protects the shim. |
| config/layout constraints, alignment, insets and geometry re-exports | Keep exact type aliases/re-exports; config owns policy, core owns geometry. | Layout/config public API tests; `examples/layout_gallery`. |
| core `Signal` / runtime `Signal` | Implemented in C: runtime/facade Signal is the application API; core reactivity owns dependency sources and subscription tokens. See REACTIVITY.md. | `examples/counter`, `controls_gallery`, `multi_window`, `simulation`; core and runtime reactive tests. |
| core/runtime/borrowed widgets `BuildContext` | Keep distinct capabilities only over one dependency engine in C. Do not collapse their lifetime differences into one untyped context. | Retained/inherited context tests and application builder examples. |
| core `Widget` / `BuildableWidget` versus widgets `Widget` | Core traits are legacy bridge; migrate consumers in F, then remove unused transport. Widgets descriptor is the application composition type. | Core public API tests and facade compile families; no application example should adopt the bridge. |
| widgets/navigation `Navigator`, route models and back dispatchers | Migrate in G to one stack owner with presentation adapters; keep current graph direction until state is extracted safely. | `examples/restoration`, navigation integration tests and Widgets composition tests. |
| desktop single-root / `run_application`; native/facade `run` | Migrate implementations in E to one host; keep convenience wrappers that forward without semantic differences. | Native resize/transient harnesses and `examples/multi_window`. |
| widgets root / `extensions` / `internal` / `devtools` | Root and neutral extensions are application vocabulary. Internal/devtools remain bridges; do not promote retained IDs or action surfaces to application API. Narrow existing bridge exports in F. | Boundary tests, sibling runtime/controls/WGPU integration and simulation tests. |
| `AsyncValue<T>`, `TokioHandle` | Keep aliases for `AsyncState<T, TaskFailure>` and Tokio handle; one task implementation. | Async state/task tests and async application examples. |
| `StringKey`, `KeyObject`, `SemanticRole`, `BaselineChild` | Keep ergonomic names for the same value; do not fork identity or role logic. | Core, semantics and layout public API tests. |
| `DirtyFlags`, `AnimatedValue`, `ImplicitAnimatedValue` | Keep exact aliases while F/G consolidate their consuming contracts; invalidation and animation have one value owner. | Invalidation and animation contract tests. |
| controls popup/dialog/drawer/preview-card slots, theme-token aliases | Keep aliases only where behavior is identical; typed wrappers may supply presentation. Consolidate divergent interactions in F. | `examples/controls_gallery`, `material_gallery`; popup and control interaction tests. |
| generated `TypedBuilder` types and fluent constructors | Keep both construction forms with the same final descriptor and validation. Never introduce a separate behavioral path for one builder. | Visibility option matrix and construction compile tests. |

## Parameters currently ignored

These are explicit migration debt, not claims that their names have the promised
effect today. An underscore is a search lead, not proof of a defect: trait default
methods and unsupported backend implementations may intentionally ignore input
when they return an explicit unsupported result.

| API | Current meaning and decision | Stage / consumers |
| --- | --- | --- |
| `Signal::with_runtime(value, _runtime)` | Removed in C. Use `Signal::new`; cloned handles explicitly share state across same-thread roots. | C; runtime tests, no current example calls this constructor. |
| Application/Runtime `process_runtime_work_at(_now)` | Supplied timestamp is unused. Remove the misleading clock-taking entry or connect it to a real clock abstraction; keep ordinary task pumping. | C/E; simulation and runtime service tests. |
| `TreeSliverController::expand_all_at` / `collapse_all_at` | Timestamp unused for immediate all-node operations. Rename to immediate operations or implement timed transitions. | G; advanced sliver tests. |
| Tooltip `show_at` / `hide_at` | Timestamp unused for immediate visibility mutation; preserve explicit outcomes and distinguish scheduling from immediate changes. | F/G; tooltip tests and popup examples. |
| controls radio/autocomplete `build(_theme)` | Custom/content composition ignores the argument. Unify themed/default/custom composition contracts; remove argument where no presentation needs it. | F; controls and Material galleries. |
| `ButtonStyle::resolve_elevation(..., _theme)` | Elevation currently resolves solely from style/state. Remove unused theme parameter or make fallback policy real. | F; style resolution tests. |

## Public fields and mutation ownership

Generated builders, descriptor fields and value structs inherit the descriptor
mutation contract. Public fields never notify the retained tree by themselves.
The following family decisions apply to all fields, not just the named examples.

| Field family | Keep / migrate rule | Stage / consumers |
| --- | --- | --- |
| Plain geometry, event records, metrics snapshots, semantic state | Keep public value fields when arbitrary values are representable; validate at the consumer boundary where required. A snapshot mutation does not change its source controller. | Ongoing; all examples and serialization/projection tests. |
| `Constraints` minima/maxima | Completed in F: private bounds, read-only accessors and `try_new`; see the migration below. Other invariant-bearing builders remain under audit. | F; `examples/layout_gallery`, layout/config and widget layout tests. |
| Layout algorithm descriptors (`Flex`, `Stack`, `Visibility`, etc.) | Keep plain algorithm configuration when its consumer validates it. Distinguish it from retained widget descriptors and their lifetime. | F/G; layout public API tests. |
| Tween endpoints/segments, physics and style parameters | Keep freely meaningful values; validate or normalize bounded parameters at the authoritative owner. Document normalization instead of silently differing by builder path. | G; animation/physics and style tests. |
| Widget/controller fields containing callbacks, Rc/Cell, listener IDs or ownership handles | Migrate writable lifecycle state behind owner operations; registrations need explicit cleanup. Retain immutable descriptor callbacks as values. | C/F/G; retained widget and controller tests. |
| Native IDs/options, request records and diagnostic edit closures | Backend/bridge contract applies. Do not expose device/tree mutation through application configuration. Reject stale generation and terminal requests. | D/E/H; service and DevTools tests. |

Stage B establishes these obligations; it does not claim every existing public
mutation already satisfies them. Stages C–H implement the listed contracts and
Stage I verifies the historical member manifests. A migration is complete only
when its consumers, examples, rustdoc, error outcomes and tests agree.

## Stage E desktop migration

- Import Winit conversion helpers (`key_event`, `ime_event`, pointer/touch/wheel/
  trackpad conversion and `apply_text_input_command`) from
  `incular_desktop::winit_adapter`. `RawWindowHandles`, `raw_window_handles` and
  `native_window_system` also move there. The platform crate retains portable
  `NativeWindowSystem` values and has no Winit/raw-window-handle dependency.
- Custom desktop backends implement `DesktopApplicationShellServices` for tray,
  notification and taskbar operations, and return that backend from
  `DesktopPlatformServices::application_shell()`. Other window service methods
  remain on `DesktopPlatformServices`; the default shell service is unsupported.
- `run_window` retains its signature and now adopts its runtime through
  `Application::from_runtime(runtime, on_action)`. This backend entry preserves
  the mounted tree, scheduler and queued work. It uses the same lifecycle,
  capabilities, menus, input, accessibility and rendering host as `run_application`.
  The callback observes committed activations (pointer release, keyboard and
  accessibility), rather than pointer hover/down hit-test targets. This corrects
  duplicate callbacks in the old standalone path.

## Stage F constraints migration

`Constraints` bounds are private. Replace reads such as `constraints.max_width`
with `constraints.max_width()`, and replace struct literals or field mutation
with `Constraints::new(min_width, max_width, min_height, max_height)`.
Use `Constraints::try_new` for external input that can be invalid; it returns
`ConstraintError::Invalid`. `new`, `tight` and `loose` keep their panic contract.
Minimums must be finite and nonnegative; maximums may be positive infinity,
but cannot be NaN or smaller than their corresponding minimum.
