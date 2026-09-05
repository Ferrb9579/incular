# Plan 14 — Architecture consolidation and behavioral quality

Status: Stage A is complete and validated. Stages B–J remain pending.

## Implementation progress

- A1: added `WindowSurfaceTarget`, moved GPU initialization/recreation to WGPU's
  safe owned-target API, and migrated both desktop runners and popup hosts to
  `Arc<Window>`. Removed detached raw handles from renderer constructors.
- Unsupported surface configuration now returns a typed renderer error.
- Added headless ownership/cancellation regressions under WGPU's `tests/`.
- A1's compiler fixtures accept owned targets and reject borrowed targets and
  detached handles. Live Windows resize and popup close/reopen/parent-close
  harnesses passed with `INCULAR_DESKTOP_LIVE_TESTS=1`. Actual device loss was
  not injected; recreation uses the same safe owned-target factory. Other
  operating systems were not exercised on this Windows host.
- A2: all preservation options now survive in the retained policy. Retained
  children are measured; size, semantics, and animation behavior match the
  documented options. Offstage keeps ticking, hidden cached pictures disconnect,
  and nested popup surfaces obey hidden ancestors. Focus behavior is documented.
- A2's five new regression tests cover all 16 preservation combinations through
  both construction APIs, identity, layout, focus, semantics, hit testing, cached
  paint, animation, and popup transitions. See [Visibility behavior](docs/VISIBILITY.md).
- Final A1/A2 host validation: format and workspace check passed; default and
  all-feature workspace suites each passed 1,046 tests; strict all-target,
  all-feature Clippy and warning-denied workspace rustdoc passed.
- A3: native clipboard initialization errors now survive as typed failures;
  removed the implicit memory fallback and its redundant write cache. Memory
  clipboard remains an explicitly selected runtime backend.
- A3: shortcut admission and enqueue share a shutdown gate. Wake callbacks run
  after admission and outside the wake mutex. Deterministic gate tests cover a
  paused enqueue, drain ordering, and rejection after stop; application tests
  cover queued/dispatched completion and stale native replies after shutdown.
  An isolated early-unlock mutation was rejected by the gate test, confirming
  that the regression check detects the unsafe ordering without timing sleeps.
- A3 passed format, workspace check, constrained tests, and strict Clippy.
- A4: default and custom checkbox visuals share one action/semantics owner.
  Mixed state has an explicit semantic value and survives AccessKit/mobile
  projection. Enabled read-only controls focus but do not activate; disabled
  controls do neither. Focused action surfaces now handle Enter/Space after
  keyboard listeners; nested editors retain their input.
- A4: guarded navigation mutations revalidate revision and candidate identity
  after user code. Regressions reproduced wrong-route removal and a stale-route
  panic before the fix; both pass afterward. Checkbox tests also reproduced
  default/custom focus-policy drift before consolidation, and now cover 72
  pointer/keyboard/semantic activation cases plus state/projection parity.
- Public migration details: [Stage A migration](docs/STAGE_A_MIGRATION.md).
- Final Stage A validation on Windows (2026-09-05): `cargo fmt --all -- --check`,
  `cargo check --workspace`, `cargo test-constrained`, and
  `cargo clippy --workspace --all-targets --all-features -- -D warnings` passed.
  Default and all-feature workspace test suites each passed 1,057 tests, including
  the compiler fixtures checked against reviewed diagnostics without overwrite.
  Warning-denied all-feature workspace rustdoc passed. The two live native
  harnesses above also passed. An existing boolean assertion found during the
  first full test build was migrated before these successful final runs.
- All later stages remain pending. The original audit is a baseline record,
  not a statement that these newly corrected paths remain defective.

Baseline: `0ba7058`, reviewed 2026-09-05. Evidence and campaign/crate coverage: [architecture audit](docs/ARCHITECTURE_AUDIT.md).

## Objective

Make Incular a coherent Rust GUI library whose public APIs faithfully control retained behavior, whose resources have explicit lifetimes, and whose features extend existing mechanisms rather than introducing parallel ones.

