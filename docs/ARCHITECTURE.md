# Architecture contract

This is the normative ownership and dependency contract for the pre-1.0
consolidation campaign. `API_DESIGN.md` defines API behavior; `AGENTS.md` and
crate READMEs summarize this contract. Historical plans and parity inventories
are evidence, not competing architecture rules. Changes to a boundary must
update its decision, manifest, migration note, and tests together.

## Dependency direction

An arrow means “uses”. Re-exporting a value does not transfer its ownership.

```text
application -> incular facade -> runtime + widgets + optional controls/material
material -> controls -> widgets -> domain mechanisms
navigation -> widgets (route composition remains here until Stage G)
native OS adapters -> desktop -> runtime + WGPU + platform
runtime -> widgets + accessibility + platform
WGPU -> rendering + image/assets + platform surface values
accessibility -> semantics -> core
text -> rendering + assets + config -> core
layout -> config -> core
rendering -> image/assets/core; image -> config/core
animation/gestures -> core; scroll -> config/core
DevTools transport -> runtime + protocol; desktop -> DevTools transport
```

`specs/architecture.json` records every crate's current allowed direct normal
and build dependencies, across all target conditions and optional features.
Tests compare Cargo metadata against that allow-list, reject cycles, and
restrict WGPU, Winit and AccessKit dependencies to their boundary crates.
Dev dependencies are excluded so integration tests can compose layers.
Removing an edge is allowed; adding one requires an explicit contract change.
This is a reviewed boundary, not a generated acceptance of whatever Cargo says.

Stage E will remove Winit from platform. Its existing edge is migration debt,
not permission to introduce more portable Winit APIs. Stage G will consolidate
navigation state without adding `widgets -> navigation` while the reverse edge
exists. Extract widget-independent route values into core only if needed; keep
route composition above widgets. WGPU must never depend on desktop. No new crate
is justified solely by file length.

## Ownership and support

The complete crate matrix is in `specs/architecture.json`; each crate README
contains its row. Support describes implemented responsibility, not feature
completeness or validation on every OS.

- **Core/config/layout:** core owns geometry, identity and lower-level reactive
  values. Config owns constraints, alignment, insets and shared policies. Layout
  re-exports those same values and owns algorithms over measured sizes. Widgets
  own retained child measurement and layout execution.
- **Assets/image/text:** assets owns font handles and bytes today. Image owns
  raster identities, decoding and CPU caches. Text owns font selection, shaping,
  metrics and editing values. No generic all-resource loader is promised by assets.
- **Rendering/painting/WGPU:** rendering owns canvas commands and compositor
  layers. Painting is a re-export shim with no implementation. WGPU owns device
  resources and command execution; native surfaces retain their window owner.
- **Semantics/accessibility:** semantics owns platform-neutral nodes, roles,
  actions and checked state. Accessibility projects them to AccessKit/mobile
  bridges. Native adapters own screen-reader integration and host lifecycle.
- **Widgets/controls/material:** widgets own neutral retained primitives.
  Controls are themed Incular controls with replaceable visual slots, not a
  headless library. Material composes them and neutral primitives. Interaction,
  editing and semantics must have one behavioral owner per control.
- **Runtime/platform/desktop/native:** runtime schedules UI work. Platform owns
  portable native contracts. Desktop owns Winit translation and host coordination.
  OS crates implement native services. Android/iOS currently provide semantic
  adapters, not complete standalone application hosts.

## API classes

Every exported type, function and generated builder inherits its crate's
`default_api_class` in the matrix, with the longest matching public-path override
in `api_overrides` taking precedence. This covers new exports, enum variants and
associated members without treating missing inventory entries as permission.
Re-exports inherit the defining API's class; facade and prelude paths never
promote a bridge to an application API. An explicit alias override may narrow
support but cannot broaden the original contract.

