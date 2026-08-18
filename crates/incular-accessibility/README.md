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
| Button, Text, TextField, TextArea | Button, Label, TextInput, MultilineTextInput |
| Checkbox, Radio, Slider | CheckBox, RadioButton, Slider |
| Image, Link, Heading | Image, Link, Heading |
| List, ListItem, ScrollView | List, ListItem, ScrollView |
| Menu, Dialog, GenericContainer | Menu, Dialog, GenericContainer |

Labels/descriptions/values, enabled/disabled, selected, checked, expanded,
read-only, text selection, list position/set size, scroll range, bounds and
the actually supported Incular actions map directly. Text selections convert
between Incular's UTF-8 byte offsets and AccessKit character positions safely.
Meaningful images require a supplied Incular label; unlabelled images are
decorative and are omitted by widget semantics. Current Incular semantics do
not expose required/password or rich-text/hypertext metadata, so this crate
does not claim those capabilities.

`Click`, focus, value/text, text selection, and supported directional scroll
requests translate to an owned `SemanticActionRequest` only when the current
node advertises the corresponding executable Incular action. Incular currently
has no retained slider/spin-controller action route, so increment/decrement
are deliberately not exposed yet. Unsupported payloads/actions and stale node
IDs are counted and discarded safely. The
desktop runner sends that request to the UI-thread `Application` dispatcher;
AccessKit callback threads never mutate a controller or widget directly.

## Diagnostics and tests

`AccessibilityDiagnostics` is value-free and covers adapter lifetime,
activation, full/incremental publications, nodes, actions, stale/unsupported
requests, focus/bounds updates, unchanged skips, and conversion failures. The
headless tests exercise role/state/text/list/scroll mapping, incremental
updates, action translation, and stale generational IDs without a desktop
session. Runtime tests cover per-window projection isolation, action dispatch,
and closing one window while another remains alive.
