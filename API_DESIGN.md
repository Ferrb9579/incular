# Incular Framework API Design Principles

## Overview

Incular is a modern, high-performance GUI framework written in Rust. It is guided by a core design philosophy:

```text
Flutter-like concepts and ergonomics
+
idiomatic Rust language design
+
Incular's retained architecture
```

Rather than mechanically translating Dart classes and dynamic runtime constructs into Rust, Incular translates Flutter's well-established design vocabulary into safe, expressive, zero-cost Rust idioms.

---

## Core Architectural Principles

### 1. Retained Architecture vs Ephemeral Descriptors

In Flutter, widgets are immutable declarative descriptions that are instantiated and destroyed on every frame.
In Incular:
- Widgets remain lightweight declarative builders and composition helpers.
- The runtime retains structural elements, layout boxes, display lists, compositor layers, and semantic nodes across frames.
- Rebuilding dirty subtrees performs structural diffing and reconciliation against retained state with generational Arena IDs (`ArenaId`).

### 2. Independent Multi-Dimensional Invalidation (`Invalidation` Bitflags)

Incular avoids coarse-grained repainting or relayout through precise, multi-dimensional damage invalidation. Rather than assuming a one-dimensional total order, damage is tracked across independent runtime phases:

```rust
bitflags::bitflags! {
    pub struct Invalidation: u16 {
        const NONE       = 0;
        const BUILD      = 1 << 0;
        const LAYOUT     = 1 << 1;
        const PAINT      = 1 << 2;
        const COMPOSITE  = 1 << 3;
        const SEMANTICS  = 1 << 4;
        const HIT_TEST   = 1 << 5;
    }
}
```

* **`BUILD`**: Subtree structural reconciliation required. Reused descendants are not automatically relaid out.
* **`LAYOUT`**: Measurement and positioning recalculation required; implies geometry-dependent repaint.
* **`PAINT`**: Display list re-recording required; does not invalidate layout.
* **`COMPOSITE`**: Retained compositor layer properties or transforms modified; zero CPU rasterization.
* **`SEMANTICS`**: Accessibility tree label/state updates; does not invalidate paint or layout.
* **`HIT_TEST`**: Hit-test target routing cache update; does not invalidate paint.

High-level `ChangeImpact` is retained as an ergonomic compatibility classification mapping into these independent flags.

### 3. Constructors & Fluent Builder Pattern

Dart relies heavily on optional named constructor arguments. Incular adopts standard idiomatic Rust builder conventions:

* **Constructors**: Take only strictly required arguments (e.g. `Text::new("Hello")`, `SizedBox::new()`, `Center::new(child)`).
* **Fluent Builders**: Configure optional properties with chainable method calls:
  ```rust
  let text = Text::new("Welcome")
      .style(TextStyle::new().font_size(16.0).color(Color::rgba(255, 255, 255, 255)))
      .max_lines(Some(2))
      .overflow(TextOverflow::Ellipsis);
  ```
* **Conversions**: Implement `From<T> for Widget` for seamless composition into trees.

### 4. Authoritative Linear Interpolation (`Lerp`)

Animation and transitions implement the authoritative `Lerp` trait:

```rust
pub trait Lerp {
    fn lerp(&self, other: &Self, t: f32) -> Self;
}
```

Implementations enforce type-specific interpolation policies:
* **Scalars & Dimensions**: Linear numeric interpolation `a + (b - a) * t`.
* **Geometry (`Offset`, `Size`, `Rect`, `EdgeInsets`, `Alignment`)**: Component-wise interpolation.
* **Colors (`Color`, `HslColor`, `HsvColor`)**: Authoritative linear-light and sRGB gamma-corrected blending.
* **Discrete & Enum Properties**: Step at threshold `t >= 0.5`.

### 5. Color & Geometry Authority

Incular delegates mathematical authority to mature ecosystem standards:
* **Geometry**: `Kurbo` owns 2D affine transforms, bezier paths, curves, and mathematical operations. `incular-core` provides thin, ergonomic wrappers (`Offset`, `Size`, `Rect`).
* **Color**: `Palette` owns color space conversions, gamma correction, and mathematical models.
* **Text & Shaping**: `Parley` owns font shaping, line breaking, and bidirectional text analysis.
* **Localization**: `ICU4X` owns locale parsing, pluralization, and message catalog formatting.
* **Keyboard Vocabulary**: `keyboard-types` standardizes key codes, logical keys, and modifiers.

### 6. Paint Exclusivity & Decoration Composition

* **`TextStyle` Exclusivity**: Setting `.color(c)` automatically clears `.foreground` and vice versa. Setting `.background_color(c)` clears `.background` and vice versa. No invalid dual-state survives.
* **`BoxDecoration` Composition**: Flutter permits simultaneous `color` and `gradient`. Incular allows both, compositing them in defined painting order with optional `background_blend_mode`. Validation via `is_valid()` ensures circle shapes omit contradictory border radii.

### 7. Unicode & Grapheme Cluster Safety

While Dart strings and selections use UTF-16 code units, Incular operates natively on UTF-8 strings:
- All character slicing, cursor movement, selection ranges, and editing operations are validated on UTF-8 byte and grapheme cluster boundaries.
- Text editors and formatters never panic on multibyte UTF-8 sequences or emoji modifiers.

### 8. Controllers, State & Async Semantics

* **Retained Controller State**: Controller state (`ScrollController`, `PageController`, `TextEditingController`, `FocusNode`) lives outside ephemeral widget descriptions.
* **Thread Affinity**: UI widgets, controllers, and elements are UI-thread bound. Background work communicates through the runtime bridge.
* **Async Semantics**: Controller methods that animate or coordinate over time return standard Rust `Future` / `Result` types. Cancellation drops pending handles cleanly without leaving unresolved element state.

### 9. Error Handling Policy

* **Programmer Errors**: Invalid configuration invariants (negative flex, singular matrix inversion during layout) produce immediate panic or normalization.
* **Runtime Failures**: Missing assets, image decode errors, I/O, or platform communication failures return typed domain `Result<T, E>`.

---

## Crate Responsibilities

| Crate | Core Responsibilities |
| :--- | :--- |
| `incular-core` | Primitive geometry (`Offset`, `Size`, `Rect`), color models, arena IDs, `Lerp`, `Invalidation`, events |
| `incular-config` | Layout constraints, alignments, insets, locale, text direction, display brightness |
| `incular-rendering` | GPU-independent display lists, vector paths, brushes, shaders, effects |
| `incular-text` | Font management, Parley layout/shaping, `TextStyle`, `TextSpan`, `StrutStyle` |
| `incular-animation` | Curves, easing functions, tweens, `Simulation`, `SpringDescription` |
| `incular-gestures` | Gesture arena, pointer routers, recognizers, `FocusNode`, `FocusManager` |
| `incular-scroll` | Viewport coordination, `ScrollMetrics`, `ScrollPhysics`, `ScrollController` |
| `incular-widgets` | Retained element reconciliation, built-in widgets, forms, editing, scrolling widgets |
| `incular-semantics` | Accessible node trees, roles, actions, labels, screen reader integration |
| `incular-runtime` | Frame scheduling, multi-window coordination, signal dispatch, restoration |
| `incular` | Top-level public facade re-exporting framework preludes |
