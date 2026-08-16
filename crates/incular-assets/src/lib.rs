//! Font handles and non-raster resource foundations for Incular.
//!
//! Raster image loading deliberately lives in `incular-image`, which keeps
//! renderer and widget users from depending on unrelated asset categories.

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
    face_index: u32,
}
impl FontHandle {
    #[must_use]
    pub fn new(id: FontId, bytes: impl Into<Arc<[u8]>>) -> Self {
        Self {
            id,
            bytes: bytes.into(),
            face_index: 0,
        }
    }
    /// Creates a handle for a face in an OpenType collection. The caller owns
    /// the identity: collection index must be included in `id`.
    #[must_use]
    pub fn with_face_index(id: FontId, bytes: impl Into<Arc<[u8]>>, face_index: u32) -> Self {
        Self {
            id,
            bytes: bytes.into(),
            face_index,
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
    #[must_use]
    pub const fn face_index(&self) -> u32 {
        self.face_index
    }
}
impl fmt::Debug for FontHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FontHandle")
            .field("id", &self.id)
            .field("face_index", &self.face_index)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collection_face_identity_is_retained() {
        let font = FontHandle::with_face_index(FontId(9), [1, 2, 3], 2);
        assert_eq!(font.id(), FontId(9));
        assert_eq!(font.face_index(), 2);
        assert_eq!(&**font.bytes(), &[1, 2, 3]);
    }
}
