# Plan 07 - Native Application Menus

## Goal

Connect the existing `PlatformMenuBar` retained model to real OS application-menu backends.

Do not redesign the portable menu tree unless implementation exposes a genuine semantic gap.

## Current foundation

Already present:

- stable `MenuOwnerId` and `MenuItemId`;
- nested `PlatformMenu` tree;
- groups/dividers;
- shortcuts;
- enabled state;
- callbacks;
- open/close callbacks;
- deterministic snapshot/update ownership;
- `PlatformMenuDelegate` abstraction.

Missing: production delegates.

## Architecture

### Backend ownership

Native menu code belongs in OS crates:

- `incular-macos`: NSMenu/application menu integration;
- `incular-windows`: native Win32 menu/application command integration where appropriate;
- `incular-linux`: capability-dependent desktop integration; do not claim a single universal Linux global-menu model.

`incular-desktop` may coordinate installation/dispatch but must not accumulate OS-specific menu APIs.

### Stable command mapping

Each backend maps retained `MenuItemId` to native command identifiers through a generation-safe registry. Native command IDs are never application-visible.

Incremental updates should preserve native item identity when semantically unchanged, avoiding whole-menu teardown for simple enabled/label changes where the platform permits.

### Shortcut authority

Define whether a native application menu shortcut or Incular shortcut system owns invocation to prevent double firing. One command action must execute exactly once.

### Platform conventions

Respect native conventions rather than forcing identical visual menu structures:

- macOS application menu/location conventions;
- standard roles such as Quit/About/Preferences where modeled;
- keyboard accelerator representation;
- enabled/check state if added.

If additional portable semantic roles are needed, add them explicitly rather than parsing labels such as "Quit".

## Hard invariants

1. One active top-level menu owner according to existing ownership model.
2. Native callbacks dispatch by stable ID, never by label/index.
3. Updating a menu cannot dispatch stale callbacks from a previous generation.
4. Unsupported Linux environment returns `NoOpUnsupported`/typed capability status honestly.
5. Native menu shortcuts do not double-invoke Incular actions.
6. Drop/detach clears native menu ownership.

## Tests

- Snapshot -> native command map roundtrip in platform adapter tests.
- Incremental label/enabled/submenu updates.
- Duplicate IDs rejected before backend mutation.
- Native selection/open/close invokes current callback only.
- Replaced menu tree invalidates old command mapping.
- App with multiple windows obeys documented application-menu ownership policy.
- Live native smoke tests where platform automation permits.

## Acceptance criteria

`PlatformMenuBar` can create a real platform application menu without application code supplying a delegate or native handles.
