# Plan 15 — Whole-codebase quality and architecture completion

Status: W1 forms (`6e6af8d`) and length formatting (`638f77e`) committed;
Material checkbox error presentation committed (`9dbcfee`); ListTile autofocus
and initial-layout focus resolution complete and validated.
Audit baseline `42befb7`, 2026-09-06. Remaining W1 items and W2–W9 are pending.
This expands the remaining scope of plan 14 F–J. Completed plan 14 commits stay
complete; its architecture decisions remain authoritative. Use this document
as the remaining-work schedule, with plan 14 retained as the implementation
history. The original planning pass changed documentation and inventory only; subsequent
implementation progress is recorded below.

Evidence: [whole-codebase audit](docs/WHOLE_CODEBASE_AUDIT.md),
[complete file inventory](docs/CODEBASE_INVENTORY.json),
[initial property ledger](docs/RETAINED_PROPERTIES.md).

## Implementation progress

- W1 ListTile / part of R01: autofocus selects the existing action through
  neutral FocusScope metadata, with no extra traversal target. Disabled and
  opted-out tiles do not request focus. Runtime now settles initial focus after
  the first successful layout if eager mount has not resolved it; explicit
  input cancels that pending decision. Later frames cannot reclaim focus.
  Regression coverage exercises Enter activation, Tab traversal, deferred
  layout, explicit clearing, eager focus and an empty initial frame.
- ListTile validation (Windows, 2026-09-07): formatting, workspace compiler
  checks, all 1,134 workspace tests and strict all-target/all-feature Clippy
  passed. All six focused Material/runtime tests also passed with all features.
  The activation regression failed both before wiring the option and with only
  the Material change, establishing the need for the runtime lifecycle fix.
  Live native desktop tests remained opt-in and were not run.
- W1 checkbox / part of R01: enabled error indicators use the scoped theme's
  error outline and checked/mixed fill; explicit fill/side overrides win and
  disabled indicators retain their existing defaults. Custom resolvers receive
  the configured disabled state as well as checked/error state. Removed the
  second state assignment that incorrectly cleared checked Material values.
  Painting and semantics regressions cover these paths. Other R01 options remain
  pending: Card, Scaffold and SliverAppBar.
- Checkbox validation (Windows, 2026-09-07): formatting, workspace compiler
  checks, constrained workspace tests and strict all-target/all-feature Clippy
  passed. All six focused checkbox tests also passed with all features enabled;
  three initial paint regressions reproduced failures before the fix. Live
  desktop regressions remain opt-in and were not run by this slice.
- Commit policy: the user authorized a commit after each completed, validated
  implementation slice on 2026-09-06. The form fix and audit are in `6e6af8d`.
- W1 length formatting / R03: the shared formatter counts extended grapheme
  clusters with the existing ICU segmenter, honors None/Enforced/deferred modes,
  preserves the incoming selection direction and clamps UTF-8 ranges after
  truncation. Empty/invalid composition no longer defers enforcement.
  Material max_length delegates to that formatter instead of scalar truncation.
- All four new formatter tests failed before the change and pass afterward;
  two Material controller tests cover grapheme truncation and composition commit.
  Validation: formatting, workspace check, strict all-target/all-feature Clippy
  and all 1,122 workspace tests passed. The six new tests also passed with all
  features enabled. Native IME interaction is not claimed by these deterministic
  controller tests. This slice is committed with the length-formatting change.
  Ignored Material options remain open.

- W1 forms / R02: both validate and save now visit every live field even after
  an invalid result, update all errors and notify after the full pass. Save
  callbacks run only when the full validation result succeeds.
- W1 callback boundary / part of R14: validator and save callback configuration
  is cloned before invocation. A callback may replace itself; its replacement
  runs on the next invocation. This does not promise arbitrary recursive
  validation is safe or make user callback mutations a transaction.
- Four new regressions reproduced the old failures: the two multi-field tests
  visited only one of three validators, and both callback replacement tests
  panicked with `RefCell already borrowed`. All ten focused form tests pass
  after the fix. Multi-field tests cover three registration orders, stale error
  clearing, exactly-once visits, save gating and one pass notification.
- W1 forms validation (Windows, 2026-09-06): formatting, workspace compiler
  check, strict all-target/all-feature Clippy and all 1,116 workspace tests
  passed. All ten focused form tests also passed with all features enabled.
  No public signatures changed; native interaction was not rerun. Changes are
  committed in `6e6af8d`. Length formatting is completed above; ignored Material
  options remain pending.

## Desired architecture

Keep signals, immutable widget descriptions, retained identity, phase-specific
invalidation and renderer-independent composition. Flutter informs concepts;
Incular defines its own Rust API and explicit deviations.