Preserve signals, immutable widget descriptions, retained elements/render objects, fine-grained invalidation, design-system independence, and WGPU rendering. Flutter is a source of useful concepts and compatibility references. Incular's documented behavior and Rust ergonomics determine its implementation architecture.

This is a sequence of independently reviewable changes, not one rewrite. Correctness fixes can precede larger migrations. Each step must leave a usable library with its examples migrated and checks passing.

## Proposed architecture decisions

1. **One dependency engine.** Core-level reactive primitives own values, dependency edges and scope cleanup; runtime owns scheduling; widgets register consumers for the phase being executed. Initially consolidate into the existing lower-level core context module with no runtime/widget dependencies. Consider a separate reactive crate only after dependency and public-surface measurements justify it. Keep domain controllers, but have them participate in the same observation contract.
2. **One desktop host.** An application with one window and one with many windows use the same event-loop implementation. Convenience APIs construct the same application/host configuration.
3. **Portable platform contracts.** Platform owns portable IDs, capabilities, commands, events and errors. Desktop owns Winit translation. OS crates inject native services into desktop. Renderer accepts a safe owned/lifetime-bound surface target; raw detached handles are an explicitly unsafe backend escape hatch.
4. **One internal native-request lifecycle.** Share admission, completion, cancellation, stale-owner and shutdown behavior. File dialogs, shortcuts and shell operations retain typed domain requests/results and their own scheduling policy.
5. **One behavioral owner per primitive.** Visual slots and Material wrappers compose interaction, editing, scrolling and semantics. They cannot fork those behaviors. Keep a closed primitive enum unless an actual extension requirement justifies dynamic dispatch.
6. **One authoritative property-to-invalidation contract.** The retained engine maps property changes to independent build/layout/paint/composite/semantics/hit-test work. Derived narrower masks must be explicit projections, not independently maintained semantics.
7. **Owned, budgeted caches.** Resource sharing and reclamation are both defined. Window-local caches do not claim to evict device-owned textures. Diagnostic state follows the same lifetime discipline.
8. **Explicit compatibility policy.** Before changing the permanent parity rules, reconcile AGENTS, API_DESIGN, QUALITY and boundary tests. Keep stable ergonomic names where useful; retire compatibility implementation paths in named steps with migration examples. Do not move unrelated code solely to fit a Flutter export graph.

Proposed dependency direction, with arrows meaning “uses”:

```text
facade / application code
  -> widgets + optional controls/material + runtime
material -> controls/widgets -> domain mechanisms + reactive primitives
runtime -> widgets + platform contracts + domain mechanisms
OS entry adapter -> desktop host -> runtime + WGPU + platform contracts
OS native service implementations -> portable contracts
WGPU -> rendering commands + asset values + surface target abstraction
rendering/text/layout/scroll/gestures/semantics -> their lower-level value owners
```

Maintain an acyclic Cargo graph. Moving Winit out of platform must not make WGPU depend on desktop. Keep lower-level geometry/layout re-exports where they denote the same type; re-exporting is not duplicated behavior.

## Priority and order

| Stage | Work | Depends on | Finding coverage |
| --- | --- | --- | --- |
| A | Fix demonstrated safety and behavior gaps | Baseline regressions | F01/F02/F04/F05/F13/F14 |
| B | Reconcile architecture and API contracts | Audit | F07/F08/F09/F10/F11/F17 |
| C | Unify reactivity and controller observation | B | F08/F12/F15 |
| D | Unify native request lifecycle | A, B | F05/F09 |
| E | Consolidate desktop host and backend boundary | A, B, D | F06/F07 |
| F | Make retained primitive behavior complete and local | A, B, C | F02/F10/F11/F13 |
| G | Consolidate navigation, scrolling, editing and animation ownership | C, F | F12/F14/F18 |
| H | Bound resource memory and classify renderer failures | A; interface decisions from B | F03/F15/F16 |
| I | Close desktop campaign acceptance gaps | D, E, F, G, H | Plans 01–13 |
| J | Tighten diagnostics, testing, features and documentation | Throughout; finish after I | F15/F17/F18 |

