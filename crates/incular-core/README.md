# incular-core

Platform-independent foundation types for Incular. This crate is intentionally
free of windowing and renderer dependencies.
# incular-core

Owns dependency-free geometry, colors, normalized input events, phase dirty
flags, and the generational `Arena`. It does not own tree semantics or layout
policy. Arena IDs are stale after removal, even if the slot is reused.
