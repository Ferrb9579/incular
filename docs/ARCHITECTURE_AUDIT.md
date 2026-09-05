# Architecture and quality audit

Date: 2026-09-05. Baseline: `0ba7058`. Working tree was clean before this audit.

This is a repository-wide architecture review with targeted source tracing, not a claim that every function is correct. All workspace manifests and the fourteen campaign plans were inventoried; the implementation paths cited below were inspected. Findings marked **confirmed** follow directly from source. Items marked **audit** require a reproducer, measurement, or native-platform verification before being treated as defects. No production implementation was changed.

The implementation plan is [plan-14.md](../plan-14.md).

## Assessment

The project has useful foundations: immutable reference-counted widget descriptors, retained generational trees, separate painting commands and GPU execution, typed platform capabilities, deterministic simulation, property tests, and operation-count performance tests. Preserve these. Signals and the absence of Stateless/Stateful widget classes are deliberate design choices, not debt.

The main problem is inconsistent enforcement of those foundations. New features have introduced parallel protocols, extra compatibility surfaces, and public options that do not survive lowering. The most urgent work is correctness and ownership, followed by convergence on fewer authoritative implementations. Splitting large files alone would leave the problem intact.

## Findings

### F01 — Safe GPU constructors do not enforce native-window lifetime — P1, confirmed

Evidence: `crates/incular-platform/src/lib.rs:704` exposes a copyable `RawWindowHandles` with public fields. `crates/incular-wgpu/src/renderer/state.rs:111` documents an outliving requirement but exposes safe `new` and `new_with_shared`. `crates/incular-wgpu/src/resources.rs:208` and `:351` call unsafe surface creation from these detached handles.

A caller can retain handles after dropping the window and pass them through a safe API. Correct field-drop ordering inside the desktop runner protects that runner, but cannot make the public API sound for arbitrary callers. The asynchronous constructor also provides no ownership tie across suspension.

Use an owned window/surface target or a lifetime-carrying borrow for safe construction. If detached raw handles remain necessary, expose only an explicitly unsafe backend constructor with complete safety obligations. Apply the same contract to surface recreation. Do not attempt to reproduce undefined behavior in a test.

### F02 — Visibility options disappear before retained execution — P1, confirmed

Evidence: `crates/incular-widgets/src/layout/basic/visibility.rs:10` stores `maintain_state`, `maintain_size`, `maintain_animation`, and `maintain_semantics`. Conversion at `:67` only uses state/size to choose whether to keep a child; the retained descriptor carries only `visible` and `child`. Hidden layout at `tree/layout/behavior/containers.rs:368` returns constrained zero size. Semantics at `tree/semantics.rs:127` drops hidden visibility subtrees unconditionally.

Thus `maintain_size(true)` does not preserve child size; animation and semantics flags are discarded. `Offstage` is implemented through this same path. The existing basic builder test checks construction equivalence, which cannot catch this.

Define an explicit retained hidden-child policy covering mount, measurement, paint, hit testing, animation ticking, focus, and semantics. Validate combinations once and carry the policy through every relevant phase. Each exposed option needs an observable behavior test.

### F03 — Shared GPU caches retain resources after local eviction — P1, confirmed

Evidence: `crates/incular-wgpu/src/resources.rs:176` owns strong image and gradient maps; image insertion occurs at `:454`, gradient insertion at `:513`. `renderer/resources.rs:193` and `:209` evict only per-renderer cache entries. No corresponding shared-map reclamation path was found.

A long-lived application cycling through distinct images/gradients retains their GPU resources for the shared-context lifetime even after all windows stop using them. Local eviction counters therefore do not establish that GPU memory was released.

Give shared resources a budget and explicit liveness policy across windows and submitted work. Report resident bytes and actual shared eviction separately from local binding-cache eviction. Extend the audit to atlas pages and resource-identity maps.

### F04 — Native clipboard failure silently changes the meaning of success — P1, confirmed

