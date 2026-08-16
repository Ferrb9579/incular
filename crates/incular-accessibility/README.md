# incular-accessibility

Native accessibility adapter contracts for Incular. It owns immutable semantic
snapshots, action requests, and the `SemanticsAdapter` contract used by native
OS bridges. The canonical roles, state, actions, generational node IDs, and
retained semantic tree live in `incular-semantics`, which this crate re-exports
for source compatibility.
