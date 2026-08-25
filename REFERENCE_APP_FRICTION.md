# Incular Public API Friction Log & Framework Hardening

This document records the exact friction points, ergonomics hurdles, and ambiguities identified while building the production reference application (`incular-studio`) exclusively against Incular's public API surface, along with the framework-level corrections applied to resolve them.

---

## 1. Missing Framework-Neutral Split View Primitive

### Problem
Desktop applications frequently require resizable split panes (e.g. Sidebar | Editor, Editor | Inspector, Editor | Terminal). The framework provided `Row`, `Column`, `Flex`, and `Expanded`, but no high-level splitter widget. Forcing applications to manually wire `GestureDetector` with global coordinate calculations resulted in repetitive, error-prone boilerplate.

### Example Code (Before)
```rust
// Required manually computing mouse deltas, constraints, flex factors, and nested containers
```

### Framework-Level Correction
Added a first-class, framework-neutral `SplitView` widget to `incular-widgets` and re-exported it in `incular::prelude`.

### Resulting API
```rust
SplitView::horizontal(sidebar_pane, editor_pane)
    .split_offset(sidebar_width)
    .min_first(160.0)
    .min_second(300.0)
    .divider_thickness(4.0)
    .on_split_changed(move |delta| {
        sidebar_width_sig.set((sidebar_width_sig.get() + delta).clamp(160.0, 600.0));
    })
```

---

## 2. Inconvenient `Container` Constructors & Styling API

### Problem
`Container::new(child)` strictly required a child widget argument. Building an empty container for padding, backgrounds, or decorative panels required `Container::empty()` or passing a dummy `SizedBox::new()`. Furthermore, `Container` lacked a `.decoration(BoxDecoration)` method, despite `BoxDecoration` being the primary Flutter-style styling descriptor.

### Example Code (Before)
```rust
Container::new(SizedBox::new())
    .color(theme.background)
    .border(Border::new(1.0, theme.border))
```

### Framework-Level Correction
1. Updated `Container::new()` to construct a default empty container.
2. Added `Container::with_child(child)`.
3. Added `Container::decoration(BoxDecoration)` to configure background color, border, and border radius directly from a `BoxDecoration` struct.

### Resulting API
```rust
Container::new()
    .padding(EdgeInsets::all(8.0))
    .decoration(
        BoxDecoration::new()
            .color(theme.surface)
            .border(Border::new(1.0, theme.border))
            .border_radius(BorderRadius::all(Radius::circular(4.0))),
    )
    .child(content)
```

---

## 3. Type Collision Between `incular_rendering::Border` and `incular_widgets::Border`

### Problem
`incular::prelude` exported `Border` from `incular_rendering` (which only supports a single uniform color/width stroke) while `incular-widgets` defined a Flutter-style 4-sided `Border` (with top, right, bottom, left `BorderSide`). This created confusing compiler type mismatch errors when using `BoxDecoration::border` and `Container::border`.

### Framework-Level Correction
1. Re-exported `incular_widgets::Border` as `Border` in `incular::prelude` and aliased `incular_rendering::Border` as `RenderBorder`.
2. Implemented `From<incular_widgets::Border> for incular_rendering::Border`.
3. Updated `Container::border`, `Card::border`, and related methods to accept `impl Into<Border>`.

### Resulting API
```rust
use incular::prelude::*;

let b = Border::new(1.0, Color::rgba(255, 255, 255, 30));
let box_dec = BoxDecoration::new().border(b);
let container = Container::new().border(b);
```

---

## 4. `Button` Inability to Wrap Custom Widget Content

### Problem
`Button::new` accepted only `impl Into<String>`, making it impossible to create complex visual buttons (e.g. icon + text, badge buttons, custom tab buttons) with retained button semantics without manually constructing `GestureDetector` trees.

### Framework-Level Correction
1. Added `Button::with_child(child: impl Into<Widget>)` constructor.
2. Added `Button::on_click` alias for `Button::on_press`.

### Resulting API
```rust
Button::with_child(
    Container::new()
        .padding(EdgeInsets::symmetric(10.0, 4.0))
        .child(Text::new("🚀 Launch")),
)
.on_click(move || {
    launch_sig.set(true);
})
```

---

## 5. Alignment Naming Divergence on `Row`, `Column`, and `Flex`

### Problem
Developers naturally expect `.alignment(CrossAxisAlignment::Center)` as a convenient shorthand for `.cross_axis_alignment(CrossAxisAlignment::Center)`.

### Framework-Level Correction
Added `.alignment(CrossAxisAlignment)` method as an ergonomic alias for `.cross_axis_alignment()` on `Row`, `Column`, and `Flex`.

### Resulting API
```rust
Row::new(items).alignment(CrossAxisAlignment::Center)
```

---

## 6. Closure Return Type Sensitivity on `Signal::set`

### Problem
`Signal::set(&self, value: T) -> bool` returns a boolean indicating whether the value actually changed. When used directly in `move || sig.set(val)`, Rust infers the closure return type as `bool` rather than `()`, triggering a mismatch in callback signatures expecting `Fn()`.

### Framework-Level Guidance
Wrapped signal assignments in statement blocks `{ sig.set(val); }` or created concise helper closures.

---

## 7. Preflight Parity Snapshot Audit Discrepancy (75 vs 74)

### Problem
`specs/flutter_api_parity.jsonl` contained duplicate entries for `FormField` and `GenericFormField`, causing a reporting discrepancy between the snapshot validator (74 types) and manifest inventory (75 types).

### Framework-Level Correction
Deduplicated `FormField` / `GenericFormField` in `specs/flutter_api_parity.jsonl`, aligning the baseline to exactly 74 deep-audited types with 100% resolution across the workspace.
