# Base UI reference for Incular Controls

This document records the design and architecture reference for Task 20.3. It
is a translation guide, not a claim that Incular Controls is a Base UI port.

[Base UI](https://base-ui.com/) is an unstyled React component library. Its
v1.7.0 component pages and release notes are the behavioral reference for this
work:

- [Component overview](https://base-ui.com/react/components)
- [Composition](https://base-ui.com/react/handbook/composition)
- [Styling](https://base-ui.com/react/handbook/styling)
- [Accessibility](https://base-ui.com/react/handbook/accessibility)
- [v1.7.0 release notes](https://base-ui.com/react/overview/releases/v1-7-0)

Incular Controls remains a native, retained Rust GUI library. It uses Incular
widgets, layout, gestures, semantics, overlays, and WGPU rather than DOM
elements, CSS selectors, or React namespaces. The package may ship a polished
neutral default preset inspired by the Base UI documentation examples, while
keeping behavior and appearance independently replaceable.

## Translation principles

| Base UI idea | Incular-native contract |
| --- | --- |
| `Root` and named parts | Rust descriptors/modules whose parts become retained widget nodes |
| `render` / behavior composition | `Slot`-style behavior wrappers that preserve the caller's visual child |
| `data-*` state attributes | typed `ControlState` flags and part-specific state values |
| controlled / uncontrolled props | one control-value convention: external signal/controller or retained default state |
| React context | typed ambient environment lookup through the Incular build context |
| `Portal` | a retained logical overlay stack today; the native window host can lift that stack into its overlay/layer tree without changing control ownership |
| floating-ui popup logic | one shared anchored positioner with side/alignment, flip, shift, and detach policies |
| outside click / Escape handling | one topmost `DismissLayer` with configurable modality |
| roving focus and typeahead | shared `CompositeController` used by menus, tabs, toolbars, radios, and selection popups |
| transition state | retained `TransitionStatus`; ending popup subtrees stay mounted until exit completes |
| ARIA/accessibility state | Incular semantics roles, names, values, actions, and AccessKit bridge data |

The key invariant is that a descriptor is not built against a copied default
theme in `From<T> for Widget`. A retained control resolves the ambient
`ControlTheme` and authoritative interaction state when it is built or
invalidated. Hover, press, focus-visible, checked, selected, open, disabled,
and invalid changes should invalidate only the affected paint, composite,
semantics, or layout domains.

## Visual reference

The default preset should feel like the Base UI documentation examples without
pretending that Base UI provides a built-in theme:

- neutral canvas and raised surfaces, with accent reserved for primary,
  selected, checked, and focus states;
- compact desktop controls with a coherent height and spacing scale;
- 1px borders, small radii, restrained shadows, and clear but quiet focus
  rings;
- strong body/label typography, muted supporting text, and an explicit disabled
  hierarchy;
- hover and pressed changes expressed as small surface/border shifts rather
  than saturated fills;
- popup, dialog, menu, tooltip, and toast surfaces using the same radius,
  elevation, and motion tokens;
- logical pixels and Incular `TextStyle`, `Decoration`, `Shadow`, and
  compositor values instead of CSS units or browser outline behavior.

Theme tokens should be grouped by palette, typography, spacing, radius,
elevation, motion, and component parts. A sparse component override inherits
unspecified values from its component theme, then shared tokens, then the one
root fallback theme. Theme scope changes must invalidate consumers at the
smallest useful domain.

## Inventory scope

`specs/base_ui_component_parity.jsonl` contains 34 architecture rows and has no
`UNRESOLVED` status. The current Base UI navigation exposes 37 named component
pages. To keep the manifest at the requested 34 architecture units while
retaining every name, it groups the related pages `Field / Fieldset / Form`
and `Toggle / Toggle Group`; those names are intentionally present in the
manifest rather than dropped. The status is a snapshot of the repository, not
an assertion that a `DEFERRED` row is complete.

Status meanings:

- `IMPLEMENTED`: the native control and its required behavior are complete and
  covered by behavioral tests;
- `MERGED`: the capability is provided by an existing shared Incular
  primitive, with no unnecessary duplicate control;
- `ADAPTED`: a native equivalent exists, but the Base UI anatomy, behavior, or
  accessibility contract is not yet complete;
- `DEFERRED`: a public anatomy/facade may exist for composition, but the
  behavior is intentionally not presented as complete until its shared
  interaction foundation is finished;
- `SKIPPED`: intentionally browser-specific behavior with no meaningful native
  contract. A skipped row must explain the reason in `notes`.

## Required foundation before marking a control implemented

1. `ControlThemeScope` installs a real ambient theme; controls do not eagerly
   resolve `ControlTheme::default()` during conversion.
2. Shared retained control state reflects actual pointer, keyboard, focus,
   disabled, checked, selected, expanded, open, invalid, and dragging state.
3. Compound controls expose meaningful parts and permit visual replacement or
   behavior wrapping without duplicate hit-test/focus/semantics nodes.
4. Popup controls share Portal, AnchoredPositioner, DismissLayer, focus
   trap/restore, z-order, and transition infrastructure.
5. Composite controls share keyboard navigation, disabled-item skipping, loop,
   orientation, RTL, and typeahead policy.
6. Every implemented control reports native semantics and has tests for
   construction, layout, painting, interaction, keyboard, focus, disabled
   state, theme resolution, controlled/uncontrolled state, and unmounting.
7. State-only interaction changes avoid rebuilding unrelated application
   hierarchy; switch/thumb, popup, and tabs-indicator motion use compositor
   updates when geometry is unchanged.

The manifest deliberately records existing native foundations separately from
unfinished controls. This keeps the parity report useful during migration and
prevents a no-op wrapper from being counted as a completed component.
