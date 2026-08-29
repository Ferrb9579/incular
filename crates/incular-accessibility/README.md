# incular-accessibility

`incular-semantics::SemanticsTree` is Incular's canonical, retained and
platform-neutral semantic model. This crate is its native adapter layer. It
uses AccessKit 0.24.1 to project one retained Incular tree into one native
desktop accessibility tree; widgets and the public Incular prelude never need
AccessKit types or IDs.

## Projection and identity

`AccessKitProjection` belongs to one live native window. It gives each
generational `SemanticNodeId` a private `accesskit::NodeId` for that window
only. The mapping is stable through ordinary updates, removed with a semantic
node, and never survives a process/window lifetime. A later reuse of an arena
slot has a new semantic generation and therefore cannot receive an old native
identity; stale requests are rejected before runtime dispatch.

The projection observes `SemanticsTree::revision()`. If the revision did not
change, it neither walks nor hashes/serializes the tree and reports an
unchanged-semantic skip. On native activation it emits one complete tree. On a
changed revision it compares retained nodes, publishing only changed/new nodes
and the changed structural ancestors needed by AccessKit; removals are expressed
through the changed parent relationship. Incular bounds are logical and are
multiplied by the owning window's DPI scale exactly once for AccessKit's
physical window coordinate convention.

## Roles, state, and actions

The mapping is deliberately narrow and documented rather than inventing roles:

| Incular role | AccessKit role |
| --- | --- |
| Material buttons, Text, EditableText / Material TextField | Button, Label, TextInput, MultilineTextInput |
| Checkbox, Radio, Slider, ProgressBar, Meter | CheckBox, RadioButton, Slider, ProgressIndicator, Meter |
| Image, Link, Heading | Image, Link, Heading |
| List, ListItem, ScrollView | List, ListItem, ScrollView |
| Menu, Dialog, GenericContainer | Menu, Dialog, GenericContainer |

Labels/descriptions/values, enabled/disabled, selected, checked, expanded,
read-only, required, invalid, busy, heading level, numeric ranges, text
selection, list position/set size, scroll range, bounds and the actually
supported Incular actions map directly. Text selections convert between
Incular's UTF-8 byte offsets and AccessKit character positions safely. Obscured
text fields use password semantics and never publish their clear-text value.
Meaningful images require a supplied Incular label; unlabelled images are
decorative and are omitted by widget semantics.

`Click`, focus, value/text, text selection, directional scroll, and
increment/decrement requests translate to an owned `SemanticActionRequest` only
when the current node advertises the corresponding executable Incular action.
Widget-level semantic callbacks can handle custom controls without coupling
them to a concrete renderer. Unsupported payloads/actions and stale node IDs
are counted and discarded safely. The
desktop runner sends that request to the UI-thread `Application` dispatcher;
AccessKit callback threads never mutate a controller or widget directly.

## Mobile projections

`MobileAccessibilityProjection` maintains stable, generation-safe native IDs and
emits a complete update on activation followed by changed nodes, removals, and
focus/bounds/value/state events. `incular-android` and `incular-ios` expose thin
host adapters around that contract; JNI/UIKit code can translate the update on
its own UI thread while actions return through the validated semantic request
boundary.

## Diagnostics and tests

`AccessibilityDiagnostics` is value-free and covers adapter lifetime,
activation, full/incremental publications, nodes, actions, stale/unsupported
requests, focus/bounds updates, unchanged skips, and conversion failures. The
headless tests exercise role/state/text/list/scroll mapping, incremental
updates, action translation, and stale generational IDs without a desktop
session. Runtime tests cover per-window projection isolation, action dispatch,
and closing one window while another remains alive.
