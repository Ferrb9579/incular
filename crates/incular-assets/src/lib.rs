//! Asset and resource foundations for Incular.
//!
//! Asset identity, loading, caching, and lifecycle management for images,
//! fonts, icons, shaders, and other application resources will live here.

use std::{fmt, sync::Arc};

/// Stable renderer-independent identity for a loaded font asset.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FontId(pub u64);

/// Font bytes and their stable identity. Paths are deliberately not part of
/// the public identity: system discovery and bundled assets both produce this
/// same handle.
#[derive(Clone, PartialEq, Eq)]
pub struct FontHandle {
    id: FontId,
    bytes: Arc<[u8]>,
}
impl FontHandle {
    #[must_use]
    pub fn new(id: FontId, bytes: impl Into<Arc<[u8]>>) -> Self {
        Self {
            id,
            bytes: bytes.into(),
        }
    }
    #[must_use]
    pub const fn id(&self) -> FontId {
        self.id
    }
    #[must_use]
    pub fn bytes(&self) -> &Arc<[u8]> {
        &self.bytes
    }
}
impl fmt::Debug for FontHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FontHandle")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}