- Core owns dependency observation, identity and basic values; runtime owns
  scheduling, tasks and application lifecycle.
- Widgets own interaction/editing/layout/paint/semantics primitives. Controls
  own themed visual composition; Material resolves Material tokens and composes
  those existing mechanisms.
- Navigation, scrolling and animation own their domain state machines, with
  explicit attachment and lifetime contracts to runtime/widgets.
- Rendering owns neutral commands/layers; WGPU owns device resources, surface
  execution and actual GPU reclamation.
- Platform owns portable values/outcomes; desktop translates Winit and owns
  the host; OS crates own native services.
- DevTools observes bounded snapshots through a versioned protocol. Diagnostics
  must not become an unbounded lifetime owner or silently swallow actionable errors.

Do not replace all bools with enums. Keep independent options such as enabled
and read-only independent. Use enums when combinations describe exclusive
states (idle/drag/driven/ballistic, pending/completed/cancelled) and newtypes when
IDs or units can be confused. Do not replace exhaustive primitive enums with a
plugin registry, add crates just to shorten files, or rebuild completed machinery.

## Execution order and review boundaries

Each numbered slice below is separately reviewable, leaves the workspace usable,
and records validation before its commit. A workstream is not complete merely
because a file was split or a setter has a construction test.

| Order | Workstream | Audit findings | Dependencies |
| --- | --- | --- | --- |
| 1 | W1: stop silent API failures and fix editing/form defects | R01–R03 | Existing retained mechanisms |
| 2 | W2: bounded resource ownership and observable GPU failures | R04–R06 | Existing owned surface/shared device |
| 3 | W3: complete retained property contracts and reduce change fan-out | R08 | W1 establishes missing behaviors |
| 4 | W4: navigation transactions and runtime ownership | R07, R14 | Existing request/reactive contracts |
| 5 | W5: scrolling and animation attachment/interruption | R09 | W3 phase contract; W4 lifetime boundaries |
| 6 | W6: text, focus, gestures and accessibility end-to-end | R03, R12–R14 | W1, W3, W5 |
| 7 | W7: controls and Material ownership/validation | R01, R08, R11 | W1, W3, W6 |
| 8 | W8: native campaign verification by host | R12 | W2, W4–W7 |
| 9 | W9: tooling, public surface and lasting quality gates | R10, R11, R13 | Integrate after each stream; final closure last |

W1 defects and W2 memory/errors are higher priority than cosmetic cleanup.
W4 can be implemented before the remainder of W3 if its isolated regression
shows immediate user impact; record that scheduling change, not a new architecture.
No calendar estimate is asserted before each slice's tests and migration size
are known.

## W1 — Behavioral honesty first

1. **Form transaction:** write a failing three-field regression that records
   every validator call and every error after validate/save. Replace short-circuit
   validation with a full pass; notify once afterward. Reject save when any
   field is invalid without skipping validation of other fields. Snapshot user
   callbacks before invocation and define behavior for reentrant changes.
2. **Length formatting:** implement a shared grapheme-safe limiter with explicit
   enforcement/composing modes. Normalize selection and composing ranges against
   the output. Test None/enforced/deferred modes, emoji/combining text, paste,
   active IME composition and commit/cancel through the actual editing path.
3. **Material no-op cluster:** trace SliverAppBar pinned/floating/snap/stretch,
   Scaffold keyboard insets/extension, Card border order/semantic-container,
   Checkbox error state and ListTile autofocus into retained execution. Implement
   using existing scroll/layout/focus/semantics mechanisms. If a behavior is
   deliberately unsupported, remove/deprecate that promise with a named migration;
   never retain a silent accepted setter solely to satisfy parity.
4. Update affected specification entries to precise behavior and tests. Keep
   ListTile spacing behavior; remove only its redundant discard code if touched.

Exit: every identified ignored option has observable behavior or an explicit
unsupported migration; all fields validate; formatting respects grapheme and
composition boundaries. No additional hidden state manager in a Material wrapper.

## W2 — Resource lifetime, budgets and rendering outcomes

1. Inventory CPU image data, encoded keys, font bytes/layout entries, path meshes,
   GPU images/gradients/glyph pages, offscreen/effect textures and identity maps.
   For each record owner, key, strong references, admission, budget, eviction,
   in-flight-use protection and counters. Existing text/offscreen bounds stay intact.
2. Add an owned image-cache policy: byte/entry limits, explicit clear/trim,
   collision-safe lookup without copying the whole input on a hit, and decode
   size/error policy. Evict cache ownership without invalidating live handles.
