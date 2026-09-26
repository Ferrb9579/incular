# Investigation of the 80 MB target

Status: **not achieved**. The latest strict three-launch result remains **109.54 MB private resident memory**, **148.05 MB private commit**, and **0% sampled idle CPU**. Installed bundle remains **10.99 MB**. These are decimal MB. See [MEMORY-DISK.md](MEMORY-DISK.md) for the validated application measurement and unchanged visual output.

## Measured graphics baseline

Short diagnostic probes on the same AMD Radeon 610M / DX12 machine, driver 32.0.21036.11002, produced:

| Isolated workload | Private resident MB | Private commit MB |
| --- | ---: | ---: |
| Native window | 2.17 | 2.87 |
| WGPU device | 39.65 | 59.60 |
| Surface configured | 32.67 | 52.31 |
| Triangle pipeline compiled | 33.01 | 52.92 |
| Triangle drawn and presented, 256 warmup frames | 93.96 | 116.91 |

Each row is a separate process. These short probes are **not** strict benchmark runs and do not establish an irreducible driver memory floor. The triangle warmup also uses a different surface size from the framework's tiny startup workaround. They localize a large increase to GPU submission/presentation rather than widget retention or shader compilation alone. All sampled idle CPU medians were zero.

The app's diagnostic allocator counted about **17.70 MB of live Rust heap allocations**. WGPU reported **13.04 MB live GPU resource allocations** within **18.55 MB reserved blocks**. These are different accounting domains and must not be added to, or subtracted from, resident RAM as if they were disjoint measurements. Native driver allocations are not counted by the Rust allocator.

Evidence: [GPU stages](results/memory-disk/under-80/gpu-stages.json), [configuration and compilation](results/memory-disk/under-80/gpu-configure-compile.json), [heap and resource snapshots](results/memory-disk/under-80/heap-and-resources.log).

## Experiments rejected

- GL backend: approximately 198 MB resident, worse than DX12.
- DX12 DirectComposition presentation: approximately 110 MB resident.
- Draining the AMD startup workaround after every present: no meaningful resident improvement.
- Reusing the startup swapchain: approximately 109 MB resident; reverted.
- Lazy window stencil attachment: this benchmark still requires a stencil attachment; reverted with its pipeline variants.
- Segment Heap manifest in a separate executable copy: approximately 109 MB resident and 143 MB commit. No shipping manifest change; the resident target was not improved. [Microsoft heapType documentation](https://learn.microsoft.com/en-us/windows/win32/sbscs/application-manifests#heaptype).

No feature, workload, rendering backend, or AMD idle-CPU workaround was removed. No working-set trimming was used. The shipping package and published strict result were not replaced by an experimental binary.

## Reproduction tools

The diagnostic-only `issue_tracker_memory_profile` example wraps the unchanged issue tracker with Rust allocation counters. `INCULAR_HEAP_STAGE=empty` or `text` isolates non-GPU startup; omit it for the full workload. It exits after 30 seconds and is not a process-memory benchmark executable.

The existing `wgpu_idle_probe` supports `INCULAR_GPU_PROBE_BINDINGS=65536` and `INCULAR_GPU_PROBE_SMALL_ALLOCATIONS=1` to match application device settings. Run `probe-gpu-stages.py` with the built probe, DX12 and stages `window device draw`; the recorded draw used `INCULAR_GPU_PROBE_WARMUP=256` and `INCULAR_GPU_PROBE_DRAIN=1`. Configuration/compilation probes omitted warmup.

The next useful experiment is a same-machine comparison of WGPU presentation/submission against a minimal native D3D12 implementation or a newer AMD driver. That can distinguish WGPU overhead from driver behavior before making a larger renderer change. The public GPUI figure is not a same-machine backend comparison.
