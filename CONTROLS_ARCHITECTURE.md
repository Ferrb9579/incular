# Incular Controls architecture

Controls are retained descriptors layered over `incular-widgets` mechanics.
`From<T> for Widget` only erases a descriptor into a retained `LayoutBuilder`;
theme resolution happens when that builder materializes under a
`ControlThemeScope`.

```text
ControlThemeScope
        │ typed retained environment
        ▼
compound root ── Slot ── visual parts
        │
        ├─ ControlState flags / StateValue
        ├─ shared CompositeController
        └─ overlay portal / positioner descriptors
```

`Slot` is transparent: behavior wrappers do not add a second visual or focus
node. The popup, dialog, tooltip, menu, select, and combobox modules share the
same portal/positioner descriptors. Tabs, toolbar, radio, menu, and select
can share `CompositeController` for roving focus, looping, disabled skipping,
orientation, RTL, and first/last navigation.

Existing `Checkbox::new(bool)`, `Switch::new(bool)`, and `TextField` remain
source-compatible convenience constructors. The `checkbox`, `switch`, and
`radio` modules expose anatomy and controlled/default-state vocabulary for new
code.

