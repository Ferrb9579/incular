//! Renderer-neutral raster image resources and loading.
//!
//! This crate owns CPU image bytes and their stable identities. GPU upload,
//! sampling, display-list commands, and widget layout belong to their own
//! crates, preserving a one-way dependency from those consumers to images.

use std::{
    collections::HashMap,
    fmt,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use incular_config::{Locale, TextDirection};
use incular_core::Size;

static NEXT_IMAGE_ID: AtomicU64 = AtomicU64::new(1);

/// Configuration used when resolving an image provider.
///
/// This is an immutable description of the target widget environment. The
/// provider may use it to choose a density variant, while the default
/// [`ImageProvider::load_with`] implementation preserves existing providers
/// that only have one source asset.
#[derive(Clone, Debug, PartialEq)]
pub struct ImageConfiguration {
    pub size: Option<Size>,
    pub device_pixel_ratio: f64,
    pub text_direction: Option<TextDirection>,
    pub locale: Option<Locale>,
}

impl Default for ImageConfiguration {
    fn default() -> Self {
        Self {
            size: None,
            device_pixel_ratio: 1.0,
            text_direction: None,
            locale: None,
        }
    }
}

impl ImageConfiguration {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn size(mut self, size: Size) -> Self {
        self.size = Some(size);
        self
    }

    #[must_use]
    pub fn device_pixel_ratio(mut self, ratio: f64) -> Self {
        self.device_pixel_ratio = if ratio.is_finite() && ratio > 0. {
            ratio
        } else {
            1.0
        };
        self
    }

    #[must_use]
    pub fn text_direction(mut self, direction: TextDirection) -> Self {
        self.text_direction = Some(direction);
        self
    }

    #[must_use]
    pub fn locale(mut self, locale: Locale) -> Self {
        self.locale = Some(locale);
        self
    }

    #[must_use]
    pub fn normalized(mut self) -> Self {
        self.device_pixel_ratio =
            if self.device_pixel_ratio.is_finite() && self.device_pixel_ratio > 0. {
                self.device_pixel_ratio
            } else {
                1.0
            };
        self.size = self.size.map(|size| {
            Size::new(
                if size.width.is_finite() {
                    size.width.max(0.)
                } else {
                    0.
                },
                if size.height.is_finite() {
                    size.height.max(0.)
                } else {
                    0.
                },
            )
        });
        self
    }
}

/// Stable renderer-neutral identity for an immutable raster image resource.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ImageId(pub u64);

/// Origin metadata for a decoded image. It is diagnostic only and does not
/// affect image equality or renderer cache keys.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImageSource {
    Embedded,
    Memory,
    FileSystem(PathBuf),
    Generated,
}

/// A Rust-native image provider. Providers are cheap descriptions; decoding
/// happens only when [`Self::load`] is called, so rebuilds do not reread or
/// recopy image bytes.
pub trait ImageProvider: Clone + fmt::Debug + PartialEq + Eq {
    fn load(&self) -> Result<ImageHandle, ImageError>;

    /// Resolves this provider for a target environment. Providers with
    /// density- or locale-specific sources can override this; the common
    /// single-source case remains a zero-cost compatibility default.
    fn load_with(&self, _configuration: &ImageConfiguration) -> Result<ImageHandle, ImageError> {
        self.load()
    }
}

/// Loads an image from an application asset path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetImage(PathBuf);

impl AssetImage {
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl ImageProvider for AssetImage {
    fn load(&self) -> Result<ImageHandle, ImageError> {
        ImageHandle::from_file(&self.0)
    }
}

/// Loads an image from encoded in-memory bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryImage(Arc<[u8]>);

impl MemoryImage {
    #[must_use]
    pub fn new(bytes: impl Into<Arc<[u8]>>) -> Self {
        Self(bytes.into())
    }

