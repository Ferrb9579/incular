# incular-gestures

Platform-neutral pointer recognizers and interaction state for Incular.

This crate owns pointer event values, tap/drag/scale recognizers, pointer hover
tracking, focus nodes, and keyboard shortcut dispatch. It depends only on
`incular-core`; retained widget regions and native event loops adapt these
primitives rather than the other way around.
