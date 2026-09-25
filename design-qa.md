# Native Electron benchmark UI comparison

Status: passed

Scope: native reproduction of the pinned Electron benchmark's layout, content,
colors, control dimensions and interactions at its required Windows viewport.
This is **not a pixel-identical rasterization pass**; the differences below remain.

## Evidence and comparison method

Reference: `benchmarks/desktop/reference/electron/web/index.html` and `app.ts`.
Implementation: `examples/issue_tracker/main.rs`.
Both clients are 1100 × 720 logical pixels at 150% scaling (1650 × 1080 pixels).
Both use the same 1,000 records, first selected issue and 100 mounted initial rows.
No browser zoom or native screenshot scaling was applied. Each comparison places
the full source and native capture side by side, with a 24-pixel label strip.

Evidence under `benchmarks/desktop/results/windows/electron-ui/`:

- `reference/`: fresh Electron captures and viewport metadata.
- `pass1/`: initial implementation with alignment defects.
- `pass2/`: corrected alignment, padding and native scrollbars.
- `pass3/`: on-demand glyph rasterization, same layout and line wrapping.
- `before-font-position/` and `font-position/`: fractional-size/phase correction;
  `font-comparison.png` shows Electron, before and after without resizing.
- `final/`: sharper text before upload batching.
- `batched/`: initial, completed filter, completion action, search and empty states;
  `compare-*.png` contain both implementations in the same image.
- `batched/compare-rows.png`, `compare-details.png`, `compare-toolbar.png`: focused crops.
- `batched/capture.log`: native interaction assertions.

The five `batched/` PNGs are byte-identical to the pre-batching `font-position/`
PNGs, verified by SHA-256 equality in `batched/pixel-identity.json`.

## Findings and fixes

| Priority | Finding | Resolution |
| --- | --- | --- |
| P1 | Original native example used a dark, differently sized three-panel layout. | Reproduced source light palette, 176/574/350 panel widths, 94 px toolbar and source content. |
| P1 | Open filtered out In progress, unlike Electron. | Open now means not Done; smoke asserts 800 Open and 200 Completed. |
| P2 | First pass placed toolbar, headers, pagination and row content at their top edges. | Matched flex alignment and intrinsic column heights. |
| P2 | Source details scroll independently; old completion and notes were always visible. | Restored scrollable details and below-fold action. Smoke scrolls before clicking. |
| P2 | Input controls added native inner padding on top of source padding. | Accounted for the editor's inset; corrected search and notes size and wrapping. |
| P2 | Scrollbar thumb missing with the initially selected wrapper. | Native visual scrollbar shares the viewport controller; arrows, track, drag and wheel work. |
| P2 | Filter/search/page changes retained an old list offset. | Reset to zero on all three actions and assert pagination reset. |
| P2 | Empty message appeared at the top; source places it below the flex list. | Matched the 64 px message region above pagination. |
| P2 | Single search result kept a 15 px gutter. | Reserve gutter only for a list that overflows, matching Electron. |
| P2 | Static labels differed. | Restored Orbit, Issue inbox, Working notes and session copy from source. |

No outstanding P0/P1/P2 layout or interaction defects were found in this viewport.
The following rendering differences remain and prevent an exact pixel-copy claim:

- **Text edges:** fractional ppem and rasterized quarter-pixel positions remove
  rounding error and repeated filtering. All three measured text crops move closer
  to Electron. Unhinted grayscale masks still differ from DirectWrite/Skia in edge
  coverage and stroke weight; [font evidence](benchmarks/desktop/FONT-RENDERING.md).
- **Focus outline:** native editing controls paint their focus border around the
  editor bounds; Chromium outlines the input's outer box.
- **Scrollbar arrows:** native triangle glyphs differ slightly from Chromium's
  painted arrows. Both support the corresponding scrolling actions.

## Functionality and accessibility

Smoke covers retained record/row counts, filter counts, pagination, scroll reset,
wheel scrolling, scrollbar arrows and thumb drag, completion/reopening, issue
selection, editing and persistence of notes, trimmed search and empty results.
Native semantic buttons, enabled/disabled pagination and editable controls remain
present. Focus is visible; text entry and Ctrl+A replacement are exercised.
The native scrollbars do not install timers; their visual updates subscribe to
actual scroll notifications. No images or external icons are needed by this UI.

## Viewport limits

This is a fixed-size desktop benchmark, as in the Windows reference host. Alternate
mobile widths and arbitrary browser zoom are outside this benchmark contract.
No new responsive website or web runtime was introduced.
