# W7 completion report — controls and Material presentation ownership

Date: 2026-09-19.

W7 started from W6 commit `b65751b` and the executable plan committed as
`ac6e1aa`. Implementation proceeded through coherent, validated slices and is
closed by the commit containing this report. The portable source implementation
settled at `0410931` (`Complete remaining W7 presentation policies`) before the
final ledger/documentation close.

## Result

Controls and Material now act as presentation/configuration layers over the
existing retained owners instead of creating parallel input, focus, editing,
scrolling or semantic engines.

- Theme resolution is sparse and deterministic. Explicit style values beat
  component themes; state tables retain every control-state dimension; family
  theme projection does not leak checkbox/slider colors into unrelated controls.
- `ControlTheme::colors` is the canonical color store. `palette()` and
  `with_palette` preserve the compatibility vocabulary without a duplicate
  stored field or typography reset.
- Retained action presentation exposes live combined interaction state from the
  existing action owner. Default and custom visuals share that owner.
- Buttons execute accepted layout, icon, state, cursor, splash/feedback,
  elevation/tint and tap-target policies.
- Selection controls and sliders share neutral controls-layer behavior. Range
  dragging uses gesture-origin values, both thumbs are keyboard/semantic
  targets, and Material radio groups invalidate mounted consumers.
- Material text fields remain wrappers around the single retained editor. Forms
  use the widgets Form registry; compound controls bind configuration to real
  retained models/editors rather than presentation-only placeholders.
- Tabs use measured page geometry and retained animation; menus execute overlay,
  clip, constraint, cursor and open/close transition policy.
- Progress indicators execute theme precedence and retained indeterminate
  motion. `AnimatedTheme` is frame-driven and retargets from the currently
  presented theme.
- Material implementation is organized by behavior owner; the private
  `component_impl`, `p0_controls` and `foundation` buckets are retired.

## Exhaustive W7 surface ledgers

W7 adds source-derived qualified API ledgers rather than a hand-maintained list:

- controls: **172 public symbols / 619 accepted options**;
- Material: **221 public symbols / 1,299 accepted options**.

The scanner covers public declarations/fields, fluent methods and constructors,
and generated `TypedBuilder` setters including transformed and prefixed setters.
An unrecognized builder grammar fails discovery rather than silently omitting an
option. The four snapshots are verified by `w7_public_surface`,
`w7_controls_ledger`, `w7_material_ledger`, and `w7_theme_ledger`.

## Focused executed validation

The final W7 tree passed the planned focused matrix:

- `w7_control_behavior`, `w7_material_behavior`, `w7_theme_invalidation`.
- Full `incular-controls` and `incular-material` all-feature package suites.
- Widgets `action_presentation`, `inherited_context_contracts` and
  `tree_performance_contracts`.
- Runtime `runtime` (**200 tests**), `semantic_action_dispatch`,
  `text_input_gating` and `performance_contracts`.
- Root `architecture_contract`, `test_placement`, `facade_surface`,
  `material_3471_surface` and `material_p0_stress`.
- Warning-denied all-target/all-feature Clippy for controls, widgets and Material.
- ThemeData small-stack/COW safety, W6 editing/formatter behavior and retained
  performance tests stayed green in those package/runtime runs.

## Repository-wide closure validation

The final source tree and closure ledgers were validated with the repository
gates after the focused matrix:

- `cargo check --workspace` — **PASS**.
- `cargo test-constrained` (`cargo test --workspace --jobs 12`) — **PASS**,
  including workspace doctests and every legacy/W7 property ledger.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` —
  **PASS**.
- `cargo test --workspace --jobs 12 --all-features` — **PASS**. This is the
  exact command underlying `cargo test-constrained --all-features`; the alias
  invocation itself was blocked by the execution layer before Cargo started,
  so the underlying command was run directly.
- `cargo check --workspace --all-targets --all-features` — **PASS**.
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps`
  (with the previous environment restored afterward) — **PASS**.
- `git diff --check` — **PASS**.

The exact `cargo fmt --all -- --check` gate cannot execute on this Windows host:
Cargo/rustfmt returns OS error 206 (`The filename or extension is too long`). It
fails the same way from a short `subst` drive, so shortening the repository path
does not change the host limitation. Formatting was therefore verified by
enumerating Cargo metadata and running `cargo fmt --manifest-path <manifest>
-- --check` for **all 32 workspace members**; every member passed. No build
configuration or workspace membership was changed to work around the host.

The first repository-wide test runs also caught two older ledger records that
needed to follow W7's new neutral widget properties:

- `EditableText::placeholder_color` and `EditableText::focused_border` were
  added to `specs/editable_text_properties.json`;
- `OverlayPortal::root_overlay` was added to
  `specs/overlay_tooltip_properties.json`.

After those records were added, every root `*ledger.rs` target was run in one
sweep and passed before the final constrained suites were rerun successfully.

Desktop live-native tests that explicitly require
`INCULAR_DESKTOP_LIVE_TESTS=1` remained skipped by the normal constrained
workspace run. W7 does not convert those skips into native verification.
