use super::prelude::*;
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GlyphCacheKey {
    pub font: FontId,
    pub glyph: u16,
    /// Rounded physical ppem. Fontdue accepts a scalar size, while the cache
    /// remains deterministic across equal logical size/DPI requests.
    pub physical_size: u16,
}
/// Physical-pixel class used to select and explain raster behavior.  These
/// bounds are intentionally expressed in ppem, never logical widget pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GlyphSizeClass {
    Micro,
    Small,
    Normal,
    Large,
    Huge,
}

pub(crate) const fn size_class(ppem: u16) -> GlyphSizeClass {
    match ppem {
        0..=7 => GlyphSizeClass::Micro,
        8..=15 => GlyphSizeClass::Small,
        16..=95 => GlyphSizeClass::Normal,
        96..=255 => GlyphSizeClass::Large,
        _ => GlyphSizeClass::Huge,
    }
}

/// Which retained atlas pool owns a mask.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum GlyphAtlasClass {
    #[default]
    Normal,
    Oversize,
}

/// The renderer's DPI-specific request for a grayscale glyph mask.
///
/// Layout and shaping stay in logical pixels; only this request enters the
/// physical-pixel raster cache.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlyphRasterRequest {
    pub logical_font_size: f32,
    pub scale_factor: f32,
    pub physical_size: u16,
    pub supported: bool,
}
impl GlyphRasterRequest {
    #[must_use]
    pub fn new(logical_font_size: f32, scale_factor: f64) -> Self {
        let scale_factor = normalized_scale(scale_factor);
        let physical_size = logical_font_size * scale_factor;
        Self {
            logical_font_size,
            scale_factor,
            physical_size: physical_size.round().clamp(1., f32::from(u16::MAX)) as u16,
            supported: physical_size.is_finite()
                && (1. ..=MAX_GLYPH_RASTER_PPEM).contains(&physical_size),
        }
    }
}
/// Deterministic information for diagnosing a cached glyph without logging on
/// the hot path. Atlas rectangles are physical texels; font size is logical.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlyphRasterDebug {
    pub font: FontId,
    pub glyph: u16,
    pub logical_font_size: f32,
    pub scale_factor: f32,
    pub requested_physical_size: u16,
    pub atlas_class: GlyphAtlasClass,
    pub bitmap_size: [u16; 2],
    pub bitmap_bytes: usize,
    pub bearing: [i16; 2],
    pub atlas_page: u16,
    pub allocation_rect: [u16; 4],
    pub content_rect: [u16; 4],
    pub uv_rect: [f32; 4],
}
/// Glyph coverage is sampled with bilinear filtering. Padding is intentionally
/// exposed for diagnostics and atlas invariant tests.
pub const GLYPH_ATLAS_PADDING: u16 = ATLAS_PADDING;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AtlasEntry {
    pub page: u16,
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    pub atlas_class: GlyphAtlasClass,
    /// Physical-pixel raster bounds relative to the shaped baseline origin.
    pub bearing_x: i16,
    pub bearing_y: i16,
}
impl AtlasEntry {
    #[must_use]
    pub fn uv_rect(self) -> [f32; 4] {
        let page = f32::from(ATLAS_PAGE_SIZE);
        [
            f32::from(self.x) / page,
            f32::from(self.y) / page,
            f32::from(self.x + self.width) / page,
            f32::from(self.y + self.height) / page,
        ]
    }
    #[must_use]
    pub fn allocation_rect(self) -> [u16; 4] {
        [
            self.x - ATLAS_PADDING,
            self.y - ATLAS_PADDING,
            self.width + ATLAS_PADDING * 2,
            self.height + ATLAS_PADDING * 2,
        ]
    }
}
#[derive(Clone, Debug)]
pub struct RasterizedGlyph {
    pub entry: AtlasEntry,
    pub bitmap: Option<Vec<u8>>,
}
/// Compact diagnostic for checking that a coverage mask has not accidentally
/// become binary during rasterization.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CoverageHistogram {
    pub transparent: usize,
    pub opaque: usize,
    pub intermediate: usize,
}

#[must_use]
pub fn coverage_histogram(bytes: &[u8]) -> CoverageHistogram {
    let mut histogram = CoverageHistogram::default();
    for &value in bytes {
        match value {
            0 => histogram.transparent += 1,
            u8::MAX => histogram.opaque += 1,
            _ => histogram.intermediate += 1,
        }
    }
    histogram
}