Stages can contain multiple small PRs. H's resource fixes need not wait for the entire widget migration. Do not combine a behavior correction with a broad file move in the same review unless inseparable.

## Stage A — Establish regressions and fix concrete defects

### A1. Native surface safety

Targets: platform raw-handle API, `wgpu/resources.rs`, `wgpu/renderer/state.rs`, desktop parent/transient host ownership.

- Design a safe constructor accepting an owned surface target or borrowing a live target for the renderer lifetime. Prefer existing WGPU ownership facilities if compatible with the pinned dependency and native targets.
- Ensure recreation after surface loss retains the same ownership guarantee.
- Make any remaining raw constructor `unsafe` and document thread, handle, display and lifetime requirements. Remove safe routes that bypass the guarantee.
- Replace unsupported surface configuration `expect` paths with a typed renderer initialization error where unsupported presentation is a runtime condition.

Acceptance: safe caller code cannot construct a renderer from expired handles; lifetime/ownership compile tests live under `tests/`; parent/popup close-order smoke tests pass. No UB-based “test”.

### A2. Complete visibility policy

Targets: widget visibility builder/spec, render lowering, layout, painting, semantics, hit testing, focus and animation gating.

- Write a behavior table for visible, removed, retained-offstage and retained-space states, including replacement behavior.
- Represent preservation policy in retained data; make invalid combinations unrepresentable or reject them at construction with a documented error policy.
- Keep fluent convenience methods if useful, translating them into the validated policy. Generated builders and manual constructors must enforce identical invariants.

Acceptance: tests measure preserved/removed layout size, element/controller identity, animation advancement, semantic presence and hit-test exclusion. Tests change each advertised option independently and cover Offstage. No field may be accepted and silently ignored.

### A3. Honest clipboard and atomic service shutdown

Targets: desktop clipboard; runtime shortcut bridge and shutdown.

- Make native clipboard unavailability/failure observable; use the memory service only by explicit selection.
- Reproduce admission-after-drain for shortcuts using a controlled concurrency seam in tests, avoiding probabilistic sleeps.
- Serialize acceptance with shutdown and resolve every accepted request exactly once, even while the stopped application object remains alive.

Acceptance: unavailable OS clipboard never reports a native write success; a table-driven request test distinguishes enqueue rejection, native unsupported/failure, cancellation and shutdown. Outstanding count returns to zero.

### A4. Consistent checkbox semantics and safe navigation callbacks

Targets: controls checkbox root/default visual, semantic checked-state value, accessibility projection, navigator guarded mutations.

- Give mixed/checkable state an explicit semantic representation and preserve it through both default and custom-child paths.
- Revalidate navigation candidate identity after user guards, or enforce a documented non-reentrant transaction contract with a typed outcome.
- Add tests before refactoring broader control/navigation APIs.

Acceptance: visual choice does not change checked/disabled/read-only semantics or activation; guard mutation never pops an unapproved route or panics on a stale candidate.

## Stage B — Set one enforceable architecture contract

Deliverables: revised API_DESIGN, AGENTS responsibility map, QUALITY, crate README support matrix and a short decision record for each architecture decision above.

- Classify each exported type as application API, renderer/backend extension API, or implementation bridge. `doc(hidden)` is visibility in documentation, not Rust access control.
- Inventory compatibility aliases, duplicate entry paths, no-op parameters and public builder fields. Record keep/remove/migrate decisions and affected examples.
- Decide whether `incular-painting` remains a temporary import shim or is retired in this pre-1.0 campaign. Give any retained shim an explicit purpose and removal condition.
- Clarify roles of config versus layout, assets versus image/text, semantics versus native accessibility, and navigation versus widget route composition.
- Keep core widgets neutral. Define whether controls are headless behavioral primitives or themed Incular controls; align implementation and descriptions instead of using both meanings interchangeably.
- Revise parity gates to permit deliberate Rust deviations while protecting actual compatibility commitments. Every unsupported or deliberately omitted member needs an explicit status.

