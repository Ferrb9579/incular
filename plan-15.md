# Plan 15 — Whole-codebase quality and architecture completion

Status: W1 forms (`6e6af8d`) and length formatting (`638f77e`) committed;
Material checkbox error presentation committed (`9dbcfee`); ListTile autofocus
and initial-layout focus resolution committed (`f8edecb`). Card policies are
committed (`ad72860`). Scaffold keyboard avoidance is committed (`ad7adf9`).
Body-extension policies are committed (`9544d54`). SliverAppBar retained
integration and pinning are committed (`835bdcd`), as is title preservation
(`8a51032`). Floating integration is committed (`7bc9cf8`); resizing bounds
and scroll invalidation are committed (`a4095c9`), followed by typed resizing
modes (`83c88b1`). Material expanded/collapsed composition is complete and validated.
Material stretch is complete and validated. Snapping is complete and validated,
including interruption, reduced motion, and the Scaffold/SliverAppBar
inventory. The AppBar slot-constraint correction is complete and validated,
and the corrected toolbar rows carry implementation and regression evidence
in the inventory below. W1 is marked complete.
Audit baseline `42befb7`, 2026-09-06. W2–W9 are pending.
This expands the remaining scope of plan 14 F–J. Completed plan 14 commits stay
complete; its architecture decisions remain authoritative. Use this document
as the remaining-work schedule, with plan 14 retained as the implementation
history. The original planning pass changed documentation and inventory only; subsequent
implementation progress is recorded below.

Evidence: [whole-codebase audit](docs/WHOLE_CODEBASE_AUDIT.md),
[complete file inventory](docs/CODEBASE_INVENTORY.json),
[initial property ledger](docs/RETAINED_PROPERTIES.md).

## Implementation progress

- W1 AppBar slot constraints: the toolbar now reserves fixed slots before
  allocating title space, in one shared Row for both alignments. Before,
  the title sat beside an empty `Expanded` spacer unconstrained, so a long
  title pushed fitting actions outside the toolbar (actions at x=242 in a
  200px bar); `leading_width` was ignored around explicit content; and the
  centered row had no defined collision behavior. After, the leading slot
  is exactly `leading_width` when configured (explicit content fills it),
  each action keeps its natural size, and the title cell takes the remainder
  through one `Expanded` with its own alignment (`CENTER_LEFT` with the
  exact gap, or `CENTER` with the gap as the minimum on each side) — composed
  from existing Row/SizedBox/Expanded/Align/Padding, no new mechanism, no
  second algorithm. A long title is constrained to its cell while actions
  stay put and clickable (click-counter evidence, not bare hits); centered
  content stays centered in the remaining width and clamps symmetrically
  under pressure; fixed slots are never shrunk or dropped (excess extends
  past the trailing edge when the toolbar is too narrow). Regressions
  (`app_bar_toolbar`: long-title action stability, two `leading_width`
  values, asymmetric centering, clamp-under-pressure, extended rebuild of
  spacing/width/content) failed before the fix and pass after. Formatting,
  workspace compilation, constrained workspace tests, strict
  all-feature/all-target Clippy, all Material all-feature tests and
  warning-denied Material rustdoc passed. Live native tests were not run.
  The inventory rows below record the verified evidence.
- W1 AppBar toolbar composition: `title_spacing`, `leading_width`, and
  `automatically_imply_leading` now reach retained geometry. Before, the
  toolbar row used distributing alignment, which absorbed the inter-item
  spacing into free space: changing `title_spacing` (or the implied-leading
  placeholder width) left identical geometry (title at x=144 regardless of
  the setting). After, the default start-aligned toolbar places each slot
  explicitly — leading (or its placeholder), an exact title gap, an
  `Expanded` spacer pushing actions to the trailing edge — reusing the
  existing Row/Padding/Expanded mechanisms with no new state; centered
  composition is preserved byte-for-byte. Scaffold `drawer`/`end_drawer`/
  `bottom_app_bar`/`background` execution is locked by a retained regression
  (dock positions, bottom-region fill, background paint, topmost click
  order), and `bottom_app_bar` precedence is now documented. Regressions
  (`app_bar_toolbar`: spacing values, placeholder, opt-out, rebuild update
  with hit test, centering lock, flexible-space order, shadow on/off;
  `scaffold_extensions`: drawer/bottom-app-bar/background execution) failed
  before the fix (spacing/geometry) and pass after. Formatting, workspace
  compilation, constrained workspace tests, strict all-feature/all-target
  Clippy, all Material all-feature tests and warning-denied Material rustdoc
  passed. Live native tests were not run. The slot-constraint follow-up
  above supersedes the centered-composition claim with the defined
  slot-constraint contract.
- W1 retained snapping: `SliverAppBar.snap(true)` maps to neutral retained
  snap behavior on all floating header slivers instead of remaining a silent
  setter. Scroll owns activity: snap runs start only from actual scroll-end
  notifications observed through the controller's listener stream (owned by
  each snap-enabled render sliver, so disposal unsubscribes), never from
  unchanged offsets; programmatic jumps never start a snap. Widgets owns the
  retained presentation animation: an explicit idle/running/settling state
  drives the effective offset over the shared 300ms EaseOut timing, animating
  presentation only while logical extent and controller offset never move.
  The revealed half (or more) completes revealed, the hidden half completes
  hidden clamped to the scrolled distance scroll-driven presentation enforces
  (an already-maximally-hidden header starts no run); floating-pinned headers
  animate only their collapse range. New scroll movement interrupts smoothly
  from the current presentation, reversal and repeated cycles re-decide on the
  next end, stretch keeps presentation ownership during overscroll, and
  descriptor replacement transfers only compatible in-flight runs (fixed
  floating headers with identical flags) while range-changing replacements
  yield coherently through anchor correction. Frames continue through the
  existing sliver tick/is_animating plumbing with a settling frame that
  presents the exact endpoint before stopping; no Material timer, polling
  loop, second controller, or duplicate engine exists. Regressions cover both
  endpoints with intermediate geometry, floating/floating-pinned,
  explicit/natural heights, reverse, disabled snapping, the documented
  non-floating policy (neutral inert, Material snap-implies-floating),
  interruption, reversal, repeated exact cycles, completion frame scheduling,
  stretch suppression, content-size and descriptor replacement, retained
  paint, targeted hit tests, semantic bounds, and Material retained-path
  integration for plain/pinned/natural headers. The regressions exercise new
  snap API and animation state absent before this slice, so none pass without
  the implementation. Intermediate states were verified along the way:
  without the settling frame the exact endpoint never presents; without the
  feasibility clamp a hide target fights scroll presentation instead of
  settling; overlay paint reads empty without its compositor pass.
  Formatting, workspace compilation, constrained workspace tests, strict
  all-feature/all-target Clippy, all Material all-feature tests and
  warning-denied Widgets/Material rustdoc passed. Live native tests were not
  run. W1 is not marked complete.
- W1 snap interruption on activity start: the snap subscription now observes
  scroll-activity starts as well as ends into one overwriting pending-activity
  cell, so a new activity cancels running and settling work and invalidates a
  stranded end even when the offset has not moved, while preserving the
  current presentation for later movement to continue from. End followed by
  Start starts nothing; Start followed by End requests a fresh decision.
  Offset-delta interruption still covers unbracketed programmatic movement,
  and subscription ownership and disposal are unchanged with no polling, extra
  controller, or Material-specific handling. Regressions freeze mid-snap
  presentation across later frames after a moveless begin, assert End-then-
  Start silence and Start-then-End restart, verify delta continuity from the
  frozen presentation with no restart without an end, cancel settling work on
  a new activity, and cover freeze plus restart through the retained Material
  path. Formatting, workspace compilation, constrained workspace tests,
  strict all-feature/all-target Clippy, all Material all-feature tests and
  warning-denied Widgets/Material rustdoc passed. Live native tests were not
  run. W1 is not marked complete.
- W1 snap reduced motion: header snapping now honors the retained
  `reduced_motion` environment policy, including changes while mounted. The
  tree pushes the retained value into each sliver-viewport delegate before
  ticking (delegates cache the last push and stamp snap-capable descendants,
  forwarding through transparent wrappers, so fresh delegates and render
  slivers observe the current policy on their first frame with no rebuild and
  all controller/delegate/element/render identities preserved). Under reduced
  motion an eligible end resolves directly to the same endpoint with no
  interpolation, and enabling it mid-flight resolves the run's own target on
  the next tick; both flow through the shared completion path so the endpoint
  still presents exactly before scheduling goes idle, with controller offset
  and logical extent unchanged. Disabling restarts nothing, and activity
  starts still cancel without resurrecting work. Regressions set the policy
  through the real environment-update path: enabled before mounting, enabled
  mid-snap toward both endpoints, disabled afterwards with a bare end staying
  quiet, start cancellation composed with a policy change, natural headers
  with semantic bounds, and a retained Material integration with endpoint
  paint, hit testing, and frame scheduling. Other animation policies remain
  untouched; reduced motion outside header snapping stays tracked under
  W5/W8. Formatting, workspace compilation, constrained workspace tests,
  strict all-feature/all-target Clippy, all Material all-feature tests and
  warning-denied Widgets/Material rustdoc passed. Live native tests were not
  run. W1 is not marked complete.
- W1 nested-viewport invalidation: the enclosing-sliver invalidation walk now
  continues outward through shrink-wrapping viewports, whose size derives
  from content, so inner content changes revalidate outer natural-header
  owners; fixed-size viewports stop propagation, so inner scrolling, inner
  content growth that cannot resize the viewport, and paint-only updates
  leave the outer measurement and overscroll untouched. Reproduction first
  showed the outer header retaining its stale total while inner content grew
  (plus an orthogonal shrink-wrap first-frame estimate lag when initial
  content differs from the lazy default, which the regression isolates by
  matching initial content to the default). Regressions cover nested growth
  beyond the old stretched total and shrinkage with authoritative geometry
  before manual recovery, render/delegate identity survival, drift-free
  repeat stretch/recovery, and the fixed-size negative. The nested growth
  case failed against the previous implementation; the fixed-size case
  passes as a preservation lock. Formatting, workspace compilation, all
  1,199 workspace tests, strict all-feature/all-target Clippy, all 87
  Material all-feature tests and warning-denied Widgets rustdoc passed. Live
  native tests were not run. Snapping remains pending; W1 is not marked
  complete.
- W1 controller-driven invalidation: text content revisions observed in the
  layout preamble now route through the existing enclosing-sliver
  invalidation channel, so a multiline editor that grows mid-overscroll
  revalidates unbounded and lands authoritatively on the next completed
  layout with no manual recovery step; genuine range changes settle through
  Scroll's policy. Caret/selection revisions stay visual-only and never
  invalidate, and revisions are consumed when the field is measured so the
  measurement pass cannot invalidate itself. Controller, element, render,
  and delegate identity is preserved throughout. Regressions cover neutral
  and Material growth beyond the old stretched total, shrinkage,
  toolbar-preserving paint, distinct-descendant hit testing, semantic
  bounds, same-height and selection-only preservation, convergence, and
  drift-free recovery; both controller tests failed against the previous
  implementation. Formatting, workspace compilation, all 1,189 workspace
  tests, strict all-feature/all-target Clippy, all 87 Material all-feature
  tests and warning-denied Widgets/Material rustdoc passed. Live native
  tests were not run. Snapping remains pending; W1 is not marked complete.
- W1 inherited and controller-driven invalidation: the invalidation drain now
  covers inherited build dependencies and layout-affecting inherited render
  updates at their source, so ordinary LayoutBuilders and reflowed text pick
  up inherited value changes with the delegate retained; paint-only updates
  structurally take no invalidation branch. Invalidation routes through
  transparent wrappers by sliver position. Regressions use an ordinary
  theme-reading LayoutBuilder bottom (growth, shrinkage, paint-only
  preservation, convergence, recovery), a font-size reflow with a color-only
  negative, wrapper routing, a drain-plus-transfer composition lock, and
  controller-driven multiline editor growth documenting the actual supported
  behavior (settled learning, single-line stability, overscroll coherence
  with authoritative recovery). The inherited, wrapper, and render-arm cases
  failed against the previous implementation. Formatting, workspace
  compilation, all 1,188 workspace tests, strict all-feature/all-target
  Clippy, all 87 Material all-feature tests and warning-denied
  Widgets/Material rustdoc passed. Live native tests were not run. Snapping
  remains pending; W1 is not marked complete.
- W1 same-delegate content invalidation: descendant rebuilds from explicit
  local-state revisions now invalidate an enclosing natural header's cached
  measurement (demoting to an estimate seed), so signal-style in-place
  content changes revalidate unbounded even mid-overscroll instead of staying
  stale until recovery. Only revision-driven rebuilds invalidate:
  constraints-driven rematerializations from scrolling, stretch presentation,
  or measurement passes carry no content signal, which keeps invalidation
  idempotent, loop-free, and drift-free with descendant identity preserved.
  Genuine range changes settle through Scroll's existing extent policy.
  Regressions grow and shrink a stateful bottom beyond the old stretched
  total during overscroll (neutral and Material, with toolbar/bottom
  placement, paint, hit testing, semantics, convergence, and subsequent
  stretch/recovery), plus same-size rebuild preservation locks. The growth
  cases failed against the previous implementation; the preservation locks
  guard the new path's precision. Formatting, workspace compilation, all
  1,182 workspace tests, strict all-feature/all-target Clippy, all 84
  Material all-feature tests and warning-denied Widgets/Material rustdoc
  passed. Live native tests were not run. Snapping remains pending; W1 is
  not marked complete.
- W1 natural-header validity: measurement validity now depends on its actual
  inputs. Validated extents record their cross extent; a cross change demotes
  back to an estimate for unbounded revalidation, including mid-overscroll,
  with genuine range changes settling through Scroll's documented extent
  policy. Transferred measurements arrive strictly as estimate seeds — never
  validity — with reversal tracking moving only across identical scroll
  behaviors, so equivalent replacements stay range-stable while taller
  replacement content re-establishes itself instead of retaining stale sizes.
  The equal-value estimate transition always requests another layout so
  presentation constraints still switch. Regressions cover taller
  replacement bottoms during overscroll (settling per Scroll policy, then
  exact re-stretch and recovery), same-delegate cross-axis rewrapping settled
  and during overscroll, first measurement during overscroll with an exact
  estimate, and a corrected width-change sequence that no longer preserves a
  stale total after narrowing — all with toolbar/bottom placement, paint,
  hit testing, semantics, and recovery verified. The width-change, taller
  replacement, and unbounded-first cases failed against the previous
  implementation. Formatting, workspace compilation, all 1,178 workspace
  tests, strict all-feature/all-target Clippy, all 82 Material all-feature
  tests and warning-denied Widgets/Material rustdoc passed. Live native
  tests were not run. Snapping remains pending; W1 is not marked complete.
- W1 Material stretch corrections: the natural header no longer sizes its
  toolbar from bottom extent hints and never silently disables stretching.
  Material uses one retained Flex structure for measurement and presentation:
  the bottom is measured first under real cross constraints (covering custom
  `LayoutBuilder` bottoms and wrapping text whose actual height differs from
  any hint) and the toolbar fills the remainder when bounded or resolves its
  natural height when unbounded. The internal hint export is removed again.
  The neutral lifecycle is now explicit (`Estimate` vs `Measured` plus settled
  vs stretched presentation): unverified estimates always measure unbounded
  first, stretched samples can never pollute logical extent, and replacing a
  viewport descriptor transfers validated measurements (plus box/floating
  extents and compatible reversal tracking) by sliver position, so a header
  replaced during overscroll keeps its true size and scroll activity survives.
  Regressions cover hint-less bottoms, wrapped-text actual heights, cross-axis
  re-wraps with the same delegate settled and during overscroll, toolbar
  paint/bottom placement/clipping/hit/semantics while stretched and after
  recovery, replacement during overscroll, and repeated cycles without drift.
  All five new regressions failed against the previous implementation
  (fallback gap, hint-mismatched toolbar, estimate clamp). Formatting,
  workspace compilation, all 1,175 workspace tests, strict
  all-feature/all-target Clippy, all 81 Material all-feature tests and
  warning-denied Widgets/Material rustdoc passed. Live native tests were not
  run. Snapping remains pending; W1 is not marked complete.
- W1 Material stretch: `SliverAppBar.stretch(true)` maps to the neutral
  overscroll policy for both explicit-height and naturally measured headers.
  Explicit heights reuse `SliverResizingHeader` with Stretch; natural headers
  use the new Widgets-owned `SliverNaturalHeader`, whose logical extent is the
  last unstretched measurement and whose stretched samples never update that
  measurement. Material resolves the toolbar against the stretched extent from
  a measured bottom hint while the bottom keeps its height, using the same
  AppBar structure in both branches so slot identity and mounted theme
  resolution survive. The viewport now reports leading overscroll as a
  negative physical offset in reversed viewports too, so either leading edge
  can stretch; Scroll still owns offsets/physics, Widgets owns geometry and
  measurement, Material owns presentation only. Regressions cover explicit and
  natural headers across all pinned/floating combinations, bottom/toolbar
  paint, hit testing, semantic bottom bounds, repeated stretch/recovery
  without drift, later natural-size changes, disabled/clamping/non-leading
  cases, theme resolution, ordinary Widget distinction, and forward/reversed
  viewports. The reversed-viewport stretch regressions failed before the
  viewport physical-offset fix, and the natural slot-identity regression
  failed before the unified presentation. Formatting, workspace compilation,
  all 1,170 workspace tests, strict all-feature/all-target Clippy, all 78
  Material all-feature tests and warning-denied Widgets/Material rustdoc
  passed. Eight new regressions cover these contracts. Live native tests were
  not run. Snapping remains pending.
- W1 stretch prerequisites: neutral resizing headers have typed Translate/Stretch
  overscroll policy. Leading negative overlap expands child and paint extent
  while preserving logical scroll extent. Sliver-owned placement fills the
  overscroll gap without automatic pinning displacing it. Bouncing positions
  survive unchanged metric publication; range changes and clamping still settle
  them, and transient overscroll is not persisted as restoration state.
  Regressions cover both axes, all header modes, overlap eligibility, restoration
  of normal size and stable scroll range. Material stretch wiring and snapping
  remain pending. Formatting, workspace compilation, all 1,162 workspace tests,
  strict all-feature/all-target Clippy and warning-denied Widgets/Scroll rustdoc
  passed. Three new regressions cover these contracts. Live native tests were
  not run.
- W1 Material collapse: explicit total-header heights select neutral resizing
  with all pinned/floating combinations. Bottom content keeps its measured height;
  the toolbar fills the remainder and retains its slots across scroll changes.
  A local clip bounds presentation to the current header extent. Constructor
  and typed-builder height inputs normalize non-finite/negative values alike.
  Headers with no explicit heights retain natural measurement; ordinary Widget
  conversion remains static. Three regressions cover geometry, defaults/bounds,
  slot identity, bottom position and toolbar paint. Formatting, workspace
  compilation, all 1,159 workspace tests, strict all-feature/all-target Clippy,
  all 72 Material all-feature tests and warning-denied Material rustdoc passed.
  The initial expanded-height regression failed before the integration.
  Live native tests were not run.
  Snapping and stretching remain pending.