3. Put shared GPU eviction at shared-device ownership, rather than claiming local
   bind-group removal frees shared textures. Track recent use across windows and
   retirement safety; bound identity metadata as well as payloads.
4. Separate presented, temporarily skipped, surface-recovered and failed outcomes.
   Validation/device failures must be observable. Preserve suboptimal frame
   presentation before reconfiguration and existing owned-window lifetimes.
5. Add deterministic churn tests with two windows sharing images/gradients; verify
   bounded resident bytes, retained live resources and reclamation after closure.
   Record GPU-driver-dependent native tests separately from policy tests.

Exit: memory stabilizes under churn within the documented budget plus live/in-flight
allowance; counters report actual shared residency; failure reasons reach the host.

## W3 — One complete property-to-phase contract

1. Complete a machine-checkable exported-widget property ledger, not only the
   initial families. Fields: public symbol/option, default, constructor and builder
   validation, retained owner, comparison/update, layout/paint/composite/input/
   semantics consumer, supported status and named regression.
2. Start with visibility, editable text, transform/origin/alignment, linked
   layers, effects, scrolling and interactive controls; then cover layout,
   collections, images, navigation scopes, overlays, platform wrappers and utilities.
3. Regroup one primitive family behind explicit transfer/comparison helpers.
   Preserve closed dispatch and small owner-specific interfaces. Demonstrate one
   new property needing no duplicate phase-policy edits in unrelated modules.
4. Core Invalidation is already unified. Finish the mapping and projections:
   semantic/hit-test geometry must follow compositor movement without relayout.
   Test translation, scale, rotation, linked followers, clipping, scroll and
   hidden semantics before/after cached paint. If caches are introduced, consume
   their specific invalidation flags rather than relying on unconditional work.
5. Audit every `internal` export against sibling consumers. Migrate callers to
   narrow backend interfaces; do not expose arenas or mutable retained storage.
6. Audit invariants beyond completed Constraints: builder bypass, NaN/infinity,
   line bounds, ratios, ranges, insets and sizes. Keep arbitrary snapshots as data;
   validate where the contract requires it. Document normalize/panic/Result policy.

Exit: every exported option has a reviewed disposition and behavioral evidence;
property changes schedule only necessary work; identity and deterministic
performance contracts pass. No arbitrary file-size threshold is the acceptance test.

## W4 — Navigation transactions and smaller runtime owners

1. Replace parallel navigation vectors with a RouteEntry carrying identity,
   widget, settings, restoration metadata and lifetime/cleanup data.
2. Introduce explicit stable page keys. Allow repeated route names when identities
   differ; reject duplicate identities with a typed result before any mutation.
3. Route push/pop/replace/set_pages/restoration through one transaction that emits
   observers and cleanup after state borrows end. Keep guarded-pop mutation checks.
4. Test observer sequences, reentrant guards/observers, keyed reorder, route
   removal, restored routes and deep-link delivery exactly once. Define ownership
   of focus restoration and route-scoped tasks.
5. Map runtime Application/Runtime fields to lifecycle, scheduling, input,
   restoration and diagnostics owners. Extract only bounded interfaces; keep the
   existing common request channel and scheduler. Make restore/rebuild failures
   explicit where they currently disappear into best-effort calls.

Exit: no parallel route arrays, no name-as-identity ambiguity, no callbacks under
mutable domain borrows, and no duplicated lifecycle engine.

## W5 — Scroll and animation state transitions

1. Decide single/multiple viewport attachment for each controller and enforce it.
   One offset/extent record cannot silently stand for unrelated viewports.
2. Specify idle, drag, driven, ballistic and suspended transitions before choosing
   representation. Keep motion origin, cancellation and notification ordering
   owned by the domain; visual scrollbar style is not the activity state.
3. Define completion/interruption outcomes for animate, jump, drag takeover,
   bounds changes, detach, route/window close and reduced motion. Use runtime's
   clock/ticker attachment and scoped subscriptions.
4. Differential/property-test variable extent insertion/removal/reorder, nested
   handoff and lazy deep jumps. Preserve sliver index reuse and keep-alive identity.

Exit: deterministic transition table tests; no stuck activity or duplicate
start/end; one view cannot silently overwrite another's geometry.

## W6 — Input, text and semantic consistency

1. Trace formatter/controller/history/IME/native command ownership end to end;
   keep UTF-8/grapheme invariants and authoritative selection in one layer.
2. Verify focus removal, disabled/read-only policy, keyboard activation, gesture
   cancellation, multi-contact arbitration and captured-pointer cleanup across
   widget, popup, route and window teardown.
