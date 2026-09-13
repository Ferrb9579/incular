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
| `Stack::text_direction` | Removed. Stack alignment factors are absolute (left/right, not start/end), so direction never reached layout and the option was an accepted no-op. Use the absolute alignment you mean, or wrap directional content in `Directionality` and resolve the alignment before constructing the `Stack`. The `incular-layout::Stack` policy struct likewise drops the field. | B; `specs/stack_layout_properties.json`, `stack_layout.rs`/`layout_stack.rs`. |
| `incular_layout::Table::alignment` | Removed. The field was set to `TOP_LEFT` by its only consumer and never read by `layout_table`; the public `Table` widget never exposed it. Cells render top-left in their slot; align content inside a cell instead. | C; `specs/collections_properties.json`, `tree_layout_painting.rs`. |
| `Scrollable::axis_direction` | Removed. The builder receives only the controller, so a direction set on `Scrollable` never reached layout and the option was an accepted no-op. Set the axis on the built viewport (`Viewport`, `CustomScrollView`, `ListView`) instead. | C; `specs/scroll_viewport_lists_properties.json`, `scroll_viewport_lists.rs`. |
| `ListView::clip_behavior`, `CustomScrollView::clip_behavior` | Removed. The retained viewport attaches its clip unconditionally (`AttachmentSpec::Clip`), so neither override ever reached paint and both were accepted no-ops. Viewports keep clipping with the shared default; a follow-up group must decide real `Clip::None` passthrough (including overdraw and hit containment) before re-exposing either option. | C; `specs/scroll_viewport_lists_properties.json`, `scroll_viewport_lists.rs`. |
| `SingleChildScrollView::clip_behavior`, `GridView::clip_behavior`, `AnimatedList::clip_behavior`, `AnimatedGrid::clip_behavior` | Removed. Same trace as above: `SingleChildScrollView` never forwarded its override anywhere, and the grid/animated overrides forwarded only into the removed viewport options. `PageView` (later group) is untouched. | C; `specs/scroll_grid_single_properties.json`, `scroll_grid_single.rs`. |
| `LayerTree::publish_leader_links(&self)` | Receiver is now `&mut self`: the pass counts `leader_publish_passes`/`leaders_published` diagnostics. Behavior is identical; the only in-repo caller (`WidgetTree::update_semantics`) already holds `&mut`. | W3; `crates/incular-rendering/tests/rendering.rs` (`leader_publication_passes_and_counts_are_measured_per_frame`). |
| `Navigator::set_pages` name matching | Changed. Route names no longer identify retained positions: only page keys do, `set_pages` returns `Result<(), DuplicatePageKey>`, and unkeyed pages mount anew on every reconciliation. Give pages stable `PageKey` values to preserve route lifetimes; handle the duplicate-key error before assuming the stack changed (rejection leaves stack, revision, and observers untouched). | W4; `crates/incular-navigation/tests/navigation.rs` (keyed reorder, same-name keys, duplicate rejection, removal/reinsertion, iterator mutation). |
| `BackDispatcher::attach_child` | Changed. Returns `Result<(), BackAttachError>`: self-attachment and attachments that would cycle back dispatch are rejected before mutation, leaving children and the active selection unchanged. The topology is an acyclic graph by design choice: sharing is permitted because dispatch follows a single active chain (pinned by the diamond test), while cycles are rejected by reachability since they would recurse forever. Reattaching the same child stays an idempotent no-op. | W4; `crates/incular-navigation/tests/navigation.rs` (self/two-node/longer cycles, diamond, duplicates, detach/reattach, dead children, deep chains, inactive branches). |
| `RoutePresentation::Modal::focus_trap` | Removed. The flag promised modal focus trapping that no traversal or dispatch path implemented — `Runtime::focus_next` and keyboard dispatch are tree-global with no scope clamp — so it was an accepted no-op. Modal routes still block background input through the barrier (`blocks_background_input` is unchanged); constraining traversal to the modal subtree is future work with no stub standing in for it. Construct modals through `RoutePresentation::modal(barrier)` as before. | W4; `crates/incular-navigation/tests/navigation.rs` (modal entries keep barrier isolation without the flag). |
| `RouteOutlet::present_frame` errors | Changed. Returns `Result<_, OutletError>`: stacks containing portal-managed overlay routes fail typed (`UnsupportedPresentation { outlet, routes }`, naming the responsible outlet for nested rejections, before any mounted or committed state changes) instead of silently omitting them, and frame failures surface as `OutletError::Frame`. The whole outlet tree preflights before revisions/capture/rebuild/frame and re-validates after the frame (builders may navigate mid-build), with the pending capture surviving for retry; handle the error before assuming the frame presented. | W4; `crates/incular-runtime/tests/runtime/route_outlet.rs` (overlay rejection, nested rejection identity, sibling recovery, mid-build introduction, rebuild/frame failure recovery). |
| `RouteOutlet::after_frame` / `restore_active` | Removed. Manual widget/frame/after_frame driving is no longer supported: the unvalidated completion path could commit against unpreflighted stacks, so every cycle now goes through the single `present_frame` operation (attach once, then rebuild, frame, and reconcile with tree-wide validation in enforced order). Custom hosts that own their frame loop gain nothing from the hatch — `present_frame` owns invalidation internally. | W4; `crates/incular-runtime/tests/runtime/route_outlet.rs` (manual test deleted, teardown coverage migrated to `present_frame`). |
| `RouteOutlet::attach` errors | Changed. Returns `Result<(), OutletError>` (was `Result<(), TreeError>`): attaching a nested child as a root mount, or attaching to a second live runtime while another owns the outlet, fails typed (`SeparateDrive`, `DriverConflict`) before registering anything. A missing mount still surfaces as `OutletError::Frame` wrapping the configuration error. Detach nested children first, or drop the owning runtime (teardown releases the claim), before attaching elsewhere. | W4; `crates/incular-runtime/tests/runtime/route_outlet.rs` (separate-child, second-runtime, cross-runtime, transfer, teardown-release tests). |
| `Page` presentation and transition | Added. `Page` carries `presentation` (`RoutePresentation::page()` by default) and `transition` (`RouteTransition::None` by default) with chainable `presentation()`/`transition()` setters beside `key()`, so keyed popups and modals build declaratively — no imperative `Route` needed for transparent or veiled routes. `set_pages` applies the latest child, transition, and presentation to the retained entry: configuration never ends a lifetime, only key removal (or no key) does. Replaced application-owned values (children, transitions, overlay entries) retire outside navigator borrows, as before. | W4; `crates/incular-navigation/tests/navigation.rs` (keyed popup/modal, same-key presentation/retention/transition updates, reorder preservation, duplicate rejection, restoration survival), `crates/incular-runtime/tests/runtime/route_outlet.rs` (mid-build keyed-popup reorder, modal barrier repaint with task continuity). |
| `RouteOutlet` nested composition | Changed. `attach_nested` is now an associated function taking outlet handles and returning `Result<(), OutletAttachError>`: self-attachment and cycles are rejected before mutation, each outlet takes at most one live parent (a shared child would drive twice per frame — detach explicitly before moving parents), and re-registering with the same parent is an idempotent no-op. Nested mounts go through the `nested_widget` composition helper (hosts no longer assemble stateful-builder slots or touch revision handles — `revision()` is removed), and presenting the outer outlet drives the whole tree in one frame with each outlet driven exactly once (see `frame_drive_count`). Register each outlet with its direct parent; dropping a nested outlet releases it, and `detach_nested` stops its frame work while retained. | W4; `crates/incular-runtime/tests/runtime/route_outlet.rs` (nearest-outlet ownership, replacement boundedness, detach release, cascade-only driving, topology rejection, drive counts). |

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
