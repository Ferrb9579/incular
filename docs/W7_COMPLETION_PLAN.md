# W7 completion plan - controls and Material presentation ownership

Status: **implemented; closure validation recorded in
[W7_COMPLETION_REPORT.md](W7_COMPLETION_REPORT.md)**.

Prepared on 2026-09-18 against commit
`b65751b749cb7d59b453392828985261dc663373`
(`Complete W6 input text and semantic consistency`). The working tree was clean
at the beginning of this planning pass. This plan is grounded in the repository
source, not an assumption that the historical parity manifests prove behavior.

## 1. Assignment, authority, and completion condition

Finish **Plan 15, W7 - Controls and Material as presentation layers**. This is
not `plan-07.md` (native application menus), and it does not close W8's native
verification campaign. The authoritative scope is `plan-15.md`, W7, immediately
after the W6 completion record.

Read `AGENTS.md`, `docs/ARCHITECTURE.md`, `docs/ARCHITECTURE_DECISIONS.md`,
`docs/RETAINED_PROPERTIES.md`, `docs/QUALITY.md`, and the current W6/W7 sections
before implementation. References below identify baseline files and symbols;
line numbers must be rediscovered after code moves.

Execute the entire plan in the next implementation pass, with reviewable
checkpoints. Completing a ledger or fixing the first control is not the stop
condition. The final stop condition is:

1. Every reachable control/Material option, including public fields and
   generated builders, has an execution or value-operation contract and tests.
2. Replacing a visual does not replace the control's action, focus, editing,
   scrolling, selection, or semantic owner.
3. Theme/state resolution preserves explicit overrides and full applicable
   states; changing a theme updates its consumers without resetting behavior.
4. Owner-based modules, migrations, ledgers, and all required final validation
   gates are finished, with the executed evidence recorded.

No unresolved W7 rows, accepted silent setters, duplicated behavior engines,
construction-only claims of behavior, or unexecuted passing claims are allowed
in the completion report. Explicitly unsupported native delivery is a separate
capability outcome; portable composition must still work.

### Non-negotiable boundaries

Keep Material -> controls -> widgets -> domain mechanisms. Widgets/runtime own
input tenure, focus/capture, retained identity, editing, and semantic dispatch;
existing scroll/animation/navigation owners retain their responsibilities.
Controls compose these mechanisms and own only appropriate control-family
coordination. Material adapts vocabulary, tokens, and presentation, not another
interaction engine. See architecture decision B05.

Do not add crates to shorten files, reverse dependencies, add WGPU/Winit to a
presentation crate, replace the reactive engine, add a global control registry,
or restart completed W1-W6 machinery. Narrow missing neutral bridges belong in
the existing owning crate. All tests and test helpers belong under `tests/`.

This completes Incular's accepted public surface, not every Flutter feature.
Pure values and documented aliases remain legitimate; their actual operations
must be tested. Do not delete working public controls to make the ledger small.
Any incompatible change needs a named migration and a working replacement.

## 2. Executed baseline and source findings

The planning pass executed:

```text
cargo test -p incular-controls -p incular-material --all-features
exit code: 0
aggregate: 193 passed, 0 failed
result blocks: 22, including zero-test and documentation-test blocks
log: target/w7-plan-baseline.log
```

No production Rust was changed. Full workspace tests, Clippy, documentation,
and native-device interaction were not rerun by planning. W6's completed
validation is historical evidence, not a new W7 test result.

Planning verification also passed: `cargo fmt --all -- --check`,
`cargo test --test architecture_contract` (5 passed, 0 failed), and
`git diff --check`. The saved document has all 11 execution stages, balanced
code fences, no trailing whitespace, and existing linked authority documents.
Plan 15 and the documentation index link to this plan. These checks validate
the planning change, not the still-unimplemented W7 fixes.

### Confirmed source gaps to reproduce first

Paths in this table are relative to `crates/` unless they begin with `tests/`.
Write behavioral failing regressions before crediting a fix. The listed paths
are starting evidence, not a claim that all their properties have been audited.

