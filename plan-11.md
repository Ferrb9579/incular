# Plan 11 - Application Activation, Deep Links, Single Instance, and Global Shortcuts

## Goal

Add an application-level activation model for events that arrive independently of a particular widget/window input stream.

Cover:

- protocol/deep-link activation;
- OS open-file/open-URL events;
- secondary-launch activation/single-instance forwarding;
- global shortcuts/hotkeys.

## Architecture

### Activation model

Define typed application activations in `incular-platform`, e.g. semantic variants for:

- launch with arguments/files;
- open files;
- open URLs/deep links;
- reopen/activate existing app;
- global shortcut invoked.

Runtime owns delivery/subscription and decides when to open/focus a window. Widgets/router can consume an activation but should not own native registration.

### Deep-link/router bridge

Reuse the existing router's platform-route/deep-link semantics. Add a deliberate adapter from application activation -> route information rather than duplicating route parsing in desktop code.

### Single-instance

Provide optional application policy, not an unconditional framework restriction.

A mature single-instance implementation must:

- acquire one app-specific instance identity/lock;
- authenticate/validate local forwarding enough to avoid accidental cross-app messages;
- forward activation payloads to the primary process;
- wake the runtime and deliver them in order;
- fail safely after crashes/stale locks.

Prefer mature local IPC/locking crates/native facilities over inventing a custom network protocol.

### Global shortcuts

Model stable `GlobalShortcutId` + key chord registration. Registration is application-level and returns typed conflict/unsupported errors.

Do not reuse focused-window `Shortcuts` as the native registration mechanism; both may share key-chord vocabulary but have different ownership/lifecycle.

## Hard invariants

1. One activation event is delivered once, in order.
2. Secondary-process forwarding never mutates UI directly from an IPC thread.
3. Global shortcut invocation cannot double-fire a focused local shortcut unless explicitly designed.
4. Registrations are released on drop/shutdown.
5. OS-specific URL/file registration mechanics remain packaging/platform concerns; runtime only handles received activations.
6. Deep-link parsing remains in the router/application route layer.

## Tests

- Activation ordering and buffering before first window exists.
- Deep link reaches existing router path.
- Open-file activation reaches application with exact paths.
- Simulated second instance forwards activation and exits according to policy.
- Stale primary endpoint recovery.
- Global shortcut conflict/unsupported/registration/unregistration.
- Multiple shortcuts dispatch by stable ID.

## Acceptance criteria

An app such as Rambler can register a global hotkey, activate from the background, receive deep links/files, and optionally enforce single-instance behavior without bespoke OS code.
