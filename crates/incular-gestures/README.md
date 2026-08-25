# incular-gestures

Platform-neutral pointer recognizers and interaction state for Incular.

This crate owns pointer event values, a pending/accept/reject/cancel gesture
arena, tap/drag/scale recognizers, pointer hover tracking, focus nodes, and
keyboard shortcut dispatch. It depends only on
`incular-core`; retained widget regions and native event loops adapt these
primitives rather than the other way around.

`GestureDetector::observe` selects an action without calling application code;
the retained `GestureDetector` adapter uses that boundary to wait for
`GestureArena` arbitration.
Direct `GestureDetector::handle` remains available for standalone use. Arena
keys include both window and pointer identity. Scale members explicitly opt
into simultaneous compatibility, while tap, long-press, pan, and directional
drag members remain exclusive and receive `GestureCallbacks::on_cancel` when
they lose.