Acceptance: no conflicting crate ownership statements; every public mutation has an outcome and ownership contract; every planned move is compatible with the documented dependency graph. No new crate merely to shorten a file.

## Stage C — One reactive model, scoped subscriptions

Targets: core context; runtime context/environment/reactive/frame; borrowed widget context; controller subscriptions.

1. Document tracked/untracked reads, equality-aware writes, batching, read-during-build restrictions, memo ordering/cycles, effect timing/cleanup, and action cancellation.
2. Implement one dependency registration/cleanup mechanism below widgets/runtime. Pass stable consumer identity and a scheduling/invalidation hook instead of depending on Element internals from the lower layer.
3. Migrate runtime signals/memos/effects onto it, then widget inherited reads and controller observation. Distinct context views may expose different capabilities while sharing the engine.
4. Remove `with_runtime`'s ignored argument or make the binding real. Explicitly define independent-app and multi-window sharing.
5. Replace manual listener ownership with RAII subscription tokens where appropriate; retain low-level registration only for intentional backend integration.
6. Make diagnostic registrations owner-scoped and update-in-place; remove stale entries when owners disappear.

Acceptance: conditional dependencies unsubscribe; branch switches and keyed moves preserve correct ownership; unmount/drop removes effects/listeners/tasks; a diamond dependency graph yields consistent values; cross-window reads invalidate only their consumers; no second public signal implementation remains. Preserve or improve existing signal and retained-operation benchmarks.

Migration example requirement: one counter, one editable form, one async search and one multi-window shared-state example demonstrate the final API without manual refresh calls.

## Stage D — Native request lifecycle as shared infrastructure

Targets: runtime window commands, file dialogs, shortcuts, application shell and Application pending maps.

- Introduce a private reusable request channel/registry with typed request IDs and domain result payloads. Do not erase domain errors into strings.
- Define phases such as queued, executing, completed and abandoned where the distinction changes behavior. Keep OS acknowledgement distinct from queue acceptance.
- Unify admission versus stop, completion-once, caller drop, owner close, late completion and cleanup behavior.
- Specify cancellation precisely: suppressing a result is not necessarily cancelling a native effect. Services unable to cancel must still retire bookkeeping and reject stale callbacks.
- Keep serialization policy domain-specific: dialogs may serialize per owner; window setters should not acquire artificial asynchronous ceremony.
- Use typed resource ownership for tray/notification/shortcut teardown; unify mechanics without merging every resource into one generic public abstraction.

Acceptance: apply one lifecycle test suite to each service, including concurrent windows, dropped futures, parent close, backend failure, shutdown race and duplicate/late completions. All pending state is reclaimed; cleanup native operations are emitted only when required.

## Stage E — One desktop event loop, portable platform layer

Targets: desktop `lib.rs`, platform `lib.rs`, OS facade entry points, desktop service trait, transient event routing.

1. Extract a window-host record owning native window, surface renderer, metrics, input and accessibility bridge with safe teardown ordering.
2. Extract domain coordinators for input normalization, presentation, environment, transient hosting and native services. Avoid modules that all mutate every field of MultiApp through broad `super::*` access.
3. Route both public entry paths through the same application/event-loop engine, using a small callback adapter for the legacy action-oriented form.
4. Move Winit event conversion, IME execution and raw native access out of platform into desktop/OS adapters.
5. Split backend service interfaces only where callers use independent domains; keep portable contracts free of Winit and OS types.

Acceptance: replay the same event sequence against one-window and multi-window cases; results match. Runtime/widgets' portable build dependency graph excludes Winit. Window creation, resize, focus, IME, pointer cancel and accessibility behavior use one authoritative path; popup-specific coordinate/routing policy remains explicit.

## Stage F — Complete retained primitives and reduce change fan-out

Targets: widget descriptors/specs, structure, lowering, render-object updates and phase dispatch; control/material wrappers.

