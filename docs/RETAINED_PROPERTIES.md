# Retained property audit

Stage F is in progress. This ledger distinguishes verified contracts from the
remaining property audit; it is not a claim that every exported builder option
has been verified.

## Phase ownership

Render configuration updates use `incular_core::Invalidation`, the same phase
vocabulary as dependency invalidation. There is no separate render bit layout.
`render_object/update.rs` owns comparisons between old and new render values.
`tree/reconciliation.rs` applies layout/paint work and updates retained layers.
Geometry-changing compositor properties include semantics and hit-test impact,
without forcing layout or picture recording. Hit testing reads current retained
transforms; `update_semantics` rebuilds semantic geometry from those transforms.
These consumers currently do not maintain separate geometry invalidation caches.
If that changes, their phase masks must be consumed by the new caches.

| Property family | Descriptor / transfer | Execution and current evidence | Remaining work |
| --- | --- | --- | --- |
| Visibility: visible, replacement, state, size, animation, semantics | `layout/basic/visibility.rs` validates hidden policy; `WidgetKind::Visibility` retains it; layout lowering projects size policy | Stage A `tests/visibility.rs` exercises combinations, retained identity, layout, input, semantics and animation | Keep coverage when primitive dispatch is regrouped |
| Transform matrix and origin | `WidgetKind::Transform` → effects lowering → retained transform layer | `tests/render_invalidation.rs` verifies no layout/paint, stable semantic identity, translated bounds and hit target before layout/paint | Extend geometry evidence to rotation, scale and linked followers |
| Opacity and effect parameters | Effects lowering → opacity/filter/blend/shader layers | Existing opacity dirty-phase regression; effect builder tests verify normalization/conversion | Per-property execution checks for filters, shader callbacks and linked-layer policy |
| Text content, style, alignment, wrap, lines, overflow | Text descriptor → visual lowering | Render update comparisons separate color from shaping metrics; text color/font-size dirty-phase tests | Inventory every style field and overflow/alignment combination |
| Editing controller, size, style, placeholder, multiline, line bounds, expands, alignment, enabled, read-only, obscuring, cursor geometry/visibility/colors, selection color | `TextFieldSpec` → `lowering/visual.rs` preserves every render field; submit callback stays in widget/runtime interaction | Editing suite covers graphemes, IME preedit, restoration, selection and focus painting | Per-option update/invalidation tests and constructor/TypedBuilder validation parity |
| Scroll controller, axis, reverse, physics | `WidgetKind::Scroll` → scrolling lowering → viewport | Existing scrolling/sliver and retained performance suites | Per-property phase matrix; activity ownership is Stage G |
| Button visuals, enabled/focus policy and action | `ButtonSpec` → visual lowering; callbacks stay in interaction | Color-only updates avoid layout; Stage A checkbox behavior is independent of visual slot | Inventory all control/Material options and reduce redundant presentation policy |
| Box constraints: four bounds | Private fields, validated `new`/`try_new`, read-only accessors | `incular-config/tests/constraints.rs` covers invalid floats/order, unbounded limits, clamping, deflation and constructor panic policy | Completed boundary migration; preserve invariants in future helpers |

## Remaining Stage F scope

Complete the exported-widget property inventory beyond the initial families,
including generated builders. Group primitive-family comparisons with their
behavior where that reduces duplicate decisions. Audit sibling-crate uses of
`internal` and narrow exports without exposing retained mutable storage. Finish
control/Material token and behavior ownership checks. Existing enum dispatch
remains closed; this work does not introduce a dynamic primitive registry.