Evidence: `crates/incular-desktop/src/clipboard.rs:69` documents a process-local fallback. `native_capabilities` reports unsupported when no OS clipboard exists, but `read` at `:251` and `write` at `:262` still use `MemoryClipboard` successfully in that state.

An application can display “copied” even though another application cannot paste the content. This contradicts plans 00/01/08's observable unsupported/failure contract.

Make production native clipboard initialization/access failures typed. Keep `MemoryClipboard` as an explicitly selected simulation/embedding service, or require an explicit fallback policy whose outcome identifies local-only storage.

### F05 — Global-shortcut enqueue and shutdown can race — P1, source-confirmed interleaving

Evidence: `crates/incular-runtime/src/global_shortcuts.rs:266` checks an atomic active flag separately from enqueue. `stop` at `:305` flips the flag. `application.rs:2301` stops and drains the queue; `Application::shutdown` can leave the application object and receiver alive.

The service is backed by thread-safe shared fields. A worker can pass the active check, pause, then enqueue after shutdown's drain. The request can return accepted while its completion remains queued in a stopped application. Window and file-dialog bridges already serialize enqueue against stop with a mutex gate; the shortcut bridge does not.

Add a deterministic enqueue/stop interleaving regression under `tests/`, then use the same atomic admission-and-drain contract for every result-bearing service. This was not race-stress-tested during the audit.

### F06 — Two desktop runners duplicate behavior — P2, confirmed

Evidence: `crates/incular-desktop/src/lib.rs:137`/`:149` run `App<F>`; `:329`/`:337` run `MultiApp`. Separate `ApplicationHandler` implementations begin at `:490` and `:2710`. Both implement input, environment, rendering, cursor, IME, and accessibility handling. `Application::into_runtime` at `crates/incular-runtime/src/application.rs:2564` is another compatibility route.

Any event fix needs multiple implementations and regression surfaces. Converge public entry points on one desktop host, representing the simple case as one window. Preserve ergonomic entry points only as thin constructors/adapters. Extract shared behavior before deleting the old path.

### F07 — Platform contracts depend directly on Winit — P2, confirmed

Evidence: `crates/incular-platform/Cargo.toml:22` has an unconditional Winit dependency. `src/lib.rs:631` executes text-input commands on Winit windows; `:704` onward contains raw-handle extraction and Winit event conversions. Widgets and runtime both depend on platform.

The nominally portable contract layer includes a concrete desktop backend, increasing dependency reach and making alternative hosts harder to implement. Move Winit conversions and native-handle extraction into desktop/backend modules. Keep capability values, IDs, requests, results, and portable events in platform. Do not introduce a reverse dependency from desktop into an OS facade; preserve injection through backend services.

### F08 — Two signals and three build-context surfaces — P2, confirmed

Evidence: core exports `Signal` and `BuildContext` in `crates/incular-core/src/lib.rs:16`, with dependency tracking in `context.rs`. Runtime defines another `Signal` at `environment.rs:423`, its own reactive queue and ambient scopes, and another context at `context.rs:27`. Widgets exposes a borrowed inherited context at `tree/context.rs:39`. Runtime `Signal::with_runtime` at `environment.rs:490` ignores its runtime argument.

These are not simply re-exports of one engine. They have different tracking and mutation mechanisms. The runtime signal's “first runtime” documentation also needs reconciling with its maps of multiple reactive roots.

Choose one dependency engine below widgets/runtime, with explicit owner/scope lifetimes and phase-aware consumers. Different borrowed context views can remain when they have distinct capabilities, but should share tracking and unambiguous names/contracts. Remove misleading no-op parameters. Preserve signal ergonomics and explicitly decide same-thread multi-window/application sharing.

### F09 — Native services repeat request/completion machinery — P2, confirmed

