# Electron interface reproduced in native Incular

The benchmark example uses the pinned Electron implementation as its source:
[HTML/CSS](reference/electron/web/index.html),
[interaction logic](reference/electron/web/app.ts), and the same 1,000 records.
The Incular implementation remains native Rust widgets and WGPU rendering.

## Visual contract

Both windows have a 1100 × 720 logical client at Windows 150% scaling:
1650 × 1080 captured pixels. The benchmark intentionally fixes this viewport;
this is not a responsive mobile layout.

| Region | Source dimensions / behavior reproduced |
| --- | --- |
| Sidebar | 176 px; Orbit branding, workspace caption, three filters, cycle footer |
| Toolbar | 94 px; 24 px heading; live open/completed counts; 280 × 38 search |
| Inbox | 574 px; 48 px header; 68 px rows; 58 px pagination footer |
| Details | 350 px; 24 px padding; 21/28 heading; metadata; 13/20 description |
| Notes and action | 100 px textarea; 36 px completion button below the initial fold |
| Scrollbars | 15 px gutter when needed; wheel, arrow, track, and thumb controls |
| Typography | Arial, source sizes/weights/line heights, matching line wraps |
| Colors | Source light surfaces, separators, muted captions, blue selection/action |

Open includes both Open and In progress; Completed includes Done. Search trims
whitespace and matches ID, title, project and owner. Filters, search and pagination
reset list scroll. Selection is retained independently of the current result page.
Notes persist per issue for the session. Completion updates both filters and totals.
All 100 issue rows remain mounted on a full page; this change does not virtualize them.

## Evidence

- [Full comparison: Electron left, Incular right](results/bundle-dist/visuals/compare-issue-tracker.png)
- [Filtered](results/bundle-dist/visuals/compare-completed.png)
- [Completion](results/bundle-dist/visuals/compare-complete.png)
- [Search](results/bundle-dist/visuals/compare-search.png)
- [Empty state](results/bundle-dist/visuals/compare-empty.png)
- [Typography/detail crop](results/bundle-dist/visuals/compare-details.png)
- [Interaction smoke](results/bundle-dist/visuals/capture.log)
- [Upload batching preserves all five captures byte-for-byte](results/bundle-dist/pixel-identity.json)
- [Design QA](../../design-qa.md)

The reproduction is not pixel-identical: Chromium and Incular use different glyph
rasterization/antialiasing. Fractional sizing and raster positioning now remove
two sources of blur; [measured font comparison](FONT-RENDERING.md). Native focus
outlines and the small scrollbar arrow glyphs also differ from Chromium's painting.
These differences are visible in the comparison artifacts and are not hidden by
resizing the individual source screenshots.

## Reproduce captures

```powershell
cargo build --release -p incular --example issue_tracker --no-default-features --features desktop,controls
python benchmarks/desktop/capture.py --states --output benchmarks/desktop/results/windows/electron-ui/repeat
node benchmarks/desktop/capture-electron-states.cjs benchmarks/desktop/results/windows/electron-ui/reference-repeat
```

The Electron capture helper uses a separate debugging-enabled validation launch.
The memory collector launches the packaged application without debugging or capture
flags. No screenshots are taken during memory sampling.