| ID | Source / symbol | Observed gap | Planned owner / fix |
| --- | --- | --- | --- |
| G01 | `incular-material/src/buttons/common.rs`, `ButtonSpec::{label,child,resolve_style}` | Constructors seed the highest-priority style with complete defaults; merging it last can overwrite component-theme padding, minimum size and tap policy. | Store sparse widget overrides; apply defaults once, below themes. |
| G02 | `incular-controls/src/styles.rs`, `ButtonStyle::{merge,resolve_background,resolve_foreground}` | Literal, compact-state and general-state representations coexist; a lower-layer state table can mask a newer explicit literal. | Merge logical properties, clearing competing lower representations. |
| G03 | `incular-material/src/buttons/style.rs`, `apply_color_property` / `StateColorFromTable` | Converting StateTable to StateColor drops invalid, expanded, open, read-only, dragging and focus-visible entries. | Preserve the full StateValue throughout lowering. |
| G04 | `incular-controls/src/button.rs`, `Button::build` | Foreground, side, shape, elevation and layer builders resolve from initial state; shape is reduced to one circular radius. Several accepted style options have no consumer in this path. | Live presentation over the existing action owner; full geometry. |
| G05 | `incular-controls/src/theme.rs`, `ControlTheme` | `palette` and `colors` are separately writable owned values, not actual aliases. Palette replacement also reconstructs typography. | One canonical color store; explicit duplicate-field migration. |
| G06 | `incular-material/src/foundation/theme_data.rs`, `control_theme` | Slider/checkbox colors overwrite global control accent/border tokens; density is assigned without the normal metric update. | Global projection only for global tokens; local family resolution. |
| G07 | `incular-controls/src/slider.rs`, `Root::build` | Custom child returns before gestures, keyboard, values and semantics are installed. | Choose visual first, then apply one shared behavioral root. |
| G08 | `incular-controls/src/switch.rs`, `Root::build` | Custom/default branches have different state owners and read-only focus policy. | One switch model/action/semantic path around either visual. |
| G09 | `incular-controls/src/progress.rs`, `Root::build` | Custom child bypasses progress semantics; the default indeterminate bar is static. | Common progress semantic root and retained motion policy. |
| G10 | `incular-controls/src/{tabs,form,number_field,select,autocomplete}.rs` | Conversions drop configuration/callbacks; autocomplete advertises editing while default content is a button. | Real compound bindings over existing editing/composite/popup mechanisms. |
| G11 | `incular-material/src/p0_controls/slider_range.rs`, `From<RangeSlider>` | Material owns a second range/value/gesture/key/semantic implementation; drag adds total displacement to current value rather than press-origin value. | Shared slider family below Material; correct displacement contract. |
| G12 | `incular-material/src/p0_controls/tabs.rs` | Generated icon/text settings can be discarded; indicators use hardcoded styling/widths; page extent assumes 600 pixels; animate_to jumps. | One normalized tab description; measured themed layout and shared page state. |
| G13 | `incular-material/src/foundation/theme_data.rs`, `From<AnimatedTheme>` | Duration is discarded; ThemeData::lerp interpolates only selected fields. | Retained theme transition with explicit interpolation policy. |
| G14 | `incular-material/src/menus/anchor.rs`, `MenuAnchor::build` | clip_behavior, cross_axis_unconstrained, use_root_overlay and animated are accepted then discarded. | Existing clip/layout/overlay/animation owners. |
| G15 | `incular-material/src/component_impl/progress.rs` | Partial theme consumption; stroke width takes a numeric maximum instead of explicit override precedence; caps and indeterminate arc are static. | Complete progress token and execution contract. |
| G16 | `tests/ledger/mod.rs` | Discovery misses arbitrary public fields, qualified same-name types, generic `fn transform` builder grammar and the complete export surface. | Extend the existing syntax-based validator, not a handwritten exemption. |

### Additional paths requiring focused reproduction

`RadioGroup` changes its selected RefCell without a dependency notification;
test existing mounted radios, not only a newly constructed radio. Controls
`TextFieldStyle::placeholder_color` and `border_focused` need execution checks.
Material TextField supplies hardcoded default caret/selection colors and adds
pointer/focus/semantic wrappers: test read-only selection, automatic focus, and
unique editor identity before changing those wrappers.

`DropdownButtonFormField::register_with` does consume validation/save callbacks;
discarding on_saved in its Widget conversion alone is not proof of a no-op.
Test changed selection after registration and duplicate visible labels: typed
selection identity must not be reconstructed by matching display text.

Likewise, SliverAppBar's static Widget conversion intentionally differs from
its Sliver conversion. Preserve the W1/W6 snapping/stretching behavior. A grep
for discarded fields is a lead, not an automatic defect verdict.

## 3. Decisions to carry into implementation

### 3.1 Retained ownership and lifecycle

Hover, press, focus, drag and presentation subscriptions belong to the mounted
owner, not a cloneable descriptor-global controller. Mounting the same Widget
description in two windows must not share transient state. Sharing an explicit
application controller is intentional and must remain supported where documented.

Builder setters configure the next description. They must not mutate a mounted
clone as a side effect. Controlled values become authoritative at reconciliation;
uncontrolled defaults initialize once per mounted identity. Theme-only updates
must preserve value, focus/capture, edit history, gesture origin and animation
progress. Replacing callbacks must not replace those owners.

Use DependencySource and existing tracked builders/controllers for observation.
Keep model mutations and callback invocation separate: snapshot callbacks, drop
mutable borrows, then notify. No arbitrary user code inside a live tree borrow.

### 3.2 One logical property and one precedence order

Highest priority first:

```text
explicit widget property / sparse style
nearest applicable local component-theme override
application component theme
supported legacy family theme
family default derived from global tokens
```

This order chooses a property; state precedence then chooses its value. Do not
collapse combined states to one exclusive visual state before resolution.
Preserve the existing documented distinction between StateProperty::resolve and
resolve_or, including sparse-map fallback; do not change it incidentally.

Family resolvers are pure:

```rust
// Proposed implementation shape, not an existing API.
fn resolve_button(
    tokens: &ButtonTokens,
    overrides: &ButtonStyle,
    state: ControlState,
) -> ResolvedButtonVisual;
```

They must not register listeners, change focus, write controllers, reconstruct
behavior owners or consult wall-clock time. Material state conversion is a
boundary adapter, not an independent state manager.

### 3.3 Canonical colors and compatibility

