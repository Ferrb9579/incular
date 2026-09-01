# Plan 12 - Tray/Status Items, Notifications, Taskbar, and Dock Integration

## Goal

Provide application-shell integrations that live outside normal windows/widgets:

- Windows/Linux system tray and macOS status items;
- desktop notifications;
- taskbar/Dock progress, badge, overlay/attention integration where supported;
- native tray/status menus using the same portable menu command model where possible.

## Architecture

### Application service resources

Use stable generational handles:

- `TrayItemId`/handle;
- `NotificationId`/handle where updates are supported;
- taskbar/Dock state associated with a window/application as appropriate.

These are runtime/application resources, not widgets.

### Tray/status item

Portable model should cover:

- icon;
- tooltip/title where supported;
- visibility;
- click/activation callbacks;
- optional menu tree.

Reuse the semantic platform menu snapshot types where they fit. Do not duplicate menu item IDs/callback semantics just because the menu is attached to a tray icon.

### Notifications

Model a conservative portable core:

- title/body;
- optional icon;
- stable notification identity/update/dismiss where supported;
- activation/action callbacks only through an explicit typed action model.

Do not build a fake in-app toast and call it a desktop notification.

### Taskbar/Dock

Expose capability-based operations such as:

- progress state/value;
- badge count/text where supported;
- overlay icon if semantically portable enough;
- user attention should reuse Plan 02's window/application attention abstraction.

Platform-specific extras may live in OS extension modules rather than polluting the portable core.

## Crate ownership

- `incular-platform`: portable service contracts/capabilities.
- `incular-runtime`: resource handles/callback dispatch.
- OS crates: native implementations.
- `incular-desktop`: common coordination only.

## Hard invariants

1. Service resources have deterministic teardown.
2. Native callbacks route by stable IDs, never raw pointers.
3. Tray menus reuse menu semantics instead of forking them.
4. Unsupported taskbar/Dock features return capability errors.
5. Notification clicks/actions are delivered on runtime/UI coordination path.

## Tests

- Create/update/drop tray item lifecycle.
- Tray menu command dispatch and stale ID rejection.
- Notification show/update/dismiss/activation.
- Taskbar progress state transitions.
- Backend capability matrices for Windows/macOS/Linux sessions.
- Opt-in live native smoke tests.

## Acceptance criteria

Background/helper applications can live naturally in the desktop shell without requiring their own native integration layer.
