//! Renderer-neutral raster image resources and loading.
//!
//! This crate owns CPU image bytes and their stable identities. GPU upload,
//! sampling, display-list commands, and widget layout belong to their own
//! crates, preserving a one-way dependency from those consumers to images.

use std::{
    collections::{HashMap, VecDeque},
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

/// Practical bound on decoded image dimensions, each side. Images wider or
/// taller are rejected by [`ImageError::DecodeTooLarge`] before any pixel
/// buffer is requested. The value matches common GPU maximum texture
/// dimensions, so anything this policy accepts can still be sampled by the
/// renderer; anything larger could never reach the screen through Incular.
pub const MAX_DECODE_IMAGE_DIMENSION: u32 = 16_384;

/// Practical bound on RGBA8 output bytes per decode (256 MiB, e.g. an
/// 8192 x 8192 image). Checked with overflow-safe arithmetic against both
/// the container header dimensions and the actual decoded dimensions, so the
/// conversion to RGBA8 — which can expand sub-byte and luminance sources up
/// to fourfold — is covered by the same bound.
pub const MAX_DECODE_OUTPUT_BYTES: u64 = 256 * 1024 * 1024;

/// Image loading and decoding failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImageError {
    Io(String),
    Decode(String),
    InvalidDimensions,
    InvalidRgbaLength {
        expected: usize,
        actual: usize,
    },
    /// Rejected by the bounded decode policy ([`MAX_DECODE_IMAGE_DIMENSION`],
    /// [`MAX_DECODE_OUTPUT_BYTES`]) before the output pixel buffer is
    /// requested. `width`/`height` are the offending dimensions: the
    /// container header dimensions when the header parsed, the actual
    /// decoded dimensions when a decoder-internal allocation check fired
    /// first, and zero when the header itself was limit-rejected before
    /// dimensions were reportable through any decoder API. Nothing is
    /// admitted to any cache on this path, so a rejection never evicts the
    /// working set.
    DecodeTooLarge {
        width: u32,
        height: u32,
    },
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
            Self::DecodeTooLarge { width, height } => write!(
                f,
                "image {width}x{height} exceeds the decode policy ({MAX_DECODE_IMAGE_DIMENSION}px per side, {MAX_DECODE_OUTPUT_BYTES} output bytes)"
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
    /// Decoder limits expressing the [`MAX_DECODE_IMAGE_DIMENSION`] /
    /// [`MAX_DECODE_OUTPUT_BYTES`] policy through the decoder's own
    /// facilities. Strict dimensions fail fast at header time on decoders
    /// that read them eagerly (PNG); the explicit gate below covers every
    /// format unconditionally, including dimension limits the decoder only
    /// learns after construction and native allocations beyond the RGBA8
    /// output size (e.g. 16-bit sources).
    fn decode_policy_limits() -> image::Limits {
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(MAX_DECODE_IMAGE_DIMENSION);
        limits.max_image_height = Some(MAX_DECODE_IMAGE_DIMENSION);
        limits.max_alloc = Some(MAX_DECODE_OUTPUT_BYTES);
        limits
    }

    /// Rejects dimensions outside the decode policy with the offending size
    /// attached. The checked product covers the RGBA8 conversion output, so
    /// a passing result means both the native decode and the conversion fit
    /// the policy before any pixel buffer is requested.
    fn check_decode_policy(width: u32, height: u32) -> Result<(), ImageError> {
        let fits_dimensions =
            width <= MAX_DECODE_IMAGE_DIMENSION && height <= MAX_DECODE_IMAGE_DIMENSION;
        let fits_bytes = u64::from(width)
            .checked_mul(u64::from(height))
            .and_then(|pixels| pixels.checked_mul(4))
            .is_some_and(|bytes| bytes <= MAX_DECODE_OUTPUT_BYTES);
        if fits_dimensions && fits_bytes {
            Ok(())
        } else {
            Err(ImageError::DecodeTooLarge { width, height })
        }
    }

    /// Maps a full-decode failure, preserving decoder limit rejections as
    /// [`ImageError::DecodeTooLarge`] with the known dimensions instead of
    /// folding them into an opaque decode string.
    fn map_decode_error(error: image::ImageError, width: u32, height: u32) -> ImageError {
        match error {
            image::ImageError::Limits(_) => ImageError::DecodeTooLarge { width, height },
            other => ImageError::Decode(other.to_string()),
        }
    }

    /// Reads container header dimensions, parsing headers up to the image
    /// data without requesting any pixel buffer on any supported path
    /// (decoders report stored header fields; nothing is decompressed).
    /// Header/limit failures are returned, never discarded for a blind
    /// retry: a header this step cannot parse is not one the decode step
    /// could salvage with different limits.
    fn read_header_dimensions(bytes: &[u8]) -> Result<(u32, u32), ImageError> {
        let mut probe = image::ImageReader::new(std::io::Cursor::new(bytes))
            .with_guessed_format()
            .map_err(|error| ImageError::Decode(error.to_string()))?;
        // No allocation budget: this probe must never reserve pixel memory.
        // Limit rejections here carry no dimensions yet, so they surface as
        // a zero-sized policy rejection rather than an opaque string.
        probe.limits(image::Limits::no_limits());
        probe.into_dimensions().map_err(|error| match error {
            image::ImageError::Limits(_) => ImageError::DecodeTooLarge {
                width: 0,
                height: 0,
            },
            other => ImageError::Decode(other.to_string()),
        })
    }

    fn decode(bytes: &[u8], source: ImageSource) -> Result<Self, ImageError> {
        // One policy, one reader configuration, applied in order: header
        // probe (no pixel budget), explicit gate, then the full decode under
        // the same limits. `load_from_memory` cannot express this — it
        // decodes under the decoder defaults with no strict dimensions —
        // so the reader is configured directly instead.
        let (width, height) = Self::read_header_dimensions(bytes)?;
        Self::check_decode_policy(width, height)?;
        let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
            .with_guessed_format()
            .map_err(|error| ImageError::Decode(error.to_string()))?;
        reader.limits(Self::decode_policy_limits());
        let decoded = reader
            .decode()
            .map_err(|error| Self::map_decode_error(error, width, height))?;
        // The allocation above was already reserved against the policy, but
        // header and actual dimensions travel separately through decoders;
        // re-gate the actual size before converting so the RGBA8 output is
        // covered by checked arithmetic rather than by assumption.
        Self::check_decode_policy(decoded.width(), decoded.height())?;
        let (actual_width, actual_height) = (decoded.width(), decoded.height());
        Self::ready(
            source,
            DecodedImage::from_rgba8(
                actual_width,
                actual_height,
                Arc::<[u8]>::from(decoded.to_rgba8().into_raw()),
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

/// Budget for an application-owned decoded-image cache.
///
/// `max_bytes` counts cache-owned residency per entry: decoded RGBA8 pixel
/// bytes plus the retained encoded key bytes. It does not count memory held
/// by live handles cloned out of the cache — eviction releases the cache's
/// references, never memory still owned elsewhere. A zero entry or byte
/// limit disables admission (loads still decode fresh on every call).
///
/// Cache limits govern retained entries only. Decoding itself is governed
/// separately by [`MAX_DECODE_IMAGE_DIMENSION`] /
/// [`MAX_DECODE_OUTPUT_BYTES`]: a cache budget never changes what decodes,
/// so an image that fits the decode policy but exceeds the cache budget
/// still loads successfully — it is served fresh without admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImageCacheLimits {
    pub max_entries: usize,
    pub max_bytes: usize,
}

impl Default for ImageCacheLimits {
    fn default() -> Self {
        Self {
            max_entries: 64,
            max_bytes: 32 * 1024 * 1024,
        }
    }
}

impl ImageCacheLimits {
    #[must_use]
    pub const fn new(max_entries: usize, max_bytes: usize) -> Self {
        Self {
            max_entries,
            max_bytes,
        }
    }
}

/// Application-owned decoded-image cache. Equal byte payloads resolve to the
/// same immutable handle, so repeated widgets share one CPU resource.
///
/// Lookup compares full payload bytes (hash plus byte equality, never the
/// hash alone) and never copies the input on a hit. Admission is bounded by
/// [`ImageCacheLimits`]: entries larger than the byte budget are served
/// fresh without admission so one oversized image cannot evict the working
/// set, and failures are never cached. Eviction and [`Self::clear`] drop the
/// cache's references only — previously returned handles stay valid because
/// pixels and keys are reference-counted.
pub struct ImageCache {
    limits: ImageCacheLimits,
    images: HashMap<Arc<[u8]>, ImageHandle>,
    /// Insertion order (front is oldest) for oldest-first eviction. Touched
    /// entries move to the back, so churn evicts the least recently used.
    order: VecDeque<Arc<[u8]>>,
    resident_bytes: usize,
    diagnostics: ImageCacheDiagnostics,
}

impl Default for ImageCache {
    fn default() -> Self {
        Self::with_limits(ImageCacheLimits::default())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ImageCacheDiagnostics {
    pub loads_requested: u64,
    pub load_cache_hits: u64,
    pub load_failures: u64,
    pub image_decodes: u64,
    pub evictions: u64,
}

impl ImageCache {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_limits(limits: ImageCacheLimits) -> Self {
        Self {
            limits,
            images: HashMap::new(),
            order: VecDeque::new(),
            resident_bytes: 0,
            diagnostics: ImageCacheDiagnostics::default(),
        }
    }

    /// Replaces the budget and immediately evicts oldest-first down to it.
    /// Live handles previously returned are unaffected.
    pub fn set_limits(&mut self, limits: ImageCacheLimits) {
        self.limits = limits;
        self.evict_excess();
    }

    /// Drops every cached entry, releasing the cache's references. Live
    /// handles previously returned stay valid. Counters are cumulative and
    /// are not reset.
    pub fn clear(&mut self) {
        self.images.clear();
        self.order.clear();
        self.resident_bytes = 0;
    }

    #[must_use]
    pub const fn limits(&self) -> ImageCacheLimits {
        self.limits
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.images.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.images.is_empty()
    }

    /// Cache-owned resident bytes (decoded pixels plus encoded keys) as
    /// defined by [`ImageCacheLimits`]. Excludes memory retained only by
    /// live handles outside the cache.
    #[must_use]
    pub const fn resident_bytes(&self) -> usize {
        self.resident_bytes
    }

    pub fn load_bytes(&mut self, bytes: impl AsRef<[u8]>) -> Result<ImageHandle, ImageError> {
        self.diagnostics.loads_requested += 1;
        let bytes = bytes.as_ref();
        if let Some(image) = self.images.get(bytes) {
            let image = image.clone();
            self.touch(bytes);
            self.diagnostics.load_cache_hits += 1;
            return Ok(image);
        }
        match ImageHandle::from_bytes(bytes) {
            Ok(image) => {
                self.diagnostics.image_decodes += 1;
                self.admit(bytes, image.clone());
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

    /// Moves a present key to the back of the eviction order. The key must
    /// be present; callers check membership first.
    fn touch(&mut self, bytes: &[u8]) {
        if let Some(position) = self.order.iter().position(|key| &**key == bytes)
            && let Some(key) = self.order.remove(position)
        {
            self.order.push_back(key);
        }
    }

    /// Admits a freshly decoded image unless it cannot fit the budget, then
    /// evicts oldest-first back within the limits.
    fn admit(&mut self, bytes: &[u8], image: ImageHandle) {
        let entry_bytes = bytes.len().saturating_add(image.decoded().byte_len());
        if self.limits.max_entries == 0 || entry_bytes > self.limits.max_bytes {
            return;
        }
        let key: Arc<[u8]> = Arc::from(bytes);
        self.resident_bytes = self.resident_bytes.saturating_add(entry_bytes);
        self.images.insert(Arc::clone(&key), image);
        self.order.push_back(key);
        self.evict_excess();
    }

    /// Evicts oldest-first until both limits hold. The just-admitted entry
    /// sits at the back, so a fitting admission is never its own victim.
    fn evict_excess(&mut self) {
        while self.images.len() > self.limits.max_entries
            || self.resident_bytes > self.limits.max_bytes
        {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            if let Some(image) = self.images.remove(&oldest) {
                let entry_bytes = oldest.len().saturating_add(image.decoded().byte_len());
                self.resident_bytes = self.resident_bytes.saturating_sub(entry_bytes);
                self.diagnostics.evictions += 1;
            }
        }
    }
}