| Class | Consumers and obligation |
| --- | --- |
| application | Supported application composition, values and controllers; public changes require a migration note and behavioral coverage. |
| backend | Renderer, platform, custom rendering and diagnostic integrations; document thread, identity, lifetime and completion invariants. |
| bridge | Cross-crate implementation transport; coordinate all workspace consumers when changed. It has no application compatibility promise. |

`#[doc(hidden)] pub` remains callable Rust API. Hiding rustdoc is not access
control. Keep bridges narrow, prefer `pub(crate)` when consumers permit it, and
do not use them in new application examples. Existing exceptions and duplicate
paths are tracked in [API migration inventory](API_MIGRATIONS.md).

## Mutation outcome and ownership contract

Every public mutation, including generated setters and public fields, falls
under one of these contracts. Type-specific rustdoc must state deviations;
known incomplete implementations are listed in the migration inventory rather
than described as already corrected.

| Mutation family | Owner and observable outcome | Failure, lifetime and cancellation |
| --- | --- | --- |
| Descriptor builder / public value field | Caller owns a local value. Returning `Self` configures the next description; it does not schedule a frame or mutate an already mounted tree. | Document normalization. Validated values must funnel through checked construction; existing writable invariant fields migrate in G. |
| Signal / controller `set`, `update`, edit, focus | Retained owner holds state; a successful write updates that state and its defined observers. Unit return means completion of a synchronous state change, not native success. | UI-thread affinity unless explicitly Send/Sync. Define equality, reentrancy and notification order; unify subscription cleanup in C/G. |
| Registration / listener / task | Scope or returned token owns registration and cleanup. Numeric manual listener IDs are legacy explicit ownership, requiring removal by that owner. | Specify unmount/drop behavior. Task cancellation must resolve or drop its owned completion; no silent orphan. Migration C/D. |
| Native request / async operation | Typed request and result belong to the originating application/window generation. Completion is distinct from queue admission. | Unsupported, cancelled, stopped, stale and backend failures have typed outcomes; document whether dropping the future cancels native work. D unifies remaining service paths. |
| Route / collection operation | Owning navigator or collection serializes mutation; returned Option/enum identifies applied, absent or blocked work. | Revalidate after external callbacks. Stage A defines reentrant guarded navigation. Never hold mutable state across user callbacks. |
| Backend tree / frame / cache mutation | Host owns thread, device and tree lifetime. Invalidation identifies affected phases; resource release follows the actual cache owner. | Return typed runtime failures. Invariant assertions are programmer errors with a stated invariant. H completes reclamation and device failure policy. |

Do not add a success boolean that conflates “queued”, “applied”, “unsupported”
and “failed”. A boolean is appropriate for a genuinely binary observation or
policy, such as enabled state. Do not replace every boolean with an enum.

## Compatibility

Flutter is a pinned reference inventory, not Incular's dependency graph or a
promise to implement every Dart member. Existing implemented behavior remains
protected by tests. Signals, explicit contexts, owned values and futures are
intentional Rust decisions. Keep Material out of the base prelude.

Every audited member has an explicit status: implemented, rustified, merged,
internal, deferred, or deliberately omitted. Non-implemented rows need a reason;
deferred rows need an owner and prerequisite; changed implemented claims need a
decision and evidence. Unreviewed rows cannot be labeled implemented to satisfy
a percentage gate. Existing manifests retain their documented status spellings.
The member manifest gate accepts these outcomes and rejects missing/unknown
states. Stage I revalidates the historical claims against behavior.

New Widgets root names may be in the pinned graph or have a reviewed Incular
extension entry in `specs/widgets_api_extensions.json`. Such entries require
an owner, reason and real test evidence. The neutral/style and private retained
taxonomy guards still apply. Namespace admission is not evidence of behavior.
The initial ledger records 163 existing names in 19 responsibility families.
The previous string-based guard skipped these exports; the replacement parses
Rust visibility, grouped imports and aliases before comparing the inventory.

## Decisions

The eight accepted decisions and their implementation/removal conditions are
recorded in [Architecture decisions](ARCHITECTURE_DECISIONS.md).