- Inventory properties across all exported widgets, starting with visibility, editing, transforms/effects, scrolling and interactive controls. For each: constructor → retained field → update invalidation → execution → observable test.
- Group primitive-family behavior behind narrow internal functions/modules. Keep the enum closed/exhaustive; colocate field comparison and invalidation decisions where practical.
- Consolidate independent invalidation flags or provide one checked projection. Verify hit-test and semantic geometry after compositor-only movement rather than assuming paint correctness establishes both.
- Reduce `internal` exports to interfaces sibling crates actually need. Do not make retained mutable storage public to avoid an interface decision.
- Audit constructors plus TypedBuilder for discarded fields and validation bypass. Tighten invariant-bearing public values such as Constraints.
- Separate control behavior/state from visuals. Material defaults should be resolved from tokens and inherited context without duplicating input, focus or semantic implementations.

Acceptance: each public option has an implemented behavior or an explicit unsupported/deprecation decision. A representative primitive can be extended without scattered duplicate policy. Existing reconciliation/identity/performance tests pass, and dirty-phase tests prove changes do only required work.

## Stage G — Domain ownership and interruption semantics

### Navigation and restoration

- Replace parallel route/restoration vectors with one route-entry record.
- Give declarative pages stable explicit identity, define duplicate identity errors, and use the same mutation/notification transaction as push/pop/replace.
- Unify domain back decisions and widget dispatch adapters; document which layer owns route lifetime, focus restoration and restoration-scope cleanup.
- Test observer parity between set_pages and imperative operations, guard reentrancy, repeated route names, removal cleanup and deep-link delivery once.

### Scrolling and animation

- Trace ownership among controller position, user activity, driven/ballistic motion, restoration and retained viewport state. Introduce an activity enum only where multiple flags/options actually encode mutually exclusive modes.
- Decide whether one controller may attach to multiple viewports; reject unsupported sharing explicitly or model separate positions.
- Define interruption results for animate, drag, jump, unmount and reduced-motion changes. Prefer one runtime clock and scoped ticker ownership.
- Preserve sliver/extent-index reuse and property-test insert/remove/reorder, variable sizes, nested scroll handoff and retained keep-alive behavior.

### Text, forms, gestures and focus

- Preserve text engine authority for shaping and editing boundaries; unify form/text/controller observation through Stage C.
- Test bidi text, grapheme deletion, selection affinity, IME composition cancellation, undo, focus removal and native focus changes.
- Replay mouse/touch/pen and aggregate trackpad events through one normalization/arena contract; do not invent touch contacts.

Acceptance: each domain has a documented state/transition table, one owner per long-lived operation, and no lost completion or listener after interruption/unmount. Add tests for domain failures, not tests mirroring private fields.

## Stage H — Reclaim memory and make GPU failures observable

Targets: image cache, shared GPU resources/atlas, per-window caches, renderer acquisition, text/font caches.

- Fix shared image/gradient retention first. Define a byte budget, recent-use/liveness tracking and safe reclamation across all windows and in-flight rendering. Preserve resource identity while resident.
- Distinguish local cache entries, device-resident resources, CPU decoded bytes and encoded cache-key bytes in diagnostics.
- Bound ImageCache and avoid allocating the encoded key for a cache hit. Use collision-safe content addressing or borrowed-key lookup. Separate async loading policy from synchronous decode.
- Audit glyph atlas growth and stale font/resource ID maps. Preserve the text engine's existing bounded layout cache rather than replacing it without evidence.
- Separate expected frame skips from validation/native initialization failures. Define device-loss policy: coordinated rebuild with a new generation or explicit terminal error; do not leave a decorative generation field implying recovery.

Acceptance: repeated load/display/drop cycles across two windows settle within a configured memory budget; live resources remain valid; reopening content produces expected cache behavior. Renderer validation failures are observable. Measure steady-state uploads, resident bytes and existing render benchmark counters before/after.

## Stage I — Revalidate plans 01–13 end to end