Keep `ControlTheme.colors` as the canonical store. Replace stored `palette` with
a borrowing `palette()` accessor; retain PaletteTokens as a type alias and
with_palette as a convenience setter. Migrate repository callers and document
the source change. Two independently writable public fields cannot be repaired
by occasional synchronization.

Preserve explicitly configured typography on palette changes. When color is
not explicit, resolve the current default foreground rather than freezing it
in a reconstructed TextStyle. Document precedence among global metrics, density
and component metrics instead of adding more synchronized copies.

Retain ButtonStyle's legacy spellings where practical. A higher layer specifying
any background representation replaces lower background representations, and
likewise for foreground. Conflicts in a single public struct literal have a
documented deterministic precedence; setter order cannot be inferred afterward.

ComponentThemeData currently serves many aliases with non-applicable fields.
Inventory field applicability explicitly. For active family APIs needing a
restricted contract, introduce small sparse family records while retaining the
canonical FooThemeData name and meaningful fields. A legacy-carrier conversion
must report non-applicable non-default fields rather than silently discard them.
Do not wholesale rename all themes or broaden the supported Flutter surface.

Some FooTheme names are data aliases, not inherited widgets. Do not claim nested
scope behavior for them. Preserve documented data-only aliases, or make an
individually tested migration when a genuine accepted scope promise requires it.

### 3.4 Validation and unsupported behavior

Normalize numeric inputs at one shared boundary used by constructors, fluent
setters, generated builders and writable public fields. Invalid non-finite
geometry must not reach layout, constraints or tessellation. Existing valid
behavior must remain unchanged. Use checked construction for invalid combinations
when normalization would conceal an error, with a documented panic policy for
infallible compatibility constructors if required.

Unsupported is valid only with an explicit contract: compile-time exclusion,
checked rejection, or typed runtime capability/result. A property accepted and
then ignored is not a capability outcome. Native effects must not expand this
work into W8, and unverified native delivery must never be called tested.

## 4. Ordered implementation

The order is coverage -> theme/state seams -> common behavior -> family fixes
-> organization -> closure. Keep current paths until the move-only stage so
ledgers and regression references do not churn repeatedly.

### W7.0 - Reconcile baseline and freeze the scope

Read HEAD and status again. Preserve concurrent/user edits; never reset to the
planning commit. Reconcile this plan against any newer implementation before
changing files. Record the actual execution baseline under Plan 15's W7 section.
During execution W7 remained pending until the final gates passed; that close is
now recorded in `W7_COMPLETION_REPORT.md`.

### W7.1 - Exhaustive public surface and property ledgers

Modify `tests/ledger/mod.rs`, adding private support modules under
`tests/ledger/` as needed. Keep existing version-2 W3 ledgers working. A W7-only
version-3 envelope is justified only for qualified identities and expanded
discovery metadata; do not rewrite unrelated families merely for a new format.

Create:

```text
tests/w7_public_surface.rs
tests/w7_controls_ledger.rs
tests/w7_material_ledger.rs
tests/w7_theme_ledger.rs
specs/w7_public_surface.json
specs/w7_controls_properties.json
specs/w7_material_properties.json
specs/w7_theme_properties.json
```

Use syn, already present in the workspace test harness. Discover root exports,
public modules, aliases/reexports, inline modules, generics and known theme
macros. Resolve aliases to definitions rather than duplicating obligations.
Use qualified identities such as `incular_controls::slider::Root`; bare Root
or TextField cannot distinguish families. Compare the full discovered public
surface against the ledger so adding an entirely new type cannot escape coverage.

Support the actual TypedBuilder grammar used here: field_defaults,
setter(skip), into/strip_option, transform expressions, and
`fn transform<F>(...)` with bounds/where clauses. Recognize public writable
fields without TypedBuilder, and setters whose name differs from storage.
Controller commands are classified separately from descriptor setters.
An unknown export macro or builder grammar fails explicitly, never vanishes.

Every option record must identify: all entrypoint spellings, default,
validation/normalization, family applicability, conversion, retained/value owner,
old/new comparison, applicable phases, exact regressions, and disposition.
Use the validator's real phase vocabulary. Do not invent input/focus phases
without implementing and testing a schema extension.

Negative fixtures must reject missing public-field options, callback transforms,
same-name types, aliases, new exports, stale references and comments pretending
to be tests. Keep exact syn-based test-reference validation. Builder parity must
compare equivalent retained outcomes, not just compile constructor expressions.

Scope includes every public controls family: buttons; checkbox/radio/switch and
groups; slider; inputs/form/field/fieldset/number/OTP; select/combobox/autocomplete;
tabs/toolbars; progress/meter; avatar/cards/dividers/separators; collapsible;
menu/menubar/context/navigation menu; popup/popover/preview/tooltip; scroll area;
dialog/alert/drawer; toast; and reexports. Material includes extras, theme data,
shell/navigation, all controls, lists/chips, menus/dialogs and surfaces.

Allow unresolved rows while implementing, but add a final closure assertion
rejecting them. Unsupported rows need an actual rejection/capability test.
Do not label a stored configuration value as retained behavior.

### W7.2 - Theme projections, sparse overrides and invalidation

Edit controls `{theme,styles,app}.rs`; Material
`foundation/{theme,theme_data,state}.rs` and `buttons/{common,style}.rs`.

