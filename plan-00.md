# Desktop Platform Completion Campaign

## Goal

Finish Incular's desktop/OS integration so the framework is not only a strong retained renderer/widget system, but a professional desktop application framework.

This campaign intentionally focuses on platform boundaries rather than adding more ordinary widgets.

## Architectural direction

Keep the existing crate ownership rules:

- `incular-core`: platform-neutral primitive values only.
- `incular-config`: stable application/environment policy.
- `incular-widgets`: declarative widget APIs and retained semantics only.
- `incular-runtime`: application/window coordination and command completion.
- `incular-platform`: portable platform contracts, capabilities, commands, events, errors.
- `incular-desktop`: shared Winit/WGPU desktop shell and common desktop behavior.
- `incular-windows`, `incular-macos`, `incular-linux`: native OS integrations that cannot be expressed portably through Winit.
- `incular-wgpu`: rendering and shared GPU resources; no window-manager policy.

Platform-specific types must not leak into public widget/runtime APIs. Native handles stay at the platform/backend boundary.

## Hard rules

1. No fake support. Unsupported operations return typed capability/operation errors or expose an unsupported capability; never silently pretend success.
2. No platform flag soup. Model semantic operations such as `begin_move_drag`, `show_popup`, or `register_global_shortcut`, then translate them in the backend.
3. No duplicate behavior. One portable semantic model, one backend implementation per OS where necessary.
4. No child-window hacks for popups. Use the platform's real transient/popup mechanism where available.
5. No manual desktop-coordinate dragging/resizing. Delegate interactive move/resize to the compositor/window manager.
6. Wayland limitations are first-class. Never promise global window positioning or other capabilities Wayland intentionally does not expose.
7. Preserve retained identity. Window, popup, menu, drag session, shortcut, tray item, and activation identities must be generational/stable rather than raw native IDs.
8. Runtime failures use typed `Result`; programmer invariant violations remain immediate errors according to `API_DESIGN.md`.
9. Public APIs remain Flutter-like where the concept is shared, but desktop-specific facilities should follow mature desktop semantics rather than forcing Flutter abstractions where they do not fit.
10. Prefer mature crates/native APIs instead of reimplementing OS protocols.

## Plans and dependency order

1. `plan-01.md` - Platform capability, command, and error foundation.
2. `plan-02.md` - Window control and custom chrome.
3. `plan-03.md` - Displays, work areas, and window placement.
4. `plan-04.md` - Pointer, mouse, and cursor completion.
5. `plan-05.md` - True native transient surface hosting.
6. `plan-06.md` - Popup placement, focus, dismissal, and accessibility.
7. `plan-07.md` - Native application menus.
8. `plan-08.md` - Rich clipboard and external drag/drop.
9. `plan-09.md` - Native file dialogs and document/file integration.
10. `plan-10.md` - System environment, preferences, lifecycle, and occlusion.
11. `plan-11.md` - Activation, deep links, single-instance behavior, and global shortcuts.
12. `plan-12.md` - Tray/status items, notifications, taskbar, and Dock integration.
13. `plan-13.md` - Advanced pen, trackpad, and pointer-device input.

Plans 02-04 can proceed after Plan 01. Plans 05-06 must remain sequential. Plans 07-13 may proceed independently once Plan 01's platform-result/capability contract is stable, except where explicitly noted.

## Campaign-wide acceptance criteria

- No new native handles or Winit types in public widget/runtime APIs.
- Capability differences are queryable/testable.
- Unsupported operations are deterministic and typed.
- Every new retained/native resource has explicit ownership, teardown, and stale-ID behavior.
- Multiple Incular windows do not share mutable per-window state accidentally.
- Platform callbacks are normalized into portable events before reaching runtime/widgets.
- Live native tests are isolated behind explicit opt-in where a display/session is required; normal workspace tests remain headless-safe.
- Windows/macOS/Linux compile paths stay clean even when a feature is unsupported on one OS.
- `cargo fmt --all -- --check`
- `cargo check --workspace`
- `cargo test-constrained`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `RUSTDOCFLAGS=-D warnings cargo doc --workspace --all-features --no-deps`
- `cargo machete --with-metadata`
- `git diff --check`

## Completion report required for every plan

Report:

- architecture/API changes;
- platform-specific behavior and intentionally unsupported cases;
- tests added and live-native coverage performed;
- validation results;
- remaining limitations that belong to a later numbered plan;
- commit hash once implementation is committed.