3. Validate semantic graph edges, roles, mixed checked state, stable node IDs,
   transformed/clipped bounds and rejection of stale native actions. Require
   equivalent actions for default/custom visual slots.
4. Add reentrant listener/validator tests and adopt callback snapshots where
   necessary; do not remove explicit backend manual-listener APIs without migration.

Exit: keyboard/pointer/accessibility invoke the same action once; editing survives
composition/undo/restoration; no stale focus/capture/semantic owner remains.

## W7 — Controls and Material as presentation layers

1. Inventory exported control/Material fields against W3's ledger, including
   generated builders. Cover buttons, selection controls, text fields, sliders,
   progress, lists, menus, dialogs, navigation and shell composition.
2. Centralize token resolution at theme/state boundaries. Preserve explicit user
   overrides and inherited dependency tracking; avoid per-component hardcoded
   defaults when the theme already owns that choice.
3. Remove duplicated action/focus/editing/scroll/semantics code from visual wrappers.
   Default and custom slots must share the behavioral root and state transitions.
4. Reconcile legacy component_impl/p0/foundation organization based on owners,
   not the historical order features were added. Keep staged API migrations small.

Exit: same control behaves identically under different visuals; theme changes
invalidate appropriate consumers; all accepted fields reach execution.

## W8 — Native verification and historical campaign closure

For each plan 01–13, record public API → portable request/event → runtime owner →
OS adapter → native outcome → focused test → capability limitation. Reuse Stage
D/E lifecycle/input tests; do not reimplement them.

- Windows: move/resize, DPI/display changes, pointer capture, menu generations,
  shortcuts once, popup focus/dismissal, clipboard/drop negotiation, dialog parent
  closure, shell teardown, activation and reduced motion.
- macOS: same contract with AppKit main-thread/ownership and child-window/menu rules.
- Linux: separate X11, Wayland and portal cases; unavailable position/menu/service
  APIs remain typed unsupported, never fabricated success.
- Android/iOS: target compile and semantic projection tests for current scope.
  A full mobile host is a separate product decision, not an implicit task here.
- Unsafe-site review: thread/lifetime/aliasing justification and owner teardown
  regression; preserve safe owned surface constructors.

Exit: per-host evidence matrix, no stale callback can affect a recycled owner,
and untested native behavior is marked unverified. Host limitations do not
justify marking a native acceptance criterion complete from a Windows build.

## W9 — Tooling and sustainable closure

1. Bound DevTools requests and coalesce wake notifications. Define backpressure,
   disconnect/reconnect and stale-response policy; retain session authentication.
   Bound model/history data and surface discovery/session failures usefully.
2. Separate diagnostic counter wrap from identity allocation; test exhausted
   slots/IDs with a small test allocator and prevent stale identity reuse.
3. Audit facade features, implementation bridges and compatibility aliases.
   Keep painting as its accepted pure re-export. Keep macros explicitly scaffolded
   unless a concrete composition requirement justifies implementation.
4. Replace contradictory parity assertions with supported/Rust-equivalent/
   omitted/deferred records carrying rationale and executable evidence. Do not
   regenerate architecture allow-lists to make an accidental dependency pass.
5. Verify locked MSRV, target/feature combinations, dependency policy and unused
   dependencies; expand documentation linting crate by crate. Retain ordinary
   Cargo commands and operation-count performance gates. Measure build bottlenecks
   before changing bounded jobs/incremental policy; no new CI/xtask by default.

Exit: every crate has an owner, API class, support statement and evidence; every
finding is fixed, deliberately omitted with migration, or explicitly deferred
with reason and acceptance test. No misleading completed parity claims remain.

## Validation and completion protocol

For code changes run `cargo fmt --all -- --check`, `cargo check --workspace`,
`cargo test-constrained`, and strict workspace/all-target/all-feature Clippy.
Public API changes also run all-feature tests and warning-denied rustdoc.
Run affected native/feature/MSRV/dependency checks as specified in QUALITY.md.
All tests/helpers live under tests/. Prefer behavioral regressions, compile-fail
API boundaries and differential/property tests over assertions mirroring internals.

For each slice record: baseline, confirmed failing behavior or measured debt,
changed owner/API, migration, tests/results, performance/resource evidence where
relevant, host coverage, residual limitations and commit. Do not mark an entire
workstream complete after only its first slice. The user has authorized committing every completed, validated slice.

Planning-pass validation (2026-09-06): inventory paths and Markdown links,
`git diff --check`, `cargo fmt --all -- --check`, all five architecture-contract
tests and `cargo check --workspace` passed. Prior baseline evidence is 1,112
tests in each configuration; this planning pass did not rerun the full suites
or native interaction. Production Rust and manifests are unchanged.