Fix G01-G03 and G05-G06 first. Initialize ButtonSpec's explicit style sparsely;
defaults belong in resolve_style. Preserve StateValue tables without compact
conversion. Merge equivalent property representations with layer precedence.
Remove component colors from global control_theme projection. Apply component
geometry/colors at that family's resolver and make density metrics consistent.

Keep pointer-sized copy-on-write ThemeData and its small-stack construction
contract. Add private shared resolved token groups for real consumer families.
Recompute a group only when its inputs change; preserve other group identities.
Do not clone the whole ThemeData into every group or every control build.

There is no BuildContext::depend_on_aspect at this baseline. Reuse typed scopes
and `depend_on_shared`. A narrow internal bridge may be needed:

```rust
// Proposed bridge in incular-widgets::internal.
pub fn shared_environment_scope<T: Any>(value: Rc<T>, child: Widget) -> Widget;
```

Store the original erased Rc and `TypeId::of::<T>()` in InheritedScopeValue,
not Rc<Rc<T>>. Existing reconciliation compares value pointer identity, so
unchanged projection groups need not notify. Files:
`incular-widgets/src/tree/context.rs`, `tree/widget/constructors.rs`,
`tree/reconciliation.rs`, and `internal.rs`.

Install family token scopes at theme boundaries and make family consumers
depend on those groups rather than full ThemeData for a single property. Keep
public Theme::of as a whole-theme dependency. Respect nearer ControlThemeScope,
input and progress scopes. Shared closure identity may use Arc::ptr_eq for
change detection; different closures must not be assumed equivalent.

Regressions must prove component-theme padding survives constructor defaults;
explicit literal/state properties beat inherited alternatives; all StateTable
flags survive conversion; checkbox/slider themes do not recolor or unnecessarily
rebuild button consumers; nearest scope removal rebinds dependencies; equal
shared groups do not notify; and palette changes preserve explicit typography.

Measure consumer executions under a stable retained child. Rebuilding an
application's ancestor explicitly is not evidence of a spurious dependency.
Keep `theme_data_safety.rs` pointer-size, copy-on-write and 256 KiB stack tests.

### W7.3 - Read live presentation state from the existing action owner

Primary files:

```text
crates/incular-widgets/src/internal.rs                     # ActionSurface
crates/incular-widgets/src/tree/specs.rs                   # ButtonSpec
crates/incular-widgets/src/tree/widget/constructors.rs     # action_surface
crates/incular-widgets/src/render_object/mod.rs            # RenderButtonState
crates/incular-widgets/src/tree/{interaction,focus,reconciliation}.rs
crates/incular-widgets/src/render_object/update.rs
crates/incular-runtime/src/frame.rs                       # existing input routing
```

Add a neutral read-only interaction snapshot and internal presentation-builder
seam to the existing action root. Preserve simultaneous hover/press/focus bits;
the legacy exclusive ButtonState remains only a compatibility visual view.
Obtain disabled/focus-visible state from authoritative owners. Controls add
checked/selected/error/read-only state from their family model. Widgets must
not depend on Material or ControlState.

```rust
// Proposed shape, not an API available at the planning baseline.
ActionSurface::with_presentation(move |context, interaction| {
    let state = map_interaction(interaction, selection_snapshot);
    let resolved = resolve_button(&tokens(context), &overrides, state);
    build_button_visual(resolved, &content)
})
.on_click(callback)
```

The snapshot dependency belongs to the mounted owner/binding. Publish only
effective changes through DependencySource, after releasing mutation borrows.
Do not recreate hover/press controllers inside a LayoutBuilder, synthesize
events from Material, or decide click completion in a visual wrapper.

Cover pointer enter/exit, completed and cancelled press, keyboard activation,
focus changes, disable/read-only/callback removal, capture cancellation,
unmount and window teardown. Rebinding presentation/callbacks must retain the
action's identity and current state. Identical state writes emit no notification.

Preserve the simple retained background-color path. State-driven local visual
rebuilds must use existing value comparisons to decide layout/paint/composite
work. Static children must not rebuild every frame; color changes cannot
unconditionally dirty layout.

Add `incular-widgets/tests/action_presentation.rs`. Prove independent mounts,
combined state delivery, callback reentrancy, observer replacement/cleanup,
no-op updates, one focus owner and exactly one actionable semantic owner.

### W7.4 - Complete button, icon and ink presentation

Edit controls `button.rs`/`styles.rs`; Material `buttons/*` and
`foundation/ink.rs`. Introduce normalized content rather than prebuilding an
icon-label Row before styles resolve. Preserve identifiable icon and label
parts so icon color/size/alignment and inherited text style apply. Explicitly
custom visuals remain custom; accessible names are resolved independently.

Route all accepted ButtonStyle property groups:

| Group | Required execution |
| --- | --- |
| Background/foreground/overlay | Full live-state resolution and consistent alpha compositing. |
| Side/border/shape/radius | Full per-side/per-corner geometry, not top-left-radius approximation. |
| Elevation/shadow/tint | Existing paint/compositor primitives with correct layer order. |
| Padding/alignment/min/fixed/max/height/density | One normalized constraints calculation, honoring parent constraints. |
| Tap target | Actual hit and semantic area, not only an empty outer sizing box. |
| Icon/label | Apply resolved tokens to normalized parts; preserve custom child overrides. |
| Mouse cursor | Existing MouseRegion/cursor mapping; explicit handling of unknown names. |
| Foreground/background builders | Live combined state; stable behavior root and documented layer order. |
| Duration/splash | Retained scheduling and animation primitives; NoSplash adds no splash work; reduced-motion policy. |
| Feedback | Observable typed feedback request/result when supported; no discarded flag or fabricated native success. |

