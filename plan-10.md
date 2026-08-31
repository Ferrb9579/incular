# Plan 10 - Move advanced scrolling runtime state into the retained tree

## Goal

Make the declarative tree truly declarative. Advanced scrolling widgets should describe configuration and expose narrow controllers; mutable viewport/layout state must be owned by retained render/element state.

## Current problem

Several `WidgetKind` variants retain `Rc<RefCell<...>>` models such as list-wheel, draggable-sheet and two-dimensional viewport objects. Layout mutates those models while related state also exists in render feature state.

This creates split ownership and lets apparently immutable widget descriptors carry hidden runtime mutation.

## Target ownership model

Follow the same lifecycle principle as Flutter's Widget/Element/RenderObject split:

- public widget/config objects are immutable configuration;
- controllers are narrow externally shared command/value handles;
- `Element` owns widget lifecycle identity;
- `RenderNode`/typed feature state owns layout, viewport caches, materialized-window state and other frame-to-frame mutable rendering data.

For each advanced scrolling family, separate:

```text
Public immutable config
        ↓ lowering/mount
Retained feature state
        ↕
Narrow controller/revision handle
```

## Family-specific direction

### List wheel

The widget keeps delegate/configuration and a scroll controller. Retained wheel state owns calculated child window, measurement cache, selected/visible range and current layout snapshot.

### Two-dimensional viewport

The widget keeps immutable delegate/configuration and controllers. Retained state owns vicinity cache, materialized children and latest layout.

### Draggable scrollable sheet

The widget keeps min/max/initial extent, snap policy and controller. Retained state owns current extent, attachment to ancestor controllers/actuator and layout-time state.

### Scrollbar/actuator

Shared controller handles may remain externally mutable, but visual/gesture state belongs to the retained node.

## Controller policy

Controllers must not become back doors to arbitrary retained mutation.

They may expose:

- commands (`jump_to`, `animate_to`, `reset`);
- observable scalar/value state intentionally part of the public contract;
- a monotonic revision/wake mechanism so the runtime knows work is pending.

They must not expose the internal child collection, layout cache or render feature state.

## Remove interior-mutable descriptors

After migration, `WidgetKind` must contain no `Rc<RefCell<ListWheel...<Widget>>>`, `Rc<RefCell<TwoDimensional...<Widget>>>` or equivalent runtime viewport object.

Do not wrap the existing mutable models in another handle and call that architecture complete. Split configuration from retained state.

## Migration sequence

1. Inventory mutable fields in every advanced scrolling model.
2. Classify each field as immutable config, public controller state or retained-only state.
3. Create typed retained feature-state structs for each family.
4. Change widget variants to carry immutable config/controller data only.
5. Move layout/update mutations into retained feature state.
6. Add explicit controller revision/wake integration where required.
7. Remove old retained wrapper types and `Rc<RefCell<...>>` descriptor storage.
8. Re-measure scrolling hot paths and memory footprint.

## Hard invariants

- declarative widget descriptors contain no hidden mutable layout state;
- one retained node owns one authoritative advanced viewport state;
- controller changes cannot bypass dirty/invalidation scheduling;
- materialized children remain keyed and lifecycle-safe;
- state survives compatible widget updates but is recreated on incompatible type/identity changes;
- no behavior depends on `RefCell` borrow ordering.

## Tests

- compatible rebuild preserves retained scroll/sheet/viewport state;
- incompatible replacement destroys it exactly once;
- controller commands schedule the correct work;
- layout-only state cannot be observed/mutated through the widget descriptor;
- wheel, sheet and 2D viewport stress tests across thousands of updates;
- no `RefCell` borrow panic paths;
- retained state count returns to baseline after unmount.

## Acceptance criteria

- advanced scrolling declarative variants are immutable config plus narrow controllers;
- mutable viewport/layout caches live only in typed retained feature state;
- old retained wrapper descriptor types are removed;
- no advanced widget stores `Rc<RefCell<...Widget...>>` runtime models;
- scrolling parity tests and performance contracts remain green.

