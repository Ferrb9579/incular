# Plan 06 - Popup Placement, Focus, Dismissal, and Accessibility

## Goal

Build one reusable desktop-grade anchored-popup policy used by menus, context menus, combo boxes, tooltips, and popovers regardless of whether presentation is native surface or in-view overlay.

Plan 05 provides hosting. This plan determines where/how a popup behaves.

## Architecture

### Placement engine

Create a renderer/platform-neutral placement module that consumes:

- anchor rect;
- desired popup size;
- preferred placement/alignment;
- alignment offset;
- text direction;
- available viewport/work-area geometry;
- safe margins;
- role-specific policy.

It returns a pure placement result:

- origin;
- final constrained size if needed;
- chosen side/alignment;
- whether flip/shift/constrain occurred.

Use a deterministic `flip -> shift -> constrain` strategy with role-specific ordering.

Do not scatter placement math across Material widgets.

### Menu/submenu behavior

Menus need additional policy:

- top-level menu prefers expected vertical side;
- submenu prefers text-direction outward side, then flips;
- context menu anchors to pointer position;
- menu chains close consistently when ownership/focus changes;
- keyboard navigation crosses submenu boundaries.

### Focus and dismissal

Define focus ownership independently of native OS focus. Native popup surfaces that cannot/should not activate must still participate in Incular's logical focus route.

Outside-click dismissal must work across parent and transient surfaces. Do not use a giant invisible native window/barrier.

Escape, focus loss, parent deactivation, and explicit selection need role-specific close policy.

### Accessibility

Transient semantic trees must preserve logical parentage/ownership even if hosted by another native surface. Screen-reader traversal and menu roles/actions must remain coherent.

## Crate ownership

- likely `incular-widgets` or a domain module already owning overlay/transient placement semantics for pure policy;
- `incular-material` only supplies Material defaults and wrappers;
- `incular-runtime` coordinates cross-surface focus/dismissal;
- `incular-desktop` supplies work area/input across native surfaces.

Do not create a new crate solely for popup placement unless dependency boundaries prove it necessary.

## Hard invariants

1. Same placement policy drives native and overlay presentation.
2. Placement is deterministic for the same geometry/direction inputs.
3. No popup placement can resize the owning top-level window.
4. Submenu direction respects RTL.
5. Outside click works across surface boundaries.
6. Logical focus cannot become trapped in a destroyed popup.
7. Tooltips do not unexpectedly steal activation/focus.

## Tests

- All four screen edges and corners.
- Very large popup constrained to work area.
- Bottom->top and right->left flips.
- RTL submenu direction.
- Mixed-DPI parent/display transitions.
- Parent movement repositions native popup without widget relayout corruption.
- Outside click in parent, sibling popup, and desktop-deactivation paths.
- Escape/selection/submenu lifecycle.
- Accessibility tree remains ordered and actionable.

## Acceptance criteria

Popup behavior is good enough that Material menus no longer need manual `Positioned(left/top)` logic for basic placement.