Before adding feedback plumbing, search the actual platform/runtime request and
capability surfaces. Reuse the standard lifecycle; if a narrow capability is
missing, give it an explicit unsupported default and fake-host tests. Native
delivery remains unverified without host execution. Do not claim a ripple or
Sparkle variant is implemented merely because it aliases the same static overlay.

Normalize size conflicts once. Negative/NaN/infinite sizes must have explicit
outcomes before Constraints construction. Test tight parents, padded-edge
clicks, full radii, live foreground/border/elevation, content builders, icon
positions, loading/disabled policy and callback replacement during a press.

### W7.5 - One selection and slider family across all visuals

Edit controls `selection.rs`, `checkbox.rs`, `radio.rs`, `switch.rs`, group and
toggle modules, `slider.rs`; adapt Material `p0_controls/{checkbox_radio,switch,
slider_range}.rs` and selection list tiles.

Use this path consistently:

```text
normalize descriptor -> retain/read family model -> choose default/custom visual
-> common action/gesture/focus root -> common semantic state/actions
```

Never return a custom child before installing behavior. Passive Track/Indicator
slots must not install competing interaction. Use visual-only indicators inside
selection tiles, not a disabled full control whose disabled appearance and
focus policy leak into an enabled tile.

Checkbox/switch: preserve W6 mixed-to-checked behavior, required/error state,
exactly-once activation and disabled/read-only mutation gating. Distinguish
read-only focusability from disabled; remove the separate switch-root paths.
Groups get one reactive typed selection model and registration lifecycle,
disabled-item filtering and stable identity under keyed reorder. Material
RadioGroup must invalidate existing subscribed controls when selection changes.

Factor slider normalization, values and interaction into a common family below
Material. Private `incular-controls/src/slider/{model,resolve,visual}.rs` modules
may support current slider::Root and a shared two-thumb root; truly neutral
input mechanisms remain in Widgets. Material RangeSlider becomes an adapter,
not its own GestureDetector/KeyboardListener/value/semantic engine.

Required invariants:

- Normalize ranges and quantization centrally. Cover reversed bounds, zero span,
  non-finite inputs, step/divisions and feasible minimum separation. Constructors,
  setters and raw builders must agree.
- Capture origin values at each real gesture start. The recognizer reports total
  displacement: calculate from that origin, never add total displacement to the
  latest value repeatedly. A second drag gets a new origin; theme rebuilds do not.
- Tap uses position; drag uses measured travel. Share forward/inverse geometry
  for horizontal/vertical/RTL. SlideThumb must differ from track dragging.
- Start/change/end ordering is documented and identical across wrappers. Clamped
  no-op samples do not emit duplicate change callbacks. Cancellation retires the
  gesture without a delayed activation or orphaned started state.
- Semantic Increment means numeric increase, even in RTL. Use the existing
  explicit semantic callback path when keyboard direction would invert it.
  Preserve W6's removal of Slider Activate. Both range thumbs must be reachable
  through keyboard/accessibility, not only a last-pointer-selected thumb.

Test custom/default slots, sequential/multi-sample drags, replacement during
drag, read-only switches, group changes, range extrema, RTL semantic increments
and multiple mounted copies. Rerun W6 retained-interaction/semantic suites.

### W7.6 - Editing, forms and compound controls must execute their settings

Edit controls `text_input`, `field`, `fieldset`, `form`, `number_field`,
`otp_field`, `select`, `combobox`, `autocomplete`, `tabs`, `composite`, and group
modules. Material input currently spans `lib.rs`, `foundation/input.rs`, and
`menus/dropdown.rs`; keep those paths until the organization stage.

EditableText remains the editor. Resolve complete TextStyle, placeholder,
caret/selection colors and focused/error/disabled decoration from the same
focus/editor owner. Do not replace typography with just font size and color.
Default internally supplied focus nodes must update decoration as well as
explicitly passed nodes.

Test real events before removing wrappers. Read-only rejects mutation but keeps
neutral editor selection/copy/focus behavior. Disabled gates pointer, keyboard,
IME, clipboard and semantic mutation. Remove redundant focus/semantic roots
only with regressions preserving correct labels/actions and editor identity.

Preserve W6's edit-transform-before-history path, grapheme formatting, restoration
baselines, composition and callback reentrancy. Do not add competing controller
formatting listeners or edit loops in Material. Theme changes must not replace
the editing controller or lose selection/history/composition.

Controls Form must compose the existing Widgets Form registry/controller,
install a real submit path and propagate disabled policy to participants. Use
existing actions/intents; do not submit on every Enter from a multiline editor.
Validate/save before the form callback, suppress it when invalid, and avoid
duplicate registration. Keep the existing explicit registration contract clear.

TextFormField needs live registered validation/error presentation, not only
conversion-time validation. Dropdown form bindings retain typed selected values,
never display labels as identity; changes must reach the saved/validated value.