Evidence: `runtime/src/window_commands.rs` implements `NativeOperationRequest`, cancellation messages, a oneshot and admission gate. `file_dialogs.rs:224` implements another request wrapper and the same plumbing. `global_shortcuts.rs:135` has registration/unregistration futures and a different bridge. `application_shell.rs` adds another request identity/operation lifecycle.

This violates plan 01's explicit reusable-primitive direction and has already produced the differing shutdown behavior in F05. Share internal admission, request ownership, completion-once, cancellation, and draining. Keep domain-specific result types and service policy; a single enormous service enum would merely relocate coupling.

### F10 — Widget behavior remains distributed across parallel taxonomies — P2, confirmed architecture debt

Evidence: `widgets/src/tree/specs.rs` owns `WidgetKind`; `tree/widget/structure.rs` is 2,197 lines of structure/equality/type handling; constructors, lowering, render invalidation, layout, painting, semantics, and interaction are separate dispatch surfaces. `internal.rs:57` onward exports a broad sibling-crate bridge. `render_object/update.rs:8` defines a second invalidation bit vocabulary.

The enum approach itself is reasonable. The risk is the number of independently maintained decisions required for one property, illustrated by F02. Group behavior by primitive family, make descriptor-to-retained property transfer explicit, and keep invalidation mapping close to the owning property. Use exhaustive dispatch and focused behavior tests. Do not replace all matching with dynamic traits or add a generator without evidence of a net improvement.

### F11 — Some validated values remain publicly constructible in invalid states — P2, confirmed

Evidence: `crates/incular-config/src/constraints.rs:11` exposes all four fields publicly while `new` at `:19` asserts invariants. Struct literals and later field writes bypass that validation.

Inventory invariants at the public boundary. Use private fields/validated construction where an invalid value can break downstream assumptions; provide read-only accessors and ergonomic named methods. Keep ordinary data snapshots public where arbitrary values are valid. Decide panic versus fallible construction per invariant instead of the current broad “panic or normalization” rule in API_DESIGN.

### F12 — Controller updates do not share an observation/lifetime contract — P2, confirmed architecture debt

Evidence: `material/src/foundation/state.rs:307` stores a `WidgetStatesController` as `Rc<Cell<WidgetStates>>`; its setters have no notification mechanism. Forms (`widgets/src/forms.rs:226`) and animations (`animation/src/controller.rs:393`) expose manually removed numeric listeners. Other areas already have RAII subscriptions.

A setter, an observable revision, and a signal are currently different concepts developers must bridge themselves. Define shared subscription ownership and invalidation semantics. Audit which exposed controllers are actually connected to retained controls; an exported controller type alone is not a working integration. Preserve callback-after-borrow-release rules.

### F13 — Checkbox semantics depend on whether a custom child is supplied — P1, confirmed

Evidence: `crates/incular-controls/src/checkbox.rs:195` reduces `CheckedState` to a boolean for the default visual path. Only the custom-child path at `:231` constructs explicit semantics from the original state. In that branch `Indeterminate` maps to `None`; `crates/incular-semantics/src/lib.rs:94` uses `Option<bool>` for checked state.

Default and custom visuals do not preserve one authoritative mixed-state model. Distinguish not-checkable, unchecked, checked, and mixed at the semantic boundary, and use the same interaction/semantic owner regardless of visual slot. Test mouse, keyboard, and accessibility activation on both paths, including read-only and disabled behavior.

### F14 — Navigation has duplicated state and a callback reentrancy hazard — P1/P2, confirmed source risks

Evidence: `crates/incular-navigation/src/navigator.rs:313` reconciles routes and restoration metadata in parallel vectors, matching pages by name with linear searches/removal. It does not dispatch the observer events used by imperative push/pop paths. At `:349` and `:391`, a user-supplied guard runs between reading the candidate and popping the current stack without checking that the candidate is still current.

