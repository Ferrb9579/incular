# Desktop issue-tracker benchmark

Latest: [AMD hardware CPU fix and memory comparison](AMD-HARDWARE.md).

This reproduces the workload behind [QuickGUI's desktop charts](https://quickgui.dev/#benchmarks)
using Incular's public application API. See [RESULTS.md](RESULTS.md) for measured results.
See [AMD-HARDWARE.md](AMD-HARDWARE.md) for the hardware driver workaround,
DX12 memory reductions, and the current same-machine Electron comparison.
This Windows measurement is **not a same-machine comparison** with the published macOS data.

## Workload

- 1,000 identical in-memory issues; the embedded JSON's SHA-256 matches the upstream result.
- One 1100 × 720 logical-pixel window, first issue selected, first page of 100 retained rows.
- Search, All issues/Open/Completed filters, pagination, issue selection, editable per-issue notes, completion/reopening.
- All 100 row widgets are retained in a scroll view, with no list virtualization.
- Native Incular controls, WGPU rendering, desktop and controls features; no Material or DevTools.
- Stock workspace Cargo release profile, no LTO/size-optimization overrides.
- No database, networking, forced collection, cache purge, or interaction during sampling.

The native interface reproduces the pinned Electron HTML/CSS layout and interactions.
See [visual parity evidence](ELECTRON-UI.md) for comparisons and remaining rasterization differences.
The local desktop uses 150% scaling (1650 × 1080 physical client pixels).
The screenshot and interaction validation run in a separate launch, never during measured launches.
The readiness worker waits for three presented frames, writes a marker, then exits.

## Reproduce (Windows x64)

From the repository root, with Rust, Node.js and Python 3.9+:

```powershell
node benchmarks/desktop/generate-dataset.mjs
cargo build --release -p incular --example issue_tracker --no-default-features --features desktop,controls
powershell -NoProfile -File benchmarks/desktop/package-windows.ps1
$env:INCULAR_BENCH_SMOKE = '1'
python benchmarks/desktop/capture.py
Remove-Item Env:/INCULAR_BENCH_SMOKE
python benchmarks/desktop/run-windows.py --executable target/desktop-benchmark/bundle/issue_tracker.exe
```

Keep the app visible and do not interact during measurement. Avoid other builds or benchmarks.
The collector rejects hidden/minimized/exited windows and unexpected child processes.
Readiness has a 30-second deadline. Each measured launch allows 180 seconds after readiness.
After at least 30 seconds, require 15 seconds of CPU ≤1% of **one core** and memory spread
≤max(1 MB, 1% of the window median), then ten further one-second stable samples.
An unstable sample restarts the stability observation without extending the deadline.
Aggregate as the median of three launch medians, with their min/max range.
The Windows runner applies memory stability to private resident, total resident, and private commit.
No compiler, test suite, or screenshot capture is started alongside sampling by these scripts.

If strict idle measurement fails, the runner saves the trace and exits with an error.
For diagnosis, this command records three launches after a 30-second warmup **without qualifying them as idle**:

```powershell
python benchmarks/desktop/run-windows.py --diagnostic --executable target/desktop-benchmark/bundle/issue_tracker.exe --output benchmarks/desktop/results/windows/diagnostic
python benchmarks/desktop/report.py
```

### AMD hardware on the measured Windows host

The renderer uses smaller GPU allocation blocks and a smaller DX12 descriptor
heap. On the known Radeon 610M driver it also performs a finite startup
presentation bootstrap, allowing the driver's worker to sleep. Windows tries
DX12 first by default; explicit Vulkan also receives the targeted workaround.

```powershell
python benchmarks/desktop/run-windows.py --output benchmarks/desktop/results/windows/amd-repeat
python benchmarks/desktop/run-windows.py --backend vulkan --output benchmarks/desktop/results/windows/amd-vulkan-repeat
python benchmarks/desktop/run-windows.py --after-smoke --output benchmarks/desktop/results/windows/amd-interaction-repeat
```

The measurement runner selects the backend explicitly and records it in JSON;
it clears an inherited `WGPU_BACKEND` when `--backend default` is used. Keep
the original bundle/results for a reproducible baseline:

```powershell
python benchmarks/desktop/capture.py --output benchmarks/desktop/results/windows/amd-capture
python benchmarks/desktop/capture.py --smoke-only --output benchmarks/desktop/results/windows/amd-smoke
```

The earlier explicit GL workaround remains available through `--backend gl`.
GL on this host does not expose surface `COPY_SRC`, so simulation screenshots
are unavailable there. The interaction smoke test can run independently of
capture. Backend support for transparency/effects remains subject to the
normal renderer contracts; GL is not forced globally.

## Metrics and limitations

Windows private working set counts non-shareable resident pages using `QueryWorkingSet`.
Total working set and private committed memory come from `GetProcessMemoryInfo`.
CPU comes from the process kernel/user CPU times, divided by elapsed wall time, without dividing by core count.
These are **not macOS physical footprint**: they do not account for compressed or GPU memory the same way.
GPU/driver and OS compositor allocations outside the process are not included.
There are no renderer helper processes; the collector fails if unexpected child processes appear.

Bundle size is the exact byte sum of the release executable and its app-local Visual C++ runtime DLL.
JSON and fonts are embedded. PDB/debug symbols, installers, the benchmark runner, and OS DLLs are excluded.
`package-windows.ps1` snapshots the installed x64 runtime by default for local benchmarking;
`-RuntimeDll` can point to the matching Microsoft redistributable runtime for another environment.
The recorded DLL and EXE hashes identify exactly what was measured; this is not a distribution installer.
`dumpbin /DEPENDENTS` output records the static dependency audit.

Reference sources:

- [QuickGUI raw measurements](https://quickgui.dev/benchmarks/desktop-macos-arm64.json), retrieved 2026-09-22.
- [Upstream workload generator](https://github.com/egoist/quickgui/blob/main/benchmarks/desktop/workload.ts), copied under its [MIT license](reference/LICENSE-MIT).
- [Original benchmark implementation](https://github.com/egoist/quickgui/blob/main/scripts/benchmark-desktop.ts).
- [Windows working-set page flags](https://learn.microsoft.com/en-us/windows/win32/api/psapi/ns-psapi-psapi_working_set_block).
- [Windows process memory counters](https://learn.microsoft.com/en-us/windows/win32/api/psapi/ns-psapi-process_memory_counters_ex).

See [distribution bundle sizing](BUNDLE-SIZE.md) for the optional smaller build and [latest retained evidence](results/README.md).