| Compound family | Required retained binding |
| --- | --- |
| Number field | Shared editor and numeric model; increment/decrement invoke the same bounded update operation. |
| OTP | Length/value constraints, editing/paste/navigation and semantics survive visual replacement. |
| Select/listbox | Item value/disabled registration, keyboard/typeahead, selection visual/callback and popup close share one model. |
| Combobox/autocomplete | Actual editable query owner, exactly-once query change, suggestions through existing popup/composite machinery, working semantic SetText. |
| Tabs/groups/toolbars | Existing CompositeController handles orientation, disabled skipping, manual/automatic activation and focus movement. |
| Field/fieldset | Inherited disabled/required/error state and labels reach participating controls without a second editor. |

Use nearest-root typed scopes and stable item registrations with cleanup.
Do not inspect arbitrary descendant text to discover IDs. Missing required root
context has an explicit documented outcome. Legitimate passive layout slots
remain passive. Every additional W7.1 family gets a real contract, not an exemption.

### W7.7 - Material navigation, menus, lists, dialogs and shell

Edit `p0_controls/tabs.rs`, `component_impl/{list_items,chips,navigation,app_shell}.rs`,
`app_shell/*`, `menus/*`, `feedback/{dialog,dialog_handles,transient}.rs`, and
the corresponding controls popup/menu/dialog modules.

Normalize Tab constructors/builders through one content function. Explicit child
has documented precedence; otherwise icon/text build content. Eliminate fields
that constructors copy manually but generated setters leave disconnected.

TabBar consumes theme tokens and measures actual label/tab indicator geometry.
TabBarView calculates page extent from layout and viewport fraction, not 600px.
Synchronize TabController with actual page movement from gestures, keys and
programmatic operations. Implement animate_to with existing ScrollController
animation, clamp index on length changes and suppress unchanged notifications.
Do not create another page scrolling engine.

Lists/chips/navigation/surfaces must consume all accepted content/layout/selection
fields and component tokens. Enabled gates long-press and auxiliary actions too.
A chip delete action may be distinct intentionally; decorative children must
not cause accidental double activation. Assert semantic roles/labels/selection
and focus order, not just paint.

Forward MenuAnchor's discarded policies to their owners:

```text
clip_behavior              -> neutral panel clipping
cross_axis_unconstrained    -> real panel measurement constraints
use_root_overlay           -> root versus nearest overlay host selection
animated                   -> retained opening/closing presentation lifecycle
```

Reuse OverlayPortal/TransientPlacement and runtime dismissal. Add only the narrow
neutral option genuinely absent from that machinery. Preserve close-chain
ordering, open/close once, outside-tap consumption, Escape, anchor transforms,
focus restoration and native-to-retained fallback. Closing animation must not
leave an invisible focus/input owner alive. Trigger visual replacement keeps
one activation root.

Dialogs/menus consume their nearest component theme and all accepted geometry,
surface, label/action and dismissal fields. Theme resolution never creates a
fresh controller. Preserve Scaffold/AppBar/SliverAppBar toolbar, insets, drawers,
extensions, floating/snapping/stretch and environment behavior; W7 is not a
shell rewrite. Rerun existing shell/sliver tests after theme work and file moves.

### W7.8 - Progress, state motion and AnimatedTheme

Edit controls `progress.rs`/`meter.rs`; Material `component_impl/progress.rs`,
`feedback/progress.rs` and theme interpolation/AnimatedTheme. Reuse
`incular-widgets/src/reactive_builders.rs`, existing animation controllers and
the retained clock. Any missing lifetime bridge belongs below Material.

Progress gets one semantic owner for default/custom visuals. Indeterminate is
busy without a fabricated numeric value. Zero progress paints zero active extent
rather than forcing one pixel. Normalize ranges/values and apply each relevant
theme field: track/color/cap/width/alignment/gap/stop marker/radius/padding and
constraints. Explicit widget stroke/color overrides theme instead of numeric max.

Animate indeterminate visuals with retained scheduling, honoring visibility,
ticker and reduced-motion policy. A documented reduced-motion static visual is
valid; an always-static default is not. Stop work on completion or unmount.

AnimatedTheme retains a transition from displayed theme to target. Do not create
or restart controllers on every build. Extend ThemeData::lerp consistently for
supported color/numeric/text fields and ensure projected legacy/global values
do not become stale. Discrete values and resolver closures have an explicit
endpoint policy; never interpolate identity or callbacks. Retarget from the
currently displayed value. Test equal target, zero duration, mid-run retarget,
reduced motion, unmount and final settling.

Use deterministic timestamps, not sleeps or build-time Instant::now polling.
Translation/opacity can be compositor-only; changing arc geometry legitimately
repaints. Color-only changes must not force unconditional application layout.

### W7.9 - Organize code by owner without changing behavior in the move

After fixes pass, retire `component_impl`, `p0_controls` and `foundation` as
implementation buckets. Move code in reviewable, behavior-preserving patches;
update imports, tests, ledger paths and explicit exports together. Do not keep
permanent forwarding/include layers solely for old private module names.

Target Material structure:

