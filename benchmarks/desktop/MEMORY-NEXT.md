# Memory optimization evidence and next candidates

The measured changes and strict Windows results are in [AMD-HARDWARE.md](AMD-HARDWARE.md).
The current benchmark retains all 100 rows, uses the hardware GPU and keeps the
same client size. Working-set trimming and switching to software rendering are
not used to improve the numbers.

## Implemented and measured

| Change | Evidence |
| --- | --- |
| Decode font outlines only on glyph-cache misses | Same native light UI: private resident 154.99 → 129.39 MB; private commit 193.72 → 166.19 MB, three strict launches each. |
| Reduce the DX12 descriptor reservation | Earlier layout: private resident 171.24 → 141.04 MB; private commit 277.87 → 191.77 MB. |
| Small initial GPU allocation blocks and one queued frame | Earlier layout: private commit 191.77 → 177.74 MB; little resident change. |
| Reuse bounded glyph/font/image caches; create pipelines on first use | Retained resources are shared across windows; eviction and multi-window tests cover lifetime behavior. |
| Stop the reproduced AMD presentation spin | Initial diagnostic approximately 100% of one core; strict hardware runs now sample 0%. No periodic redraw workaround remains. |

These measurements isolate different changes and must not be added together as
if they came from one identical before/after workload.

## Upload batching added after the font-precision change

The sharper-font build exposed per-glyph and per-draw staging overhead. Glyph
copies now share a bounded upload buffer, and per-pass instance data uses at most
six stream writes. Five UI captures remain byte-identical. Three strict launches confirm 146.40 → 111.22 MB private resident and
185.47 → 148.59 MB private commit, with 0% sampled idle CPU. See
[GPU-UPLOADS.md](GPU-UPLOADS.md) and the hardware report for final comparisons.

| Additional change | Measured result |
| --- | --- |
| Batch glyph and per-pass vertex uploads | 35.18 MB less resident memory with byte-identical UI captures. |

## Candidates identified in code, not yet measured or implemented

1. **Share font source bytes with the parsed rasterizer.**
   `incular-assets::FontHandle` already owns `Arc<[u8]>`, but
   `incular-wgpu/src/glyph_rasterizer.rs` copies it into `FontVec`.
   The installed Arial regular and bold files total 2,035,500 bytes. A safely
   lifetime-bound borrowed table parser over the original shared allocation
   could avoid those copies without parsing tables on every cache miss.
   This is a byte-allocation opportunity, not a claimed resident-memory saving.
   Collection faces, cache eviction, custom font lifetime and multi-window use
   must remain correct.

2. **Allocate stencil attachments only for scenes that need them.**
   `create_stencil_attachment` creates a full-size `Depth24PlusStencil8`
   texture at window creation/resizing; offscreen render targets do likewise.
   Rectangular scissor-only scenes may not need it. This requires compatible
   render-pipeline variants and correct transitions when rounded/path clips
   first appear, plus resize, effects and multi-window tests. Changing the
   texture name to `Stencil8` alone does not establish a smaller DX12 allocation.

3. **Measure the real widget/text/layout working set before reducing retention.**
   Avoid duplicate retained strings/layouts or unnecessary per-row subscriptions
   where a profiler proves them. Visible-row virtualization is useful in real
   apps, but applying it only to Incular would change this benchmark's required
   100-retained-row workload.

4. **Right-size transient GPU resources from actual frame demand.**
   Investigate oversized instance buffers and offscreen targets with resource
   counters/allocation traces, preserving warm reuse to avoid reallocations and
   CPU churn during scrolling. Do not lower cache limits indiscriminately.

The fractional-font update reports one live 1024 × 1024 R8 atlas page and
525 rasterized glyph variants at startup. The scheduler's four idle snapshots
show identical frame/redraw counts: the improved font precision has not added an
idle rendering loop. [Profile log](results/validation-uploads/scheduler-profile.log).
