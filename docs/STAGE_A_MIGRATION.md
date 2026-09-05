# Stage A API and behavior changes

## GPU surfaces

Renderer and shared GPU constructors now accept `WindowSurfaceTarget` instead
of `RawWindowHandles`. Keep the native window in an `Arc` and pass
`WindowSurfaceTarget::new(window.clone())`. The surface retains that owner for
its entire lifetime, including asynchronous initialization and recreation.
Detached raw handles cannot satisfy this constructor.

## Visibility

Every preservation option is independent and implies retaining the hidden
child. See [Visibility behavior](VISIBILITY.md) for the layout, animation,
semantics, focus, and popup contract.

## Clipboard and shutdown

Desktop clipboard initialization failure is now returned by result-bearing
clipboard operations. It no longer silently selects an in-memory clipboard.
Applications wanting local-only clipboard behavior can explicitly install
`MemoryClipboard` with `Runtime::set_clipboard`.

Global shortcut shutdown closes request admission before draining queued work.
Requests already admitted receive their shutdown result; later requests return
`ApplicationStopped` without entering the queue.

## Checkbox semantics and input

`SemanticState::checked` is now `Option<incular_semantics::CheckedState>`:
`None` means not checkable; `Unchecked`, `Checked`, and `Indeterminate` are
explicit checkable values. Change `Some(value)` for a boolean to
`Some(value.into())`. `Semantics::checked` accepts both booleans and the enum.
The existing `incular_controls::checkbox::CheckedState` import remains valid
through a re-export. AccessKit receives `Toggled::Mixed` for indeterminate
state, and mobile semantic trees preserve that state too.

Checkbox default visuals and custom children share the same action surface.
Enabled, read-only checkboxes remain focusable but cannot activate. Disabled
checkboxes cannot focus or activate. Mixed activation requests `Checked`.
Pointer, keyboard, and semantic activation use this same policy. Enter and
Space activate a focused action surface after keyboard listeners have had the
opportunity to handle the event; nested editors keep their own keyboard input.

## Reentrant navigation guards

Guards may change the navigation stack. Guarded pop and replacement revalidate
the stack revision and candidate identity after calling the guard. If either
changed, the outer pop returns `PopResult::Blocked`, or replacement returns
`None`; the guard's own changes remain. This prevents removing an unapproved
route or accessing a candidate that the guard already removed.
