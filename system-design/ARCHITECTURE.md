# Incular architecture

Incular is a retained-mode Rust GUI framework. This document defines ownership,
dependency direction, and runtime boundaries. Public API shape is defined in
[API_DESIGN.md](API_DESIGN.md). The machine-checked package graph lives in
`../specs/architecture.json`.

## Layering

```text
application
  -> incular facade
     -> runtime
     -> widgets
     -> controls / material (optional)

material -> controls -> widgets
runtime -> widgets + accessibility + platform
desktop -> runtime + wgpu + platform
windows / macos / linux -> desktop
wgpu -> rendering + image/assets + platform
accessibility -> semantics -> core
text -> rendering + assets + config -> core
layout -> config -> core
animation / gestures / scroll -> core/config
devtools -> runtime + protocol
```

Re-exporting a type never transfers ownership.

## Ownership

- **core**: identities, geometry, low-level reactive/context primitives.
- **config**: constraints, alignment, insets, locale and shared policy values.
- **layout**: pure layout algorithms over measured data.
- **assets**: shared font/resource bytes and identities.
- **image**: raster decoding, identities and CPU image caching.
- **text**: shaping, line layout, editing values and text metrics.
- **rendering**: display lists, paint commands and compositor structures.
- **wgpu**: GPU device/resource ownership and command execution.
- **semantics**: platform-neutral accessibility tree and actions.
- **accessibility**: projection into native/mobile accessibility systems.
- **widgets**: neutral retained UI, reconciliation, layout/paint/input/semantics.
- **controls**: Incular-styled controls over widget behavior.
- **material**: Material presentation/composition over widgets and controls.
- **runtime**: scheduling, application/window state, tasks and service lifetimes.
- **platform**: portable native IDs, capabilities, operations and errors.
- **desktop**: the single Winit desktop host and native-service coordination.
- **OS crates**: Win32/AppKit/Linux-specific integration.
- **devtools**: bounded diagnostic transport and tooling, never app-state ownership.

## Hard rules

1. **One reactive engine.** Do not introduce parallel signal/dependency systems.
2. **One desktop host.** All desktop entry paths share one event loop, input path,
   renderer path, accessibility path and teardown model.
3. **Portable platform contracts.** Winit/raw handles/native APIs stay behind
   desktop or OS boundaries; `platform` remains portable.
4. **One behavioral owner.** Visual wrappers must not duplicate focus, input,
   editing, scrolling, selection or semantics.
5. **One invalidation model.** Build, layout, paint, composite, semantics and
   hit-test invalidation use one authoritative contract.
6. **Typed native outcomes.** Queued, applied, unsupported, cancelled, stopped,
   stale and failed are distinct states.
7. **Real cache ownership.** Budget, eviction and lifetime belong to the actual
   CPU/GPU owner; local cache removal must not pretend shared resources were freed.
8. **Rust-first design.** Flutter is a vocabulary/reference, not an architecture
   or a requirement to reproduce Dart lifecycle patterns.
9. **No boundary bypasses.** Do not add crates, bridges or re-exports solely to
   circumvent dependency rules.

## API classes

- **application**: supported application-facing APIs.
- **backend**: renderer/platform/native integration APIs with explicit lifetime,
  thread and identity rules.
- **bridge**: internal cross-crate transport with no application compatibility
  promise.

Re-exports retain the defining API's class. `#[doc(hidden)] pub` is still public
Rust API; prefer `pub(crate)` when external visibility is unnecessary.

## Runtime ownership rules

- Widget descriptors configure future reconciliation; mutating a descriptor does
  not mutate an already mounted tree.
- Controllers/signals own persistent state outside descriptors.
- Registration/task lifetime is owned by a returned token or containing scope.
- Never hold mutable domain state across user callbacks; revalidate after
  reentrant callbacks before committing an outer operation.
- Native requests belong to the originating application/window generation.
- Backend failures are typed runtime errors. Panics are reserved for documented
  programmer-invariant violations.

## Compatibility

- Keep one implementation per concept. Aliases/re-exports are acceptable only
  when semantics and ownership are identical.
- Unsupported platform behavior must be explicit; never fabricate success.
- Material remains optional.
- `incular::painting` is an application-facing alias of `incular-rendering`; it
  does not require a separate compatibility crate.
- Do not reserve empty crates for hypothetical future features. Add a crate only
  when a concrete ownership boundary exists.

Any new dependency edge must update `../specs/architecture.json` and pass the
architecture contract tests.
