# Architecture decisions

Accepted for Stage B on 2026-09-05. These decisions constrain future stages;
acceptance does not claim their implementations are finished.

## B01 — One dependency engine

Core owns values, dependency edges and scope cleanup without widget/runtime
dependencies. Runtime schedules work; widgets identify consumers and phases.
Keep controller-specific commands, but connect their observations to this engine.
Stage C replaces the duplicate signal implementation and ignored runtime binding.
Verify branch changes, diamond graphs, unmount and multi-window scope behavior.
Do not create a reactive crate without measured boundary benefit.

## B02 — One desktop host

Single-window and multi-window entry APIs delegate to one desktop event loop.
Native OS crates supply services; they do not duplicate frame coordination.
Stage E routes the convenience runner through runtime adoption and removes
the second loop. See [Desktop host ownership](DESKTOP_HOST.md).
Retain convenience names only as forwarding APIs with identical semantics.
Verify resize, parent/popup lifetime, input, menus and shutdown before removal.

## B03 — Portable platform contracts

Platform owns portable IDs, metrics, capabilities, commands and errors. Desktop
owns Winit conversions; native crates own FFI. Stage E removes the platform Winit dependency. Rendering stays below desktop and independent of it.
Stage A already requires owned safe WGPU surface targets; no detached-handle
constructor is retained. Accessibility projects semantics; mobile host wiring
is explicitly incomplete. Verify native target builds and lifecycle tests.

## B04 — Shared native request lifetime

Use one internal admission/completion/cancellation/shutdown mechanism with typed
domain operations. Do not erase results into strings or invent one scheduling
policy for every service. Stage D migrates dialogs, shortcuts, windows and shell
operations onto it. Stage A fixed shortcut admission ordering. Verify stopped,
stale, cancelled and duplicate completions with deterministic service tests.

## B05 — One behavioral owner

Widgets supply neutral primitives; controls supply themed Incular components;
Material composes those owners with Material presentation. A custom visual slot
must not change focus, activation or semantics. Retain the closed primitive enum
until a concrete extension requirement calls for another design. Stage F reduces
behavioral duplication and Stage G consolidates navigation/controller state.
Do not reverse the navigation/widgets dependency while both stacks exist.

## B06 — One invalidation contract

Independent build/layout/paint/composite/semantics/hit-test effects belong to one
authoritative property-change mapping. Narrow masks are explicit projections.
Stage F migrates the parallel taxonomies without a broad rewrite or lost cached
layer behavior. Verify each affected phase with retained operation-count tests.
Stage A's visibility policy is the model: preserve intent until the owning phase.

## B07 — Owned, budgeted caches

Each cache states its actual owner, budget and release condition. Local removal
cannot claim device eviction while shared GPU storage retains the resource.
Diagnostic registrations likewise follow their owner. Stage H implements release,
budget and failure reporting; Stage C handles reactive diagnostic lifetimes.
Verify churn returns to a bounded plateau and preserves resources still in use.

## B08 — Explicit compatibility and API ownership

Incular's Rust semantics take precedence over Flutter export layout. Preserve
documented application behaviors and test them; record omissions and deviations
honestly. Re-exports of one type are allowed; duplicated state engines are not.
Stage B fixes policy/gates; Stage I verifies old parity claims; Stage J closes
migrations. No permanent blanket “100% implemented” requirement remains.

Resizing headers expose `SliverHeaderScrollBehavior` in Widgets as an Incular
extension. Its four modes select one neutral geometry owner. Material maps its
compatibility properties into this policy instead of maintaining scroll state.
The pinned default preserves existing resizing-header behavior; the API inventory
links the retained mode regression as evidence.

`SliverHeaderOverscrollBehavior` likewise belongs to Widgets. Stretching consumes
the viewport's negative physical overlap and changes presentation extent only.
Scroll owns bounded bouncing offsets and preserves them across unchanged metric
updates; a widget must not synthesize a separate overscroll position.

`SliverNaturalHeader` is the measured counterpart to the explicit resizing
header. Its logical extent moves explicitly from an unverified estimate to a
validated measurement via unbounded learning passes; validity is tied to the
actual inputs (child content as measured, cross extent as constrained), and
stretched samples are presentation-only and can never accumulate into the
range. A cross change demotes back to an estimate for revalidation.
Replacing a viewport descriptor seeds the new header from the predecessor's
validated value but never grants validity — positional correspondence and
matching types authorize a seed, not proof — so equivalent replacements stay
range-stable while changed content re-establishes itself; reversal tracking
moves only across identical scroll behaviors, and stretched presentation is
never inherited. Material maps `stretch` into this policy through one Flex
structure shared by measurement and presentation: the bottom is measured
under real cross constraints and the toolbar fills the remainder, with no
size hints, caches, scroll controllers, or animation engines in Material.
The viewport reports leading overscroll as a negative physical offset in
both forward and reversed directions so either leading edge can stretch.

Retain `incular-painting` as a pure import shim for this pre-1.0 campaign so
existing imports continue compiling. All new work uses `incular-rendering`.
Stage J removes the shim and facade alias after examples and downstream migration
guidance use rendering and a repository search finds no consumers other than
the shim's compatibility test. It must never acquire implementation or become a
dependency of a domain crate. Config remains the value owner; layout aliases
remain useful exact re-exports and need no retirement.
