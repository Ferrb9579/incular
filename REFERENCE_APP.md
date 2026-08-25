# Incular Reference Application: Incular Studio

`Incular Studio` ([`examples/studio/main.rs`](file:///home/fanisus/CODEBASE/incular/examples/studio/main.rs)) is the official desktop reference application built exclusively using the public Incular API surface (`incular::prelude::*`). It serves as empirical proof of the framework's architecture, ergonomics, performance, and soundness.

---

## 1. Application Overview & Architecture

Incular Studio models a modern IDE / developer workspace with dockable split panes, virtualized project navigation, multi-document tabbed editing, live diagnostic inspector, bottom dock (Search, Problems, Output, Canvas preview), floating command palette, typed configuration settings, multi-locale localization (including full RTL text directionality for Arabic), multi-theme switching, and atomic crash-resilient state restoration.

```text
+-----------------------------------------------------------------------------------+
|  [◀ Sidebar]  [Incular Studio]  [🔍 Palette]   [en]  [🌙 Dark]  [⚙️ Settings] [...] |  Menu Bar
+-----------------------------------------------------------------------------------+
|              |  [main.rs ✕] [lib.rs ✕] [+]                                        |
|  EXPLORER    |---------------------------------------------|  INSPECTOR           |
|  ▼ src       |  1 | //! Incular Studio Application         |  Document: main.rs   |
|    main.rs   |  2 | use incular::prelude::*;               |  Status: Saved       |
|    state.rs  |  3 |                                        |  Lines: 152          |
|    theme.rs  |  4 | fn main() {                            |  Size: 4834 bytes    |
|  ▶ views     |  5 |     // Retained editing                |  Encoding: UTF-8     |
|              |---------------------------------------------|                      |
|              |  [🔍 Search]  [⚠️ Problems (2)]  [Output]     |                      |
|              |  Search in open buffers: [                     ]                    |
+-----------------------------------------------------------------------------------+
|  ⚡ Ready                      Line: 1, Column: 1   Spaces: 4   UTF-8   en   Dark  |  Status Bar
+-----------------------------------------------------------------------------------+
```

---

## 2. Structural & State Architecture

### Reactive Signals + Retained Controllers (No Monolithic Blobs)
The application adheres strictly to Incular's recommended state management pattern:
- **No Global `Arc<Mutex<AppState>>`**: Granular reactive `Signal<T>` primitives drive incremental, fine-grained rebuilds of only the affected visual components.
- **Persistent Retained Controllers**: Controllers (`TextEditingController`, `ScrollController`, `FocusNode`) are owned persistently in application state structs (`DocumentTab`, `SearchState`, `StudioState`) across frame redraws and never reallocated per rebuild.
- **Pure Public API**: The application does NOT access `Element` arenas, `RenderObject` trees, `Layer` internals, `ActionId` primitives, or platform `Winit` window handles.

```rust
pub struct DocumentTab {
    pub id: String,
    pub title: String,
    pub path: String,
    pub is_dirty: Signal<bool>,
    pub controller: TextEditingController,
    pub scroll_controller: ScrollController,
    pub focus_node: FocusNode,
    pub cursor_line: Signal<usize>,
    pub cursor_col: Signal<usize>,
    pub word_wrap: Signal<bool>,
}
```

---

## 3. Core Subsystems

### 1. Resizable Split Panes (`SplitView`)
- Implemented as a framework-neutral widget (`SplitView::horizontal` / `SplitView::vertical`) in `incular-widgets`.
- Dynamically partitions Sidebar, Editor Area, Inspector, and Bottom Panel with configurable min/max constraints and draggable dividers.

### 2. Virtualized Project Tree (`ListView::builder`)
- Virtualized viewport rendering thousands of file tree entries using on-demand item construction.
- Tested and verified with synthetic tree stress mode (`INCULAR_STUDIO_STRESS_TREE=100000`).
- Expand/collapse folders, file selection, active document synchronization.

### 3. Multi-Document Text Editor
- Tab bar with dirty status indicators, close buttons, and new buffer creation.
- Line-numbered gutter synchronized with text scrolling.
- `TextArea` multiline editing with retained `TextEditingController`.

### 4. Interactive Command Palette (`Stack` + Modal Overlay)
- Floating modal backdrop and palette box (`Ctrl+Shift+P`).
- Instant keyboard shortcut discovery and action dispatching.

### 5. Asynchronous Search & Problem Diagnostics
- Search panel querying open document buffers.
- Structured problem diagnostics categorized by severity (`Error`, `Warning`, `Info`).

### 6. Interactive Vector Canvas
- Vector preview canvas with zoom controls (`20%` to `500%`), display list transformations, and GPU compositor preview.

### 7. Full RTL Localization & Theming
- Runtime localization catalogs for English (`en`), Arabic (`ar` with `TextDirection::Rtl`), Hindi (`hi`), and Japanese (`ja`).
- Dynamic theme switching between `Dark`, `Light`, and `HighContrast` modes.

### 8. Atomic State Restoration (`directories` + JSON)
- Persists panel dimensions, active document, theme, locale, font size, and zoom level into standard OS application config directories.
- Writes to temporary files and atomically renames them to prevent corruption.

---

## 4. Verification & Stress Testing

Run Incular Studio:
```bash
cargo run -p incular --example studio
```

Stress testing modes:
```bash
# 100,000 project tree nodes
INCULAR_STUDIO_STRESS_TREE=100000 cargo run -p incular --example studio
```
