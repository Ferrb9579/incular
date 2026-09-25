# Avoiding one GPU staging allocation per draw or glyph

The fractional font correction increased initial glyph variants to 525, still
fitting one R8 atlas page. It exposed upload-allocation overhead: the identical
UI with sharper text measured **146.40 MB** private resident and **185.47 MB**
private commit across three strict launches, despite the atlas remaining small.
Unbatched baseline (historical artifact removed; latest evidence is in results/README.md).

## Cause and changes

`upload_glyph` called `Queue::write_texture` once per new mask, while
`upload_instance_data` called `Queue::write_buffer` once per draw batch.
[WGPU documents that these native writes allocate staging storage individually](https://docs.rs/wgpu/30.0.0/wgpu/struct.Queue.html#method.write_texture).
The upload allocation pressure was much larger than the final atlas contents.
This is not evidence of an unbounded leak: WGPU releases the temporary buffers
after submissions complete, while allocator/driver memory residency can persist.

- Glyphs now share an explicitly mapped upload buffer with a 4 MiB pending limit.
  Each glyph still copies a separate padded rectangle, preserving existing
  neighbors. There is no permanently retained CPU atlas mirror.
- Pending glyph uploads belong to the shared GPU context. Cache admission and
  staging happen under the same lock. Either window can flush before drawing;
  failed frames flush admitted glyphs before returning their error.
- Instance data now uses at most one write per nonempty vertex stream per pass
  (six possible streams), rather than one per draw batch. The draw order and
  offsets within each stream are unchanged.
- Temporary CPU upload arrays are released after submission. Warm glyphs do not
  upload again; idle windows do not submit additional frames.

## Validation

The five native screenshots—initial, Completed filter, completion action,
single search result and empty result—are **byte-for-byte identical** to the
sharper-font build before upload batching.
[Identity check](results/bundle-dist/pixel-identity.json),
[side-by-side UI comparison](results/bundle-dist/visuals/compare-issue-tracker.png).

Real-GPU tests cover sparse-copy pixels, transparent padding, neighboring masks,
old and replacement page textures, automatic budget flushing and copy order.
The native error-recovery test now admits text before a deliberately unsupported
effect and verifies that cached glyphs draw when the window recovers. Existing
multi-window churn/eviction and image/effect tests cover the shared resource path.

## Strict result

Three fresh launches of the final default/DX12 build all qualified under the
unchanged idle policy. Resident medians ranged from 111.19 to 111.42 MB.

| Same native UI and font pixels | Private resident MB | Private commit MB | Idle CPU, % of one core |
| --- | ---: | ---: | ---: |
| Individual glyph/draw uploads | 146.40 | 185.47 | 0.00 |
| Batched glyph and instance uploads | 111.22 | 148.59 | 0.00 |

This removes 35.18 MB (24.0%) resident and 36.88 MB (19.9%) private commit.
[Strict run data](results/windows/electron-ui-batched/incular-windows.json),
[validation logs](results/validation-uploads/status.json),
[hardware comparison](AMD-HARDWARE.md).

The intermediate glyph-only diagnostic measured 139.03 MB; the combined short
probe measured 111.19 MB. These exploratory probes are retained separately and
are not used in the strict table. The final installed native bundle is 16.79 MB
including the required VC runtime DLL; [file sizes and hashes](results/validation-uploads/bundle-manifest.json).
