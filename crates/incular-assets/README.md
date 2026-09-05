# incular-assets

## Architecture and support

| Contract | Status |
| --- | --- |
| Ownership | Font identities and shared font bytes. |
| API class | application; re-exports and path overrides follow the [architecture contract](../../docs/ARCHITECTURE.md). |
| Support | Partial resource foundation; no general image/font/shader loader. |

Font handles and future non-raster resource foundations for Incular. Raster
image identities, decoding, and caching live in the dedicated `incular-image`
crate, so renderers and widgets can depend on images without pulling in future
asset categories.