    #[must_use]
    pub fn bytes(&self) -> &Arc<[u8]> {
        &self.0
    }
}

impl ImageProvider for MemoryImage {
    fn load(&self) -> Result<ImageHandle, ImageError> {
        ImageHandle::from_bytes(&*self.0)
    }
}

/// Loads an image from a filesystem path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileImage(PathBuf);

impl FileImage {
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl ImageProvider for FileImage {
    fn load(&self) -> Result<ImageHandle, ImageError> {
        ImageHandle::from_file(&self.0)
    }
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
    ) -> Result<Self, ImageError> {
        let pixels = pixels.into();
        let expected = width
            .checked_mul(height)
            .and_then(|value| value.checked_mul(4))
            .ok_or(ImageError::InvalidDimensions)? as usize;
        if pixels.len() != expected {
            return Err(ImageError::InvalidRgbaLength {
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

/// Image loading and decoding failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImageError {
    Io(String),
    Decode(String),
    InvalidDimensions,
    InvalidRgbaLength { expected: usize, actual: usize },
}
impl fmt::Display for ImageError {
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
impl std::error::Error for ImageError {}

/// Cheap `Send + Sync` strong handle. Cloning never reads or copies pixels.
#[derive(Clone)]
pub struct ImageHandle(Arc<ImageResource>);
#[derive(Debug)]
struct ImageResource {
    id: ImageId,
    source: ImageSource,
    image: DecodedImage,
}
impl ImageHandle {
    pub fn from_bytes(bytes: impl AsRef<[u8]>) -> Result<Self, ImageError> {
        Self::decode(bytes.as_ref(), ImageSource::Memory)
    }
    pub fn embedded(bytes: impl AsRef<[u8]>) -> Result<Self, ImageError> {
        Self::decode(bytes.as_ref(), ImageSource::Embedded)
    }
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, ImageError> {
        let path = path.as_ref();
        let bytes = std::fs::read(path).map_err(|error| ImageError::Io(error.to_string()))?;
        Self::decode(&bytes, ImageSource::FileSystem(path.to_owned()))
    }
    pub fn from_rgba8(
        width: u32,
        height: u32,
        pixels: impl Into<Arc<[u8]>>,
    ) -> Result<Self, ImageError> {
        Self::ready(
            ImageSource::Generated,
            DecodedImage::from_rgba8(width, height, pixels)?,
        )
    }
    fn decode(bytes: &[u8], source: ImageSource) -> Result<Self, ImageError> {
        let decoded = image::load_from_memory(bytes)
            .map_err(|error| ImageError::Decode(error.to_string()))?
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
    fn ready(source: ImageSource, image: DecodedImage) -> Result<Self, ImageError> {
        if image.width == 0 || image.height == 0 {
            return Err(ImageError::InvalidDimensions);
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
pub struct ImageCache {
    images: HashMap<Vec<u8>, ImageHandle>,
    diagnostics: ImageCacheDiagnostics,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ImageCacheDiagnostics {
    pub loads_requested: u64,
    pub load_cache_hits: u64,
    pub load_failures: u64,
    pub image_decodes: u64,
}
impl ImageCache {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    pub fn load_bytes(&mut self, bytes: impl AsRef<[u8]>) -> Result<ImageHandle, ImageError> {
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
    pub const fn diagnostics(&self) -> ImageCacheDiagnostics {
        self.diagnostics
    }
}

#[cfg(test)]
mod tests {
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
    fn cache_reuses_a_successfully_decoded_payload() {
        // This 2x2 PNG is deliberately decoded through the public cache API.
        const PNG: &[u8] = &[
            137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 2, 0, 0, 0, 2,
            8, 6, 0, 0, 0, 114, 182, 13, 36, 0, 0, 0, 24, 73, 68, 65, 84, 120, 156, 5, 193, 129, 1,
            0, 0, 4, 192, 160, 248, 220, 229, 83, 34, 105, 71, 226, 30, 63, 110, 6, 127, 180, 47,
            0, 167, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
        ];
        let mut cache = ImageCache::new();
        let first = cache.load_bytes(PNG).unwrap();
        let second = cache.load_bytes(PNG).unwrap();
        assert_eq!(first, second);
        assert_eq!(cache.diagnostics().image_decodes, 1);
        assert_eq!(cache.diagnostics().load_cache_hits, 1);
    }

    #[test]
    fn memory_provider_decodes_on_demand() {
        const PNG: &[u8] = &[
            137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 2, 0, 0, 0, 2,
            8, 6, 0, 0, 0, 114, 182, 13, 36, 0, 0, 0, 24, 73, 68, 65, 84, 120, 156, 5, 193, 129, 1,
            0, 0, 4, 192, 160, 248, 220, 229, 83, 34, 105, 71, 226, 30, 63, 110, 6, 127, 180, 47,
            0, 167, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
        ];
        let provider = MemoryImage::new(Arc::<[u8]>::from(PNG));
        assert_eq!(provider.load().unwrap().decoded().width(), 2);
    }

    #[test]
    fn image_configuration_resolution() {
        let configuration = ImageConfiguration::new()
            .size(incular_core::Size {
                width: -10.,
                height: f32::NAN,
            })
            .device_pixel_ratio(f64::NAN)
            .text_direction(TextDirection::Rtl)
            .locale("ar".parse().expect("valid locale"))
            .normalized();
        assert_eq!(configuration.size, Some(incular_core::Size::ZERO));
        assert_eq!(configuration.device_pixel_ratio, 1.);
        assert_eq!(configuration.text_direction, Some(TextDirection::Rtl));
        assert_eq!(
            configuration.locale.as_ref().map(ToString::to_string),
            Some("ar".into())
        );
    }

    #[test]
    fn provider_load_with_preserves_single_source_resolution() {
        let provider = MemoryImage::new(Arc::<[u8]>::from([
            137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 2, 0, 0, 0, 2,
            8, 6, 0, 0, 0, 114, 182, 13, 36, 0, 0, 0, 24, 73, 68, 65, 84, 120, 156, 5, 193, 129, 1,
            0, 0, 4, 192, 160, 248, 220, 229, 83, 34, 105, 71, 226, 30, 63, 110, 6, 127, 180, 47,
            0, 167, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
        ]));
        let configuration = ImageConfiguration::new().device_pixel_ratio(2.);
        assert_eq!(
            provider
                .load_with(&configuration)
                .unwrap()
                .decoded()
                .height(),
            2
        );
    }
}
