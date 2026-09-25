# AMD hardware idle CPU and memory investigation

Measured September 23–25, 2026 on Windows 11 build 26200, Ryzen 9 8940HX,
Radeon 610M, driver 32.0.21036.11002. MB are decimal. The changes here retain AMD hardware rendering; earlier software-adapter probes are superseded.

## Findings

Four independent costs were identified:

1. **AMD presentation-worker spin.** A standalone Winit/WGPU program, with no
   Incular scheduler, widgets, text or compositor, reproduces a full busy core
   after drawing and presenting one triangle. Creating a device, compiling a
   shader, clearing/presenting, or drawing without presenting all idle at 0%.
   Vulkan and DX12 reproduce it. Native OpenGL does not.
2. **An oversized DX12 descriptor reservation.** WGPU's default
   `max_non_sampler_bindings = 1_000_000` reserves a large heap at device
   creation. [WGPU documents the integrated-GPU RAM cost explicitly](https://docs.rs/wgpu/30.0.0/wgpu/struct.Limits.html#structfield.max_non_sampler_bindings).
   Incular now requests 65,536 simultaneous non-sampler bindings per shared
   device. This is a device-wide capacity, not a row, widget or glyph limit.
   Applications retaining more simultaneous GPU bindings would need a larger
   device limit; the heap does not grow automatically beyond this capacity.
3. **Eager font outline retention.** The previous rasterizer parsed and retained
   outlines for every mapped glyph when a font was first used. The matching
   Electron UI uses regular and bold Arial. It measured 154.99 MB resident with
   that rasterizer. An on-demand outline rasterizer reduced this to 129.39 MB
   across three strict launches, with the same line wrapping and glyph atlas.
   Only font tables/source bytes are retained; each outline is transient.
   [ab_glyph documents its per-glyph outline and coverage API](https://docs.rs/ab_glyph/0.2.32/ab_glyph/struct.OutlinedGlyph.html).
   Independent coverage/baseline tests compare against the former rasterizer at
   1x, 1.5x and 2x, alongside the existing font-cache and DPI tests.

4. **Many small GPU upload allocations.** Each glyph texture write and each draw
   batch buffer write created separate staging allocations. Shared glyph staging
   now packs sparse rectangles into bounded uploads, and instance data is written
   once per nonempty stream. With the improved fractional-size/subpixel fonts,
   this reduces resident memory from 146.40 to 111.22 MB (24.0%) without changing
   any pixels in five captured UI states. See [GPU upload evidence](GPU-UPLOADS.md)
   and [font rendering](FONT-RENDERING.md).

[AMD driver issue #106](https://github.com/GPUOpen-Drivers/AMD-Gfx-Drivers/issues/106)
reports the same idle-worker behavior and that continued presentation lets it
settle. Its reported game-profile trigger was not established on this host.
No driver installation, registry edits or global graphics-settings changes
were made.

## Implemented workaround

The tested driver stays busy after 128 presentations, but becomes idle after
256. Incular now makes 256 one-pixel draws/presentations on its temporary
adapter-selection surface before creating the normal renderer. Every eight
submissions it drains the queue, with a two-second poll timeout, to bound queued
resources. It then drops the temporary surface and pipeline. The UI is not
rebuilt 256 times and no periodic redraw or CPU timer remains installed.

This automatically applies only on Windows, AMD vendor `0x1002`, device
`0x164e`, and the reproduced DX12 driver `32.0.21036.11002` or Vulkan driver
info prefix `25.10.36.11 `. Other driver versions/devices do not incur this
work. This is an application-side workaround, not a repair of AMD's driver.
Unexpected initialization failures return typed renderer errors.

The standalone probe confirmed that the worker stays idle after a pause,
dropping/recreating the surface, restoring full resolution and drawing again.
The one-pixel bootstrap took approximately 0.43–0.56 seconds including process,
window and device creation in that probe. Full-app readiness is recorded
separately in the newer measurement JSON files; cold startup can take longer.

Windows now tries DX12 alone first, avoiding initialization of unused graphics
backends. If it cannot meet the surface contract, or finds only a CPU adapter,
selection retries WGPU's broader backend set. Explicit `WGPU_BACKEND` and
`WGPU_ADAPTER_NAME` choices remain honored. Small 4 MiB initial allocation
blocks can grow to 64 MiB, and the normal swapchain requests one queued frame.
These latter changes reduced private commit here, with little resident change.
One queued frame reduces pipeline depth; active animation throughput has not
been benchmarked by this idle workload.

## Matching visual workload

The final native interface is reproduced from the pinned Electron HTML/CSS and
TypeScript: [visual comparisons and scope](ELECTRON-UI.md). Older optimization
rows below use the previous dark layout; they isolate earlier changes but should
not be mistaken for the final visual-parity build. The two matching-layout font
runs isolate the rasterizer change: before (historical artifact removed; latest evidence is in results/README.md)
and after (historical artifact removed; latest evidence is in results/README.md).

## Same-machine comparison

All strict results use three fresh launches, the same 1,000-issue workload,
100 retained rows, 1100 × 720 logical client, at least 30 seconds warmup,
15 seconds stable memory and <=1% of one CPU core, then ten further samples.
No working-set trimming, cache purge, hidden window or disabled GPU flags.

| Configuration | Private resident MB | Private committed MB | Idle CPU, % of one core | Strict launches |
| --- | ---: | ---: | ---: | --- |
| Original AMD/default diagnostic | 299.02 | 535.89 | 99.65 | Failed |
| AMD DX12, presentation workaround | 171.24 | 277.87 | 0.00 | 3/3 |
| AMD DX12, smaller descriptor heap | 141.04 | 191.77 | 0.00 | 3/3 |
| AMD DX12, small blocks + one queued frame (old layout) | 140.98 | 177.74 | 0.00 | 3/3 |
| Matching Electron layout, eager font outlines | 154.99 | 193.72 | 0.00 | 3/3 |
| Matching Electron layout, on-demand font outlines | 129.39 | 166.19 | 0.00 | 3/3 |
| Fractional-size/position fonts, unbatched uploads | 146.40 | 185.47 | 0.00 | 3/3 |
| Current Incular, same fonts + batched uploads | **111.22** | **148.59** | **0.00** | **3/3** |
| Electron 44.2.0, fresh whole-process-tree measurement | **104.40** | **196.98** | **0.00** | **3/3** |

Current Incular and Electron measurements were collected on September 25, 2026,
with the same visible workload on this AMD 610M host. The preceding rows are
historical optimization stages; the original diagnostic and early stages used
the older layout. Values are decimal MB and medians of three run medians.

Upload batching alone saves **35.18 MB resident (24.0%)** and **36.88 MB private
commit (19.9%)** versus the identical sharper-font build. Incular still uses
**6.82 MB more private resident memory than Electron**, so that target is not
met. Its private commit is 48.39 MB lower. Installed bundles are 16.79 MB for
Incular (including the VC runtime DLL) and 386.14 MB for Electron.

Zero sampled idle CPU is not a claim about animation, scrolling throughput or
total GPU memory. GPU and OS compositor memory outside the measured process tree
are excluded. The earlier WARP result must not replace the hardware result.
The earlier `amd-final-electron` refresh failed its stability deadline; the fresh
Electron result above passed all three launches.

QuickGUI's [published chart](https://quickgui.dev/#benchmarks) uses macOS physical
footprint. Its Electron 136.1 MB is not the same metric as Windows private
working set, so the locally measured Electron process tree is the comparator.

## Reproduction and evidence

```powershell
cargo build --release -p incular --example issue_tracker --no-default-features --features desktop,controls
python benchmarks/desktop/run-windows.py --output benchmarks/desktop/results/windows/amd-repeat
python benchmarks/desktop/run-windows.py --backend vulkan --output benchmarks/desktop/results/windows/amd-vulkan-repeat
python benchmarks/desktop/run-windows.py --after-smoke --output benchmarks/desktop/results/windows/amd-interaction-repeat
python benchmarks/desktop/capture.py --output benchmarks/desktop/results/windows/amd-capture
```

The post-smoke runs validate idle after search, filters, pagination, completion,
selection and note editing. Their final scene differs from the initial workload
and must not be used as the headline comparison. Current post-interaction runs
passed 3/3, with medians of 115.94 MB resident, 155.38 MB private commit and
0.00% idle CPU. A separate short Vulkan diagnostic also sampled 0.00% CPU;
it is not a strict three-launch memory result.

- CPU workaround runs (historical artifact removed; latest evidence is in results/README.md)
- Descriptor heap runs (historical artifact removed; latest evidence is in results/README.md)
- Small-block/latency runs (historical artifact removed; latest evidence is in results/README.md)
- [Current Incular strict runs](results/windows/electron-ui-batched/incular-windows.json)
- [Fresh Electron strict runs](results/windows/electron-ui-electron-final/electron-windows.json)
- [Current post-interaction runs](results/windows/electron-ui-batched-after-smoke/incular-windows.json)
- [Current bundle manifest](results/validation-uploads/bundle-manifest.json)
- Tiny surface bootstrap (historical artifact removed; latest evidence is in results/README.md)
- Surface recreation and later draws (historical artifact removed; latest evidence is in results/README.md)
- Clear-only initialization does not work (historical artifact removed; latest evidence is in results/README.md)
- [Current workspace and GPU validation](results/validation-uploads/status.json)
- Earlier AMD validation (historical artifact removed; latest evidence is in results/README.md)

The manual probe lives in `crates/incular-desktop/tests/wgpu_idle_probe.rs`.
It runs only when `INCULAR_GPU_PROBE_STAGE` is set. Build with
`cargo test --release -p incular-desktop --test wgpu_idle_probe --no-run`, then
pass the emitted executable to `benchmarks/desktop/probe-gpu-stages.py`.
For the surface-recreation experiment set `INCULAR_GPU_PROBE_TINY=1`,
`INCULAR_GPU_PROBE_WARMUP=256`, `INCULAR_GPU_PROBE_PACE_MS=0`,
`INCULAR_GPU_PROBE_DRAIN=1`, `INCULAR_GPU_PROBE_RESTORE=1`, and
`INCULAR_GPU_PROBE_RECREATE=1`, and run `--backends vulkan dx12 --stages draw`.
These probe measurements use a short diagnostic window, not the strict
application benchmark policy.
