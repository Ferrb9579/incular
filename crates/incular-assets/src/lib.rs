//! Asset and resource foundations for Incular.
//!
//! Asset identity, loading, caching, and lifecycle management for images,
//! fonts, icons, shaders, and other application resources will live here.

use std::{
    collections::HashMap,
    fmt,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

static NEXT_IMAGE_ID: AtomicU64 = AtomicU64::new(1);

/// Stable renderer-neutral identity for an immutable raster image resource.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ImageId(pub u64);
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImageSource {
    Embedded,
    Memory,
    FileSystem(PathBuf),
    Generated,
}
/// Decoded 8-bit sRGB source pixels in straight-alpha RGBA order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedImage {
    width: u32,
    height: u32,
    pixels: Arc<[u8]>,
}
impl DecodedImage {
    pub fn from_rgba8(
        width: u32,
        height: u32,
        pixels: impl Into<Arc<[u8]>>,
    ) -> Result<Self, AssetError> {
        let pixels = pixels.into();
        let expected = width
            .checked_mul(height)
            .and_then(|value| value.checked_mul(4))
            .ok_or(AssetError::InvalidDimensions)? as usize;
        if pixels.len() != expected {
            return Err(AssetError::InvalidRgbaLength {
                expected,
                actual: pixels.len(),
            });
        }
        Ok(Self {
            width,
            height,
            pixels,
        })
    }
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }
    #[must_use]
    pub fn pixels(&self) -> &Arc<[u8]> {
        &self.pixels
    }
    #[must_use]
    pub fn byte_len(&self) -> usize {
        self.pixels.len()
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssetError {
    Io(String),
    Decode(String),
    InvalidDimensions,
    InvalidRgbaLength { expected: usize, actual: usize },
}
impl fmt::Display for AssetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "image I/O error: {error}"),
            Self::Decode(error) => write!(f, "image decode error: {error}"),
            Self::InvalidDimensions => f.write_str("invalid image dimensions"),
            Self::InvalidRgbaLength { expected, actual } => write!(
                f,
                "invalid RGBA8 byte length: expected {expected}, got {actual}"
            ),
        }
    }
}
impl std::error::Error for AssetError {}
/// Cheap `Send + Sync` strong handle. Cloning does not read or copy pixels.
#[derive(Clone)]
pub struct ImageHandle(Arc<ImageResource>);
#[derive(Debug)]
struct ImageResource {
    id: ImageId,
    source: ImageSource,
    image: DecodedImage,
}
impl ImageHandle {
    pub fn from_bytes(bytes: impl AsRef<[u8]>) -> Result<Self, AssetError> {
        Self::decode(bytes.as_ref(), ImageSource::Memory)
    }
    pub fn embedded(bytes: impl AsRef<[u8]>) -> Result<Self, AssetError> {
        Self::decode(bytes.as_ref(), ImageSource::Embedded)
    }
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, AssetError> {
        let path = path.as_ref();
        let bytes = std::fs::read(path).map_err(|error| AssetError::Io(error.to_string()))?;
        Self::decode(&bytes, ImageSource::FileSystem(path.to_owned()))
    }
    pub fn from_rgba8(
        width: u32,
        height: u32,
        pixels: impl Into<Arc<[u8]>>,
    ) -> Result<Self, AssetError> {
        Self::ready(
            ImageSource::Generated,
            DecodedImage::from_rgba8(width, height, pixels)?,
        )
    }
    fn decode(bytes: &[u8], source: ImageSource) -> Result<Self, AssetError> {
        let decoded = image::load_from_memory(bytes)
            .map_err(|error| AssetError::Decode(error.to_string()))?
            .to_rgba8();
        Self::ready(
            source,
            DecodedImage::from_rgba8(
                decoded.width(),
                decoded.height(),
                Arc::<[u8]>::from(decoded.into_raw()),
            )?,
        )
    }
    fn ready(source: ImageSource, image: DecodedImage) -> Result<Self, AssetError> {
        if image.width == 0 || image.height == 0 {
            return Err(AssetError::InvalidDimensions);
        }
        Ok(Self(Arc::new(ImageResource {
            id: ImageId(NEXT_IMAGE_ID.fetch_add(1, Ordering::Relaxed)),
            source,
            image,
        })))
    }
    #[must_use]
    pub fn id(&self) -> ImageId {
        self.0.id
    }
    #[must_use]
    pub fn source(&self) -> &ImageSource {
        &self.0.source
    }
    #[must_use]
    pub fn decoded(&self) -> &DecodedImage {
        &self.0.image
    }
}
impl fmt::Debug for ImageHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ImageHandle")
            .field("id", &self.id())
            .field("source", self.source())
            .field("width", &self.decoded().width())
            .field("height", &self.decoded().height())
            .finish()
    }
}
impl PartialEq for ImageHandle {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}
impl Eq for ImageHandle {}

/// Application-owned decoded-image cache. Equal byte payloads resolve to the
/// same immutable handle, so repeated widgets share one CPU resource.
#[derive(Default)]
pub struct AssetCache {
    images: HashMap<Vec<u8>, ImageHandle>,
    diagnostics: AssetDiagnostics,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AssetDiagnostics {
    pub loads_requested: u64,
    pub load_cache_hits: u64,
    pub load_failures: u64,
    pub image_decodes: u64,
}
impl AssetCache {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    pub fn load_image_bytes(&mut self, bytes: impl AsRef<[u8]>) -> Result<ImageHandle, AssetError> {
        self.diagnostics.loads_requested += 1;
        let key = bytes.as_ref().to_vec();
        if let Some(image) = self.images.get(&key) {
            self.diagnostics.load_cache_hits += 1;
            return Ok(image.clone());
        }
        match ImageHandle::from_bytes(&key) {
            Ok(image) => {
                self.diagnostics.image_decodes += 1;
                self.images.insert(key, image.clone());
                Ok(image)
            }
            Err(error) => {
                self.diagnostics.load_failures += 1;
                Err(error)
            }
        }
    }
    #[must_use]
    pub const fn diagnostics(&self) -> AssetDiagnostics {
        self.diagnostics
    }
}

#[cfg(test)]
mod image_tests {
    use super::*;
    #[test]
    fn generated_rgba_is_shared_and_validated() {
        let image =
            ImageHandle::from_rgba8(2, 1, Arc::<[u8]>::from([1, 2, 3, 4, 5, 6, 7, 8])).unwrap();
        assert_eq!(image.decoded().width(), 2);
        assert_eq!(image.clone().id(), image.id());
        assert!(ImageHandle::from_rgba8(2, 1, Arc::<[u8]>::from([0; 7])).is_err());
    }
    #[test]
    fn odd_row_widths_keep_exact_rgba_row_layout() {
        for width in [1_u32, 3, 17, 63, 257] {
            let pixels = vec![127; (width * 4) as usize];
            let image = ImageHandle::from_rgba8(width, 1, pixels).unwrap();
            assert_eq!(image.decoded().byte_len(), (width * 4) as usize);
        }
    }
}

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
