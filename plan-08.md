# Plan 08 - Rich Clipboard and External Drag/Drop

## Goal

Replace the text-only desktop data-transfer boundary with a typed, extensible data-transfer system shared by clipboard and native drag/drop.

Internal widget drag/drop remains separate; this plan handles data crossing application/native boundaries.

## Architecture

### Shared transfer model

Introduce a portable `DataTransfer`/`TransferItem` model with typed standard representations:

- UTF-8 plain text;
- URI list;
- file paths/handles where platform-safe;
- image payload/encoded image type;
- HTML/rich text where supported;
- custom MIME/UTI-like byte payload identified by a stable media type string.

Do not create separate incompatible clipboard and drop payload vocabularies.

Payloads should support lazy/providers where OS APIs require delayed rendering; avoid eagerly copying huge data into every representation.

### Clipboard

Evolve `Clipboard` from text-only methods into capability-based read/write of transfer items.

Keep ergonomic `get_text`/`set_text` convenience over the richer model, not a second implementation.

Use mature crates where they provide reliable cross-platform clipboard plumbing, but do not force the lowest-common-denominator if native adapters are needed for richer formats.

### External drag/drop

Normalize native drag lifecycle:

- enter;
- over/update;
- leave/cancel;
- drop;
- allowed/requested operation (`copy`, `move`, `link`) where available.

Winit file hover/drop events can seed file support, but the public design must not be permanently limited to files.

Add widget-level external drop target semantics that consume portable transfer data, distinct from existing typed local drag/drop.

## Hard invariants

1. Internal Incular drag/drop and OS data transfer remain separate protocols.
2. Text convenience APIs are implemented through the rich transfer model.
3. Large/binary payloads are not cloned gratuitously through the retained tree.
4. Native paths/data have explicit lifetime/ownership.
5. Unsupported representations are skipped/typed, not corrupted into text.
6. Drop operation negotiation never reports `move` unless the backend can honor it.

## Implementation sequence

1. Define transfer types/capabilities in `incular-platform`.
2. Refactor current text clipboard onto the model without regression.
3. Add image/HTML/custom representations where backend support exists.
4. Add external drag lifecycle/events starting with Winit files.
5. Add richer OS-native drag adapters if required.
6. Add neutral external drop target widgets and semantics.

## Tests

- Plain text compatibility.
- Multiple simultaneous representations of same clipboard item.
- Image/custom payload roundtrip in memory backend.
- File enter/hover/leave/drop lifecycle.
- Multiple-file drop ordering.
- Large lazy payload not materialized until requested.
- Unsupported MIME representation handled explicitly.

## Acceptance criteria

An Incular desktop app can accept files dropped from Explorer/Finder/file managers and can exchange richer clipboard content without platform code in the app.