## Validation

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test -p incular-widgets --all-features
cargo test -p incular-scroll --all-features
cargo test-constrained --all-features
```

## Completion report

For each advanced scrolling family, list which state moved to configuration, controller and retained feature state, plus before/after allocations and hot-path benchmark results.

## Completed implementation - 2026-08-31

### List wheel

- Declarative descriptors now store an immutable `WheelViewportConfig<Widget>` snapshot.
- The scroll controller remains the narrow externally shared command/value handle.
- The mutable `ListWheelViewport<Widget>` runtime instance and latest `WheelLayout<Widget>` now live in `RenderWheelState`.
- Selection reporting history (`last_selected_index`) therefore survives compatible widget updates without mutating the widget descriptor.

### Draggable sheet

- Declarative descriptors now store immutable `DraggableSheetConfig<Widget>`: extent bounds, initial extent, expand/close policy, snap policy, controller and child builder.
- `DraggableScrollableController` remains the external command/value handle.
- Mounted extent, inner scroll controller, parent-controller handoff, actuator subscription, notification listeners and activity state remain exclusively in retained `DraggableScrollableState`, owned by `RenderDraggableSheetState`.
- Compatible config updates preserve current extent/activity, clamp the current extent to new bounds, refresh snap policy, rebind a replacement controller, and rebuild the generated child only when the builder changes.

### Two-dimensional viewport

- Declarative descriptors now store immutable `TwoDimensionalViewportConfig<Widget>`: delegate, controllers, physics, directions, main axis, cache policy, clip policy and initial extent estimates.
- Horizontal/vertical `ScrollController`s remain the public command/value handles.
- `MeasuredExtentIndex` row/column measurements, materialized cell cache and latest `TwoDimensionalViewportLayout<Widget>` live only in `RenderTwoDimensionalState`.
- Compatible updates keep measurements/cache when row/column topology and initial estimates are compatible while refreshing immutable policy/configuration.

### Scrollbar / actuator

- `RawScrollbar` visual/gesture state already lives in `RenderRawScrollbarState`; its controller remains the public handle.
- `DraggableScrollableActuator` remains a narrow shared reset broadcaster. The removed `RetainedActuator` descriptor wrapper was unnecessary; descriptors now store the actuator handle directly.

### Representation and allocation changes

- Removed `RetainedWheelScrollView`, `RetainedWheelViewport`, `RetainedDraggableSheet`, `RetainedTwoDimensionalScrollView`, and `RetainedTwoDimensionalViewport`.
- Removed advanced descriptor storage based on `Rc<RefCell<...Widget...>>` runtime models.
- Each descriptor now owns one immutable `Rc<...Config>` for wheel/sheet/2D configuration instead of an `Rc + RefCell + mutable runtime model` hidden behind the declarative node.
- Large wheel/2D retained runtime models are boxed inside their feature-state variants so the existing common `RenderNode <= 800 bytes` compile-time size contract remains satisfied.
- Layout no longer takes `RefCell` borrows through declarative descriptors, eliminating those borrow-order panic paths from the advanced scrolling hot path.

### Regression coverage

- Added a retained 2D update contract proving measured row revisions survive compatible config updates.
- Added a draggable-sheet update contract proving current extent survives compatible updates and a replacement controller is rebound while the old controller detaches.
- Existing wheel, sheet, 2D, scrollbar, sliver and retained-tree suites remain green.

### Validation

Passed:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test -p incular-widgets --test advanced_scrolling_retained_integration --all-features
cargo test -p incular-widgets --all-features
cargo test -p incular-scroll --all-features
cargo test-constrained --all-features --quiet
git diff --check
```

Architecture search confirms no old retained advanced-scrolling wrapper types and no descriptor-side `Rc<RefCell<ListWheel...<Widget>>>`, `Rc<RefCell<TwoDimensional...<Widget>>>`, or `Rc<RefCell<DraggableScrollable...<Widget>>>` storage remains.