- W1 resizing scroll modes: SliverResizingHeader exposes a typed Scroll/Pinned/
  Floating/FloatingPinned policy with the existing pinned default. Fixed and
  resizing floating headers share bounded scroll state and floating geometry.
  A retained regression exercises collapse, scrolling out, reversal and repeated
  layout across all modes without rebuilding children. Material integration is
  the next slice. Formatting, workspace compilation, all 1,156 workspace tests,
  strict all-feature/all-target Clippy and warning-denied Widgets rustdoc passed.
  The public enum is registered under B08 with retained regression evidence.
- W1 collapse prerequisites: SliverResizingHeader now normalizes constructor
  and typed-builder bounds consistently, including non-finite values and a
  maximum below the minimum. Its explicit ScrollOffset layout dependency keeps
  shrinking/expanding geometry current inside an already materialized cache
  window. Regressions cover invalid bounds and retained size changes without
  widget rebuilds. Material expanded/collapsed composition remains pending.
  Validation (Windows): formatting, workspace compilation, all 1,155 workspace
  tests, strict all-feature/all-target Clippy and warning-denied Widgets rustdoc
  passed. Two new regressions cover the corrected contracts. Live native tests
  were not run.
- W1 floating SliverAppBar / part of R01: Material delegates floating headers
  to the neutral SliverFloatingHeader; fixed-height pinning keeps precedence
  when both options are enabled. Neutral headers measure deferred content and
  bound hidden distance to the header extent so reversal reveals immediately.
  The retained protocol declares scroll-layout dependency through wrappers and
  distinguishes Flow/Pinned/Floating placement instead of conflating overlay
  ordering with pinning. Paint and hit order agree for returning headers.
  Tests cover long-scroll reversal, repeated layout, both axis/direction inputs,
  deferred measurement, padded cache-window reuse in layout/compositor phases,
  paint/hit ordering and Material bottom content plus pinning. Snapping,
  stretching and expanded-to-collapsed motion remain pending.
- Floating validation (Windows): formatting, workspace compiler checks, all
  1,153 workspace tests and strict all-feature/all-target Clippy passed. All 69
  Material tests passed with all features; Widgets and Material rustdoc passed
  with warnings denied. Four new neutral regressions and one Material regression
  cover the changes. The reversal, deferred-measurement and overlay-hit tests
  failed before their corresponding fixes. Live native tests were not run.
- W1 SliverAppBar title updates: replace only the title slot on the configured
  AppBar instead of reconstructing it. Leading/actions/bottom slots and visual
  policy survive a title change. A paint regression verifies the preserved
  slots/background and replacement of the old title. Formatting, workspace
  compiler checks, all 1,148 workspace tests and strict all-feature/all-target
  Clippy passed; all three SliverAppBar regressions passed with all features.
- W1 SliverAppBar / part of R01: the Material descriptor implements the neutral
  Sliver protocol so it can be passed directly to CustomScrollView. Pinned
  headers delegate to PinnedHeaderSliver; ordinary headers use SliverToBoxAdapter.
  Both paths measure app-bar bottom content and resolve the mounted theme.
  Regression coverage checks forward/reverse scrolling without widget rebuilds,
  measured extent and theme paint. Conversion to Widget remains box presentation.
  Floating, snapping, stretching and expanded-to-collapsed motion remain pending.
- SliverAppBar integration validation (Windows): formatting, workspace compiler
  checks, all 1,147 workspace tests and strict all-target/all-feature Clippy
  passed. All 67 Material tests passed with all features, including two new
  regressions. Material rustdoc passed with warnings denied. Live native desktop
  tests remained opt-in and were not run.
- W1 Scaffold extensions / part of R01: the app-bar and bottom regions can
  overlay the body instead of reserving space. Region sizes come from their
  content, including app-bar bottom content and the sheet/bottom-bar combination.
  Incular's extend_body covers the entire bottom region, including a sheet.
  App-bar bottom columns use minimum main-axis sizing in either placement.
  Private slot keys protect body/FAB identity and distinguish reserved regions.
  Keyboard avoidance applies to the resulting layout; SliverAppBar remains pending.
- Scaffold extension validation (Windows): formatting, workspace compiler checks,
  all 1,145 workspace tests and strict all-target/all-feature Clippy passed.
  Three extension regressions and the three existing inset tests passed with
  all features. The two initial geometry regressions failed before the fix.
  Live native desktop tests remained opt-in and were not run.
- W1 Scaffold insets / part of R01: enabled avoidance excludes the bottom view
  inset from the scaffold's usable layout area through neutral Padding. Body,
  bottom bars and floating controls reflow together; the property opts out of
  that behavior. Insets remain physical environment data, so a nested scaffold
  can opt out when its parent owns avoidance. The stable wrapper preserves
  retained identity as the keyboard opens/closes. Body-extension policies and
  SliverAppBar remain pending.
- Scaffold inset validation (Windows): formatting, workspace compiler checks,
  all 1,142 workspace tests and strict all-target/all-feature Clippy passed.
  Three focused all-feature tests cover slot geometry, keyboard open/close,
  opt-out, retained identity, oversized insets and nested avoidance ownership.
  The interrupted workspace test run was rerun to completion. Live native
  desktop tests remained opt-in and were not run.
- W1 Card / part of R01: the outline paints above the Material background and
  before or after content as requested, uses the surface radius, and stays
  outside layout sizing. Private keyed slots preserve the child when order
  changes. The optional semantic group preserves child actions. A Positioned
  layout proxy now declines legacy hits when its child declines, so ignored
  foreground decoration cannot block underlying controls.
- Card validation (Windows, 2026-09-07): formatting, workspace compiler checks,
  all 1,139 workspace tests and strict all-target/all-feature Clippy passed.
  Four Card tests and one neutral positioned-hit test also passed with all
  features enabled. Both initial policy regressions failed before the fix;
  pointer activation additionally reproduced the Positioned fallback defect.
  Live native desktop tests remained opt-in and were not run.
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
  pending: Scaffold and SliverAppBar.
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

### W1 retained-behavior inventory

Every option below was traced through retained execution (not just storage);
disposition reflects observed geometry, paint, input, or semantics plus a
named regression. Box presentation (`Widget` conversion) stays static by
contract wherever noted.

| Option | Retained path | Behavior + evidence | Disposition |
| --- | --- | --- | --- |
| SliverAppBar pinned/floating/snap/stretch, expanded/collapsed heights, title/slots/theme | Neutral resizing/natural/floating headers own geometry; Material maps config | Collapse, float, snap, stretch, title, theme regressions (`sliver_app_bar*`) | implemented |
| AppBar title/leading/actions/bottom/background/foreground/elevation/toolbar_height | `AppBar::build` composition + Material surface | Slot paint, bottom measurement, toolbar background, shadow on/off (`sliver_app_bar`, `app_bar_toolbar`) | implemented |
| AppBar title_spacing | Was absorbed by distributing alignment (no effect); now an exact gap in the shared slot geometry (minimum per side when centered) | `title_spacing_separates_title_from_leading` failed before, passes after; rebuild + centered rows below | implemented |
| AppBar leading_width, automatically_imply_leading | Leading slot is exactly `leading_width` around explicit content as well as the implied placeholder | `leading_width_and_imply_place_title_deterministically`, `leading_width_constrains_explicit_leading_content` (24/56), extended rebuild | implemented |
| AppBar center_title | Centered in the remaining width between fixed slots (not the full bar); symmetric-gap minimum, clamps centered under pressure | `center_title_keeps_centered_composition` (80), asymmetric + clamp rows | implemented |
| AppBar long-title/action reservation | Fixed slots reserved first via one `Expanded` title cell; long titles constrained, actions stay put and clickable | `long_title_does_not_displace_fitting_actions` failed before (actions at x=242), passes after | implemented |
| AppBar flexible_space, shadow_color, surface_tint_color, shape | Stack background layer; Material surface paint params | Paint presence/order, shadow on/off (`app_bar_toolbar`) | implemented |
| Scaffold body/app_bar/bottom_navigation_bar/bottom_sheet/FAB/regions/insets/extensions | Slot composition, measured regions, neutral padding | Extension, inset, identity regressions (`scaffold_*`) | implemented |
| Scaffold drawer/end_drawer/bottom_app_bar/background | Positioned slots; bottom-bar precedence documented; body paint | `drawers_bottom_app_bar_and_background_execute` (positions, paint, topmost clicks) | implemented |
| Card border order/semantic-container; Checkbox error; ListTile autofocus | Prior W1 slices | Card, checkbox, ListTile regressions | implemented |
| Form validate/save/callbacks; grapheme length formatting | Prior W1 slices | Form, formatter regressions | implemented |

No option above is silently ignored and none required an explicit-unsupported
migration: every accepted setter reaches retained execution with observable
behavior. Per-option styling exhaustiveness beyond this table belongs to the
W3 ledger (property-by-property evidence) and W7 (control/Material inventory).

W1 is marked complete: every inventory row above is implemented with named
regression evidence, all numeric fields funnel through checked construction,
grapheme/composition formatting holds per the cited slices, and Material
adds no hidden state manager (the toolbar composes existing neutral
primitives). No W1 rows remain open.

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

### W2 resource inventory

Surveyed against the implementation paths cited; "guarantee" means observed
code, "missing" means absent with no compensating path.

| Resource | Owner / path | Key, identity | Budget, eviction | Live/in-flight protection, counters |
| --- | --- | --- | --- | --- |
| CPU decoded images + encoded keys | `incular-image` `ImageCache` (`crates/incular-image/src/lib.rs`) | Full payload bytes (`Arc<[u8]>`, hash + byte equality); `ImageId` per decode | **Now:** `ImageCacheLimits` (default 64 entries / 32 MiB decoded + key bytes), oldest-first LRU, `clear`/`set_limits`, oversized served fresh | Eviction drops cache refs only; `Arc` handles stay valid. Counters: requests/hits/failures/decodes/evictions + `resident_bytes()` gauge |
| CPU font bytes | `incular-text` `TextEngine::font_handles` (`crates/incular-text/src/engine.rs`) | `(usize, usize, u32)` blob identity, one shared `Arc` per blob | No byte budget; unbounded map | Shared `Arc` keeps bytes alive while layouts reference them; no counters |
| CPU text layouts | `incular-text` `TextEngine::{cache, order}` | `LayoutKey`, `Arc<TextLayout>` | **Guarantee:** 2048-entry FIFO (`LAYOUT_CACHE_CAPACITY`) | Shared `Arc`; eviction by count only, no byte bound, no counters surfaced |
| GPU images | `incular-wgpu` `SharedGpuResources::{images, image_textures}` + per-renderer `image_cache` (`crates/incular-wgpu/src/resources.rs`, `renderer/resources.rs`, `pipelines.rs`) | Content `ImageId` → one `Arc<SharedGpuImage>`; context-local `SharedGpuResourceId` while retained | **Now:** `SharedTextureBudget` (default 256 entries / 256 MiB nominal texel bytes), cross-renderer LRU, oldest-first eviction at shared-device ownership, oversized served without admission, generation-checked touches, host dispatch on combined revision; per-renderer 600-unused-frame eviction retained | Shared `Arc` (map + renderer maps + frame locals); evictions report entry drops + still-referenced counts, never freed bytes; `texture_upload_bytes` stays cumulative traffic, residency is the policy gauge |
| GPU gradients | `SharedGpuResources::{gradients, gradient_textures}` + per-renderer `gradient_cache`, same files | Full `(stops identity, surface format)` key → one `Arc<SharedGpuGradient>`; stops identities mint per construction (clones share, distinct builds never alias); no registry | **Now:** same generic `SharedTextureCache`/`RendererImageCache` machinery (fixed 1 KiB nominal bytes, per-family generation sequence, bypass + age bound, combined-revision dispatch) | Same `Arc` graph as images; per-family counters; oversized path defined though unreachable at real sizes |
| GPU gradients | `SharedGpuResources::gradients` | `(GradientId, TextureFormat)` key | **Missing:** same as GPU images | Same as GPU images |
| GPU glyph pages/entries/fonts | `GlyphAtlas` (`crates/incular-wgpu/src/glyphs.rs`) | `GlyphCacheKey` entries, `FontId` fonts (hash of bytes + face index; faces never alias), resident/vacant page slots | **Now:** eager page-budget tightening with vacant-slot reuse under bumped generations (default 8 live pages) plus parsed-font entry LRU (default 8, `set_max_fonts`, zero parses transiently); oversize-page split, `MAX_GLYPH_BITMAP_BYTES` (8 MiB) + `MAX_GLYPH_RASTER_PPEM` (1024) raster guards | Slots never compact; per-page generations gate bindings; counters: page evictions/pressure skips/stale refreshes, `live_page_count` vs slot capacity vs cumulative allocations, `font_parser_cache_hits/misses/evictions` + `font_count()`; `memory()` counts resident pages only |
| GPU pipelines/identity maps | `SharedGpuContextInner::pipelines`, `SharedGpuResourceRegistry` | Surface `TextureFormat` / `ImageId`→`SharedGpuResourceId` | **Pipelines bounded by construction:** one entry per configured surface format (single registration site at renderer init, format-keyed lookup, no removal path), each holding the closed 26-contract registry built exactly once; racing duplicate creations drop unretained via `or_insert_with`. No budget or eviction machinery — the key space cannot churn. Identity maps track their owning caches (image registry mirrors the bounded image cache; glyph registry drains through retired placement keys). Entry counts in diagnostics; pipeline memory is driver-opaque, no byte estimates | Registry length counter only |
| Offscreen/effect textures, path meshes | Renderer passes (`crates/incular-wgpu/src/renderer/`) | Per-frame transient allocations | **Guarantee:** frame-scoped; no cross-frame retention to budget | Upload-byte counters only |

- W2 CPU image-cache slice: requests byte payloads through
  `ImageCache::load_bytes` with hit lookup that borrows the input (no copy
  on hits; one shared `Arc` on admission). Failures never cached; oversized
  entries decode fresh without admission; `clear`/`set_limits` release cache
  ownership while live handles stay valid. Regressions (`cache_policy`:
  shared identity, LRU order, exact byte accounting, oversized/zero-limit
  admission, clear with live handles, oldest-first trim, uncached failures,
  alias safety) pass; prior cache tests unchanged. Formatting, workspace
  compilation, constrained workspace tests, strict all-feature/all-target
  Clippy, focused image tests and warning-denied image rustdoc passed. Live
  native tests were not run.
- W2 decode-size/error policy: cache limits govern retained entries while a
  separate decode policy governs the work to produce an image — a cache
  budget never becomes a decode limit. Inspected the installed decoder
  (image 0.25.9): `load_from_memory` decodes under defaults with no strict
  dimensions; `into_decoder` applies the reader limits (PNG eagerly at
  construction, JPEG/WebP in `set_limits`, whose default never reports
  unsupported); native `max_alloc` cooperation is best-effort per decoder;
  and the RGBA8 conversion can expand sources up to fourfold. Decoding now
  runs through one bounded decoder instance carrying finite limits from its
  construction — no unlimited header probe: dimensions are read from that
  instance, an explicit gate checks strict 16384px dimensions plus checked
  `w*h*4` output bytes against a 256 MiB cap, the native output is reserved
  against the same budget, and pixels decode through the instance, with
  decoder limit errors preserved as `DecodeTooLarge` (dims attached when
  known; zero-sized when the limit fired during header construction before
  dims were available). Actual decoded dims are re-gated before conversion,
  and `from_rgba8` verifies the exact output length by construction.
  Oversized-for-cache images decode normally within the decode policy;
  failures and rejections never touch admission. Regressions assert the
  rejection reason precisely (header-construction enforcement in PNG and
  JPEG with a cheap 20000x100 fixture, absurd dims, valid-arithmetic 1
  GiB-output headers, malformed/truncated/error discrimination, grayscale
  conversion sizing, valid-but-uncacheable loads, rejection-without-
  eviction). Separate guarantees: checked RGBA output size (explicit gate),
  checked native output size (`total_bytes` reservation — the RGBA gate
  alone does not establish it), best-effort decoder-cooperative scratch,
  and conversion/ownership-transfer peak accounting. The consuming
  `into_rgba8` hands over already-RGBA8 buffers instead of cloning them;
  other sources still allocate fresh output beside the native buffer, and
  the `Vec`-into-`Arc` handoff allocates `Arc` storage and moves the bytes,
  so source and destination coexist transiently. 16384 is a CPU policy choice, not
  a GPU capability promise, and 256 MiB is a per-stage output bound, not an
  aggregate peak bound. Decoder-internal gaps (PNG post-construction
  buffers, best-effort cooperation, header-parse scratch) are stated, not
  closed; caller-decoded buffers for `from_rgba8` stay out of scope. Pixel
  correctness for native RGBA8 and converting sources rests on the existing
  pixel-asserting tests; no allocation behavior is claimed from them. Same
  validation as above; no GPU/rendering changes.
- W2 shared image-texture cache: admission and eviction moved to
  shared-device ownership (`SharedGpuContext::image_resource`) instead of
  living independently per window. Ownership graph, in liveness order:
  device-owned map entries (one `Arc` each, keys always equal the policy
  entries) → per-renderer maps (`GpuImage.resource`, bounded by the
  existing 600-unused-frame eviction, released on window disposal) →
  transient frame locals and in-flight submissions (wgpu keeps submitted
  work valid after `Arc` drop — verified against the vendored wgpu 30
  sources and the codebase's existing submit-then-evict practice — so no
  retirement delay was invented). Bind groups hold views at the wgpu level
  without holding the `Arc` and die with their renderer's entry. The
  headless `SharedImageTextureCache` owns limits (entries + nominal
  `w*h*4` texel bytes via checked arithmetic; row-pitch/driver overhead
  explicitly uncounted), cross-renderer LRU ticks, oldest-first eviction,
  and counters; evictions report dropped entries, nominal bytes, and how
  many stayed alive elsewhere — never freed GPU memory. Oversized textures
  upload with a fresh non-registry identity and bypass admission (per-
  renderer frame-evicted retention only); shrinking limits trims
  immediately; identity metadata is pruned on eviction so the registry
  cannot grow past retained entries (re-uploads mint fresh identities).
  Temporary budget excess from live/in-flight holders is reported, not
  enforced. Regressions (`shared_image_textures`, 11 tests, display-server
  free): cross-client reuse, LRU order, live-flag accounting, exact byte
  residency, oversized/zero-limit admission, oldest-first trim, re-upload,
  checked sizing incl. overflow, missing touch, and client-close
  reclamation with real `Arc`/`Weak` counts mirroring the wiring ops.
  Upload-path (`create_texture`/`write_texture`) coverage needs a native
  window target and is recorded as unverified here, not substituted.
  Gradients, glyphs, pipelines, and rendering outcomes are untouched.