Use the audit's campaign table as the checklist. For each capability and desktop session, publish **implemented and live-tested**, **implemented but not live-tested**, **explicitly unsupported**, or **unfinished**.

- Windows/macOS/X11/Wayland: window commands, monitor movement/DPI, native popup placement/activation, outside dismissal, parent destruction, menu accelerators, clipboard, file dialogs, environment changes, activation/IPC, shortcuts and shell callbacks.
- For transients, verify actual screen-reader ownership/actions. A single logical tree does not automatically prove correct OS accessibility behavior for separate native windows.
- For dialogs, verify native cancellation/parent close in addition to runtime future completion.
- For rich transfer and gestures, list the exact supported formats, negotiation operations and hardware metadata. Portable type support is not native backend support.
- Wayland native popups and X11 work areas remain explicit gaps until implemented through an appropriate native path. Do not emulate forbidden geometry to mark the plan complete.
- Record Android/iOS as scaffold/data adapters until their lifecycle/window/render/input hosts exist. Full mobile implementation is a separately estimated feature campaign, not a prerequisite for desktop debt cleanup.

Acceptance: every original acceptance criterion has test evidence or a named limitation/remaining task. Completion reports include platform/session, command, result and implementation commit. No false blanket “desktop complete” status.

## Stage J — Sustainable quality gates and migration closure

- Replace source-substring assertions with Cargo metadata checks, compile/pass-fail API fixtures or behavioral tests where those establish the intended contract. Keep file-placement tests and intentional architecture checks.
- Keep parity inventory and behavior coverage separate. Add “export exists but semantics incomplete” as a meaningful state during remediation.
- Scope DevTools registrations to live owners; bound/coalesce UI transport updates; test reconnect and obsolete request replies. Keep authentication, bounded target channels and nonblocking telemetry.
- Refresh unsafe-site inventory and review each lifetime/thread invariant. Use targeted Miri for eligible pure ownership code, native/GPU tests for FFI paths.
- Define checked identity allocation/slot retirement. Test exhaustion using a bounded testable allocator; avoid billions of iterations or production test-only code.
- Verify facade minimal/default/control/material/devtools combinations, declared MSRV on the locked graph, rustdoc and relevant native-host builds.
- Make required checks repeatable in the repository's chosen automation. Direct Cargo commands remain canonical; a custom task framework is unnecessary.
- Update examples, README support claims and migration notes. Remove replaced implementations and obsolete compatibility tests after migration.

Acceptance: the supported API has one documented path per concept, no dead compatibility engine, and required checks protect behavior plus boundaries. New contributors can add a property without reverse-engineering multiple historical plans.

## Validation required for each implementation change

Run the repository-mandated commands:

```text
cargo fmt --all -- --check
cargo check --workspace
cargo test-constrained
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Also use focused tests before broad checks. On API/feature changes run all-feature tests and warning-free rustdoc; on dependency changes run metadata/feature checks and the documented dependency policy. Native changes need the affected OS/session validation, not just cross-platform crate stubs on Windows. Benchmark hot-path changes with existing Criterion suites and deterministic performance contracts.

All test code and helpers belong under the applicable `tests/` directories. Do not put `#[cfg(test)]` implementations in `src/`.

## Completion criteria and per-change report

Each review should state the original observable problem, the final owner/API contract, migration impact, removed implementation paths, tests and measured performance/memory effects where relevant. Distinguish confirmed fixes from remaining audits.

The campaign is complete when:

- F01–F17 are resolved with evidence or an explicit, justified design disposition; F18 has a tested identity policy and documented device semantics.
- All public widget options carry through to their promised behavior.
- One reactive engine, one native-request lifecycle and one desktop event loop remain.
- Resource memory is bounded and native failure/cancellation outcomes are truthful.
- Architecture guidance, Cargo boundaries, public exports and tests agree.
- Original desktop acceptance criteria have evidence or explicit support limitations, and all prescribed validation passes on the claimed platforms.

Start with A1/A2/A3/A4 in separate focused changes, while settling Stage B's contract. Do not begin by reorganizing every module or renaming every boolean.
