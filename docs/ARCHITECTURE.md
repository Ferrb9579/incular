# Incular architecture

This is the single human-readable architecture contract. Public API behavior is
defined in `../API_DESIGN.md`; the machine-checked crate graph and API classes
live in `../specs/architecture.json`.

## Dependency direction

```text
application -> incular facade -> runtime + widgets + optional controls/material
material -> controls -> widgets -> domain crates
runtime -> widgets + accessibility + platform
native OS crates -> desktop -> runtime + wgpu + platform
wgpu -> rendering + image/assets + platform
accessibility -> semantics -> core
text -> rendering + assets + config -> core
layout -> config -> core
animation / gestures / scroll -> core/config
devtools -> runtime + protocol
```

Rules:

- Re-exporting a type does not transfer ownership.
- `platform` stays free of Winit; Winit/raw-window translation belongs to
  `desktop`.
- `wgpu` never depends on `desktop`.
- Material stays optional and does not own input, focus, editing, scrolling, or
  semantics.
- Do not add a crate just to shorten files or bypass a dependency boundary.
- Adding a dependency edge requires updating `specs/architecture.json` and its
  architecture tests.

## Ownership

- **core/config/layout** — identities, geometry, reactive primitives, shared
  configuration values, and pure layout algorithms.
- **assets/image/text** — font bytes, raster decoding/cache, shaping, metrics,
  editing values, and text layout.
- **rendering/wgpu** — display lists/compositor policy and GPU execution/resources.
  `incular-painting` is only a compatibility re-export.
- **semantics/accessibility** — platform-neutral semantics first; native/mobile
  projection second.
- **widgets** — retained neutral UI primitives and tree behavior.
- **controls/material** — presentation and composition over existing behavioral
  owners; custom visuals must not create a second interaction engine.
- **runtime** — scheduling, application/window coordination, tasks, and retained
  service ownership.
- **platform/desktop/native** — portable contracts, shared desktop host, then
  OS-specific implementation.
- **devtools** — bounded diagnostics/tooling bridges; never an application-state
  owner.

## Non-negotiable design rules

1. **One reactive dependency engine.** Runtime schedules work; widgets identify
   consumers. Do not create parallel signal/invalidation systems.
2. **One desktop host.** Single- and multi-window entry points use the same event
   loop, input path, rendering path, and teardown rules.
3. **Portable native contracts.** Public/runtime APIs use Incular types; Winit,
   raw handles, AppKit/Win32/X11/Wayland details stay at backend boundaries.
4. **One native-request lifecycle.** Admission, completion, cancellation,
   shutdown, stale-generation handling, and unsupported results are typed and
   observable.
5. **One behavioral owner.** Replacing presentation must not replace focus,
   activation, editing, scrolling, selection, or semantic ownership.
6. **One invalidation vocabulary.** Build/layout/paint/composite/semantics/hit-test
   work is driven by one authoritative property-change contract.
7. **Caches have real owners and bounds.** A local cache drop must not claim a
   shared GPU resource was freed. Lifetime, budget, eviction, and in-flight use
   belong to the actual owner.
8. **Rust semantics beat superficial Flutter parity.** Flutter is a reference
   vocabulary, not Incular's dependency graph or a requirement to reproduce Dart
   lifecycle/API structure.

## API classes

- **application** — supported app-facing composition, values, and controllers.
- **backend** — renderer/platform/native integration; lifetime/thread/identity
  invariants must be explicit.
- **bridge** — cross-crate implementation transport; no application compatibility
  promise.

Re-exports keep the defining API's class. `#[doc(hidden)] pub` is still public
Rust API; use `pub(crate)` when external visibility is unnecessary.

## Mutation and lifetime rules

- Builders/public value fields configure values; they do not mutate an already
  mounted tree.
- Signals/controllers mutate their authoritative owner and define equality,
  notification order, and reentrancy.
- Registrations/tasks are owned by a scope or returned token and clean up on
  drop/unmount/shutdown as documented.
- Native async operations distinguish queued, applied, unsupported, cancelled,
  stopped, stale, and failed outcomes. Do not collapse them into one boolean.
- Never hold mutable domain state across user callbacks; revalidate state after
  reentrant callbacks before committing an outer operation.
- Backend/frame/cache failures are typed runtime failures; invariant violations
  are programmer errors with a stated invariant.

## Compatibility and support

- Keep only one implementation per concept; aliases/re-exports may exist when
  they preserve identical semantics.
- Unsupported platform behavior must be explicit rather than silently faked.
- Android/iOS support remains limited to the implemented adapters unless a full
  host exists.
- `incular-macros` is intentionally empty until a concrete macro requirement
  exists.

Historical plans, audits, migration notes, and completion reports belong in Git
history, not in the active documentation set.
