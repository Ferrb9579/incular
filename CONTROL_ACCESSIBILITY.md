# Control accessibility contract

Controls use the retained semantics tree rather than native widget handles.
Roots own the semantic role and state; visual parts (`Indicator`, `Thumb`,
`Arrow`, and `Slot`) remain implementation details unless an application
explicitly exposes a part.

Disabled controls suppress pointer activation and advertise `enabled = false`.
`Button::focusable_when_disabled(true)` is available for loading/help states.
Checkboxes expose `CheckedState::{Unchecked, Checked, Indeterminate}` and
compound fields provide label/description/error anatomy for associations.

Composite controls should route arrow/Home/End behavior through
`CompositeController`, skip disabled items, and honor orientation/RTL. Popup
parts preserve logical ownership while the shared overlay portal owns visual
stacking and dismissal.