A guard that mutates a cloned navigator can invalidate the subsequent `expect` or cause a different route to be removed. Revalidate route identity/revision after callbacks or explicitly reject reentrant mutation. Use one route-entry record and one mutation/notification transaction for imperative and declarative updates. Decide page identity independently of display names and handle duplicate keys explicitly. Reproduce these cases before implementation.

### F15 — Diagnostics retain dead registrations and hide some errors — P2, confirmed

Evidence: runtime `environment.rs:496` allocates a new DevTools ID per registration; `:793` stores registrations in a thread-local vector; `:796` only appends. Weak signal captures avoid retaining signal values, but registration metadata/closures remain. Repeated naming can add more entries. GPU `renderer/frame.rs:25` groups validation failure with timeout/occlusion as `Ok(None)`.

Scope diagnostic registration to live owners with replace/unregister behavior. Distinguish expected frame skips from renderer validation errors. In the DevTools UI, also audit the unbounded request/update channels in `tools/incular-devtools/src/transport.rs:22`; the target already uses bounded channels and should retain that design.

### F16 — CPU image cache has no budget or release policy — P2, confirmed

Evidence: `crates/incular-image/src/lib.rs:359` holds `HashMap<Vec<u8>, ImageHandle>`. `load_bytes` at `:375` copies the complete encoded payload before lookup, retains encoded bytes and decoded handles, and exposes no eviction/clear/budget operation.

Add explicit asset/cache ownership and byte budgets, avoid a full key allocation on a hit, and define async loading/cancellation above the decode primitive. A synchronous decode API is useful; doing it implicitly during UI work needs a deliberate policy. Keep deduplication collision-safe if switching to content hashes.

### F17 — Documentation and validation protect conflicting architectures — P2, confirmed

Evidence: AGENTS says built-in widgets stay in widgets and painting owns drawing commands. API_DESIGN assigns headless controls to controls, Material to material, and commands to rendering. `incular-painting/src/lib.rs` is now a compatibility re-export. API_DESIGN calls Flutter 3.47.1 export ownership permanent; `tests/widgets_3471_boundary.rs:177` enforces that graph. `docs/QUALITY.md:46` says production unsafe sites are only GPU surface creation and Windows crash registration, despite extensive native FFI elsewhere.

Reconcile the architecture documents and tests in one change. Flutter parity can guide vocabulary and compatibility without becoming an immutable internal ownership rule. Existing useful behavior must survive; names/manifests cannot stand in for implementation coverage. There is no checked-in `.github` workflow directory in this baseline, so local policy does not itself prove cross-platform checks run on every change.

### F18 — Identity exhaustion and device lifetime need explicit policy — P3/audit

Evidence: `core/src/arena.rs:74` narrows slot length to `u32`; `:104` wraps a `u32` generation. Some request counters use checked allocation, while routes and listeners wrap. `desktop/src/pointer.rs` maps native device IDs without a removal path; desktop environment discovery records device classes once observed.

The arena wrap issue requires extreme reuse, so it ranks below present-day API failures. Specify exhaustion/retirement and stale-ID guarantees, then test boundaries through a small-capacity testable implementation. Separate “backend supports”, “device currently present”, and “device class seen this session”; verify actual device removal support before changing discovery semantics.

## Coverage of the desktop campaign

These are source-review dispositions, not retroactive completion certificates. Native behavior on macOS/Linux was not exercised from this Windows host.