```text
src/
  lib.rs                 # explicit public surface; no TextField implementations
  theme/                 # state properties, data, projections, interpolation
  buttons/               # shared adaptation and named variants
  inputs/                # TextField/TextFormField/decorators/selection text
  selection/             # checkbox/radio/switch/slider adaptation
  surfaces/              # Material/Ink/Card/dividers/avatar/badge
  lists/                 # ListTile family and chips
  navigation/            # tabs/destinations/bars/rails
  app_shell/             # MaterialApp/Scaffold/AppBar/SliverAppBar
  menus/                 # existing menu/dropdown/anchor responsibilities
  feedback/              # dialogs/progress/transient feedback
```

Place extras.rs definitions with their owners instead of another catch-all.
Preserve root/prelude/components public paths unless a named migration requires
a change. Existing controls family module paths remain useful; private model,
resolve and visual submodules may share code. Do not leave selection.rs as a
competing implementation after moving its behavior to common family roots.

Inspect `tools/generate_material_p0.py` and affected manifests before regeneration.
Change canonical inputs as needed, never restore placeholders over completed
code. Actual manifest/surface tests must pass after moves. Do not regenerate
architecture allow-lists to legitimize an accidental dependency.

### W7.10 - Ledgers, migrations and executed completion report

Finish every W7 record with real definitions and executed meaningful tests.
One mount-success test is not sufficient evidence for all fields on a component.
Record counts only after the exhaustive discovery gate succeeds.

Update `docs/RETAINED_PROPERTIES.md`, crate READMEs,
`docs/W7_API_MIGRATIONS.md`, `plan-15.md`, and add
`docs/W7_COMPLETION_REPORT.md` with baseline/final commits, source findings,
owner/API changes, test results, work-count evidence and actual limitations.

Mark W7 complete only after the final gates run against the final tree.
W8/native screen readers, OS event delivery, real-device rendering and native
feedback remain unverified unless separately executed. A headless test is not
a native acceptance test.

## 5. Test design and acceptance matrix

Use root integration helpers under `tests/w7/mod.rs`, plus focused crate tests
where that is a better boundary. Reuse current Runtime simulation and retained
inspection; no new test framework. The workspace facade dev dependency already
enables Material. Resolve actual reexports before using helpers in code.

Compare traces of callback values/order, focus identity and semantic roles,
values/actions across default/custom visuals under equivalent logical events.
For equal-sized visuals compare bounds; for different visuals target each
control's real center. Do not require custom art to have the same dimensions.

| Dimension | Scenarios |
| --- | --- |
| Construction | Constructor, fluent, generated builder, public-field configuration. |
| Presentation | Default, custom slot/child, inherited theme, explicit override. |
| Input | Completed/cancelled pointer, multi-sample drag, keyboard, semantic action. |
| Policy | Enabled, disabled, read-only, required/error, selected/checked/mixed. |
| Retention | Unrelated rebuild, theme update, callback replacement, keyed reorder, remount. |
| Lifetime | Disable/cancel mid-input, dropped registration, two mounted copies/windows. |
| Geometry/value | Zero/extrema, reversed bounds, invalid floats, tight constraints, RTL, scaling. |

Use pure table/property tests for shared resolvers and concrete retained tests
for distinct paths. Do not explode all combinations into mounted applications.
Exhaustive 12-bit ControlState resolution is cheap; thousands of full tree
mounts for the same resolver truth table are unnecessary.

Add root retained suites:

```text
tests/w7_control_behavior.rs
tests/w7_material_behavior.rs
tests/w7_theme_invalidation.rs
```

Mandatory regression names or equally precise equivalents:

```text
component_padding_survives_constructor_defaults
explicit_literal_overrides_inherited_state_property
state_table_conversion_preserves_every_control_state
checkbox_theme_does_not_recolor_or_rebuild_button_consumer
nearest_scope_wins_and_removed_scope_rebinds_dependencies
equal_shared_projection_does_not_notify_consumers
palette_change_preserves_explicit_typography
state_layer_builders_receive_combined_live_state
theme_rebuild_during_press_preserves_one_activation
padded_button_region_is_really_clickable
custom_slider_keeps_pointer_keyboard_and_semantic_actions
switch_read_only_policy_is_identical_with_custom_visual
range_drag_uses_press_origin_not_accumulated_total_delta
second_slider_drag_uses_the_new_start_value
slider_semantic_increment_is_numeric_increase_in_rtl
both_range_thumbs_are_keyboard_and_semantically_reachable
form_submit_reaches_registered_validation_and_save_once
dropdown_form_saves_typed_selection_with_duplicate_labels
select_item_disabled_and_value_reach_selection_owner
autocomplete_semantic_set_text_reaches_editor
tab_builder_icon_and_text_reach_measured_content
tab_page_extent_follows_real_layout_and_viewport_fraction
menu_anchor_policies_reach_overlay_layout_clip_and_lifetime
custom_progress_keeps_busy_range_and_label_semantics
animated_theme_retargets_from_presented_value
unmounted_presentation_has_no_subscriptions_or_animation_work
```

Protect existing tests in controls `controls_test`, `checkbox_contract`,
`context_menu_pointer`; Material `theme_data_safety`, `length_formatting`,
`system_environment`, `checkbox_error`, `list_tile_focus`, `card_policies`,
`app_bar_toolbar`, `scaffold_insets`, `scaffold_extensions`, and all
`sliver_app_bar*`; widgets `inherited_context_contracts` and
`tree_performance_contracts`; runtime `runtime/retained_interactions`,
`semantic_action_dispatch`, `text_input_gating`, `performance_contracts`; root
`material_3471_surface`, `material_p0_stress`, facade/architecture/ledger tests.

