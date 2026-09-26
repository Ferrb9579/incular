# Shared fonts and compressed application assets

Measured on 2026-09-26 on the same Windows AMD Radeon 610M host. Both binaries
use the same `dist` profile and capabilities. The baseline was rebuilt from
the committed production code before these changes. Three fresh launches per
binary passed the existing strict idle policy; no compilation ran during sampling.
All sizes below use decimal MB.

| Metric | Before | After | Reduction |
| --- | ---: | ---: | ---: |
| Private resident memory, median | 111.36 MB | 109.54 MB | 1.82 MB (1.6%) |
| Private committed memory, median | 151.62 MB | 148.05 MB | 3.57 MB (2.4%) |
| Idle CPU, percent of one core | 0.00% | 0.00% | unchanged |
| Installed EXE plus VC runtime DLL | 11.79 MB | 10.99 MB | 0.80 MB (6.8%) |

The resident run medians ranged from 110.68–111.68 MB before and
109.13–109.95 MB after. Commit measurements vary more: 151.13–152.51 MB
before and 147.26–152.14 MB after. These are process metrics, excluding
GPU/OS compositor allocations, and are not peak-memory or throughput results.
The retained Electron measurement is 104.40 MB private resident memory;
Incular still exceeds it by 5.14 MB. Electron was not rerun in this experiment.

## Framework memory change

The glyph rasterizer previously copied every font file into `FontVec` even
though `FontHandle` already held shared immutable bytes. It now keeps an
`Arc<[u8]>` and a borrowed `FontRef` in a lifetime-bound cell. Parsed tables
remain cached; outlines are decoded only on glyph misses, as before. The
two benchmark Arial files total 2,035,500 bytes that no longer need duplicate
source allocations. Cache limits, face indices and rendering are unchanged.

The [self_cell API](https://docs.rs/self_cell/1.3.0/self_cell/macro.self_cell.html)
ties the borrowed parser to its owner, including movement and destruction.
No handwritten unsafe lifetime conversion was introduced.

## Application bundle change

The example's 817,825-byte JSON dataset is embedded as 15,132 bytes of raw
DEFLATE data. The generator still writes the exact original JSON for Electron
and verifies its reference SHA-256. Incular decodes once, with a 1 MiB output
limit, parses the same 1,000 records, and drops the temporary JSON buffer before
building the UI. The final installed bundle is 10,992,056 bytes, down from
11,789,752 bytes. This asset reduction applies to the benchmark application;
it is not a claim that every Incular app shrinks by the same amount.

No backend, image codec, accessibility, localization or UI feature was removed.
The generator uses lossless compression; the integration test verifies exact
decoded bytes. The [bounded decompression API](https://docs.rs/miniz_oxide/0.8.9/miniz_oxide/inflate/fn.decompress_to_vec_with_limit.html)
avoids an unbounded decoded buffer.

## Evidence and validation

- [Before measurements and executable hash](results/memory-disk/before.json)
- [After measurements and executable hash](results/memory-disk/after.json)
- [Installed bundle manifest](results/bundle-dist/bundle-manifest.json)
- [Five identical screenshot states](results/memory-disk/pixel-identity.json)
- [Validation status](results/memory-disk/status.json)

Ten focused font tests and the dataset integrity test passed. Five-state
capture and interaction smoke passed with identical PNG hashes. Validation
also includes formatting, targeted renderer Clippy and workspace compilation.
The full test suite was not rerun.