#[derive(Default)]
struct AtlasPage {
    next_x: u16,
    next_y: u16,
    row_height: u16,
    class: GlyphAtlasClass,
    content_area: u32,
    allocated_area: u32,
}
impl AtlasPage {
    fn normal() -> Self {
        Self {
            class: GlyphAtlasClass::Normal,
            ..Self::default()
        }
    }
    fn oversize() -> Self {
        Self {
            class: GlyphAtlasClass::Oversize,
            ..Self::default()
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GlyphAtlasMemory {
    pub normal_pages: u16,
    pub normal_bytes: usize,
    pub normal_content_area: u64,
    pub normal_allocated_area: u64,
    pub oversize_pages: u16,
    pub oversize_bytes: usize,
    pub oversize_content_area: u64,
    pub oversize_allocated_area: u64,
}
/// CPU metadata for retained atlas pages. `WgpuRenderer` maps each page index
/// lazily to one persistent `R8Unorm` texture; entries never move or compact.
pub struct GlyphAtlas {
    pages: Vec<AtlasPage>,
    entries: HashMap<GlyphCacheKey, AtlasEntry>,
    /// Parsed once per stable font identity, then reused for every uncached
    /// glyph bitmap. `FontSettings::collection_index` preserves TTC/OTC faces.
    fonts: HashMap<FontId, Font>,
    counters: GpuCounters,
}
impl Default for GlyphAtlas {
    fn default() -> Self {
        Self::new()
    }
}
impl GlyphAtlas {
    #[must_use]
    pub fn new() -> Self {
        Self {
            pages: vec![AtlasPage::normal()],
            entries: HashMap::new(),
            fonts: HashMap::new(),
            counters: GpuCounters {
                glyph_atlas_pages: 1,
                ..GpuCounters::default()
            },
        }
    }
    #[must_use]
    pub const fn counters(&self) -> GpuCounters {
        self.counters
    }
    #[must_use]
    pub(crate) const fn page_count(&self) -> usize {
        self.pages.len()
    }
    #[must_use]
    pub fn entry(&self, key: GlyphCacheKey) -> Option<AtlasEntry> {
        self.entries.get(&key).copied()
    }
    #[must_use]
    pub fn memory(&self) -> GlyphAtlasMemory {
        let mut memory = GlyphAtlasMemory::default();
        let page_bytes = usize::from(ATLAS_PAGE_SIZE).pow(2);
        for page in &self.pages {
            match page.class {
                GlyphAtlasClass::Normal => {
                    memory.normal_pages += 1;
                    memory.normal_bytes += page_bytes;
                    memory.normal_content_area += u64::from(page.content_area);
                    memory.normal_allocated_area += u64::from(page.allocated_area);
                }
                GlyphAtlasClass::Oversize => {
                    memory.oversize_pages += 1;
                    memory.oversize_bytes += page_bytes;
                    memory.oversize_content_area += u64::from(page.content_area);
                    memory.oversize_allocated_area += u64::from(page.allocated_area);
                }
            }
        }
        memory
    }
    pub fn lookup_or_rasterize(
        &mut self,
        run: &GlyphRun,
        glyph: u16,
        scale: f64,
    ) -> Option<RasterizedGlyph> {
        let request = GlyphRasterRequest::new(run.font_size, scale);
        if !request.supported {
            self.counters.glyphs_skipped += 1;
            return None;
        }
        let key = GlyphCacheKey {
            font: run.font.id(),
            glyph,
            physical_size: request.physical_size,
        };
        if let Some(entry) = self.entries.get(&key).copied() {
            self.counters.glyph_cache_hits += 1;
            if entry.atlas_class == GlyphAtlasClass::Oversize {
                self.counters.oversize_cache_hits += 1;
            }
            return Some(RasterizedGlyph {
                entry,
                bitmap: None,
            });
        }
        self.counters.glyph_cache_misses += 1;
        let font = match self.fonts.entry(key.font) {
            Entry::Occupied(entry) => {
                self.counters.font_parser_cache_hits += 1;
                entry.into_mut()
            }
            Entry::Vacant(entry) => {
                let settings = FontSettings {
                    collection_index: run.font.face_index(),
                    ..FontSettings::default()
                };
                let parsed = Font::from_bytes(run.font.bytes().as_ref(), settings).ok()?;
                self.counters.font_parser_cache_misses += 1;
                entry.insert(parsed)
            }
        };
        let started = Instant::now();
        let (metrics, bitmap) = font.rasterize_indexed(key.glyph, f32::from(key.physical_size));
        self.counters.raster_time_total = self
            .counters
            .raster_time_total
            .saturating_add(started.elapsed());
        let bitmap_bytes = metrics.width.checked_mul(metrics.height)?;
        if bitmap_bytes > MAX_GLYPH_BITMAP_BYTES || bitmap.len() != bitmap_bytes {
            self.counters.glyphs_skipped += 1;
            return None;
        }
        let width = u16::try_from(metrics.width).ok()?;
        let height = u16::try_from(metrics.height).ok()?;
        self.counters.glyphs_rasterized += 1;
        match size_class(request.physical_size) {
            GlyphSizeClass::Micro => self.counters.micro_glyph_rasters += 1,
            GlyphSizeClass::Small => self.counters.small_glyph_rasters += 1,
            GlyphSizeClass::Normal => self.counters.normal_glyph_rasters += 1,
            GlyphSizeClass::Large => self.counters.large_glyph_rasters += 1,
            GlyphSizeClass::Huge => self.counters.huge_glyph_rasters += 1,
        }
        let bearing_x = clamp_i16(metrics.xmin);
        let bearing_y = clamp_i16(metrics.ymin);
        // Spaces deliberately have a cache entry but no texture write or quad.
        if width == 0 || height == 0 {
            let entry = AtlasEntry {
                page: 0,
                x: 0,
                y: 0,
                width: 0,
                height: 0,
                atlas_class: GlyphAtlasClass::Normal,
                bearing_x,
                bearing_y,
            };
            self.entries.insert(key, entry);
            return Some(RasterizedGlyph {
                entry,
                bitmap: None,
            });
        }
        let entry = self.allocate(width, height, bearing_x, bearing_y)?;
        if entry.atlas_class == GlyphAtlasClass::Oversize {
            self.counters.oversize_glyph_rasters += 1;
            self.counters.oversize_cache_misses += 1;
        }
        self.entries.insert(key, entry);
        self.counters.glyph_atlas_uploads += 1;
        Some(RasterizedGlyph {
            entry,
            bitmap: Some(bitmap),
        })
    }
    /// Returns cache metadata for a glyph already requested at `scale`.
    #[must_use]
    pub fn debug_glyph(&self, run: &GlyphRun, glyph: u16, scale: f64) -> Option<GlyphRasterDebug> {
        let request = GlyphRasterRequest::new(run.font_size, scale);
        let entry = self.entry(GlyphCacheKey {
            font: run.font.id(),
            glyph,
            physical_size: request.physical_size,
        })?;
        Some(GlyphRasterDebug {
            font: run.font.id(),
            glyph,
            logical_font_size: request.logical_font_size,
            scale_factor: request.scale_factor,
            requested_physical_size: request.physical_size,
            atlas_class: entry.atlas_class,
            bitmap_size: [entry.width, entry.height],
            bitmap_bytes: usize::from(entry.width) * usize::from(entry.height),
            bearing: [entry.bearing_x, entry.bearing_y],
            atlas_page: entry.page,
            allocation_rect: entry.allocation_rect(),
            content_rect: [entry.x, entry.y, entry.width, entry.height],
            uv_rect: entry.uv_rect(),
        })
    }
    pub(crate) fn allocate(
        &mut self,
        width: u16,
        height: u16,
        bearing_x: i16,
        bearing_y: i16,
    ) -> Option<AtlasEntry> {
        let stored_width = width.checked_add(ATLAS_PADDING * 2)?;
        let stored_height = height.checked_add(ATLAS_PADDING * 2)?;
        if stored_width > ATLAS_PAGE_SIZE || stored_height > ATLAS_PAGE_SIZE {
            return None;
        }
        let stored_area = u32::from(stored_width) * u32::from(stored_height);
        let page_area = u32::from(ATLAS_PAGE_SIZE) * u32::from(ATLAS_PAGE_SIZE);
        if stored_area >= page_area / OVERSIZE_PAGE_AREA_DIVISOR {
            let page_index = self.pages.len();
            let mut page = AtlasPage::oversize();
            let entry = AtlasEntry {
                page: page_index.try_into().ok()?,
                x: ATLAS_PADDING,
                y: ATLAS_PADDING,
                width,
                height,
                atlas_class: GlyphAtlasClass::Oversize,
                bearing_x,
                bearing_y,
            };
            page.next_x = stored_width;
            page.row_height = stored_height;
            page.content_area = u32::from(width) * u32::from(height);
            page.allocated_area = stored_area;
            self.pages.push(page);
            self.counters.glyph_atlas_pages += 1;
            return Some(entry);
        }
        let page_index = self
            .pages
            .iter()
            .rposition(|page| page.class == GlyphAtlasClass::Normal)?;
        let page = &mut self.pages[page_index];
        if page.next_x + stored_width > ATLAS_PAGE_SIZE {
            page.next_x = 0;
            page.next_y = page.next_y.saturating_add(page.row_height);
            page.row_height = 0;
        }
        if page.next_y + stored_height > ATLAS_PAGE_SIZE {
            self.pages.push(AtlasPage::normal());
            self.counters.glyph_atlas_pages += 1;
            return self.allocate(width, height, bearing_x, bearing_y);
        }
        let entry = AtlasEntry {
            page: page_index.try_into().ok()?,
            x: page.next_x + ATLAS_PADDING,
            y: page.next_y + ATLAS_PADDING,
            width,
            height,
            atlas_class: GlyphAtlasClass::Normal,
            bearing_x,
            bearing_y,
        };
        page.next_x += stored_width;
        page.row_height = page.row_height.max(stored_height);
        page.content_area += u32::from(width) * u32::from(height);
        page.allocated_area += stored_area;
        Some(entry)
    }
}