`crates/incular-runtime/tests/runtime/retained_interactions.rs` is a module of
the runtime integration binary, not a standalone --test target. Run through
`--test runtime` with an optional name filter.

## 6. Commands, resource policy and commits

Do not alter build configuration for this work or launch multiple heavy Cargo
commands in parallel. At this baseline test-constrained is
`test --workspace --jobs 12`; nearby comments describe different limits.
If resource limits require the existing test-sequential alias, record the
substitution and exact outcome. Do not claim the other command executed.

Focused commands, after the named new targets exist:

```powershell
cargo test --test w7_public_surface --test w7_controls_ledger --test w7_material_ledger --test w7_theme_ledger
cargo test --test w7_control_behavior --test w7_material_behavior --test w7_theme_invalidation
cargo test -p incular-controls -p incular-material --all-features
cargo test -p incular-widgets --test action_presentation --test inherited_context_contracts --test tree_performance_contracts --all-features
cargo test -p incular-runtime --test runtime --test semantic_action_dispatch --test text_input_gating --test performance_contracts --all-features
cargo test --test architecture_contract --test test_placement --test facade_surface --test material_3471_surface --test material_p0_stress
```

Required final gates, on the final source tree, checking each exit code:

```powershell
cargo fmt --all -- --check
cargo check --workspace
cargo test-constrained
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test-constrained --all-features
cargo check --workspace --all-targets --all-features
```

Warning-denied rustdoc on the current Windows/PowerShell host:

```powershell
$previousRustdocFlags = $env:RUSTDOCFLAGS
try {
    $env:RUSTDOCFLAGS = (($previousRustdocFlags, '-D warnings') -join ' ').Trim()
    cargo doc --workspace --all-features --no-deps
    if ($LASTEXITCODE -ne 0) { throw 'warning-denied rustdoc failed' }
} finally {
    if ($null -eq $previousRustdocFlags) {
        Remove-Item Env:RUSTDOCFLAGS -ErrorAction SilentlyContinue
    } else {
        $env:RUSTDOCFLAGS = $previousRustdocFlags
    }
}
git diff --check
git status --short
```

Use the facade feature/target/MSRV/dependency checks in QUALITY.md where the
changed boundary requires them. Preserve MSRV 1.88 and locked dependencies;
do not upgrade crates to sidestep a local implementation problem. Compile examples
through all-targets; record native interactive execution separately.

Performance acceptance is deterministic: unchanged groups notify zero times;
unaffected stable consumers rebuild zero times from theme notifications;
no-op updates emit nothing; visual changes retain behavioral IDs; idle static
controls schedule no animation; detached subscriptions/registrations release.
Do not claim millisecond speedups without measurements.

Suggested coherent commits (validate each under existing Plan 15 policy):

```text
1. Cover W7 public exports, fields and generated builder discovery
2. Centralize family theme projection and sparse precedence
3. Expose retained action presentation without duplicating input
4. Complete button, icon and ink style consumers
5. Unify selection and slider behavior across visual slots
6. Complete editor, form and compound-control bindings
7. Complete Material navigation, menu, dialog and surface properties
8. Complete progress and retained theme transitions
9. Organize modules by owner and migrate affected public APIs
10. Close W7 ledgers and record final executed validation
```

Intermediate commits are checkpoints, not W7 completion. Stage only intended
files, never unrelated user changes. Record failing behavior, changed owner/API,
regression results and limitations while resolving each slice. Rerun required
gates after final moves or fixes; an earlier green run does not validate later code.

## 7. Closure checklist

- [x] Current HEAD/user edits reconciled with the planning baseline.
- [x] Complete export inventory, qualified identities, public fields and every used builder form covered.
- [x] Every accepted W7 option has a meaningful execution/value contract and exact tests; no unresolved rows.
- [x] Explicit style overrides beat themes across all representations; no state-table truncation.
- [x] Component tokens neither leak into global colors nor spuriously notify unaffected family consumers.
- [x] One canonical palette and clear migrations; explicit typography preserved.
- [x] Default/custom visuals share live action, focus and semantic owners.
- [x] Selection/sliders/editing/forms/compound controls/navigation/menus execute their accepted settings.
- [x] Progress, state effects and AnimatedTheme honor retained lifecycle and deterministic clock tests.
- [x] W6 behavior, small-stack theme safety and retained performance contracts pass.
- [x] Historical private buckets retired without breaking intentional public paths.
- [x] Final checks/constrained/all-feature tests/Clippy/rustdoc/diff gates passed; the Windows `cargo fmt --all` OS-206 host limitation and equivalent 32-member formatting pass are recorded in the completion report.
- [x] Completion report and Plan 15 status match executed evidence; W8/native limits are honest.
- [x] Validated commits contain only intended work; no unfinished W7 source fix is left unstaged.

## 8. Closed execution record

W7.0 through W7.10 are complete. The source-grounded decisions in this document
remain the maintenance contract for future controls/Material work. Executed
closure evidence is recorded in [W7_COMPLETION_REPORT.md](W7_COMPLETION_REPORT.md),
public migrations in [W7_API_MIGRATIONS.md](W7_API_MIGRATIONS.md), and native
outcome verification continues separately under Plan 15 W8.