| Plan | Existing implementation evidence | Follow-through required |
| --- | --- | --- |
| 00 campaign | Capabilities, semantic commands, native facade injection, substantial tests | Resolve F06/F07/F17; record native evidence and limitations per plan |
| 01 results/capabilities | `platform/{capabilities,operation}.rs`, runtime request bridges | F04/F05/F09; one admission/completion contract, consistent failure reporting |
| 02 window control | `platform/window_control.rs`, desktop `operate_window`, widget chrome | Preserve requested versus observed state; converge runners; test interactive drag/resize live |
| 03 displays | `platform/display.rs`, desktop display registry/identity, OS work-area seams | Preserve typed coordinate spaces; X11 work areas are explicitly unsupported; test mixed-DPI/hotplug live |
| 04 pointer/cursor | `platform/pointer.rs`, desktop pointer registry and cursor coordination | Converge normal/popup/simple-runner translation; replay multi-button and cancel/leave sequences |
| 05 transients | Desktop registry/partitioning, shared GPU, Windows ownership and AppKit attachment | F01/F03; Wayland native hosting explicitly remains unsupported; native teardown/activation evidence required |
| 06 popup policy | `widgets/transient.rs`, desktop transients and one retained owner tree | Verify cross-window dismissal, nested focus, RTL, work areas and accessibility; README leaves distinct native accessibility roots conditional |
| 07 native menus | Windows/macOS delegates and retained menu snapshots | Preserve incremental updates/stale-ID rejection; test shortcut authority live; unsupported Linux is a limitation, not fake implementation |
| 08 transfer | Typed transfer values, native clipboard plan, file-drop lifecycle | F04; rich/custom/lazy native support differs from the portable model; document format/operation matrix |
| 09 dialogs | Typed requests, per-window serialization, native-dialog and portal adapters | F09; distinguish runtime cancellation from closing native modal UI; prove owner lifetime after parent close |
| 10 environment | OS preference providers, window snapshots, field invalidation | F08/F18; unify subscription semantics and verify reduced motion/occlusion retain pending work |
| 11 activation/shortcuts | Typed activation, authenticated local IPC/lock, shortcut registration | F05/F09/F14; cancellation races, routing once, restart/stale endpoint and native shortcut tests |
| 12 shell integration | Typed shell services, RAII resource handles and native facades | F09; verify pending native completion and teardown across shutdown, notification actions and stale callbacks |
| 13 advanced input | Device/sample types, Win32 pointer hook, native gesture normalization | F06/F18; test real stylus/trackpad metadata and device lifecycle; preserve absent metadata |

## Whole-workspace coverage and remaining checks

Every crate is assigned a disposition below. “Audit” means a required follow-up, not a discovered failure. Shared re-export paths were distinguished from duplicated implementations.

