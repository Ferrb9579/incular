# incular-semantics

## Architecture and support

| Contract | Status |
| --- | --- |
| Ownership | Platform-neutral semantic nodes, roles, labels, state and actions. |
| API class | application; re-exports and path overrides follow the [architecture contract](../../docs/ARCHITECTURE.md). |
| Support | Available; screen-reader projection belongs to accessibility. |

Renderer- and platform-neutral semantic values and the retained semantic tree
for Incular. This crate owns generational semantic node IDs, roles, state,
bounds, supported actions, diagnostics, and the tree lifecycle. It depends only
on `incular-core` and has no widget, renderer, or native accessibility API
dependency.

Widgets derive these values from retained controls, runtimes resolve actions,
and `incular-accessibility` supplies the adapter-facing snapshot and request
contracts. Keeping those responsibilities separate lets native bridges evolve
without coupling semantic state to a particular platform or renderer.

`SemanticsTree::revision()` changes only for retained semantic graph changes
(node/root insertion, removal, property, structure, or geometry updates), not
for paint work alone. Native projections use it to skip unchanged semantic
frames without serializing or hashing the entire tree.