- W2 shared image-cache integration: renderer acquisition now returns an
  explicit admitted/bypassed distinction, and the identity assertion only
  covers admitted textures — oversized-for-budget uploads (fresh
  per-upload identity, no registry entry) pass debug builds by
  construction. Shared recency reflects real renderer use including local
  hits: each renderer records use lock-free during the frame and flushes
  one deduplicated, generation-checked batch per frame after submit; a
  touch naming a superseded generation refreshes nothing and counts a
  stale touch. The shared map, policy entries, and registry identity stay
  in lockstep (debug-asserted in diagnostics). Renderer-local retention
  moved into the generic `RendererImageCache` coordinator — the same code
  production and headless tests run: frame-use batching, stale pruning
  (dropped entries release with their bind groups; submitted work stays
  valid by the wgpu lifetime contract), and age eviction. Shared eviction
  converges locally within one frame; idle clients reclaim explicitly via
  generation comparison with no frame activity. Coherence granularity is
  one frame by design: a mid-frame churn admission can evict an entry
  whose touch has not landed yet, and the owner re-resolves next use.
  Regressions drive production components with test resources (local hits
  protecting across churn, stale-touch refusal with replacement
  convergence, idle reclaim with live-clone safety, retention-vs-
  outstanding gauges, age eviction, drain dedup); standalone LRU and
  Arc-simulation tests remain for the policy alone. Same validation as
  above; upload-path GPU coverage still recorded as unverified.
- W2 idle reclamation and bypass retention: renderer-local entries now
  carry an explicit retention state — shared-generation-backed versus
  locally retained bypass — instead of a bare generation. Shared-
  generation pruning applies only to shared-backed entries; bypassed
  entries never contact shared state and are bounded by the same local age
  rule, so a repeatedly drawn oversized image reuses its resource without
  lingering once unused. Idle reclamation is a production path, not just
  an exposed helper: `RendererImageCache::reclaim_stale` compares against
  live shared generations with no frame activity, `WgpuRenderer::reclaim_stale_images` runs it on the owning thread under one shared
  lock, and the unconfigured early-return path in `frame.rs` invokes it
  for renderers still driven while presenting nothing. Otherwise the
  window owner calls it during idle maintenance (event-loop idle work,
  memory-pressure handling, pre-resume) or drops the renderer; no
  background reaper keeps idle renderers alive, shared eviction never
  reaches into a renderer it does not own, and no frame-delay retirement
  was invented (submitted work stays valid by the wgpu lifetime
  contract). Coherence granularity remains one frame by design. Regressions
  use the production orchestration throughout: bypassed reuse with zero
  shared contact then age release, generation pruning skipping bypassed
  entries, idle reclaim after churn via the production method (never the
  raw helper), and identical cumulative counters with different live sets
  proving counters alone cannot establish current retention. Same
  validation as above; upload-path GPU coverage still recorded as
  unverified.
- W2 host-driven idle reclamation: shared evictions advance a
  device-owned eviction revision (one bump per dropped entry), and the
  desktop host compares it once per event-loop pass in `about_to_wait`
  before sleeping — the pass that always follows the event handling where
  evictions happen, so no extra wakeup, redraw, or polling timer exists.
  On advance only, the production `SharedImageMaintenance::maintain`
  dispatch drives `reclaim_stale_images` on every window's renderer on the
  owning thread: the revision is read first and no shared lock is held
  across renderer calls (the frame path's lock order). Bypassed age
  honesty: the local age rule counts presented frames, never idle time —
  an idle renderer keeps bypassed entries until it resumes, reclaims, or
  drops, and no claim to the contrary is made. Regressions run the real
  dispatch with fake renderers owning production coordinators: idle-A/
  churning-B release without rendering or helper calls, active references
  surviving, unchanged revisions contacting zero clients, plus a resting
  gate test. Upload-path GPU coverage and per-window native dispatch
  remain recorded as unverified; gradients, glyphs, pipelines, and
  rendering outcomes are untouched.
- W2 shared gradient-cache eviction: gradients reuse the image-cache
  mechanisms exactly where the ownership contract is identical — the
  generic `SharedTextureCache<K>` policy and `RendererImageCache<K, R>`
  coordinator, one combined revision gating host maintenance for both
  families, and the same per-frame sync / idle-reclaim / disposal paths.
  Gradient-specific and deliberately not factored: the full-description
  key (stops identity plus surface format; per-construction identities
  mean distinct builds never alias, verified by regression), the fixed
  1 KiB nominal size, the per-family generation sequence (no registry),
  upload wiring, and per-family counters. Renderer-local gradient keys
  widened to the full shared key, so reconfigured lookups cannot reuse
  old-format entries (retirement itself is bounded decay, not immediate —
  see the correction slice below). Regressions
  (`shared_gradient_textures`, 8 tests, display-server free): no-alias
  descriptions, reuse, churn eviction, stale refusal, bypass bound, idle
  reclaim skipping bypassed entries, metadata reclamation, and
  cross-client protection; the image dispatch suite additionally proves
  gradient-only evictions still visit image-holding clients and image
  reclamation is unregressed. Upload-path GPU coverage stays recorded as
  unverified, as for images.
- W2 gradient ownership/accounting correction: renderer-local gradient
  entries retain the shared `Arc` (as images already did) instead of
  cloning the inner handle, so eviction liveness checks observe renderer
  ownership; drawing borrows the bind group through the wrapper. The
  acquisition-to-local conversion is now one shared generic,
  `SharedTextureAcquisition::into_local_parts`, used by both `ensure_*`
  paths — generic over the resource, it cannot name inner handles.
  Reconfiguration traced: `config.format` is fixed at construction and
  `resize` never touches the caches, so old-format entries were never at
  risk of wrong-format use; nothing retires them immediately, and that is
  now the documented contract (locally unreachable, hence age-evicted;
  shared-untouched, hence LRU-evicted under pressure) rather than the
  previously claimed eager retirement. Regressions prove outer-`Arc`
  identity through the production conversion, end-to-end eviction
  observation with real strong counts, and the decay (not immediate)
  behavior. Same validation as above; upload-path GPU coverage still
  recorded as unverified.
- W2 acquisition validity by construction: `SharedTextureAcquisition`
  previously took a redundant admission flag alongside the generation,
  reconciled only by a debug assertion absent in release builds. The flag
  is gone: the constructor takes `(resource, uploaded, generation)` and
  admission derives from the generation (`Some` admits), so contradictory
  state is unrepresentable in every build while upload occurrence stays an
  independent answer. Both `ensure_*` paths and all conversion tests use
  the narrowed API; the type remains exported under this crate's backend
  class as the renderer-integration handoff, not as test scaffolding.
  Same validation as above; upload-path GPU coverage still recorded as
  unverified.
- W2 bounded shared glyph-atlas ownership: page slots carry a content
  generation (`AtlasEntry.generation`, `AtlasPage::{generation,last_use}`)
  under a device-owned budget (`DEFAULT_MAX_GLYPH_ATLAS_PAGES=8`,
  `with_max_pages`/`set_max_pages` for tuning). Eviction retires the
  victim's placement/identity metadata (`take_retired_keys` cleanup in
  the shared rasterize path), bumps the page generation so stale
  (page, generation) pairs fail validation, and resets the shelf cursor
  for in-place reuse. Renderer bindings live in the generation-validated
  `RendererGlyphPages` table (replace-on-mismatch, reclaim against
  `page_generation`); the current frame's pages are pinned through
  lowering (`frame_pinned_glyph_pages`, cleared after submit) and a
  fully pinned budget degrades to one counted pressure skip, never a
  loop. Host `SharedImageMaintenance` dispatch is unchanged: the
  combined revision already sums glyph evictions, and renderer reclaim
  now also drops superseded atlas bindings. Over-budget LRU (recency
  then index) is deterministic across identical passes. Font-object
  budgeting stays pending. Evidence:
  `crates/incular-wgpu/tests/shared_glyph_pages.rs` (7 tests, production
  `GlyphAtlas`/`RendererGlyphPages`/dispatch, `u32` stand-ins only for
  GPU page textures): two-client reuse + recency refresh, churn
  retirement, pre-draw stale detection, no cross-glyph display through
  old identities, host-driven idle reclaim, pinned-active validity with
  clean pressure skips, over-budget determinism. Native/headless
  coverage only; GPU-backed upload/bind/draw paths recorded as
  unverified. Same validation as above plus warning-denied rustdoc.
- W2 glyph-page budget tightening: `set_max_pages` now enforces eagerly
  instead of awaiting the next allocation. Page-slot identity is split
  from live residency (`AtlasPage.resident`): tightening retires excess
  unprotected pages LRU-first into vacant, index-stable slots — never
  compacting beneath retained references — with bumped generations,
  dropped placements for the `take_retired_keys` drain (now public so
  hosts/tests drive the same production handoff the shared context
  uses), and a per-page eviction-revision advance. Protected pages may
  temporarily exceed the budget; the pending excess enforces against the
  next resolve's protection set even on a cache hit, so no unrelated
  future allocation is required. A zero budget retires everything
  unprotected and the allocation gate refuses new placements through
  vacant slots. Vacant slots reuse lowest-index-first under the bumped
  generation, so old identities resolve to `None`, never new contents.
  The shared-context resolve path additionally prunes shared page
  textures for vacant pages through the generic production path
  `prune_vacant_glyph_page_slots` (revision-gated; stand-in resources in
  tests, real textures in production), and diagnostics report live pages
  (`live_page_count`) while `page_count` keeps slot capacity and
  `glyph_atlas_pages` stays cumulative allocations; `memory()` counts
  resident pages only. Frame pins still guard the active frame, and
  pruning touches only vacant pages, so submitted work rests on the
  wgpu lifetime contract as before. Evidence: three new tests in
  `shared_glyph_pages.rs` (10 total) populate several pages, tighten to
  two and zero, and cover protection deferral/release plus re-expansion,
  asserting live/slot counts, placements, registry metadata via the
  production `SharedGpuResourceRegistry`, shared-slot pruning,
  binding reclamation with identity continuity, and exact generation
  succession. Font-object budgeting stays explicitly pending. Same
  validation as above.
- W2 glyph-budget submission closeout: builds on `5a72eb9` vacant-slot
  residency. `WgpuRenderer::set_glyph_page_budget` applies the device policy
  and drains retired identities/shared textures immediately; post-submit
  protection release enforces pending excess without another text lookup.
  Failed glyph resolves also drain retirement. Zero-budget construction
  starts with no resident page and zero budget rejects new resolves.
  Production retirement is shared with ownership tests using Arc payloads;
  active clones survive shared/local release and old generations stay invalid
  after re-expansion. GPU upload/draw behavior remains unverified here.
  Validation: `cargo fmt --all -- --check`,
  `cargo check --workspace`, `cargo test-constrained`,
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
  `cargo test -p incular-wgpu -p incular-desktop --all-features`, and
  `RUSTDOCFLAGS="-D warnings" cargo doc -p incular-wgpu -p incular-desktop
  --all-features --no-deps` all passed (12 glyph-page regressions).