| Crate / area | Inspected boundary or path | Disposition |
| --- | --- | --- |
| incular-core | arena, input, context, exports | F08/F18; preserve primitive math and independent invalidation |
| incular-config | constraints, environment, shared policy | F11; audit public invalid-state construction and environmental provenance |
| incular-layout | algorithms, descriptors, constraints re-export | Keep pure algorithms; clarify descriptor API versus widget adapters; property-test unbounded/RTL layout |
| incular-assets | font handle/ID and manifest | Much narrower than historical loading/cache responsibility; document exact owner versus image/text |
| incular-image | providers, decode, cache | F16; loading lifetime, byte budgets and cancellation |
| incular-painting | compatibility export and manifest | F17; deliberate removal/migration policy, no second implementation |
| incular-rendering | display lists, compositor, layers | Preserve backend independence; unify invalidation terminology and shared resource ownership |
| incular-text | engine/cache, editing, style surface | Keep Parley shaping; audit IME/grapheme/bidi and font/cache lifetimes with existing tests |
| incular-animation | controller, status/repeat state, listeners | F12; audit interruption/completion, reduced motion, ownership and deterministic clock |
| incular-gestures | focus/keyboard/arena inventory and input consumers | F18; audit cancellation, focus removal and arbitration through event replay |
| incular-scroll | controller/activity, sliver/extent inventory | Audit activity/animation ownership, pending jumps, nested scrolling and controller sharing; do not replace valid changed/not-changed booleans |
| incular-widgets | descriptor → retained → layout/paint/semantics, interaction, forms | F02/F10/F12; property-by-property behavior matrix and retained invariants |
| incular-controls | button, checkbox, theme/style and composition surface | F13; shared behavior independent of visual slot; clarify “headless” styling contract |
| incular-material | input components, foundation state, export surface | F12/F13/F17; tokens and composition above neutral mechanisms; remove option-only parity |
| incular-navigation | route records, declarative/imperative mutations, back handling | F14; clarify domain navigation versus widget router/back-dispatch ownership |
| incular-semantics | semantic node/state/action model | F13; mixed/absent states and actionable semantics |
| incular-accessibility | AccessKit and mobile projections | Preserve stable mapping; native popup projection, focus and action validation audits |
| incular-runtime | application, frame, contexts, reactivity, tasks and services | F05/F08/F09/F12/F15; smaller ownership-oriented coordinators |
| incular-platform | values plus concrete Winit helpers | F07; portable dependency boundary and service contract consistency |
| incular-desktop | both runners, transfer, dialogs, transients, input, IPC | F04/F06; one native host and thin domain adapters |
| incular-windows | services, transients, pointer, environment/menu/shell paths | Native lifecycle tests and updated unsafe inventory; retain OS ownership |
| incular-macos | services, child-window attachment, environment/menu/shell paths | Native-host compile and interaction evidence needed |
| incular-linux | X11 services, capability fallbacks, portal paths | X11/Wayland matrix; distinguish explicit unsupported from unfinished acceptance |
| incular-android | data-only accessibility adapter | Not a complete activity/window/render/input backend; mark support accordingly |
| incular-ios | data-only accessibility adapter | Not a complete UIKit/window/render/input backend; mark support accordingly |
| incular-wgpu | constructors, shared resources, frame acquisition/local eviction | F01/F03/F15; GPU lifetime, memory and error classification |
| incular-macros | empty proc-macro crate | Intentional scaffold; defer implementation until API stabilizes; avoid implied macro support |
| incular-devtools-protocol | versioned serde wire boundary | Keep runtime-independent; audit version negotiation and bounded payload contracts |
| incular-devtools | handshake and bounded UI/telemetry bridge | Keep authenticated local bridge; F15 for runtime-owned registrations |
| tools/incular-devtools | transport/session/model/view inventory | F15; bound/coalesce queues, reconnect and request ownership |
| incular facade | exports, features, native entry points | F06/F17; clear default/minimal/design-system API and migration examples |
| workspace tests/examples/specs | API, parity, placement, behavior and benchmark inventory | Preserve behavior/property tests; replace brittle source spelling checks where compiler/metadata assertions suffice |

## Validation baseline

Validation is recorded after the audit and plan are written. Native interactive tests, other-host compilation, Miri and performance benchmarking are separate from host workspace checks; none should be inferred from a green Windows build.

- `cargo fmt --all -- --check`: passed.
- `cargo check --workspace`: passed.
- `cargo test-constrained`: passed; 1,038 passed, 0 failed, 0 ignored across 233 harness summaries, including doctests. This count does not establish live-native coverage; tests can gate native work internally.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: passed.
- `git diff --check`: passed for tracked changes; the only additions are this audit and `plan-14.md`.

No interactive native smoke tests, macOS/Linux-host builds, mobile-target builds, MSRV run, benchmark run, Miri, dependency advisory scan, or separate warning-denied rustdoc build were performed. The Windows workspace baseline passing does not invalidate the source-level findings above.

## What not to do

- Do not rewrite the framework or reintroduce Stateless/Stateful classes to resemble Flutter.
- Do not replace every boolean with an enum. Independent facts, predicates, and a documented “changed” result are appropriate booleans. Use enums for mutually exclusive phases, meaningful outcomes, and configuration choices with invariants.
- Do not treat every `Rc`, `RefCell`, clone, match, panic, or large file as a defect without following its ownership and behavior.
- Do not solve dependency cycles by moving everything into core or exposing more internals.
- Do not call a feature complete because its builder or parity manifest compiles.
- Do not treat supported-but-not-live-tested native behavior as verified, or explicit unsupported behavior as an architectural hack.
