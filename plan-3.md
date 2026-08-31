# Plan 3 - Introduce typed compositor attachments and layer ownership

## Goal

Replace the current collection of optional compositor-layer fields and positional tuples with an explicit typed representation of the compositor subtree owned by each render object.

## Current problem

Mounting currently builds a large tuple containing fields such as clip/content/opacity/blur/shadow/color-filter/blend/shader-mask/backdrop/annotation/leader/follower layers. Later code reconstructs similarly large tuples to update, synchronize and remove those layers.

This is fragile because:

- positional tuples have no semantic names at the call boundary;
- invalid combinations are representable;
- layer creation, child attachment, update and teardown logic are scattered;
- adding an effect adds fields and branches in several locations;
- ownership is manual, increasing the chance of orphaned or double-managed layers.

## Target architecture

Represent compositor ownership explicitly.

```rust
pub(crate) struct RenderLayers {
    root: LayerId,
    picture: Option<LayerId>,
    focus_picture: Option<LayerId>,
    attachment: LayerAttachment,
}

pub(crate) enum LayerAttachment {
    Direct,
    Transform { layer: LayerId },
    Clip { layer: LayerId, content: LayerId },
    Opacity { layer: LayerId },
    Blur { layer: LayerId },
    DropShadow { layer: LayerId },
    ColorFilter { layer: LayerId },
    Blend { layer: LayerId },
    ShaderMask { layer: LayerId },
    BackdropFilter { layer: LayerId },
    Annotation { layer: LayerId },
    Leader { layer: LayerId },
    Follower { layer: LayerId },
}
```

The exact shape may use nested typed structs where a render object genuinely owns a chain. The key invariant is that impossible attachment combinations should not be modeled as 12 independent `Option<LayerId>` values.

## Ownership API

Create one compositor attachment lifecycle API responsible for:

- constructing the owned layer subtree;
- connecting render-child roots in painter order;
- updating effect/transform properties;
- removing all owned layers;
- exposing the correct content insertion point;
- exposing picture/focus-picture placement.

Example conceptual API:

```rust
impl RenderLayers {
    fn create(compositor: &mut LayerTree, spec: LayerSpec) -> Self;
    fn set_render_children(&self, compositor: &mut LayerTree, children: &[LayerId]);
    fn update(&mut self, compositor: &mut LayerTree, update: LayerUpdate);
    fn remove(self, compositor: &mut LayerTree);
}
```

Do not implement `Drop` that mutates `LayerTree` through global/shared state. Teardown should remain explicit and deterministic because the compositor owns the actual arena.

## Separate specification from retained IDs

Use a declarative `LayerSpec`/`CompositingSpec` produced by render behavior and a retained `RenderLayers` holding actual IDs.

This lets update logic answer whether the structure is compatible:

- same structural spec -> mutate retained layer properties;
- structural spec changed -> rebuild the local attachment subtree while preserving the render-node identity and child roots.

Do not encode structural compatibility with ad-hoc `if let Some(layer)` checks spread through reconciliation.

## Painter order

Preserve current explicit painter-order rules:

- normal picture position;
- child roots;
- focus picture when applicable;
- Banner-specific ordering;
- IndexedStack selected-child behavior;
- pinned sliver ordering;
- clip/content nesting;
- effect and follower/leader wrapping semantics.

Encode exceptional ordering as named attachment behavior or a small child-order policy, not another unrelated tuple flag.

## Migration sequence

1. Add `RenderLayers` wrapping the existing fields without changing behavior.
2. Replace positional tuple construction/extraction with named construction.
3. Add `LayerAttachment` and migrate simple direct/transform/opacity cases.
4. Migrate clip and effect attachments.
5. Migrate leader/follower/annotation/shader/backdrop cases.
6. Centralize `set_children` policy in `RenderLayers`.
7. Centralize teardown in `RenderLayers::remove`.
8. Delete old optional layer fields and duplicated reconciliation branches.

## Hard invariants

- every compositor layer created for a render object has one explicit owner;
- removing a render object removes all and only its owned layers;
- child render roots are owned by their child render nodes, never by a parent's attachment object;
- no layer ID exists simultaneously in two ownership fields;
- structural effect changes cannot leave stale old layers attached;
- compositor-only property updates remain compositor-only;
- layer ordering is deterministic.

## Tests

Add focused structural tests against `LayerTree` diagnostics/snapshots:

- direct node hierarchy;
- scroll clip -> content hierarchy;
- opacity/effect wrappers;
- transform updates reuse the same retained layer ID;
- structural effect replacement removes obsolete layer IDs;
- Banner picture/shadow ordering;
- focus picture ordering;
- leader/follower structure;
- IndexedStack and pinned-sliver child order;
- unmount leaves no owned layer reachable or retained.

Add a stress test repeatedly replacing effect types and mounting/unmounting the subtree, asserting stable compositor node count.

## Acceptance criteria

- no large positional compositor tuples remain;
- `RenderObject`/`RenderNode` does not contain a long list of independent layer `Option`s;
- layer create/update/connect/remove behavior has one owner;
- effect structural compatibility is explicit;
- existing compositor-only update performance remains intact;
- all layer lifecycle tests pass.

## Validation

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test -p incular-rendering
cargo test -p incular-widgets
cargo test --workspace --all-features
```

## Completion report

Include diagrams/examples of old vs new layer ownership for Scroll, Opacity, Blur and Banner, plus layer-count stress-test results.
