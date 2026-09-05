# Visibility behavior

`Visibility` resolves removal before creating the retained widget description.
Any preservation option implies retention, so all combinations accepted by the
fluent API and generated builder have defined behavior. `replacement` is used
only when hidden with no preservation option; it defaults to `SizedBox::shrink()`.

| State | Mounted child | Child measurement | Space occupied | Paint / pointer hits | Animation ticks | Semantics | Focus / keyboard |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Visible | Yes | Normal | Measured size | Normal | Normal | Normal | Normal |
| Hidden, no preservation | No | Replacement only | Replacement size | Replacement only | Replacement only | Replacement only | Replacement only |
| Hidden, retained offstage | Yes | Normal | Zero, constrained by parent minimums | None | Only with `maintain_animation` | Only with `maintain_semantics` | Retained |
| Hidden, `maintain_size` | Yes | Normal | Measured size | None | Only with `maintain_animation` | Only with `maintain_semantics` | Retained |
| `Offstage` with `offstage = true` | Yes | Normal | Zero, constrained by parent minimums | None | Normal | None | Retained |

`maintain_state`, `maintain_size`, `maintain_animation`, and `maintain_semantics`
each independently keep the child mounted. Size preservation does not imply
animation or semantics preservation. Removing all preservation options while
hidden removes the child and uses the replacement. A retained child keeps its
element identity across visibility changes when its kind and key remain stable.

Retained focus matches the previous behavior: hiding does not remove the child
from keyboard traversal or revoke focus. Compose `ExcludeFocus` when hidden
content must not receive keyboard input. Semantics preservation exposes the
measured descendant geometry even when its wrapper occupies zero space.

Animation muting applies to tree-driven controllers and sliver delegates. It
suppresses ticks and their continuous-frame request, without freezing the
animation timeline. Showing the child catches up to elapsed animation time.
An outer muted wrapper takes precedence over an inner animation opt-in.
Controllers shared with visible content can still advance there, and explicit
controller updates still synchronize retained compositor properties.

Cached pictures remain owned while hidden but are disconnected from the
compositor output. Showing reconnects them and paints any accumulated changes.
Hidden ancestors also suppress popup surface snapshots and the separate popup
hit-test path, including nested popup chains.

## Migration

Previously `maintain_size` kept an element mounted but did not preserve its size;
animation and semantics options were discarded. Applications that relied on
hidden content continuing to animate should set `maintain_animation(true)` or
use `Offstage`. Offstage now measures its child even while hidden. The public
diagnostic `RenderKind::Visibility` variant now includes `maintain_size`; match
with `..` when only inspecting visibility.

Regression coverage: `crates/incular-widgets/tests/visibility.rs` exercises all
16 preservation combinations through both construction APIs, hide/show identity,
cached paint, layout, focus, semantics, hit testing, and animation gating.
