# Windows Electron reference

The unchanged `electron/main.ts`, `web/app.ts`, and `web/index.html` come from
[QuickGUI's MIT benchmark](https://github.com/egoist/quickgui/tree/811d6e2816d5229711f59683c4c9dfbb6fc74133/benchmarks/desktop).
`provenance.json` records verified source hashes. The license is in
`../LICENSE-MIT`. The build reuses `../workload.ts` and Incular's identical
1,000-record `examples/issue_tracker/issues.json` dataset.

`windows-main.ts` replaces only the Electron host. Windows at 150% DPI initially
gave `useContentSize` a 1100 × 722 CSS viewport. The host corrects the measured
viewport to 1100 × 720 before loading the unchanged workload, and removes the
unused default menu. Sandbox and context isolation remain enabled. No GPU,
background scheduling, garbage collection, or memory flags are changed.

An optional `ELECTRON_CAPTURE` launch validates readiness, viewport and 100 DOM
rows and saves a screenshot. The collector clears this variable for measured
launches. Every launch uses the upstream first-selected ready state; notes from
separate validation runs are not carried into the workload's in-memory data.

From the repository root, with Node/npm and Python installed:

```powershell
python benchmarks/desktop/build-electron-windows.py
python benchmarks/desktop/run-electron-windows.py --executable 'target/desktop-benchmark/electron-package/Benchmark Electron-win32-x64/Benchmark Electron.exe'
```

The locked dependencies pin Electron 44.2.0, packager 20.3.0 and esbuild 0.25.12.
Build files and the unpacked application bundle stay under `target/`.
The collector sums private resident bytes and CPU across the browser and all
descendants. Summed total working sets include duplicate shared pages and are
not the comparison metric. All three launches must pass the same idle gate as
Incular; traces include process IDs so missing child processes are visible.
