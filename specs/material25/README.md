# Material Task 25 Wave 0 audit index

This directory contains the durable, read-only discovery record for Flutter
3.47.1 Material P0. The pinned source is commit
`6655482ec06e547f90abf8ae7590466f4415978d`; the canonical 551-row inventory is
[`../flutter_material_3471_parity.jsonl`](../flutter_material_3471_parity.jsonl).

The generated review projection is `target/material25/priority.jsonl`. It is
ephemeral build output and is recreated by `tools/generate_material_p0.py` from
the canonical metadata, including the explicit P1 overrides for pickers,
bottom-sheets, and segmented buttons. The checked-in generated P0 projection is
`../P0_MATERIAL_3471.jsonl`.

The audit is intentionally a deployability review, not a claim of all 551
Flutter declarations. The generated manifest identifies what is implemented,
what is platform/renderer-specific, and which retained Incular layer owns
behavior.

## Evidence conventions

`RUSTIFIED_IMPLEMENTED` and `MERGED_CONTROLS_IMPLEMENTED` identify ordinary
application-facing APIs. `DEFERRED_PLATFORM` is reserved for supporting
platform/renderer branches and is not a completion status for an ordinary P0
component. The Material package never adds Material concepts to
`incular-widgets` or `incular-controls`.