- W2 parsed-font retention slice: `GlyphAtlas.fonts` is now a bounded
  least-recently-used map (`ParsedFont { font, last_use }`, default
  `DEFAULT_MAX_PARSED_FONTS = 8` entries) keyed by the existing stable
  `FontId`, owned by the shared rasterization layer — no per-window font
  caches, no application-asset eviction (source bytes stay app-owned
  `Arc`s the atlas only borrows during a parse). Guarantee: every resolve
  (hit or miss, across sharing renderer clients) refreshes the requested
  font's recency, but atlas hits never parse merely to serve a cached
  glyph; eviction drops parsed objects only — placements, pages, handles,
  registry identities, and submitted work are untouched, and the next miss
  re-parses from retained source bytes with existing error/size behavior.
  Zero limit parses transiently per request instead of skipping text.
  Limitation (documented at the constant): the bound counts entries, not
  bytes — fontdue exposes no reliable parsed-object memory measure, and
  source-file length is not used as a proxy. Eviction metadata is the
  per-entry stamp plus `font_parser_evictions`; recency stamps come from a
  strictly increasing tick so victim selection is deterministic. Evidence:
  `crates/incular-wgpu/tests/parsed_font_retention.rs` (6 tests on
  production `lookup_or_rasterize` with minted identities over real shaped
  bytes, asserting parse/hit/eviction counters and `font_count()`):
  reuse, font/face non-aliasing (including an out-of-range face that
  resolves to `None` without hitting another font's entry), cross-client
  recency ordering, churn + tightening enforcement, hit survival across
  font eviction with re-parse of new glyphs, zero-limit rendering, and
  registry/handle stability. Same validation as above.
- W2 shared pipeline retention: inspection proves the map bounded, so no
  eviction machinery was added. `SharedGpuContextInner::pipelines` admits
  one entry per surface format through the single renderer-init
  registration site, each entry building the closed `pipeline_contracts()`
  registry (11 fixed classes + 11 Porter-Duff blends + 4 clip-mask
  directions = 26 contracts) exactly once; acquisition clones shared
  reuse (`pipeline_resources`) instead of rebuilding, racing duplicates
  drop unretained, renderers hold internally-refcounted wgpu handles so
  dropping shared ownership cannot invalidate active renderers or
  submitted work, and disposal follows context/renderer drop with no
  removal path to exercise. The bound and its evidence are documented on
  the map field and both accessors; pipeline creation/binding itself
  requires a GPU device and stays recorded as unverified native behavior
  (no standalone cache simulation was substituted). Companion cleanup in
  the same commit: the `GlyphAtlas` declaration doc was restored to the
  atlas (it had attached to `ParsedFont`), and the font-recency claim now
  states tick saturation explicitly, with exhaustion handling tracked
  under the existing identity/counter work rather than claimed unique
  indefinitely.
- W2 frame presentation outcomes: `render()` now returns a typed
  `FrameOutcome::Presented(RenderStats) | Skipped(FrameSkipReason)` instead
  of `Ok(RenderStats::default())` for non-presenting paths, so hosts never
  infer success from counters or default statistics. Five skip reasons
  (unconfigured surface, acquisition timeout, occlusion, inline
  reconfiguration, inline recreation) drive host retry scheduling:
  recovered surfaces and timeouts re-request, while unconfigured/occluded
  surfaces wait for resize/unocclude (plus an explicit repaint on
  `Occluded(false)`) instead of spinning full-frame work. Pacing is now
  bounded and host-owned (`PresentationRetry`, one per normal/transient
  window, no runtime/renderer retry state): the first retryable skip
  retries immediately, then backoff runs 16/32/64/128ms capped at 250ms;
  success and resize/unocclude recovery reset to idle, unconfigured and
  occluded outcomes park dormant without deadlines, and new demand waits
  for the armed deadline instead of bypassing it. Scheduling states are
  explicit and mutually exclusive (idle, owed-by-deadline, dispatched,
  dormant): one authoritative `poll(now, runnable)` produces both the
  redraw dispatch and the wake-deadline contribution, so hidden or
  minimized windows contribute neither a redraw nor an expired wakeup
  while preserving the owed retry silently. Requesting a redraw consumes
  the deadline into a queued-attempt state with no deadline of its own,
  so delayed delivery never re-dispatches or re-wakes; if the window
  stops being runnable first, the retry falls back to owed-immediately
  and restores promptly. The event loop
  dispatches due retries, gates all demand/redraw paths on the same
  component, and waits on the earliest contributed deadline
  (`ControlFlow::WaitUntil`, indefinite `Wait` when none); closing a
  window drops its policy and cancels its retry. Acquisition
  routes backend results through one neutral classifier
  (`SurfaceAcquisitionStatus::of`/`disposition`, wgpu 30 semantics:
  suboptimal presents before reconfiguring, outdated/lost recover inline
  with exactly one attempt per call, timeout/occlusion skip, validation
  failures travel the new `RendererError::SurfaceValidation` error channel
  rather than becoming empty successes). Skips keep simulator waiters
  eligible and record no presentation; device/out-of-memory failures keep
  the existing actionable host error paths. Renderer-neutral outcome
  values stay separate from wgpu error types; the runtime is untouched.
  Evidence: `incular-wgpu/tests/frame_outcomes.rs` (injected backend
  results through the real classifier, retry mapping, outcome accessors),
  `incular-desktop/tests/presentation_retry.rs` (7 tests driving the
  production `poll` operation with a fake clock, asserting redraw
  dispatches and wake deadlines together: hidden windows with expired
  deadlines contribute neither, suppressed-then-restored dispatches fire
  once promptly, delayed delivery never re-dispatches, close cancels
  waiting and queued retries, visible backoff progresses and resets on
  success, dormant parking recovers promptly, and normal/transient
  windows share the scheduling contract), and a runtime presented/skipped
  accounting contract. Actual GPU presentation stays
  recorded as unverified. Same validation as above.
- Terminal render errors settle the retry lifecycle explicitly:
  `PresentationRetry::note_failed` clears obsolete retry debt and
  backoff back to idle (not dormant — dormancy would park the window
  until resize/unocclusion, blocking corrected content from rendering
  on plain demand, while idle stays demand-eligible with nothing
  auto-dispatched). It is wired into the normal-window generic error
  branch only; the out-of-memory "renderer stopped" branch keeps its
  existing semantics, and transient error arms already end the policy
  lifetime by destroying the transient host, so neither needs it.
  Evidence: an 8th `presentation_retry` test drives the full sequence
  (retryable skip, dispatch, terminal error, repeated maintenance,
  hide/restore, new demand) asserting no stale automatic retry
  survives while demand stays eligible, alongside the runtime waiter
  test asserting failed frames settle pending waiters with typed
  errors and no presentation success.
- W2 two-window GPU churn (live, opt-in): `incular-desktop/tests/
  two_window_resource_churn.rs` (harness=false, runs only when
  `INCULAR_DESKTOP_LIVE_TESTS` is exactly `1`, fails on any
  init/presentation error, 480s watchdog exits nonzero while the native
  loop runs plus the pre-existing 180s completion bound) drives two real
  windows sharing one GPU context through production acquisition,
  retirement, host maintenance, and presentation paths. Window A renders
  fixed shared content (one image handle, one gradient built once and
  cloned into both windows, one text run) then idles; window B starts
  with the same shared handles, then churns without touching them.
  Observability is a narrow `GpuResourceSummary` DTO (plain integers in
  `incular-runtime`, no WGPU types) served per window from the existing
  shared diagnostics plus new local-binding counters, fulfilled on the
  host maintenance path through a simulation query that never presents.
  Verified live on Windows (AMD Radeon 610M, wgpu 30 default backend
  selection), exit 0, per family: exactly 1 shared image and 1 shared
  gradient upload serve both windows with 10 shared glyph rasterizations
  plus A's own oversize glyph (reuse, not per-window duplication; A
  holds two page bindings); 24 churn epochs admit 289
  distinct images/gradients for exactly 33 evictions each with caches at
  the 256-entry budget (deterministic LRU arithmetic, so A's untouched
  entries are necessarily gone); 12 B oversize glyphs plus A's own
  oversize glyph (scale-adaptive ~900px physical, inside raster limits,
  above the page threshold; A's 'A' lives on a fresh page nobody else
  refreshes, established by the fresh-page-per-oversize policy plus the
  two-binding local count, not page-order assumptions) make 13
  placements for exactly 5 retirements with live pages capped at 8 —
  A's page retires first as least recently used. Idle A shows local
  bindings (0, 0, 1): stale image/gradient bindings reclaimed by host
  maintenance while A rendered nothing further (the test issues no A
  frame or render-requesting call between the reads; a presented-frames
  delta is not asserted, so this is construction-plus-contract evidence,
  not a counted non-presentation proof). The surviving glyph binding is
  the shared page 0, which B's button and small-text hits kept hot —
  that half of the reading proves preservation of a valid binding, held
  as a separate assertion. The dropped half proves stale reclamation of
  A's evicted oversize page. A resumes pixel-identical with exactly one
  fresh rasterization (the evicted oversize glyph under a new
  generation; the shared Alpha glyphs are cache hits, proving the
  retained page-0 binding is the live one) and local bindings (1, 1, 2);
  a stale page identity resolving to reused contents would corrupt the
  screenshot or leave the binding missing, so both are covered. A's
  close completes observably (polled to WindowNotFound/Closed) before
  B's continued rendering is checked; then B closes and the loop exits.
  Pixel assertions (alignment-free region scans, opaque colors, stable
  interior samples) sit alongside — never instead of — the ownership
  reads. No physical GPU reclamation is claimed from handle counts.
  Protected tightening runs backend-covered with real GPU page textures
  (`protected_tightening_defers_through_release_with_real_page_textures`:
  headless device, real 1024px `R8Unorm` textures through the production
  ensure/retire/prune/reclaim functions — tighten defers protected pages,
  submission release retires the pending excess, registry and shared
  slots drain coherently). Same-slot reuse is asserted on the retired
  page's own identity (page, then generation, then
  `page_generation` — never generations across indices): re-expansion
  refills the exact retired slot under a bumped generation while the old
  key stays absent from the registry and the replacement key resolves
  idempotently; an old-generation local binding cannot survive
  reclamation. Submitted work is proven, not assumed: a known pattern is
  uploaded to the protected page, copied to a readback buffer, and
  submitted before release; retirement and reclamation then drop every
  test-side clone, and the copy still completes with correct bytes
  (cache-ownership release only — not physical reclamation, not the
  renderer's submission orchestration). The test requires
  `INCULAR_WGPU_REQUIRE_GPU=1` for hardware validation (fails loudly
  without an adapter, reporting adapter and backend) and otherwise keeps
  the ordinary headless convention. It cannot run through actual GPU
  submission: frame pins exist only mid-frame and no test-only injection
  was added to hold them, so that final composition step stays cited
  from the submission boundary that runs it live every frame. One
  `incular-image`
  dev-dependency was added for raw test handles (dev-deps are excluded
  from the reviewed boundary). Earlier-review corrections folded in:
  the shared gradient is one cloned identity (rebuilding per window
  never shared it); churn counts are 12+12 per epoch with exact
  eviction arithmetic asserted; the worker closes both windows
  unconditionally via panic-caught teardown (a failing worker previously
  stranded open windows and hung the loop).
- W2 query lifecycle completion: pending GPU-resource queries now settle
  through the established close/shutdown cleanup (`fail_simulation_window`
  fails them with `WindowClosed`, no maintenance pass required), so no
  waiter outlives its window or a shutdown. A live runtime window without
  a native renderer yet keeps its waiter for a later turn (native
  creation in flight); terminal failure removes the runtime window, and
  the drain fails those waiters promptly — nothing is retained
  indefinitely. Late completion after closure is a no-op that cannot
  report success or touch a replacement window (generational ids).
  `take_gpu_resource_queries` documents that it returns live pending
  windows rather than draining their waiters, and the desktop fulfill
  path documents the pending-vs-terminal distinction it implements.
  Evidence: five deterministic runtime simulation tests asserting actual
  reply results — completion without a frame request, close and shutdown
  settlement, late-completion silence with replacement isolation, and
  pending retention across repeated takes.
- Remaining W2 work: none open beyond one explicitly documented
  boundary — W2 acceptance is the live run above plus the cited
  headless/backend suites. The residual boundary: protected tightening
  invoked through a literal frame submission is not directly covered,
  because frame pins exist only mid-submission and holding them from
  outside would require an inappropriate architectural change; the
  backend test above sequences the identical production calls in the
  identical order with real textures, and the submission boundary
  itself runs live on every presented frame. Shared source-font-byte
  budgeting stays separate and explicitly tracked (unbounded
  `font_handles` map noted above — app-owned `Arc` retention, not cache
  ownership; layouts already bounded by count).

Exit: memory stabilizes under churn within the documented budget plus live/in-flight
allowance; counters report actual shared residency; failure reasons reach the host.

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

## W3 — Current status (reconciled against public exports; W3 complete)

This table is the current status. The historical slice notes below are
evidence, not a duplicate status; where they disagree with this table,
the table wins. The per-slice "still open" markers predate this
reconciliation and refer to their slice's review state, not to remaining
W3 scope: every family row below is inventoried and complete.

| family | exported options inventoried | behavioral gaps | evidence | status |
| --- | --- | --- | --- | --- |
| Visibility / Offstage | 9/9 | none known | `specs/visibility_properties.json`, `tests/visibility_ledger.rs` | complete |
| Transform / Rotation / Scale / Translate / FractionalTranslation | all exported | none known | `specs/transform_properties.json`, `tests/transform_ledger.rs` | complete |
| Clip / Opacity | all exported | none known | `specs/clip_opacity_properties.json`, `tests/clip_opacity_ledger.rs` | complete |
| Linked layers | all exported | none known | `specs/linked_layers_properties.json`, `tests/linked_layers_ledger.rs` | complete |
| ShaderMask / BackdropFilter | all exported | GPU pixel verification reported separately | `specs/shader_backdrop_properties.json`, `tests/shader_backdrop_ledger.rs` | complete |
| Pointer blocking (IgnorePointer/AbsorbPointer) | 4/4 | none known | `specs/pointer_blocking_properties.json`, `tests/pointer_blocking_ledger.rs` | complete |
| Focus wrappers (Focus/FocusScope/KeyboardListener) | 21/21 | none known | `specs/focus_wrappers_properties.json`, `tests/focus_wrappers_ledger.rs` | complete |
| Action wrappers (CallbackShortcuts/ActionListener/FocusableActionDetector) | 17/17 | none known | `specs/action_wrappers_properties.json`, `tests/action_wrappers_ledger.rs` | complete |
| EditableText | 22/22 | none known | `specs/editable_text_properties.json`, `tests/editable_text_ledger.rs` | complete |
| Stack / Positioned / IndexedStack | 18/18 | none known | `specs/stack_layout_properties.json`, `tests/stack_layout_ledger.rs` | complete |
| Row / Column / Flex / Flexible / Expanded / Spacer | 36/36 | none known | `specs/flex_layout_properties.json`, `tests/flex_layout_ledger.rs` | complete |
| Padding / Align / Center | 12/12 | none known | `specs/padding_alignment_properties.json`, `tests/padding_alignment_ledger.rs` | complete |
| Sizing / constraints (11 types) | 35/35 | none known | `specs/sizing_constraints_properties.json`, `tests/sizing_constraints_ledger.rs` | complete |
| Baseline / AspectRatio / Fractional / Fitted | 10/10 | none known | `specs/baseline_aspect_fractional_properties.json`, `tests/baseline_aspect_fractional_ledger.rs` | complete |
| Scrolling — viewport configuration and ordinary lists | 26/26 | controller multi-attachment unenforced (W5 gap) | `specs/scroll_viewport_lists_properties.json`, `tests/scroll_viewport_lists_ledger.rs` | complete |
| Scrolling — grids/single-child/animated lists | 43/43 | controller multi-attachment unenforced (W5 gap) | `specs/scroll_grid_single_properties.json`, `tests/scroll_grid_single_ledger.rs` | complete |
| Scrolling — pages/sliver animated+reorderable | 32/32 | controller multi-attachment unenforced (W5 gap) | `specs/scroll_pages_reorder_properties.json`, `tests/scroll_pages_reorder_ledger.rs` | complete |
| Scrolling — scrollbar/2D engine | 20/20 | controller multi-attachment unenforced (W5 gap) | `specs/scroll_scrollbar_2d_properties.json`, `tests/scroll_scrollbar_2d_ledger.rs` | complete |
| Scrolling — wheel fixed-extent engine | 22/22 | no looping delegate by design (bounds clamp); retained scroll actions absent (callback-only selection, locked as contract); controller multi-attachment unenforced (W5 gap) | `specs/wheel_scrolling_properties.json`, `tests/wheel_scrolling_ledger.rs` | complete |
| Scrolling — draggable sheet | 15/15 | snap animations are caller-driven (dropping supersedes); controller multi-attachment unenforced (W5 gap) | `specs/draggable_sheet_properties.json`, `tests/draggable_sheet_ledger.rs` | complete |
| Scrolling — physics policy | 11/11 | snap velocity rule corrected in docs (fling strength above 120 rounds directionally; exact multiples stay); controller multi-attachment unenforced (W5 gap) | `specs/scroll_physics_properties.json`, `tests/scroll_physics_ledger.rs` | complete |
| Collections (Wrap/Table) | 13/13 | Table cell alignment has no widget option by design | `specs/collections_properties.json`, `tests/collections_ledger.rs` | complete |
| Images (Image/RawImage/ImageIcon) | 19/19 | none known | `specs/image_properties.json`, `tests/image_ledger.rs` | complete |
| Overlays (OverlayPortal/tooltips/transients) | 51/51 | follower custom-content semantics noted below | `specs/overlay_tooltip_properties.json`, `tests/overlay_tooltip_ledger.rs` | complete |
| Navigation scopes (dispatch/scopes/storage/restoration/barrier) | 51/51 | none known (barrier semantic ownership resolved: label survives dismissal flags, Activate needs both flags, modal blocking unconditional) | `specs/navigation_scopes_properties.json`, `tests/navigation_scopes_ledger.rs` | complete |
| Platform wrappers (menus/chrome) | 26/26 | router/transaction surface stays with W4 | `specs/platform_wrappers_properties.json`, `tests/platform_wrappers_ledger.rs` | complete |
| Utilities (SafeArea/SplitView/OverflowBar) | 32/32 | none known | `specs/utility_properties.json`, `tests/utility_ledger.rs` | complete |

## W3 — Stack API honesty and positioning policy (same workstream, still open)

- Removed `Stack.text_direction`: alignment factors are absolute, so
  the option never reached layout and was an accepted no-op. Removal
  follows the compatibility policy with a migration note in
  `docs/API_MIGRATIONS.md`; no production caller used it.
- Reproduced four real `Positioned` defects: right/bottom insets were
  wrong by the difference between the algorithm size and the
  measured render size (right+width gave a 40px inset for 20);
  left+right and top+bottom left the child at its measured size, so
  the derived size never applied; and negative offsets were dropped
  instead of honored. Now a single shared `positioned_axis_size`
  policy tightens configured axes so measurement and anchoring agree;
  opposing edges win over explicit size; negative offsets are kept
  while negative dimensions drop as unset.
- Evidence: corrected and extended
  `crates/incular-widgets/tests/stack_layout.rs` (per-axis
  combinations with exact insets, edge-beats-size, meaningful
  negative offsets with invalid-dimension drop, alignment rebuild),
  the stack ledger, and `docs/API_MIGRATIONS.md`.

## W3 — Wrap and Table audit (same workstream, still open)

- Geometry is correct: Wrap flows into bounded runs with item and
  run spacing, main/run/cross alignment, RTL and vertical reversal,
  and one intrinsic run when the main axis is unbounded; Table uses
  max-content columns and row heights, floors `columns` at one,
  fills ragged tails left to right, and reconfigures on a column
  change. No production defect found in the layout math.
- Fixed one stored-but-unread field: `incular_layout::Table.alignment`
  was set to `TOP_LEFT` by its only consumer and read by nothing, so
  it is removed following the compatibility policy (migration note).
  Cells render top-left; per-cell alignment is the app's job.
- Evidence: 17 tests in
  `crates/incular-widgets/tests/wrap_table_layout.rs` (spacing,
  run spacing, unequal children, main/run/cross alignment,
  RTL/vertical reversal, unbounded main axis, keyed reorder,
  builder parity, max-content geometry, ragged tails, column
  reconfiguration, column floor with cell replacement, empty
  table, hit and semantic geometry) plus
  `specs/collections_properties.json` validated by
  `tests/collections_ledger.rs`.

## W3 — Image layout and retained updates (same workstream, still open)

- Reproduced a real defect: `Image::width`/`height` stored raw, so a
  negative dimension panicked at layout in core `Size::new`, while
  the sibling `RawImage`/`ImageIcon` setters floored. The descriptor
  now floors at its owner, matching the family.
- Layout sizes from the intrinsic ratio; fit resolution (Fill,
  Contain, Cover, FitWidth, FitHeight, None, ScaleDown) and alignment
  live in `image_fit_rects` at paint, with alignment a paint-only
  change after a cached paint; repeat tiles within bounds; a handle
  swap re-measures; identical reapplication bails out. Decoding and
  CPU caches stay in incular-image; GPU resources in incular-wgpu.
- Unresolved (recorded, not claimed): `RawImage.scale`,
  `RawImage.color`, and `ImageIcon.color` are stored but never read
  by their conversions, so no tint or intrinsic scaling occurs.
- Evidence: 10 tests in
  `crates/incular-widgets/tests/image_layout.rs` (intrinsic/ratio
  sizing, negative flooring with fail-first, all fit modes with
  independent source/destination rects, alignment after cached
  paint, replacement, identical reapplication, repeat tiling,
  transform geometry with hit testing, filter-quality sampling,
  RawImage lowering) plus `specs/image_properties.json` validated
  by `tests/image_ledger.rs`.

## W3 — IndexedStack fit and clipping (same workstream, still open)

- `IndexedStack.fit` and `clip_behavior` previously survived
  conversion only as dead descriptor fields. Both are now carried
  through `WidgetKind`, lowering, `RenderKind`, and comparison, and
  resolved at their existing owners: fit applies the shared Stack
  policy to every measured child (inactive children keep state and
  identity), clip routes through the existing compositor clip
  attachment with no picture work on toggle.
- Reproduced the gaps fail-first: the fit test failed against the old
  always-loosen arm, and dropping the `RenderKind` fields fails to
  compile. Retained identity is stable across index and fit changes;
  only the selected child paints, hits, and exposes semantics.
- Evidence: 5 new tests in
  `crates/incular-widgets/tests/stack_layout.rs` (fit policies,
  mounted fit change, clip toggle after cached paint, overflow
  hit/semantic geometry, plus the existing indexed behavior tests)
  and the stack ledger records both options implemented.

## W3 — Image scale and tint (same workstream, still open)

- `Image.scale` is defined as source pixels to intrinsic logical size
  (`logical = decoded / scale`), resolved once at the descriptor with
  explicit width/height precedence and invalid-scale fallback to one;
  it threads through `WidgetKind`/`RenderKind` and is layout-only, so
  the decoded handle is never duplicated and identical reapplication
  still bails out.
- Tint is a constant-color recolor (`R' = tint.r`, `G' = tint.g`,
  `B' = tint.b`, `A' = source.a * tint.a` in straight RGBA),
  expressed through the RGB bias column and alpha row of a 4x5 matrix
  whose convention the WGSL shader and `ColorFilter::apply` share
  (verified row-by-row). `RawImage.color` tints the decoded source;
  `ImageIcon.color` is the flat icon recolor. Both reuse the retained
  color-matrix layer (no ShaderMask shortcut); the decoded source
  identity is unchanged on tint changes. `ColorFilter::modulate`
  remains as an independent primitive for white-mask multiplication
  only. The earlier "matrix cannot flatten" limitation is withdrawn:
  it confused coefficients with biases.
- Evidence: neutral CPU equations in
  `crates/incular-rendering/tests/rendering.rs` (white/black/colored
  flatten, transparent and partial-alpha gating), filter-identity
  assertions in `crates/incular-widgets/tests/image_layout.rs`
  (black/green/transparent/partial sources share one tint filter),
  and live hardware pixels in
  `crates/incular-desktop/tests/image_tint_execution.rs` (opaque
  white/black/green flatten exactly, transparent shows the
  background, both partial-alpha cases composite to computed values).

## W3 — Intrinsic steps and constraint-transform clipping (same workstream, still open)

- `IntrinsicWidth.step_width`/`step_height` now round the measured
  child extent up to a multiple at the measurement owner
  (`round_intrinsic_step`, `ceil`), so a stepped box never shrinks
  below the child's intrinsic size. Zero, negative, and non-finite
  steps are no-ops; the step fields stay off the public builder via
  `setter(skip)` so no new API is exposed. The quotient runs in
  `f64` so tiny steps and large extents stay exact; unrepresentable
  results saturate to `f32::MAX` (following `safe_add`), and bounded
  parents clamp the stepped size through the normal constrain path.
- `ConstraintsTransformBox.clip_behavior` now routes through the
  existing `ClipRect` compositor attachment; `Clip::None` adds no
  layer, so there is no duplicated paint-time clip.
- Closed the FittedBox ledger gap: all seven fit modes (Fill,
  Contain, Cover, FitWidth, FitHeight, None, ScaleDown) are asserted
  with independently derived placement rects.
- Evidence: 4 new tests in
  `crates/incular-widgets/tests/layout_padding_sizing.rs` (step
  rounding, invalid-step no-op, builder parity, transform-box clip,
  all fit modes); the sizing and baseline ledgers record the
  resolved options.

## W3 — PreferredSize reachability (same workstream, still open)

- `PreferredSize` was `pub` only inside the private `layout` module,
  so the parity manifest's `incular::prelude::PreferredSize` claim
  was unreachable. It is now exported from the widgets facade and the
  `incular` prelude with a compile-level usage test that also checks
  the lowered tight-constraints behavior and `preferred_size()`; the
  sizing ledger records both options implemented. Flutter's
  app-bar metadata protocol is explicitly not claimed.
- Current W3 status after packages A–C: complete families are
  Visibility, Transform, Clip/Opacity, Linked layers, ShaderMask/
  BackdropFilter (GPU pixel evidence separate), pointer blocking,
  focus wrappers, action wrappers, EditableText, Stack/Positioned/
  IndexedStack, flex, padding/alignment, sizing/constraints, baseline/
  aspect/fractional, Wrap/Table, images, utilities, scrolling
  viewports/lists, scrolling grids/single/animated, overlays, and
  scrolling pages/reorder, and scrolling scrollbar/2D. Still pending
  at the time: wheel/draggable-sheet/physics internals (since
  inventoried; see the status table) and the router/transaction
  surface (W4).

## W3 — IndexedStack inactive-child lifecycle (same workstream, still open)

- Established contract, no production change: retention without
  eligibility. An inactive child keeps its laid-out subtree, retained
  identities, controller state, and node flags; focus membership,
  keyboard resolution, pointer delivery, and semantic exposure follow
  only the selected child. Index selection is separate from child
  ownership: keyed reorder permutes identities while delivery follows
  the index, removal degrades to the stack itself, and an out-of-range
  index hides everything while measuring and restoring delivery when
  valid again. Controller subscriptions belong to the retained
  controller, so a geometry listener on a hidden area still fires.
- Evidence: 5 new tests in
  `crates/incular-widgets/tests/stack_layout.rs` (focused child
  going inactive, removal and keyed reorder, out-of-range round trip,
  hidden text-controller edits applying on reselection, hidden
  selection-area subscriptions delivering).

## W3 — Utility-family coverage (same workstream, still open)

- SafeArea inset ownership: `safe_insets` is the currently usable
  margin (the shell reduces covered edges), `view_padding` is the new
  persistent obstruction margin the shell never reduces under
  occlusion, and `view_insets` is the transient occlusion itself
  (keyboard avoidance consumes it directly, e.g. Scaffold). The
  earlier committed behavior was wrong: it maintained the keyboard
  height. A maintained bottom edge now takes
  max(safe, minimum, view padding) from the current snapshot alone —
  no per-SafeArea history, so mounting under occlusion works — and
  occlusion changes no longer dirty SafeArea. Disabled edges keep
  only the minimum; nested SafeAreas accumulate from the same
  snapshot.   `SystemEnvironmentPreferences` carries the same trio with
  reset-to-default semantics; the runtime change mask tracks the new
  signal.
- SafeArea construction paths are unified: one shared
  `SafeAreaPolicy::padding` feeds retained layout, the
  explicit-insets `resolve` (documented narrower contract: maintenance
  needs the persistent signal, so it has no effect there), and the new
  environment-aware `resolve_with_padding`. The runtime
  `BuildContext::safe_area` helper now applies the same policy and
  records the safe margin always plus view padding only when the
  descriptor maintains its bottom edge; occlusion never participates.
  Nesting accumulates by design on every path, so callers scope edges
  explicitly with `sides` instead.
- SplitView: the divider hit strip and the colored visual both
  collapsed to zero on the cross axis, so the divider was grabbable
  only on its exact midline and a colored divider was invisible. Both
  now fill the cross axis: enforcement clamps the open maximum to the
  incoming extent and the inner centered Align expands into it (finite
  explicit sizes stay on the main axis only); unbounded parents shrink
  the divider to its child instead of failing. Siblings keep their
  geometry. Pan on the divider reports the main-axis delta; the
  stateless view never moves itself. Callback replacements apply to
  gestures started after the update while in-flight streams keep the
  recognizer cloned at down-time, and removing the divider mid-drag
  delivers nothing. Fraction shares bind only oversized panes under
  loose flex; out-of-range fractions clamp to endpoint geometry.
- OverflowBar: verified facade mapping with no production change —
  spacing to Wrap spacing, overflow_spacing to run spacing,
  overflow_alignment to cross alignment, with single-row and wrapped
  transitions. No main-axis alignment option by design; Wrap covers
  it (same precedent as Table cell alignment).
- Evidence: 24 tests in
  `crates/incular-widgets/tests/utility_coverage.rs` plus
  `specs/utility_properties.json` / `tests/utility_ledger.rs`
  recording all 32 exported options implemented.

## W3 — Scrolling viewport and list coverage (same workstream, still open)

- Initial coherent group: `Scrollable`, `Viewport`, `CustomScrollView`,
  `ListView`, `SliverList`, and restored `ScrollController` attachment
  (26/26 options). Offset ownership stays with the controller;
  retained layout stays in Widgets.
- Audit removals: `Scrollable::axis_direction` never reached layout
  (the builder receives only the controller), and the
  `ListView`/`CustomScrollView` `clip_behavior` overrides never reached
  paint (the retained viewport attaches its clip unconditionally).
  Viewport pins cache to zero and exposes no reverse by design;
  `CustomScrollView` covers both. `SingleChildScrollView`/`GridView`/
  animated lists keep their clip overrides for their own groups.
- Verified: axis/reverse updates reflow and re-anchor (reverse needs
  overflowing content to observe), padding, shrink-wrap transitions
  without rebuilds, controller replacement with old-handle isolation,
  physics replacement gating input only, cache-window materialization
  and release, resize with retained children and offsets, and
  paint/hit/semantics agreement after scrolling with retained pictures
  replayed (`display_lists_reused` up, zero new item builds).
- Evidence: 20 tests in
  `crates/incular-widgets/tests/scroll_viewport_lists.rs` (reusing the
  existing reverse/cache/restoration sliver regressions) plus
  `specs/scroll_viewport_lists_properties.json` /
  `tests/scroll_viewport_lists_ledger.rs`. W5 gaps recorded: one
  controller across unrelated viewports is unenforced, and variable-
  extent estimates converge only through measurement.

## W3 — Scrolling grids, single-child, and animated lists (same workstream, still open)

- Second group: `SingleChildScrollView`, `GridView` (with
  `SliverGridDelegate`), `AnimatedList`, `AnimatedGrid`, and the
  animated collection controller (43/43 options). Animated scroll
  positions stay internal; only the structure controllers attach
  externally, which replacement/isolation tests verify.
- Audit removals: `SingleChildScrollView`/`GridView`/`AnimatedList`/
  `AnimatedGrid` `clip_behavior` overrides never reached paint (the
  retained viewport attaches its clip unconditionally; the single-child
  override was not even forwarded), following the same trace as the
  first group's removals. `PageView` keeps its override for its own
  group. No examples, aliases, or other specifications referenced the
  removed setters.
- Verified: controller and physics replacement with old-handle
  isolation, axis/reverse updates (reverse needs overflowing content),
  delegate swaps reflowing retained rows/cells, fixed/prototype/
  per-index extents, constructor agreement, shrink-wrap, padding,
  cache-window materialization and release with unmount accounting,
  insert/remove structure updates, and the unconditional viewport clip
  (locked as current behavior for the follow-up `Clip::None` decision).
- Evidence: 22 tests in
  `crates/incular-widgets/tests/scroll_grid_single.rs` plus
  `specs/scroll_grid_single_properties.json` /
  `tests/scroll_grid_single_ledger.rs`.

## W3 — Overlay and tooltip lifecycle (same workstream, still open)

- Group: `OverlayPortal` (11 options), `RawTooltip` (39), and the
  `RawTooltipController` attachment (1). Surfaces live in the existing
  retained transient registry; dismissal is a controlled callback
  contract — the production path reports closures without mutating
  state, and the owner rebuilds hidden.
- Verified: show/hide idempotence without leaks, content replacement
  updating popups in place, anchor moves without child rebuilds, owner
  unmount clearing registry/paint/hit/semantics together, nested
  z-order with parent links, outside dismissal with reasons, popup
  semantic exposure, controller replacement isolation, constructor and
  setter agreement, trigger/enabled/duration behavior, pointer
  dismissal without trigger refire, ignore-pointer passthrough, and
  follower placement (below/above flip, offsets, anchor composition,
  shared links) at paint time.
- Defect fixed: an empty text message (`Some("")`) shadowed the rich
  fallback in `semantic_value`, so rich tooltips never exposed their
  plain text; empty messages now fall through. Clearing an explicit
  semantic override suppresses rather than reverting, locked as the
  contract.
- Defect fixed: follower-wrapped custom tooltip content painted and
  hit-tested while the semantic walk culled it as hidden. The content
  was incorrectly classified: the follower gate reads compositor
  leader publication, which previously happened only inside paint,
  while the frame runs semantics pre-paint. `update_semantics` now
  republishes leader links from current layer state first (new
  `Compositor::publish_leader_links`, the same pass `flatten` runs),
  so paint, hit testing, and semantics agree through the normal frame
  sequence. The gate itself is unchanged — unlinked, culled, and
  singular-chain followers still stay out — and no second visibility
  flag was added. Content semantics are distinct from trigger
  descriptions in the ledger.
- Evidence: 17 tests in
  `crates/incular-widgets/tests/overlay_tooltip_lifecycle.rs` (reusing
  the barrier/anchor/partition/clock regressions) plus
  `specs/overlay_tooltip_properties.json` /
  `tests/overlay_tooltip_ledger.rs`. Native-host popup availability
  stays separate from neutral correctness.

## W3 — Scrolling pages and reorderable/animated slivers (same workstream, still open)

- Fourth group: `PageView`, `SliverAnimatedList`,
  `SliverAnimatedGrid`, and `SliverReorderableList` with their
  structure controllers (32/32 options). Sliver-only animated/
  reorderable slivers fix their viewport to vertical non-reverse;
  axis and reverse come from the hosting viewport, not the sliver.
- Verified: controller replacement with old-handle isolation,
  page/item identity across updates (reorder permutes retained
  elements in place), insert/remove through the exit lifetime with
  release from elements/semantics/hit testing together, reverse/axis
  updates, viewport resize with retained offsets, snap settle versus
  plain clamping, fraction floors, and unmount cleanup. Drag-path
  reorder callbacks stay covered by the existing `reorderable` suite
  and are referenced, not duplicated. No second scroll engine or
  attachment model; the W5 multi-attachment gap stands.
- Evidence: 18 tests in
  `crates/incular-widgets/tests/scroll_pages_reorder.rs` plus
  `specs/scroll_pages_reorder_properties.json` /
  `tests/scroll_pages_reorder_ledger.rs`.

## W3 — Scrolling scrollbar and 2D engine (same workstream, still open)

- Fifth group: the retained overlay scrollbar, the standalone
  `RawScrollbar`, and the two-dimensional engine (20/20 options).
  Both scrollbar implementations share one geometry owner each
  (`scrollbar_geometry` with `offset_for_thumb_top` in
  `incular-scroll`; `RawScrollbar::geometry` for paint and input), so
  no normalized-position formula is duplicated across painting and
  input — verified by driving drags and track input from painted
  geometry.
- Contracts locked: paint-only controller mutations (style, thumb
  visibility) apply on the next viewport repaint — the revision bump
  is for reactive observation, not push invalidation; hover-reveal
  hides the whole overlay until hovered while the viewport keeps hit
  ownership; 2D deltas before layout consume nothing for lack of
  extents; estimate constructors panic unless finite and positive.
- Verified: thumb sizing/position under changing extents, drag and
  track paging through `scrollbar_pointer`, controller replacement
  isolation, all four orientations plus reversed contexts, empty
  extents, viewport resize, diagonal behaviors (free/lock/weighted
  with tie-breaking), independent per-axis clamps, reversed trailing
  anchors, cache styles, measurement feedback, and cached repaint
  with updated drag geometry.
- Evidence: 13 tests in
  `crates/incular-widgets/tests/scroll_scrollbar_2d.rs` (reusing the
  existing raw-scrollbar and 2D regressions) plus
  `specs/scroll_scrollbar_2d_properties.json` /
  `tests/scroll_scrollbar_2d_ledger.rs`.

## W3 — Navigation scopes and platform wrappers (same workstream, still open)

- Scopes group (51/51 options): back dispatch, pop scopes, page
  storage, restoration scopes, and the modal barrier. Router and
  route-transaction types stay untouched with W4; only scope
  property/lifecycle contracts are claimed here.
- Defect fixed: `PopScopeController::new()` documented "may initially
  pop" but derived `Default` gave `can_pop: false`, so fresh scopes
  silently blocked. Fresh scopes now default to may-pop like the
  sibling nested handler and the platform convention; blocking is
  opt-in. Existing tests set the flag explicitly and still pass.
- Verified: scope replacement retargeting dispatch, callback swaps,
  unmount unregistration, fallback replacement and hierarchy order,
  bucket/scope replacement updating descendants, barrier options with
  merged-button semantics, and window-chrome replacement/unmount.
  Legacy `on_pop` callbacks alone never claim ownership — a pop
  handler is required — locked as contract.
- Wrappers group (26/26): menu descriptors, the menu-bar
  controller/binding lifecycle, and window chrome. Mounting publishes
  the binding without native calls; the adapter attaches through
  `connect()`; reconciliation reinstalls; unmount detaches once held
  clones (which keep the lease alive by design) drop. Unbound or
  unsupported operations report explicit `NoOpUnsupported`/build
  errors; duplicate ids fail installs without partial state.
- Resolved follow-up (barrier semantic ownership): three defects
  fixed at the existing lowering owner. The block flag lived only on
  the inner veil, so outer background siblings were never blocked —
  the modal failed its core contract. Withdrawing either dismissal
  flag also dropped the label node (role-less), and a dismissible
  barrier without a label exposed no Activate at all. Now pointer
  interception stays unconditional (opaque tap vs absorb), the veil
  owns exactly one semantic node (Button+Activate iff `dismissible`
  and `barrier_semantics_dismissible`, else neutral label carrier),
  the inner veil block hides the background child, and a new outer
  block on the builder root hides preceding outer siblings. Removing
  dismissal removes only Activate — never the label or the blocking.
- Evidence: 19 tests in
  `crates/incular-widgets/tests/navigation_platform_scopes.rs`
  (reusing the navigation/window-chrome suites) plus
  `specs/navigation_scopes_properties.json` /
  `tests/navigation_scopes_ledger.rs` and
  `specs/platform_wrappers_properties.json` /
  `tests/platform_wrappers_ledger.rs`.

## W3 — Phase costs: leader publication and exit reconciliation (same workstream, still open)

- Measured: `CompositorDiagnostics::{leader_publish_passes,
  leaders_published}` count publication work per pass. The regression
  `leader_publication_passes_and_counts_are_measured_per_frame`
  (`crates/incular-rendering/tests/rendering.rs`) pins the numbers: an
  empty flatten runs 1 pass with 0 publications; a live leader flattens
  as 1 pass with 1 publication; three repeated unchanged flattens add
  exactly 3 passes and 3 publications; the explicit pre-semantics
  republish adds 1 and 1 through the same counter. Publication
  recomputes unconditionally, and a frame running both semantics and
  paint accounts two passes (`update_semantics`, then paint via
  `flatten`).
- Cost shape, inspected not timed: one pass is a clear walk plus a
  collect walk, each visiting every layer node a bounded number of
  times, plus one ancestor-chain fold with follower projection per
  leader (chain-depth work each, cycle-guarded). The counters above
  measure passes and successful publications only; they establish no
  timing claim. The pass is idempotent; nothing is cached between
  passes.
- Decision: recomputation stays; no evidence justifies a cache.
  Publication validity spans layer topology (mount/unmount),
  transforms and clips (composite), visibility, and cross-tree link
  winners (first in paint order, cycle-guarded) — none of which the
  semantics phase owns. Paint also runs standalone (tests and
  embedders call `tree.paint()` without `update_semantics`), so
  `flatten` must always be able to publish itself. Sharing one
  prepared projection across composite, semantics, and paint (two
  crates) would need a validity token covering topology, transforms,
  visibility, and link publications — more synchronized state than the
  repeat walks it would save. The pre-semantics publish stays because
  semantics runs pre-paint while follower gating reads publication
  (the Package `808539b` defect class).
- Public boundary: only `LayerTree::publish_leader_links` changes
  (`&self` to `&mut self`, for the counters; behavior identical), noted
  in `docs/API_MIGRATIONS.md`. New tree/wheel internals stay
  crate-internal (`controller_revision`, `refresh_wheel_ranges`,
  `wheel_scroll_revision`). Barrier, wheel, and draggable fixes change
  no other public signatures; `reset()` now reports genuine change per
  its docs.
- Exit reconciliation against the W3 workstream
  (`complete retained property contracts and reduce change fan-out`):
  - Exported-option coverage: 27 ledger families, 595 records, 0
    `unresolved`, 0 `intentionally_unsupported` record dispositions.
    The ledger validator enforces an exact-set match with source
    discovery in both directions, and every implemented record carries
    named regressions plus resolving references. Physics policy was
    the last uninventoried family and is now covered (11/11).
  - Three separate quantities, not one claim: record dispositions
    count option contracts (all implemented); inventory completeness
    is the status table reconciled against public exports (every row
    complete); implemented backend behavior is tracked separately —
    ShaderMask/BackdropFilter GPU execution and pixel verification
    stay `unresolved` in the shader/backdrop ledger's backend block
    with typed-unsupported outcomes, which is W2/native scope, not
    W3 record debt. Deferred design gaps are recorded as contracts,
    not silence: no looping wheel delegate (bounds clamp), wheel
    selection callback-only (no retained scroll actions), draggable
    snap handles caller-owned (dropping supersedes), bouncing
    positions transient in memory, multi-viewport attachment
    unenforced (W5).
  - Change fan-out reduced, not moved: reset commits then notifies
    exactly once through the existing listener mechanism; wheel
    prepares acknowledge the consumed stamp so quiet layouts rebuild
    nothing (pinned by test); scrollbar geometry, SafeArea policy,
    and extent validation each have one owner; dead clip/axis
    overrides were removed rather than wrapped; this batch adds no
    second registry, engine, or blanket reentrancy suppression, and
    drops the `config.clone()` from the refresh scan.
  - Behavioral evidence: new suites pin the eight packages (barrier
    ownership, 16 wheel tests, 19 draggable-sheet tests, 1
    publication-measurement test, 10 physics-audit tests plus
    retained swap tests); existing suites are referenced, not
    duplicated.
  - Verdict: W3 meets its exit criteria. Every status-table family is
    inventoried with behavioral evidence, and fan-out reductions are
    pinned by regression. Known remaining work belongs to other
    workstreams and is not W3 debt: router/transaction surface (W4),
    attachment policy and transitions (W5), GPU execution and pixel
    verification (W2), per-host verification (W8).
  - Necessary phase scheduling: frames run layout, composite,
    semantics, then paint (`run_frame_at`); wheel windows refresh from
    the controller revision in the layout prologue like slivers; reset
    and actuator reports resolve synchronously.
  - Validation bypasses: the only allows are
    `#![allow(clippy::float_cmp)]` in the new float-asserting
    integration tests, matching the existing convention. No checks are
    skipped: every package ran fmt, workspace check,
    `cargo test-constrained`, strict Clippy, focused suites, ledgers,
    and warning-denied rustdoc.
  - W3/W4 line: W3 keeps the veil-ownership split, scope/storage/
    restoration property contracts, and phase scheduling above. W4
    keeps route transactions and identity (RouteEntry, observers,
    guarded-pop checks), the navigation-crate descriptors
    (`RouteSettings`, `OverlayEntry`, `Dialog`, `BottomSheet`,
    navigation `ModalBarrier`), and runtime owner mapping. No uncovered
    widget property was moved into W4 for being navigation-adjacent.

## W3 — Retained property contracts (Visibility slice; W3 remains open)

- Ledger: `specs/visibility_properties.json` records all nine exported
  Visibility/Offstage options (visible, maintain_state/size/animation/
  semantics, replacement, child, offstage, Offstage child) with default,
  validation, structured conversion/owner/consumer references, prose
  comparison plus its references, named regressions, and disposition.
  References are explicit `{path, kind, owner, name}` shapes (methods,
  structs, enums, variants, free functions, and `trait`/`target`/
  `source` trait implementations); prose never carries a reference.
  `tests/visibility_ledger.rs` resolves every reference by parsing Rust
  syntax with `syn` (already a workspace dev-dependency; no new parser),
  so comments and string literals can never satisfy a reference.
  Mechanically checked: schema version, nonempty records, duplicate
  entries, allowed dispositions, required evidence for implemented
  records, resolution of every structured reference (including
  trait/source-disambiguated `From` conversions), exact `#[test]`
  functions for regressions (helpers and prefix matches fail; nested
  modules resolve), consumer phases against a validator-fixed vocabulary
  (invented phases fail even when also listed in the ledger), and family
  completeness as one discovered set — constructor inputs plus fluent
  setters plus generated builder setters from private fields (skipped
  setters excluded, fluent duplicates merged; renaming `prefix`/`suffix`
  and `strip_bool`/`transform` forms are rejected, per the installed
  typed-builder 0.23.2 semantics) — failing added/removed options in
  both directions. Generated-builder coverage is an explicit per-struct
  `builder_parity` test reference, never a name heuristic. Nine
  fixture groups prove each rejection, including builder-only fields,
  skipped setters, and wrong-trait sources. Human-reviewed and NOT
  mechanical: whether a named test's assertions actually establish its
  property contract (including builder parity), and scope beyond the
  Visibility family.
- Reviewed policy: public configuration resolves in
  `From<Visibility>/From<Offstage>` (hidden with no maintain flag mounts
  the replacement and unmounts the child; any maintain flag retains).
  Retained owner is `WidgetKind::Visibility` with `HiddenVisibility`;
  `maintain_state` is conversion-only with no retained field. One
  comparison drives phases: `RenderKind::Visibility { visible,
  maintain_size }` equality via `update_kind` (visible or size change
  schedules layout/paint/semantics/hit-test; visible toggle additionally
  resyncs child layers); animation/semantics policy is read live from
  `WidgetKind` on every compositor tick and semantics rebuild, so those
  option changes need no phase scheduling. Closed dispatch preserved; no
  new cached booleans, property engine, or tree rewrite.
- Transitions tested in `crates/incular-widgets/tests/visibility.rs`
  (existing combinations, animation opt-in/resume, nesting mute,
  offstage, popup suppression, plus four new): changing
  maintain_animation or maintain_semantics while hidden takes effect on
  the next tick/rebuild with zero layout/paint delta and preserved
  identity; changing maintain_size while hidden resizes through layout
  only (layouts delta, paints unchanged, reversible); reapplying an
  identical hidden configuration schedules no layout/paint/composite
  with `identical_child_bailouts` proving the no-work path. Hit testing
  stays excluded and paint stays empty throughout.
- Finding: no defect. The four new transition tests passed on the first
  behavior run (the only failure was test scaffolding assuming a Padding
  wrapper level); comparison and phase classification are not duplicated,
  so no consolidation was manufactured. Fail-first evidence for the
  validator: it failed on a boolean `default` before the schema check
  accepted bools.

## W3 — Transform slice (same workstream, still open)

- Fixed two demonstrated divergences, both fail-first. `From<
  FractionalTranslation>` passed the fraction straight through as pixels
  (0.5 fraction on a 40x20 child painted at (0.5, 0.5)) and dropped
  `transform_hit_tests`: `WidgetKind::Transform` and
  `RenderKind::Transform` now retain `fraction: Option<Offset>` and
  `transform_hit_tests: bool` on the existing owner (absolute
  constructors record `None`/`true`). One shared resolver,
  `resolve_transform`, takes both size inputs explicitly — fraction
  against the measured child size, ordinary pivot against the bounds
  (node) size — for the compositor tick, hit testing, and semantics
  alike, with the child-size lookup shared as `transform_child_size`;
  no call site selects an ambiguous size, so the paths cannot diverge
  again. Flag-false transforms fall through to the ordinary
  untransformed hit path while painting and semantics follow the visual
  transform, matching the setter contract. Comparison is unchanged
  RenderKind equality (transform changes stay
  compositor/semantics/hit-test-only, never layout); no new formula, no
  duplicated phase logic. Correction follow-up: the first version
  resolved the ordinary pivot against the child size too; the new
  tight-100x80/40x20 regression failed on that code at the first
  node-basis assertion (expected -50, got -20) and passes with the
  explicit two-input resolver.
- `RotatedBox` contract decision, from specs, rustdoc, and history: the
  rustdoc promises rotation only, no layout behavior; the implementation
  has delegated to a layout-neutral rotation since introduction
  (`da509e1`), with no architecture decision or spec entry promising
  dimension swapping. Layout-neutral is therefore the intentional
  supported contract, now stated explicitly in the public rustdoc
  (allocated dimensions never swap, including odd quarter turns).
  Regression coverage unchanged; no implementation change, kept separate
  from the fractional fix. Singular/non-finite policy is documented
  as-is (inverse fails, hits miss, cached paint/semantics persist).
- Geometry contract in
  `crates/incular-widgets/tests/transform_geometry.rs` (10 tests):
  translation/scale/rotation updates with counter deltas proving the
  compositor-only contract (including byte-identical cached pictures
  across a translation update), nonzero origin pivots, fractional
  scaling on non-square children, the hit-test flag both ways,
  quarter-turn asymmetric rotation, identical reapply scheduling
  nothing, the tight-constraints basis split (scale/rotation node-basis
  pivots, explicit-origin control, child-basis fraction — with
  hand-derived coordinates, hits, semantics, identity, and counter
  deltas), and builder parity for both builders. Asymmetric 40x20
  fixtures throughout; fail-first evidence for the fraction, the flag,
  the basis split on the single-size resolver, and two corrected
  hand-arithmetic mistakes of mine (both caught by running, never
  described as production findings).
- Ledger: `specs/transform_properties.json` (13 records: eight
  constructor parameters for `Transform`, three
  `FractionalTranslation` options, two `RotatedBox` options) with
  `builder_parity` for both builders. The validator moved to
  `tests/ledger/` without copying: `FamilySpec`/`StructSpec` parameterize
  source paths and per-struct discovery (`MethodNames` vs
  `ConstructorParams`, both restricted to `pub Self`-returning methods so
  getters are never options). Existing visibility negatives preserved
  (9/9); transform adds constructor-parameter coverage plus a
  getter-exclusion fixture (2/2).
- Remaining W3 families: editable text; effects; scrolling;
  interactive controls; then layout, collections, images, navigation
  scopes, overlays, platform wrappers, utilities.

## W3 — Linked-layer slice (same workstream, still open)

- Shared the leader data, not yet the formula. `CompositedTransformFollower`
  resolution previously ran a Widgets-local computation that agreed
  with the compositor on translation/scale but returned the
  stale-leader translation for a removed leader and the layout
  placement for an unlinked follower whose `show_when_unlinked` is
  false. That commit made `follower_resolved_transform` ask the
  `LayerLink` (same shared owner as the compositor), return `None`
  for unresolved links, and fall back to the pre-existing unlinked
  behavior. Correction to the earlier report: no renderer-neutral
  geometry was shared then — the anchor/offset math stayed duplicated
  between the compositor and the widget tree — and no identity gate
  was added (`LayerLink` identity was and remains `Rc::ptr_eq` in its
  own `PartialEq`). The shared formula and the ownership fix below
  close those gaps. No second registry, no per-frame rebuilds.
- Proved fail-first where the behavior allowed it: a temporary
  staged-vs-flattened probe (kept out of the final tree) confirmed the
  follower translation was computed from public attachment points;
  new retained regressions cover translation, leader moves that are
  compositor-only (counter deltas), anchor resolution in leader space
  under scale, offset plus both anchors, layer-anchor setter parity,
  transformed ancestors, unlink/relink across links and leaders, both
  `show_when_unlinked` policies, two followers on one link, link
  replacement, identical reapply scheduling, multiple-leader
  first-wins, and singular culling of paint, hits, and semantics (15
  tests in `crates/incular-widgets/tests/linked_layers.rs`, unequal
  60x30 / 20x10 fixtures). Corrections to the earlier report: the
  follower-before-leader test pinned paint culling only — hit testing
  and semantics read post-flatten link state and disagreed with the
  culled paint — and compositor `remove` released the link on *any*
  leader's removal, including leaders that never published. Both are
  demonstrated by the ownership regressions below failing on that
  code. The unlinked-hidden pair and the relink test forced two real
  fixes that stand: hit testing now gates on follower visibility in
  both hit entry points (previously a hidden follower stayed
  hittable), and semantics skips unlinked-hidden subtrees without
  clearing sibling state (previously it dropped the whole parent's
  children).
- Ledger: `specs/linked_layers_properties.json` (10 records: two
  `CompositedTransformTarget` options, eight
  `CompositedTransformFollower` options) validated by
  `tests/linked_layers_ledger.rs` through the reused
  `tests/ledger/` validator (`MethodNames` discovery, observer
  getters excluded) plus a method-name fixture negative (2/2).

## W3 — Linked-layer ownership (same workstream, still open)

- Publications now have an owner. Each leader layer mints a
  process-wide token at creation (`create_leader`; a tree-local arena
  index cannot name the publisher because one link may be shared
  across trees whose indices overlap) stored on
  `LayerKind::Leader` and recorded in `LeaderData` by
  `LayerLink::publish` (first in paint order still wins).
  `remove` and `update_leader` release via
  `clear_if_owned_by`: removing or rebinding a non-owner leaves the
  winner untouched, while removing the winner (or rebinding it away)
  invalidates at once and the next flatten's resolution pass elects a
  survivor. The per-frame clear-then-publish framing is unchanged.
- Paint order no longer strands followers. `flatten` runs a dedicated
  leader-publication pass after the reset and before follower
  resolution, so a follower earlier in child order links exactly like
  one after the leader; paint, hit testing, and semantics all read
  the same post-publication state by construction.
- One shared formula half. `resolve_follower_target`
  (incular-rendering, re-exported at the crate root) computes the
  leader-space anchor point both the compositor's
  `follower_transform` and the widget tree's
  `follower_resolved_transform` invert into their own retained frame
  (layer world vs. render parent); the inversion still differs
  because the frames differ, and is documented as such.
- Regressions, all fail-first on the previous code (6 compositor, 4
  widget plus a rewritten ordering test, 19 widget tests total):
  removing the non-winner preserves the winner with no new flatten;
  removing the winner invalidates immediately and the survivor
  publishes next flatten; rebinding the loser preserves the winner;
  rebinding the winner releases the old link at once; distinct links
  with identical geometry resolve independently; follower-before-
  leader agrees across paint, hit testing, and semantics. The old
  paint-only ordering test is replaced by the agreement test; the
  ledger is unchanged (no public options added).

## W3 — Clip/opacity slice (same workstream, still open)

- Found by tracing every option to raster output: `clip_behavior`
  was retained but never read at paint, so `Clip::None` still
  clipped and radius/oval/path geometry collapsed to a plain rect.
  Worse, the single emission site wrote into the discarded transient
  traversal, so widget clips never reached the flattened scene at
  all. `Stack.clip_behavior` was documented ("clipping behavior for
  children overflowing stack bounds") but consumed nowhere.
- Fix at the responsible owners, reusing the scrolling clip
  mechanism: new `LayerKind::ClipRRect/ClipOval/ClipPath` stages
  beside `ClipRect` (local-space shapes, world resolution and
  bounding-box culling at flatten, annotation/bounds/debug
  coverage), a shared `clip_world_bounds` helper, and a `Clip`
  layer attachment for clipping `Clip*`/`Stack` widgets with
  `Clip::None` mapping back to direct rendering. Geometry flows
  through `update_from_kind` (immediate, off the retained size) and
  `update_layout_geometry` (size changes); the dead transient
  emission is removed. No new registries, no per-frame rebuilds.
- Invalidation narrowed at its owner: same-kind clip changes
  (behavior, radius, path) and pure `Stack` behavior flips are
  compositor-only instead of full layout+paint+semantics+hit-test;
  cross-kind changes keep full invalidation so relayout refills
  rebuilt layers. Diagnostics separate the phases in the tests
  (`layouts`/`paints` deltas zero, `opacity_updates` moving).
- Contracts established in rustdoc and pinned by tests rather than
  assumed: `Opacity`/`FadeTransition` alpha (including zero) never
  gates hit testing or semantic exposure — that is `Visibility`'s
  job; clips bound raster output only, hits follow layout bounds
  through the clip node, semantic bounds stay whole.
- Regressions: 16 tests in
  `crates/incular-widgets/tests/clip_opacity.rs` (9 fail on the
  previous code: shaped clips, `None` handling, stack overflow,
  toggle/radius invalidation, nested balance, transformed
  overflow), plus `specs/clip_opacity_properties.json` (15 records
  across the four clip widgets, `Opacity`, and `FadeTransition`)
  validated by `tests/clip_opacity_ledger.rs` through the reused
  validator with both discovery styles and one shared
  builder-parity test for the four `TypedBuilder` clip widgets.
- Known residuals, documented where they bind: shaped-clip hit
  testing stays bounding-box (corners hit though pixels clip);
  an empty clip path culls its subtree; `Stack`'s remaining options
  (alignment, fit, text direction) ledger with the layout family.

## W3 — Transformed clip geometry (same workstream, still open)

- Replaced the world-box emission (with unscaled radii) by exact
  world shapes: translation-only worlds keep every shape analytic;
  uniform scales additionally keep rounded corners analytic with
  scaled radii — resolving the reported unscaled-radius limitation
  instead of recording it. Anything a shape cannot represent
  (rotated/skewed rects and rounded rects, nonuniformly scaled
  corners, rotated ellipses) falls back to an equivalent
  tolerance-flattened path (0.1 logical px), exact for every affine
  map; misclassification can only cost the fallback, never
  correctness. One consolidated emitter serves all four variants;
  bounding boxes stay the culling/annotation input (conservative
  there, never the emitted shape).
- The backend contract needed no change: world-space emission meets
  identity stream position, so existing scissor logic stays a
  superset and stencil instances receive exact geometry. Verified
  against the real backend tessellator (CPU-deterministic, no
  device): fallback meshes match path area within 5% and stay far
  under bbox area for rotated rounded rects and ovals.
- ClipPath resolutions memoize per layer (reset only by
  `update_clip_path` with the values it derives from): static clips
  keep one path identity and one backend mesh across flattens;
  identity worlds reuse the supplied arc without transforming at
  all. Hit-test and semantic policies are untouched and covered
  alongside the new geometry (center hits, whole transformed
  semantic bounds).
- Regressions: compositor shape-vs-box distinctions via
  `Path::contains` (rotated rect/rrect/oval emit paths excluding
  their own bbox corners), scale representation selection
  (uniform doubles radii, nonuniform falls back), path identity
  stability across flattens with single re-resolution on move,
  annotation lookup through a rotated variant, widget integration
  for both scale cases, and two backend tessellation tests. All
  fail on the previous emission; the clip/opacity ledger prose now
  states the policy.

## W3 — Clip reflections and tolerance (same workstream, still open)

- Analytic rounded rectangles now require positive uniform scale:
  mirrors and nonuniform scales fall back to paths instead of
  permuting (or silently misplacing) corner ownership. Four
  distinct corner radii in every reflection regression keep
  per-corner mistakes observable.
- Tolerance is defined, not assumed: kurbo subdivides arcs into
  cubics in shape-local space, so the local tolerance divides the
  0.1 world-space target by the transform magnification, floored
  locally past 100x. No world-space error bound is claimed beyond
  that derivation, and no device-pixel accuracy is claimed from a
  world/logical-space tolerance. Containment holds with margins far
  above tolerance at 0.5x, 1x, and 8x against independently mapped
  geometry, and curve counts prove the tolerance engages where
  kurbo's 4-segment floor allows observing it (full ellipses, not
  quarter-arcs).
- Magnification is the Frobenius norm (`Transform::magnification_bound`
  in core), a conservative operator-norm bound that also covers shear,
  where diagonal directions stretch more than either basis vector
  (skew(1,0): diagonal ~1.581 vs longest column ~1.414). The previous
  largest-column calculation underestimated exactly that case.
  Degenerate zero maps report 0.0; non-finite inputs report infinity,
  consistent with `inverse()` failing on them — downstream geometry is
  culled rather than measured. Core tests pin the bound against
  independently transformed unit vectors plus tightness and
  degenerate/non-finite cases; a shear regression verifies curved
  clip geometry at 1x and 8x. Reflection and rotation/scale coverage
  is retained unchanged.

## W3 — Retained fallback paths (same workstream, still open)

- Memoization now covers every clip variant that emits a path,
  not just direct ClipPath stages: one private `ResolvedClipPath`
  per layer holds at most the current resolution, with geometry
  updates invalidating through their own update paths and removal
  dropping the cache with the layer. No global map, no transform
  history; tolerance derives from the world, so world equality
  keeps the memo valid.
- Identity stability is asserted per variant (rect, rrect, oval
  fallbacks keep one path across unchanged flattens), replacement
  on move and on geometry change produces correct new shapes,
  removal releases ownership down to the flattened output's share
  (strong-counted), and translation still emits analytic commands
  for all three shapes.
- Mesh-cache reuse is traced, not executed: the backend keys its
  CPU path cache by path identity, so stable identities imply
  cache hits by construction — but no headless lowering seam
  exists to observe the counters, exactly like every other
  lowering error and cache in the backend. Path-identity evidence
  is executed; mesh-cache evidence is structural.

## W3 — ShaderMask/BackdropFilter slice (same workstream, still open)

- Audited, no implementation change: the lifecycle proved sound.
  The callback runs at resolve points only — first paint, node
  repaints including child resize (which relayouts and repaints the
  mask node upward through layout propagation), and mask
  configuration changes on the update path — never per frame
  (identical reapply resolves nothing). `mask_size`/`mask_transform`
  snapshot resolve time while stage bounds recompute live every
  flatten; blend/backdrop params refresh on the compositor tick.
  Same-kind changes stay compositor-only on the existing family
  rule; no stale geometry and no excessive invalidation was
  demonstrated, so nothing was narrowed.
- Contracts defined in rustdoc and pinned by renderer-neutral
  command assertions: callbacks receive local bounds (zero origin,
  node size); callback equality is allocation identity, not output
  equality; placement follows the live stack while the recorded
  transform stays a resolve-time snapshot (both halves pinned, so
  the boundary cannot drift); the backdrop is whatever painted
  before the stage in paint order with bounds covering the child
  only; disabled and zero-sigma filters pass children through with
  no stage; an empty mask culls raster but keeps semantics,
  mirroring clips; hits and semantics pass through both stages
  untouched. GPU pixel execution (stencil masking, backdrop
  sampling) belongs to the native backend, which has no in-repo
  coverage to extend — the neutral contracts above are the
  deterministic boundary, and pixel fidelity stays an explicit
  unresolved item for backend work.
- Regressions: 15 tests in
  `crates/incular-widgets/tests/shader_backdrop.rs` (callback
  timing/identity/bounds, resize, child swap, blend-only updates,
  ancestor-move snapshot boundary, clip+transform nesting with
  hits and semantics, empty-masked semantics, backdrop ordering
  and bounds with hits and semantics, disable/zero-sigma
  passthrough with phase counters, constructor aliases, identity
  stability), plus `specs/shader_backdrop_properties.json` (11
  records: callback, mask, and all seven backdrop options)
  validated by `tests/shader_backdrop_ledger.rs` through the
  reused validator with a constructor-parameter fixture negative.
- Residual noted, not enshrined: a blend-only mask change
  re-invokes the callback once on the update path (same output, no
  extra phases). Untouched as harmless; revisit only with a real
  cost case.

## W3 — Effect backend honesty (same workstream, still open)

- Traced the skip: the WGPU lowering arm for both stages recursed
  into children and extended batches, drawing content unaffected.
  Inspected the offscreen infrastructure first (layer-keyed target
  pool and cache, Filtered/Shadow/Blend batches, fixed-function
  Porter-Duff pipelines plus a blend shader). Both effects need
  larger work on top of it, so neither was rushed:
  - ShaderMask needs a masked offscreen composite — the child
    target composited through the brush pattern with the blend
    mode. Missing: a mask-composite shader and pipeline plus
    brush-parameterized composite bind groups and cache keys.
  - BackdropFilter needs backdrop capture — sampling the
    already-painted destination with invalidation keyed to
    backdrop content, which no cache key covers, plus
    pass-splitting to sample the active target.
- Explicit outcome instead of silent passthrough: lowering now
  fails the frame with `RendererError::UnsupportedShaderMask` /
  `UnsupportedBackdropFilter` (frame errors are logged, never
  fatal to the host). Supported passthroughs keep the direct path:
  disabled and zero-sigma backdrop filters (matching the neutral
  contract), and stages under fully-clipped subtrees (matching the
  opacity/blur early-outs). The per-command decision lives in one
  queryable predicate (`unsupported_effect`) consulted by the
  lowering arm, so the two cannot disagree.
- Ledger updated first: a `backend` block per effect separates
  execution (unresolved) from pixel verification (unresolved, no
  harness in-repo), names the missing mechanism, and references
  the outcome test; the validator resolves those references like
  `builder_parity` (ledgers without the block skip it). Option
  records stay implemented: configuration and retained transport
  were already correct. Public rustdocs state the compatibility
  impact: scenes containing an executable stage fail presentation
  with the named error; transport, hit testing, and semantics are
  unaffected.
- Regressions: four CPU-deterministic backend tests over the
  predicate (both errors with messages, both passthroughs,
  ordinary commands), referenced from the ledger. Neutral widget
  tests stand unchanged as complementary evidence. No in-repo
  widget, control, or example uses either stage, so nothing that
  rendered before changes outcome except previously-misrendered
  effect scenes, which now fail loudly.

## W3 — Effect-error integration (same workstream, still open)

- The predicate stays public as a capability query with documented
  limits: one command only, no traversal context (a stage under a
  fully-clipped subtree still reports while lowering skips it),
  no resource limits, conservative direction. It is the lowering
  arm's single decision site, so classification cannot disagree
  with execution; no host pre-scans scenes with it yet.
- Host handling verified at the reachable production boundary: a
  runtime test registers real frame and capture waiters, fails
  them through the exact call the desktop error branch makes, and
  asserts typed errors with the backend message and no lingering
  waiters — presentation success is never reported to them.
  Retry-bypass is structural (errors never construct a
  `FrameOutcome`, and the error branch never touches retry state),
  traced rather than executed: no headless lowering seam exists,
  exactly like every other lowering error and cache in the
  backend, which likewise have zero direct tests.
- GPU execution ran nowhere in-repo: tessellation tests are CPU
  geometry, predicate tests are classification, and the waiter
  test is host handling. Pixel presentation of masks and
  backdrops remains unresolved pending backend execution.
- Clip cache state is private: `resolved` left the public
  `LayerKind` clip variants and `ResolvedClipPath` left the crate
  root export; one `clip_cache` slot per private `Layer` owns the
  single current resolution, reset by the geometry update paths
  and released with the layer. Construction paths were compatible
  (all sites go through `create_clip_*`/`insert`), and all 64
  compositor plus 20 widget linked-layer tests pass unchanged,
  including the memo identity/replacement/release coverage.
- Native execution evidence:
  `crates/incular-desktop/tests/unsupported_effect_execution.rs`
  (opt-in `INCULAR_DESKTOP_LIVE_TESTS=1`, watchdog-bounded) drives
  one window through mask, backdrop, then plain phases: both
  effect attempts fail with the typed `FrameFailed` waiter outcome
  carrying the backend message plus `CaptureUnavailable` with no
  presentation, and the plain phase presents, proving failure is
  scoped per attempt and the loop survives it. Executed live on
  Windows: full run completes in ~15s, exit 0. Failed frames show
  only the uncleared (black) surface, which is the correct
  observable for presented-nothing.

## W3 — Linked-layer traversal (same workstream, still open)

- Reproduced first: a target nested inside a linked follower
  published in its layout frame, so the inner follower landed at
  (2,3) instead of (72,3) while paint, hit testing, and semantics
  followed the (correct) flatten-time resolution. The old publish
  pass propagated ordinary transforms only.
- Fix without a second transform algorithm: the publish pass now
  collects leaders in paint order with a parent map, then ensures
  each leader once with cycle-guarded recursion. Each leader's
  frame folds its ancestor chain with flatten's own rules —
  transforms compose, ancestor followers resolve through the shared
  `follower_transform` (ensuring their links' candidates in paint
  order first), showing-but-unlinked followers keep the parent
  frame, and hidden followers, emptied clips, and singular
  inversions resolve to nothing. The publish set is exactly the
  leader set flatten traverses; the first resolvable leader per
  link wins with runner-up fallback. Immutable walks borrow child
  vectors instead of cloning them.
- Declared policy, all pinned: nested chains resolve through outer
  frames (compositor placement plus widget paint/hit/semantics,
  outer moves, outer unlink releasing the inner link at once);
  dependency cycles terminate — hidden members never publish,
  showing members publish in the parent frame by the same
  unlinked-showing rule flatten applies (self-cycles included);
  cross-tree links distinguish publishers even when arena indices
  overlap. Prior ownership tests (non-winner removal, rebinding,
  winner election) pass unchanged.
- The raw global publisher counter is replaced by an owned
  `PublisherIdentity` token (`Rc` allocation per leader layer,
  `same_owner` comparison, construction kept crate-private,
  exported only because the public `LayerKind` carries it).
  Owner-checked clearing is preserved.
- Also removed the absolute "can never diverge again" claim on the
  shared transform resolver, reworded to what the sharing
  actually guarantees.

## W3 — EditableText configuration and measurement (same workstream, still open)

- Numeric policy resolves once, in the descriptor setters: line
  counts clamp at one, caret sizes at zero, and `multiline` tracks
  `max_lines` with the last setter winning. The single-funnel
  `editable_text_configured_with_cursor` conversion trusts this
  invariant instead of re-clamping; conflicting `min_lines` above
  `max_lines` stays unnormalized and deterministic (minimum height
  wins). Also removed a stale "legacy constructor" comment with no
  referent.
- Fail-first alignment fix across two layers: single-line
  `text_align` was inert because the engine fed the wrap width
  (empty without wrapping) to parley's align call, and translation
  dropped parley's per-line alignment offset from glyph and caret
  positions. Both fixed at their owners; caret, selection, and hit
  testing follow the translated layout from the same funnel.
- Evidence: 15 tests in
  `crates/incular-widgets/tests/editable_text.rs` (setter order,
  floors, conflicting ranges, desired/expands/unbounded sizing,
  placeholder/text paint, caret geometry and color, selection,
  alignment shift, byte-per-glyph obscuring with intact semantics,
  identical reapplication, content/style invalidation, semantic
  actions) plus `specs/editable_text_properties.json` (all 22
  discovered options) validated by
  `tests/editable_text_ledger.rs` with missing/stale/unresolved
  negatives.

## W3 — EditableText interaction and controller replacement (same workstream, still open)

- No tree-level reentrancy vector found on inspection: submit reads
  the element through an immutable borrow and invokes `Fn(String)`
  with no tree access; caret/focus paths clone the controller
  handle before mutating; `reset_caret` notifies nothing, so the
  listener-clone-then-notify hardening lives in
  `TextEditingController::update`, covered by a reentrant
  `add_listener`-during-notify regression (2 notifications,
  terminates). Native IME claims stay limited to CPU-side dispatch
  gating through `InputEvent::Ime`, not platform behavior.
- Controller replacement detaches the old controller: content and
  visual revisions are polled per retained entry, so later edits to
  the previous controller never reach the new presentation.
  Pointer focus resets the caret to the click point, so selection
  assertions select after mount, not before.
- Evidence: 6 tree tests in
  `crates/incular-widgets/tests/editable_text_interaction.rs`
  (enabled/read-only gates, controller replacement, submit
  replacement, hints while focused with focus survival,
  selection-only versus content revisions, caret point mapping),
  6 dispatch tests in
  `crates/incular-runtime/tests/text_input_gating.rs` (disabled
  rejects edits/submit, read-only selects without mutating,
  single-line Enter submits while multiline inserts, Done/Newline
  actions, IME preedit/commit gating, cut/paste gating), and the
  reentrancy case in `crates/incular-text/tests/editing.rs`.

## W3 — Pointer blocking and focus wrappers (same workstream, still open)

- Pointer flags are element state, never render state: both hit
  paths read the live element configuration, so a mounted flag flip
  takes effect on rebuild alone with no re-layout. Blocking implies
  no semantic removal; both subtrees still collect semantics.
  `AbsorbPointer` additionally hardens the window-chrome dismissal
  walk. `SliverIgnorePointer` delegates from the scrolling crate and
  stays outside this family.
- `Focus` without node, autofocus, or refusal passes its child
  through with no wrapper element; refusal excludes the element from
  traversal and autofocus advertisement. Node replacement detaches
  tree resolution while the old node's local flag is untouched;
  removing a focused child clears resolution and stale-id mirroring
  is a no-op. Scope autofocus resolves to the first focusable
  descendant. Listener callbacks compare by `Rc` pointer, so
  replacement swaps handlers and identical reapplication bails out.
- Resolved gap (package A below): `KeyboardListener.include_semantics`
  now gates the listener's own increment/decrement affordance instead
  of sitting unread; the ledger records the established contract.
- Adjacent finding while testing: sibling roots share no semantic
  parent, so the debug dump follows one root; multi-root assertions
  go through node handles, not dump text.
- Evidence: 19 tree tests in
  `crates/incular-widgets/tests/pointer_focus_wrappers.rs`
  (Ignore/Absorb overlap, disabled flags, layout-free flag change,
  semantics retention, Focus pass-through/refusal/replacement/
  removal, scope autofocus and all three traversal policies,
  listener replacement/bailout/alias/autofocus-needs-node/
  include_semantics reader absence/typed shortcuts/node alias,
  neighbor-rebuild preservation), 2 mount tests in
  `crates/incular-runtime/tests/focus_mount.rs`, plus
  `specs/pointer_blocking_properties.json` (4 options) and
  `specs/focus_wrappers_properties.json` (21 options) validated by
  `tests/pointer_blocking_ledger.rs` and
  `tests/focus_wrappers_ledger.rs`.

## W3 — KeyboardListener semantic policy (same workstream, still open)

- `include_semantics` governs exactly the listener's own keyboard
  affordance: the increment/decrement executability arm in
  `semantic_action_is_executable`, which exists so a listener can
  service those actions through key dispatch without a bound semantic
  callback (the slider path). Gating withdraws that advertisement
  from the listener node only; child and sibling nodes collect
  normally and `dispatch_keyboard` never reads the flag. No new
  role: the existing `Group` node plus `Increment`/`Decrement`
  actions carry the contribution.
- Evidence: `keyboard_listener_include_semantics_governs_listener_affordance`
  mounts a labeled `Group` listener over a `Text` child beside a
  sibling root with a working `on_key`, then drives true to false to
  true asserting the affordance flips while labels, node handles,
  element ids, dispatch counts, and layout/paint/composite counters
  stay put. Fail-first verified by reverting the one-line gate. The
  old identical-output test is replaced, and the focus ledger record
  now names the real semantic consumer.

## W3 — Semantic-action execution agreement (same workstream, still open)

- Advertisement and execution shared only a name: with the flag off
  and no explicit callback, nothing was advertised but the keyboard
  fallback still executed. Both now read one predicate,
  `GestureCallbacks::services_semantic_keys`, owned by the gestures
  crate: the executability arm and the semantic fallback walk, which
  shares the ancestor traversal with plain key routing through a
  per-caller filter. Explicit callbacks keep their independent fast
  path on both sides, and plain keys serve every listener
  unfiltered. No parallel checks were added.
- Evidence: 6 runtime tests in
  `crates/incular-runtime/tests/semantic_action_dispatch.rs`
  (fallback delivery enabled, withdrawal disabled with fail-first
  against the unfiltered walk, explicit-callback independence,
  `CallbackShortcuts` neither advertising nor executing, child and
  sibling preservation across a mounted flag toggle with keys
  intact, mounted callback-plus-flag replacement). The focus ledger
  `include_semantics` record cites the shared predicate and the
  execution evidence.

## W3 — EditableText validation and alignment audit (same workstream, still open)

- Validation funnel holds: `EditableText` fields are private with
  normalized `new()` defaults, every numeric setter clamps, and the
  single `From` conversion (`descriptors.rs`) feeds the single
  `pub(crate)` constructor holding the only `TextFieldSpec` literal.
  No public low-level constructor, alias, or sibling path bypasses
  the setters, so no bypass regression was needed; the audit itself
  is the evidence.
- Real defects found and fixed. The package-A alignment fix rested
  on a false premise: parley 0.7 seeds positioned glyph x with the
  line offset, so adding `metrics.offset` translated centered/right
  text twice (center rendered end-aligned, end painted off-field;
  only start, offset zero, looked right). `translate_layout` now
  reuses positioned x verbatim. Separately, `line_caret_x` and
  `caret_for_line_x` treated `TextLine.width` (unshifted advance) as
  an absolute edge, misplacing selection trailing edges and line-end
  fallbacks on aligned text. `TextLine.offset` records the
  translation once; paint and hit testing share the stored layout,
  so one fix covers both.
- Hardening-claim correction: `f8e853c` added only the reentrant
  `add_listener` regression — the clone-then-notify protection in
  `update`/`notify_listeners` predates package B, and every mutation
  path funnels through `update`. No controller production change was
  needed. Replacement review also holds: swaps invalidate through
  render-kind controller identity (covering A-B-A), the revision
  poll serves in-place edits only, and `reset_caret` notifies
  nothing.
- Evidence: 8 tests in
  `crates/incular-widgets/tests/editable_text_alignment.rs` (exact
  mode offsets, direction-sensitive start/end, empty/placeholder,
  overflow translation-once, multiline per-line offsets, mounted
  width/alignment changes, aligned selection edges, pointer
  mapping). Fail-first verified per fix by reverting each
  production change independently. The editable-text ledger's
  `text_align` record now states the single-translation policy and
  cites the new regressions.

## W3 — Multi-root semantic diagnostics (same workstream, still open)

- `debug_dump_limited` walked only the designated root, silently
  omitting sibling roots. It now visits every parentless node with
  the designated root first (single-root output byte-identical),
  then the rest in id order under the shared node/depth budgets.
  Collection and parentage are untouched; each node still prints
  once under its first-visited parent.
- Adjacent finding while testing: explicit semantics merge onto the
  same element, so a group-with-descendant case needs the role on a
  container, not on the text itself.
- Evidence: 5 tests in
  `crates/incular-widgets/tests/semantic_roots.rs` built through
  real collection (two roots, nested-plus-sibling without
  duplication, root removal, deterministic recollection,
  hidden/excluded absence). The neighbor-rebuild test now asserts
  the corrected dump alongside its node-handle assertions.

## W3 — RTL caret and selection geometry (same workstream, still open)

- Shaped caret stops were already direction-correct, but
  `selection_rects` mapped two byte edges through the LTR-assuming
  glyph walk: partial RTL selections collapsed to a sliver and
  mixed-direction ranges could not split. The engine now retains
  per-line visual cluster spans (`TextClusterSpan`, rebased across
  document paragraphs); Widgets projects intersecting spans and
  merges them, so one logical range yields every visual segment it
  covers. Mid-cluster ranges snap to cluster edges, matching
  grapheme-granular editing. Paint, hit testing, and selection
  areas share the rewrite through the stored layout.
- `line_caret_x` and `caret_for_line_x` remain solely as documented
  stop-free fallbacks (affinity fallback, empty lines, and the
  pre-existing selectable-text click path); no byte or glyph order
  is reversed anywhere as a bidi substitute.
- Adjacent discipline confirmed while testing: `paint()` reuses the
  picture cache until the layout preamble consumes revisions, so
  select-then-paint steps must re-run layout like real frames; the
  tests do. Intra-char bytes never reach mapping (selection clamps
  down to char boundaries first).
- Evidence: 7 tests in
  `crates/incular-widgets/tests/editable_text_bidi.rs` (RTL edges
  under all alignments, RTL partial span with fail-first sliver,
  mixed partial spans, run-edge affinities, RTL/mixed pointer
  mapping, RTL wrapping with mounted width determinism,
  ligature/combining cluster snaps). The `text_align` ledger record
  cites the direction evidence. Limitation: no constructible
  single-range case yields disjoint visual segments in this
  shaper's output, so merging is covered by exact contiguous
  bounds; selectable-text edge clicks keep the approximate
  fallback path.

## W3 — Shortcut/action wrapper audit (same workstream, still open)

- Subscription ownership is Weak-at-the-source with Drop removal
  everywhere: action invocation listeners snapshot before delivery
  (reentrant invokes terminate with exact nested phase order and no
  live borrows), focus behaviors observe nodes weakly (rebuilds keep
  exactly one live observer, unmounts go silent), and rebuilt
  shortcut scopes replace handlers instead of duplicating them.
- Scope lookup is dispatch-walk order: nested `CallbackShortcuts`
  and nested detector scopes both resolve nearest-first with
  outward fallthrough, pinned per activator. Detector `enabled`
  gates the node and both highlight visibilities but not the
  independently owned shortcut/action scopes; that composition is
  the established contract, not Flutter parity.
- Found gap, behavior unchanged: `FocusBehavior::mouse_enter` has no
  retained caller, so `on_show_hover_highlight` is unreachable
  through tree dispatch. The ledger records wired storage with an
  explicitly undriven tree path plus owner-level execution
  evidence; wiring hover dispatch would be new behavior.
- Evidence: 9 tests in
  `crates/incular-widgets/tests/action_wrappers.rs` (shortcut
  replacement/unmount, nested scope preference, reentrant invoke
  order, single focus subscription across rebuilds, callback
  replacement, unmount release with highlight observation,
  autofocus advertisement, nested detector scopes, disabled
  contract), 1 owner-level hover test in
  `crates/incular-gestures/tests/focus_highlight.rs`, plus
  `specs/action_wrappers_properties.json` (16 options) validated by
  `tests/action_wrappers_ledger.rs`.

## W3 — Retained hover highlights (same workstream, still open)

- Detector behaviors join the existing mouse-hover sets through
  hovered descendants (one map, one diff, no second tracker, no
  synthesized events); raw hit lists only carry `RawInput`, so
  membership climbs from the deepest hit, which also keeps nested
  detectors tracked and absorbing overlays exclusive. Window exit
  and unmount follow the regions' silent-drop discipline, so a
  remount re-enters instead of staying suppressed.
- Rebuilds carry hover presence via the previously dead
  `restore_hovering`, fixed at its owner to also sync derived
  visibility (otherwise the next exit computed no flip and stayed
  silent). Replacement callbacks learn only subsequent
  transitions, consistent with focus callbacks; synchronous reads
  expose current state. Disabling carries presence silently and the
  next exit still notifies exactly once.
- Evidence: 7 tests in
  `crates/incular-widgets/tests/detector_hover.rs` through real
  pointer dispatch (enter/move/exit, rebuild without duplicate
  enter, replacement transitions-only, disable/re-enable, unmount
  clearing, nesting plus blocking sibling, stationary mode
  changes). The action ledger hover record now names the retained
  path and execution evidence.

## W3 — Bidi and cluster selection distinctions (same workstream, still open)

- Boundary levels are now explicit where each is enforced:
  selections clamp to UTF-8 char boundaries (`clamp_to`, snapping
  down); shaping clusters are the coverage granularity
  (`TextClusterSpan`, projected by `TextLine::selection_spans`);
  visual positions come from caret stops and stored spans, never
  from re-sorted bytes or glyphs. Mid-grapheme edges survive
  clamping and resolve to cluster edges in mapping.
- Shaped spans tile contiguously in visual order, but that does
  not imply single-segment selections: a logical prefix ending
  inside the Hebrew run covers two separated visual intervals
  with unselected ink between them, and the merge preserves the
  gap (proven by regression, not by tiling). Synthetic span tests
  stay separately labeled as algorithm coverage. Newline-only and
  invisible-joiner selections highlight nothing (sane, pinned);
  collapsed selections highlight nothing; intra-char bytes never
  reach mapping. No multi-grapheme cluster arises in this
  environment (Latin ligatures split per char, lam-alef per
  grapheme), so that limitation is documented, not claimed.
- Evidence: `crates/incular-text/tests/selection_spans.rs`
  (synthetic disjoint/empty merge plus shaped tiling) and 3 new
  paint tests in `editable_text_bidi.rs` (collapsed, newline-only,
  joiner-only). Non-ASCII fixtures use explicit escapes so editors
  cannot normalize them.

## W3 — SelectableText shaped pointer mapping (same workstream, still open)

- Selectable clicks went through the approximate glyph walk with
  an unshifted width cutoff; they now resolve bytes and affinities
  from shaped caret stops like text fields do. `StaticSelectionPoint`
  carries the click affinity into handle placement with identity
  still defined by (element, byte), so collapsed detection and
  anchor/extent roles cannot flip on affinity alone.
- Real defect alongside: single-element backward drags selected
  nothing because per-element ranges never normalized, unlike the
  cross-element content range. Endpoints now normalize within one
  element too.
- Evidence: 7 tests in
  `crates/incular-widgets/tests/selectable_text_pointer.rs`
  through real clicks and drags (RTL logical bytes both drag
  directions, mixed byte/affinity handles via the area controller,
  aligned interior anchors with suffix highlight geometry, wrapped
  partition exactness, cross-run exact substrings,
  ligature/combining cluster snaps, width-change agreement with a
  fresh mount). Fail-first: RTL and mixed tests fail on the walk;
  the reverse drag fails without normalization. Non-ASCII
  fixtures use explicit escapes where normalization would change
  bytes.

## W3 — Cluster extraction audit and separated spans (same workstream, still open)

- Parley source audit: `glyphs()` and `positioned_glyphs()` both
  iterate `Run::visual_clusters().flat_map(glyphs)`, the latter
  only adding running x (which already carries the line offset),
  so order and cardinality correspond structurally for RTL runs,
  ligatures, and multi-glyph clusters alike; the window
  re-locates the style-uniform slice within that same sequence.
  No production change: extraction is correct as written, now
  with the invariant documented at the site.
- Evidence: a logical-prefix regression over "hi שלום bye"
  ([0..5), boundaries calculated from the string, not magic
  numbers) asserting two rects with an uncovered gap middle at
  both paint and engine level. Fixture correction: my earlier
  tiling claim is withdrawn above.

## W3 — Selection-span API contract (same workstream, still open)

- `TextLine::selection_spans` takes a half-open logical byte
  range: collapsed and reversed ranges yield nothing by policy
  (caret rendering stays on caret stops), callers pass normalized
  ranges. Partial-cluster ranges expand to whole shaped clusters,
  documented including multi-grapheme ligatures. Line-edge
  abutment after clamping yields nothing instead of a stray
  cluster rect. Mounted EditableText and SelectableText suites
  keep agreeing through both consumers.
- Evidence: edge cases in
  `crates/incular-text/tests/selection_spans.rs` (collapsed
  inside clusters, reversed, out-of-line, empty lines,
  partial-cluster expansion) alongside the existing paint tests
  for collapsed, newline-only, and joiner-only selections.

## W3 — Selection-area lifetime and affinity (same workstream, still open)

- Audit verdict: the endpoint-identity design (logical identity on
  element and byte, affinity for handle edges only, manual Eq/Ord
  ignoring affinity, same-element backward normalization) is
  correct as built; fail-first toggle experiments confirm the
  mapping, the normalization, and the overflow expectations all
  depend on it. No production change in this package.
- Forward and backward drags across elements produce identical
  document-ordered copies with identical handles. Keyed reorder
  permutes identities: endpoints stick to elements while coverage
  follows document order (reordering back restores the copy).
  Removed endpoints resolve to nothing through generational IDs
  with no extra registry; queries degrade to empty without
  panicking and fresh clicks recover. Shortened text clamps
  through the existing boundary helper. Hidden content without
  maintain flags is structurally absent (replaced at conversion);
  maintained offstage content still copies.
- Evidence: 6 tests in
  `crates/incular-widgets/tests/selection_area_lifetime.rs`.
  Limitation: paragraph-final bytes coinciding with earlier
  boundary stops stay pointer-unreachable (keyboard motion
  reaches them); far-overflow clicks resolve to the nearest
  caret.

## W3 — Stack, Positioned, and IndexedStack (same workstream, still open)

- Conflict policy, established in `incular-layout` and pinned, not
  changed: left wins over right, top wins over bottom; left+right
  and top+bottom derive the algorithm size while explicit
  width/height still bounds measurement (fixed children shrink,
  never grow); renders keep measured size, which right/bottom
  anchoring math does not reuse; negative and non-finite edges
  drop to unset. Clip behavior stays with the clip ledger.
- `Stack.text_direction` is retained but inert (absolute
  alignment factors; nothing reads it): documented honestly, with
  a mounted flip resolving identically. IndexedStack lays out
  every child but paints, hits, traverses, and exposes semantics
  for the indexed one only; focus resolution follows membership
  while node flags stay untouched; out-of-range indexes hide
  gracefully with the stack itself as the hit.
- Evidence: 13 tests in
  `crates/incular-widgets/tests/stack_layout.rs` (axis
  combinations, conflicts, negatives, setter order,
  builder parity, fit modes, alignment, direction flip,
  indexed switching/semantics/focus/OOB, keyed reorder/removal,
  identical reapplication) with independently derived positions.

## W3 — Row, Column, Flex, Expanded, and Flexible (same workstream, still open)

- Flex factors floor at one at every entry point with no
  divergence; spacing stays nonnegative and is shared before flex
  allocation; unbounded main axes measure flex children
  intrinsically. Tight fit forces allocations while loose fit
  only bounds (fixed children shrink, never grow). Public
  Row/Column APIs stay separate over one retained Flex kind.
- Real defect fixed: cross-axis baseline alignment silently
  behaved as start because render baselines never reached the
  algorithm. The Flex arm now forwards them; fail-first verified.
  Reversal flips child order only, never packing edges; overflow
  hits stop at parent bounds while semantics follow laid-out
  bounds.
- Evidence: 13 tests in
  `crates/incular-widgets/tests/flex_layout.rs` (finite versus
  unbounded axes, tight/loose children, zero factors, spacing
  with distribution, RTL/vertical reversal, mounted factor
  changes with fresh agreement, baselines, overflow hit and
  semantics, main-axis size, shared representation, builder
  parity, identical reapplication).

## W3 — Padding, alignment, and sizing constraints (same workstream, still open)

- Size and ratio validation floors at descriptors and conversion
  (negative factors to zero, aspect ratios to epsilon, box
  dimensions clamped); core `Size::new` and `Constraints::new`
  panic loudly on invalid input instead, and negative padding
  stays meaningful by expanding. Overflow minimums trust the
  lowering path; additional constraints clamp into incoming
  bounds exactly like Flutter's enforce. `PreferredSize` is not
  publicly reachable and stays out of scope.
- Aliases resolve through shared policies: Center delegates to
  Align, IntrinsicWidth/Height to UnconstrainedBox, and builder
  forms equal fluent construction everywhere checked.
- Evidence: 16 tests in
  `crates/incular-widgets/tests/layout_padding_sizing.rs`
  (insets with insufficient space, negative padding, constructor
  parity, align factors with negative flooring, fractional
  scaling, aspect ratios with invalid flooring, baseline target
  deltas, fitted contain, overflow reporting with paint past the
  edge, constrained/limited/overflow bounds, unconstrained
  aliases, loud invalid constraints, sized-box parity with
  clamping, mounted updates with identical reapplication,
  constraint transforms).

## W3 — Batch architecture review (same workstream, still open)

- Corrected a stale fallback comment (selectable clicks no longer
  use the walk) and narrowed an overbroad ligature claim to
  observed granularity.
- Validation lives once per layer and agrees everywhere checked:
  flex factors floor at one, spacing and box factors at zero,
  aspect ratios at epsilon, box dimensions at conversion, invalid
  `Constraints`/`Size` construction panics loudly. Builders cannot
  bypass rules (transforms mirror setters); OverflowBox minimums
  stay raw and fail deferred at layout, unlike LimitedBox floors
  — established asymmetry, documented, not papered over.
  Row/Column/Flex share one retained kind with parity tests;
  aliases (Center, IntrinsicWidth/Height, SizedOverflowBox
  composition) resolve through the same policies.
- Parent data compares field-wise and transfers through lowering
  with invalidation proven by reallocation tests. Tests assert
  measured geometry and dispatch, never bare field values.
- No shared helper was extracted: the one-line floors are
  consistent, and indirection would add no safety. No ledger
  validator change was required; no new ledgers were added.
- Visibly unresolved: `PreferredSize` unexported; Stack direction
  inert by design; multi-grapheme clusters unobserved here;
  paragraph-final coinciding stops pointer-unreachable;
  select-all and overflow edge semantics as pinned.

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

Progress (W4 complete on its exit criteria; residuals recorded below):
items 1–3 are implemented — single `RouteEntry` storage, explicit `PageKey`
identity with pre-mutation duplicate rejection, same-key rename,
topmost-wins duplicate claims, indexed lookup, and a shared commit/effect
path (cleanups before observer events, after borrows) for
push/pop/replace/set_pages/restoration, with guarded-pop checks preserved
and retirement outside borrows (unmatched entries, restored stacks,
replaced callback registrations, plus an explicit retired-children
collection for keyed child replacement at any stack position). Item 4 is
covered: two-observer event ordering with reentrant cleanup, reentrant
guards/observers/cleanup, keyed reorder, removal, restored routes,
destructor reentrancy, deep-link normal operation end-to-end,
restoration/builder failure reporting with fallback and recovery, and
exactly-once failure reporting per attempt — the earlier
swallowed-platform-failure claim is retracted: every apply failure
notifies inside `apply_route`, `receive_route_information` adds nothing,
the missing-parser path is unreachable by construction (`try_from_parts`
pairing guard), and superseded transactions report `Ok` by design. Router
transactions attribute delegate reactions to the open application (staged
reactions complete, diverge-adopt, or discard; ordinary completions are
never suppressed), and `BasicRouterDelegate` acceptance is atomic with
retirement outside borrows. Back attachment is acyclic-by-construction
with typed rejection; reorder carries a deterministic
large-permutation pin. Route lifetimes are neutral per-entry handles that
survive keyed reorder, end exactly once on every removal path (pop,
replace, declarative, restored-stack, fallback) plus explicit disposal,
and never leak through event snapshots. Route-scoped tasks bind lifetimes
to `TaskScope` children with no new engine (removal cancels, reorder and
deactivation do not, late completions discard, shutdown/close cascade
unchanged). Focus restoration ownership is a runtime `RouteFocusState`:
per-route saves restore only validated targets (mounted, focusable,
enabled, oracle-owned) with deterministic clear fallback and drop
disposal; `focus_trap` is removed as an accepted no-op (barrier input
blocking unchanged). Item 5 (runtime owners): field-to-owner inventory in
plan-19, `SimulationWaiters` extracted, activation delivery audited
(service queue as the only queue, exactly-once per live listener, no
dedup, panic-safe drain), restore/rebuild failures explicit. Residual
future work, not W4 debt: modal focus containment (no scope clamp exists;
kept explicitly separate — removing `focus_trap` implemented nothing),
and further runtime-owner extractions only with a demonstrated defect.
The mount-outlet residual is closed twice over: first by `RouteOutlet`
with post-frame automation and a pinned panic policy, now by an explicit
placement policy (opaque/popup/modal/disposal/transitions/caller keys),
a single ordered `present_frame` operation with pre-reconcile saves and
frame-scheduled deferred convergence, nearest-outlet ownership with a
documented nested-mounting pattern, and lifecycle verification across
reorder, modals, disposal, shutdown, transient navigation, and teardown.
W4 re-closed on the acceptance-review pass: a bounded host loop
converges in exactly one follow-up and idles, deferred revisions
re-request per new navigation with remount executing, the receipt path
audits to a single writer/promoter/cleanup owner with no
counterexample, and all original exit criteria hold (no parallel
arrays, no name ambiguity, no callbacks under borrows, no duplicated
engine). Earlier passes established transactional frames, restoration
frame contracts, owned nested and driver lifecycles, consumed
snapshots, stale scheduling, declarative presentation, bounded
key-error recovery, and the combined ownership flow. See plan-19 for
the itemized evidence; non-key update partiality stays explicitly
partial.

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

Attachment ownership inventory (W5.1 decision, Package B — no behavior
change; enforcement follows in Package C for the ordinary path):

| Family | Position record | Extent writers | Multi-attach today | Replacement | Unmount / window close |
| Ordinary `ScrollController` (+`PageController` alias) | shared `ScrollState` (clones are one handle, `PartialEq` by `Rc` identity) plus authoritative `metric_owner` tree id | claiming viewport layouts first; public `update_extents` stays open for hosts | enforced: second live viewport fails typed before overwriting (same-tree names the owner element; cross-tree names the owner tree — pinned) | render takes the new controller; old keeps isolated record (pinned) | unmount releases; tree drop releases owned slots |
| Wheel (`FixedExtentScrollController` over an inner `ScrollController`) | same inner record | wheel layout feeds the ordinary record | allowed, unenforced | same as ordinary | nothing |
| Two-dimensional (H/V pair) | two independent `ScrollController`s | 2D layout per axis | N/A by construction | per axis, as ordinary | nothing |
| Draggable sheet (+ inner list) | controller↔sheet binding (`attach`/`detach_from` on handle change) plus sheet-owned inner `ScrollController` | sheet extent logic; `set_inner_extents` | sheet attach tracked; inner shared with the inner list by design | previous handle detaches | sheet state drops with the tree |

Sharing that is coordination, not attachment (left alone): scrollbar
read-only geometry reads; `parent_controllers` receiving leftover
deltas; 2D H/V independence; `FixedExtent` wrapping (same handle);
sheet/inner-list sharing owned by sheet state. Chosen contract:
ordinary path takes a single live viewport attachment (reject a second
live attachment before it overwrites geometry; deterministic release on
unmount via liveness); clones are the same attachment, never new ones.
Wheel/2D/draggable keep their models (wheel inner follows the ordinary
rule in principle — future family work, explicitly out of scope).

Activity transition table, ordinary `ScrollController` (W5.2, actual
code only — no ballistic driver exists in production:
`apply_spring_step` has no production callers; sheet motion uses its
own generation tokens, untouched):

| From → event → to | Notifications | Notes |
| idle → begin → active | Start | duplicate begin: `false`, nothing |
| active → begin → active | none | idempotent while open |
| active → end → idle | End | duplicate end: `false`, nothing |
| idle → end → idle | none | |
| * → programmatic move → * | Update (or nothing if unmoved) | takeover inside open activity needs no End |
| * → unchanged-offset jump → * | none | offset-unchanged never moves the flag either way |
| active → viewport detach → idle | End | first defect: unmount ends app-open activities; re-begin starts fresh |
| * → controller replacement → * | none | old handle keeps its isolated flag; new starts fresh |
| wheel sample | Start, UserScroll, Update?, End | each sample is a complete activity by adapter policy |
| reentrant jump during Start | nested Update delivered immediately | pinned exactly (A sees Start,Update; B sees Update,Start) |

Clocks: ordinary activity is fully synchronous (no timers), so
determinism needs no clock control — sequences are exact. First
defect implemented through the existing owner (viewport unmount ends
the controller's activity after unmount work, covering both `Scroll`
and sliver render kinds). Completed scope: ordinary activity
lifecycle + ordinary attachment enforcement. Remaining families,
explicit: wheel/2D/draggable attachment rules; window-close activity
sweep; broader animation (ballistic driver) policy; scrollbar-thumb
drags stay unbracketed programmatic moves by current design.

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
