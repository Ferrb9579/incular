# Distribution bundle size

The opt-in `dist` Cargo profile uses size optimization (`s`), ThinLTO, one codegen unit and symbol stripping. The regular release profile remains available for throughput comparisons. Panic unwinding is preserved because the runtime catches callback panics. GPU backends and application features are unchanged.

```powershell
cargo build --profile dist -p incular --example issue_tracker --no-default-features --features desktop,controls
powershell -NoProfile -File benchmarks/desktop/package-windows.ps1 -Profile dist
python benchmarks/desktop/capture.py --executable target/desktop-benchmark/dist-bundle/issue_tracker.exe --states --output benchmarks/desktop/results/bundle-dist/visuals
```

Measured installed bytes include the executable and VC runtime DLL; PDB files are excluded from both builds. The system Arial fonts and Windows-provided components are excluded from both.

| Build | Installed bundle |
| --- | ---: |
| Previous release | 16.79 MB |
| Size-optimized distribution, before profiler separation | 12.24 MB |
| Distribution, profiler code excluded | 12.17 MB |
| Distribution, build-time shader parsing | **11.79 MB** |

The current bundle is 11,789,752 bytes, approximately 29.8% smaller than the original release. Making GPU profiling truly optional saved 72,192 bytes. Moving built-in and WGPU-internal shader parsing to build time then saved another 375,296 bytes (3.1%) with the same distribution compiler settings and capabilities. See [shader implementation and validation](SHADER-PRECOMPILATION.md).

All five captured UI states are pixel-identical to the previous release; the initial capture clears hover deterministically. Exact bytes and hashes are in the [manifest](results/bundle-dist/bundle-manifest.json). The supplied chart lists QuickGUI Rust at 13.7 MB and GPUI at 6.3 MB on macOS; these are directional targets, not same-platform wins.

Validation includes the distribution build, five-state screenshot comparison and interaction smoke, formatting, workspace compilation, targeted WGPU tests on DX12/Vulkan/OpenGL, native-window regression checks and targeted Clippy. No full test suite was rerun. The existing 111.22 MB resident-memory result belongs to the previous release build; memory and frame throughput of the distribution profile have not been rebenchmarked.

Further size investigations should attribute linked code before changing capabilities: WGPU backend selection, text shaping/font fallback and image decoders. Optional GPU profiling has now been separated. Neither measured change establishes that Incular can reach the chart's GPUI size while retaining all native backends. No feature set was reduced for these measurements. Removing Vulkan fallback or supported codecs merely to lower this number would change the supported feature set.

[Cargo profile settings](https://doc.rust-lang.org/cargo/reference/profiles.html) document these build controls; size optimization is measured here rather than assumed to be smaller.
