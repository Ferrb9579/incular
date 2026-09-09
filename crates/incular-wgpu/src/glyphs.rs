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
    /// Page content generation this placement was allocated into. Page
    /// slots are reused across evictions with a bumped generation, so a
    /// placement is only valid alongside a matching slot generation —
    /// resolution paths validate this before drawing, and a mismatch
    /// re-rasterizes instead of sampling replacement glyphs.
    pub generation: u64,
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
    /// Whether this slot currently holds live content. Budget tightening
    /// retires slots in place — index-stable but vacant — instead of
    /// compacting indices beneath retained references.
    resident: bool,
    /// Content epoch of this slot. Bumped whenever the slot's content is
    /// retired (eviction reuse or budget tightening); placements name the
    /// epoch they were allocated into.
    generation: u64,
    /// Atlas tick of the most recent resolve that hit or placed into this
    /// page. Drives least-recently-used victim selection; shared across
    /// renderer clients like the texture-cache ticks.
    last_use: u64,
}
impl AtlasPage {
    fn normal() -> Self {
        Self {
            class: GlyphAtlasClass::Normal,
            resident: true,
            ..Self::default()
        }
    }
    fn oversize() -> Self {
        Self {
            class: GlyphAtlasClass::Oversize,
            resident: true,
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
/// CPU metadata for retained atlas pages. `WgpuRenderer` maps each live page
/// index lazily to one persistent `R8Unorm` texture. Placements name their
/// page-content generation and are validated on resolve: evicted pages are
/// reused under a bumped generation, and stale placements re-rasterize
/// instead of sampling whatever replaced them.
/// One retained parsed rasterizer object with its recency stamp. The
/// parsed `Font` is a CPU-only rasterizer built from application-owned
/// source bytes; the atlas never retains those bytes. Dropping the entry
/// releases cache ownership only — placements, pages, handles, and
/// submitted work are unaffected, and the next miss re-parses.
struct ParsedFont {
    font: Font,
    last_use: u64,
}

pub struct GlyphAtlas {
    pages: Vec<AtlasPage>,
    entries: HashMap<GlyphCacheKey, AtlasEntry>,
    /// Parsed rasterizer objects by stable font identity
    /// (`FontSettings::collection_index` preserves TTC/OTC faces, and the
    /// `FontId` itself hashes the face index, so faces never alias).
    /// Bounded by [`Self::max_fonts`] least-recently-used entries: every
    /// resolve — hit or miss, across all renderer clients sharing this
    /// atlas — refreshes the requested font's recency, but a hit never
    /// parses merely to serve an already-cached glyph. The bound counts
    /// entries, not bytes (see `DEFAULT_MAX_PARSED_FONTS`).
    fonts: HashMap<FontId, ParsedFont>,
    /// Device-owned retention limit for parsed rasterizer fonts. A zero
    /// limit parses transiently for the current request without retaining,
    /// so supported text still renders while every miss re-parses.
    max_fonts: usize,
    counters: GpuCounters,
    /// Device-owned page budget over *live* pages. Lowering the budget
    /// retires excess unprotected pages eagerly (see
    /// [`Self::set_max_pages`]); protected excess stays pending and
    /// enforces at explicit frame-protection release (and defensively on
    /// resolve), without waiting for another allocation. Every retirement funnels through the
    /// drained placement/identity path instead of stranding identities.
    max_pages: usize,
    tick: u64,
    eviction_revision: u64,
    /// Placement keys retired by eviction since the last drain. Drained by
    /// the shared-context resolve path, which drops their registry
    /// identities; kept as an explicit buffer (rather than returned from
    /// allocation) so the allocation signature stays total.
    retired_keys: Vec<GlyphCacheKey>,
}
impl Default for GlyphAtlas {
    fn default() -> Self {
        Self::new()
    }
}
impl GlyphAtlas {
    #[must_use]
    pub fn new() -> Self {
        Self::with_max_pages(DEFAULT_MAX_GLYPH_ATLAS_PAGES)
    }

    /// Builds an atlas with an explicit page budget, for tests and hosts
    /// that tune residency. See [`Self::set_max_pages`] for the eager
    /// enforcement contract.
    #[must_use]
    pub fn with_max_pages(max_pages: usize) -> Self {
        Self {
            pages: if max_pages == 0 {
                Vec::new()
            } else {
                vec![AtlasPage::normal()]
            },
            entries: HashMap::new(),
            fonts: HashMap::new(),
            counters: GpuCounters {
                glyph_atlas_pages: u64::from(max_pages != 0),
                ..GpuCounters::default()
            },
            max_pages,
            max_fonts: DEFAULT_MAX_PARSED_FONTS,
            tick: 0,
            eviction_revision: 0,
            retired_keys: Vec::new(),
        }
    }

    /// Replaces the parsed-font retention limit and immediately evicts
    /// least-recently-used excess. Eviction drops parsed objects only;
    /// placements, pages, source handles, and submitted work are untouched.
    /// A zero limit retains nothing and parses transiently per request.
    pub fn set_max_fonts(&mut self, max_fonts: usize) {
        self.max_fonts = max_fonts;
        self.evict_fonts();
    }

    /// Retained parsed-font count against the [`Self::set_max_fonts`]
    /// entry limit. Test and diagnostics surface for retention assertions.
    #[must_use]
    pub fn font_count(&self) -> usize {
        self.fonts.len()
    }

    #[must_use]
    pub const fn max_fonts(&self) -> usize {
        self.max_fonts
    }

    /// Evicts least-recently-used parsed fonts beyond the entry limit.
    /// Recency stamps are assigned from a strictly increasing tick, so the
    /// minimum is unique and victim selection is deterministic regardless
    /// of map iteration order. Eviction metadata is the per-entry stamp
    /// plus the `font_parser_evictions` counter — no unbounded log.
    fn evict_fonts(&mut self) {
        while self.fonts.len() > self.max_fonts {
            let victim = self
                .fonts
                .iter()
                .min_by(|a, b| a.1.last_use.cmp(&b.1.last_use))
                .map(|(id, _)| *id);
            let Some(victim) = victim else {
                break;
            };
            self.fonts.remove(&victim);
            self.counters.font_parser_evictions += 1;
        }
    }

    /// Parses the run's font from application-owned source bytes without
    /// retaining anything. Collection faces resolve through the run's face
    /// index, matching the `FontId` identity, and parse failures (including
    /// out-of-range faces) yield `None` with existing error behavior.
    fn parse_font(run: &GlyphRun) -> Option<Font> {
        let settings = FontSettings {
            collection_index: run.font.face_index(),
            ..FontSettings::default()
        };
        Font::from_bytes(run.font.bytes().as_ref(), settings).ok()
    }

    /// Replaces the page budget and immediately retires resident
    /// unprotected pages down to it, least-recently-used first. Protected
    /// pages may temporarily exceed the limit; the excess stays pending
    /// and enforces when [`Self::release_frame_protection`] is called after
    /// submission, even if no further text is resolved. Retired
    /// slots go vacant in place — indices never compact — with a bumped
    /// generation, dropped placements (reported through
    /// [`Self::take_retired_keys`]), and an advanced
    /// [`Self::eviction_revision`] so host maintenance reclaims idle
    /// renderer bindings. A zero budget retires everything unprotected
    /// and admits no new placements.
    pub fn set_max_pages(&mut self, max_pages: usize, protected: &std::collections::HashSet<u16>) {
        self.max_pages = max_pages;
        self.enforce_budget(protected);
    }

    /// Ends the caller's frame protection and retires pending excess pages.
    /// Call after submission, even when no further text will be resolved.
    /// Shared owners must drain retired identities and release vacant GPU
    /// slots before publishing the resulting eviction revision.
    pub fn release_frame_protection(&mut self) {
        self.enforce_budget(&std::collections::HashSet::new());
    }

    #[must_use]
    pub const fn max_pages(&self) -> usize {
        self.max_pages
    }

    /// Live content generation of `page`, if the slot is resident.
    /// Renderer page tables and host reclamation compare against this;
    /// `None` means the slot was never allocated or was retired vacant by
    /// budget tightening, so no old identity can resolve to new contents.
    #[must_use]
    pub fn page_generation(&self, page: u16) -> Option<u64> {
        self.pages
            .get(usize::from(page))
            .filter(|slot| slot.resident)
            .map(|slot| slot.generation)
    }

    /// Eviction revision: advances once per retired page. Hosts compare
    /// this across maintenance passes alongside the texture revisions.
    #[must_use]
    pub const fn eviction_revision(&self) -> u64 {
        self.eviction_revision
    }

    /// Drains placement keys retired by eviction or budget tightening
    /// since the last drain, for registry-identity cleanup by the owning
    /// resolve path. Exposed so hosts and ownership tests drive the same
    /// production drain the shared context uses.
    #[must_use]
    pub fn take_retired_keys(&mut self) -> Vec<GlyphCacheKey> {
        std::mem::take(&mut self.retired_keys)
    }
    #[must_use]
    pub const fn counters(&self) -> GpuCounters {
        self.counters
    }
    /// Allocated page slots, resident or vacant. Slot capacity, not live
    /// residency: indices stay stable across tightening, so this only
    /// grows. Compare [`Self::live_page_count`] for budget accounting.
    #[must_use]
    pub const fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// Live (resident) pages against the [`Self::with_max_pages`] budget.
    /// Tightening retires slots to vacant without compacting, so this —
    /// not [`Self::page_count`] and not the cumulative
    /// `glyph_atlas_pages` counter — is the residency measure.
    #[must_use]
    pub fn live_page_count(&self) -> usize {
        self.pages.iter().filter(|page| page.resident).count()
    }
    #[must_use]
    pub fn entry(&self, key: GlyphCacheKey) -> Option<AtlasEntry> {
        self.entries.get(&key).copied()
    }
    /// Live-residency accounting: vacant slots hold no content and back no
    /// texture, so only resident pages count toward pages, bytes, and
    /// areas. Slot capacity is [`Self::page_count`].
    #[must_use]
    pub fn memory(&self) -> GlyphAtlasMemory {
        let mut memory = GlyphAtlasMemory::default();
        let page_bytes = usize::from(ATLAS_PAGE_SIZE).pow(2);
        for page in self.pages.iter().filter(|page| page.resident) {
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
        protected: &std::collections::HashSet<u16>,
    ) -> Option<RasterizedGlyph> {
        // Pending tightening enforces against this resolve's protection
        // set first — even on a cache hit — so released protection sheds
        // excess residency without waiting for an unrelated allocation.
        // Skipped while slot capacity fits the budget, which implies live
        // residency fits too.
        if self.pages.len() > self.max_pages {
            self.enforce_budget(protected);
        }
        if self.max_pages == 0 {
            self.counters.glyph_pressure_skips += 1;
            return None;
        }
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
            // Generation validation: the slot may have been evicted and
            // reused for different content since this placement resolved. A
            // mismatch re-rasterizes below instead of sampling replacement
            // glyphs through the stale coordinates.
            let live = self
                .pages
                .get(usize::from(entry.page))
                .is_some_and(|page| page.resident && page.generation == entry.generation);
            if live {
                self.tick = self.tick.saturating_add(1);
                let tick = self.tick;
                if let Some(page) = self.pages.get_mut(usize::from(entry.page)) {
                    page.last_use = tick;
                }
                // Retention recency without parsing: an already-cached
                // glyph never parses its font merely to be served. If the
                // font is still retained its recency refreshes; if it was
                // evicted earlier, nothing happens and the hit still serves.
                if let Some(parsed) = self.fonts.get_mut(&key.font) {
                    parsed.last_use = tick;
                }
                self.counters.glyph_cache_hits += 1;
                if entry.atlas_class == GlyphAtlasClass::Oversize {
                    self.counters.oversize_cache_hits += 1;
                }
                return Some(RasterizedGlyph {
                    entry,
                    bitmap: None,
                });
            }
            self.entries.remove(&key);
            self.counters.glyph_stale_refreshes += 1;
        }
        self.counters.glyph_cache_misses += 1;
        // A zero retention limit parses transiently for this request
        // without retaining, so supported text renders while every miss
        // re-parses. Failed parses keep existing behavior: `None` with no
        // retention counters beyond the miss above.
        let unretained: Option<Font>;
        let font = if self.max_fonts == 0 {
            let parsed = Self::parse_font(run)?;
            self.counters.font_parser_cache_misses += 1;
            unretained = Some(parsed);
            unretained.as_ref().expect("parsed font just stored")
        } else {
            let inserted = match self.fonts.entry(key.font) {
                Entry::Occupied(slot) => {
                    self.counters.font_parser_cache_hits += 1;
                    self.tick = self.tick.saturating_add(1);
                    let tick = self.tick;
                    slot.into_mut().last_use = tick;
                    false
                }
                Entry::Vacant(slot) => {
                    let parsed = Self::parse_font(run)?;
                    self.counters.font_parser_cache_misses += 1;
                    self.tick = self.tick.saturating_add(1);
                    let tick = self.tick;
                    slot.insert(ParsedFont {
                        font: parsed,
                        last_use: tick,
                    });
                    true
                }
            };
            if inserted {
                // The fresh entry holds the newest stamp, so it can never
                // be its own eviction victim below.
                self.evict_fonts();
            }
            &self
                .fonts
                .get(&key.font)
                .expect("parsed font just ensured")
                .font
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
                generation: self.pages.first().map_or(0, |page| page.generation),
            };
            self.entries.insert(key, entry);
            return Some(RasterizedGlyph {
                entry,
                bitmap: None,
            });
        }
        let entry = self.allocate(width, height, bearing_x, bearing_y, protected)?;
        self.tick = self.tick.saturating_add(1);
        let tick = self.tick;
        if let Some(page) = self.pages.get_mut(usize::from(entry.page)) {
            page.last_use = tick;
        }
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
        protected: &std::collections::HashSet<u16>,
    ) -> Option<AtlasEntry> {
        if self.max_pages == 0 {
            self.counters.glyph_pressure_skips += 1;
            return None;
        }
        let stored_width = width.checked_add(ATLAS_PADDING * 2)?;
        let stored_height = height.checked_add(ATLAS_PADDING * 2)?;
        if stored_width > ATLAS_PAGE_SIZE || stored_height > ATLAS_PAGE_SIZE {
            return None;
        }
        let stored_area = u32::from(stored_width) * u32::from(stored_height);
        let page_area = u32::from(ATLAS_PAGE_SIZE) * u32::from(ATLAS_PAGE_SIZE);
        if stored_area >= page_area / OVERSIZE_PAGE_AREA_DIVISOR {
            let page_index = self.fresh_or_evict(true, protected)?;
            let page = &mut self.pages[page_index];
            page.class = GlyphAtlasClass::Oversize;
            let entry = AtlasEntry {
                page: page_index.try_into().ok()?,
                x: ATLAS_PADDING,
                y: ATLAS_PADDING,
                width,
                height,
                atlas_class: GlyphAtlasClass::Oversize,
                bearing_x,
                bearing_y,
                generation: page.generation,
            };
            page.next_x = stored_width;
            page.row_height = stored_height;
            page.content_area = u32::from(width) * u32::from(height);
            page.allocated_area = stored_area;
            return Some(entry);
        }
        if let Some(entry) = self.place_normal(
            width,
            height,
            stored_width,
            stored_height,
            bearing_x,
            bearing_y,
        ) {
            return Some(entry);
        }
        let page_index = self.fresh_or_evict(false, protected)?;
        let page = &mut self.pages[page_index];
        page.class = GlyphAtlasClass::Normal;
        let entry = AtlasEntry {
            page: page_index.try_into().ok()?,
            x: page.next_x + ATLAS_PADDING,
            y: page.next_y + ATLAS_PADDING,
            width,
            height,
            atlas_class: GlyphAtlasClass::Normal,
            bearing_x,
            bearing_y,
            generation: page.generation,
        };
        page.next_x += stored_width;
        page.row_height = page.row_height.max(stored_height);
        page.content_area += u32::from(width) * u32::from(height);
        page.allocated_area += stored_area;
        Some(entry)
    }

    /// Fits into the most recent normal page with space, exactly as before
    /// budgeting existed. Appending to a live page never disturbs placed
    /// content, so protection applies to eviction only, never to placement.
    fn place_normal(
        &mut self,
        width: u16,
        height: u16,
        stored_width: u16,
        stored_height: u16,
        bearing_x: i16,
        bearing_y: i16,
    ) -> Option<AtlasEntry> {
        let page_index = self
            .pages
            .iter()
            .rposition(|page| page.resident && page.class == GlyphAtlasClass::Normal)?;
        let page = &mut self.pages[page_index];
        if page.next_x + stored_width > ATLAS_PAGE_SIZE {
            page.next_x = 0;
            page.next_y = page.next_y.saturating_add(page.row_height);
            page.row_height = 0;
        }
        if page.next_y + stored_height > ATLAS_PAGE_SIZE {
            return None;
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
            generation: page.generation,
        };
        page.next_x += stored_width;
        page.row_height = page.row_height.max(stored_height);
        page.content_area += u32::from(width) * u32::from(height);
        page.allocated_area += u32::from(stored_width) * u32::from(stored_height);
        Some(entry)
    }

    /// Finds a page slot for one placement: a vacant slot reused first
    /// while live residency is under budget, a fresh slot while slot
    /// capacity is under budget, otherwise an evicted victim reused in
    /// place. Reuse keeps the generation bumped at retirement and the
    /// cursor reset there, so no retired identity can name the new
    /// content. Returns `None` — a counted pressure skip, never a loop —
    /// when the budget admits no pages at all or every live page is
    /// protected by the current frame.
    fn fresh_or_evict(
        &mut self,
        oversize: bool,
        protected: &std::collections::HashSet<u16>,
    ) -> Option<usize> {
        if self.live_page_count() < self.max_pages {
            if let Some(vacant) = self.pages.iter().position(|page| !page.resident) {
                if let Some(page) = self.pages.get_mut(vacant) {
                    page.resident = true;
                }
                return Some(vacant);
            }
            if self.pages.len() < self.max_pages {
                self.pages.push(if oversize {
                    AtlasPage::oversize()
                } else {
                    AtlasPage::normal()
                });
                self.counters.glyph_atlas_pages += 1;
                return Some(self.pages.len() - 1);
            }
        }
        let victim = self.evict_victim(protected);
        if victim.is_none() {
            self.counters.glyph_pressure_skips += 1;
        }
        victim
    }

    /// Retires resident unprotected pages while live residency exceeds the
    /// budget, least-recently-used first. Retired slots go vacant in place
    /// (see [`Self::retire_page`]); fully protected excess stays pending
    /// for a later call with a narrower protection set.
    fn enforce_budget(&mut self, protected: &std::collections::HashSet<u16>) {
        while self.live_page_count() > self.max_pages {
            let victim = self
                .pages
                .iter()
                .enumerate()
                .filter(|(index, page)| {
                    page.resident
                        && match u16::try_from(*index) {
                            // Slots beyond u16 can never be named by a
                            // placement, so they are always eligible.
                            Err(_) => true,
                            Ok(slot) => !protected.contains(&slot),
                        }
                })
                .min_by(|a, b| a.1.last_use.cmp(&b.1.last_use).then_with(|| a.0.cmp(&b.0)))
                .map(|(index, _)| index);
            let Some(victim) = victim else {
                break;
            };
            self.retire_page(victim);
        }
    }

    /// Drops one slot's placements for the retired-keys drain, reporting
    /// each eviction through the revision and counter that drive host
    /// maintenance. Shared by in-place eviction reuse and vacancy
    /// retirement; the caller sets the slot's resulting state.
    fn drop_page_placements(&mut self, index: usize) {
        let mut retired = Vec::new();
        self.entries.retain(|key, entry| {
            if usize::from(entry.page) == index {
                retired.push(*key);
                false
            } else {
                true
            }
        });
        self.retired_keys.extend(retired);
        self.eviction_revision = self.eviction_revision.saturating_add(1);
        self.counters.glyph_page_evictions += 1;
    }

    /// Retires one slot to vacant: placements drop for the
    /// registry-identity drain, the generation bumps so retired identities
    /// can never name a future reuse of this index, the allocator cursor
    /// resets, and the slot leaves live residency without compacting.
    /// Submitted work stays valid through the wgpu lifetime contract;
    /// renderer bindings re-resolve or reclaim through host maintenance.
    fn retire_page(&mut self, index: usize) {
        self.drop_page_placements(index);
        if let Some(page) = self.pages.get_mut(index) {
            page.generation = page.generation.saturating_add(1);
            page.resident = false;
            page.class = GlyphAtlasClass::Normal;
            page.next_x = 0;
            page.next_y = 0;
            page.row_height = 0;
            page.content_area = 0;
            page.allocated_area = 0;
        }
    }

    /// Retires the least-recently-used unprotected page: drops its
    /// placements (reported through [`Self::take_retired_keys`] for
    /// identity cleanup), bumps its generation so no stale placement can
    /// name the replacement content, and resets its allocator cursor.
    /// Returns `None` — a counted pressure skip, never a loop — when every
    /// page is protected or the budget admits no pages at all.
    fn evict_victim(&mut self, protected: &std::collections::HashSet<u16>) -> Option<usize> {
        let victim = self
            .pages
            .iter()
            .enumerate()
            .filter(|(index, page)| {
                page.resident
                    && match u16::try_from(*index) {
                        // Slots beyond u16 can never be named by a placement, so
                        // they are always eligible.
                        Err(_) => true,
                        Ok(slot) => !protected.contains(&slot),
                    }
            })
            .min_by(|a, b| a.1.last_use.cmp(&b.1.last_use).then_with(|| a.0.cmp(&b.0)))
            .map(|(index, _)| index)?;
        self.drop_page_placements(victim);
        if let Some(page) = self.pages.get_mut(victim) {
            page.generation = page.generation.saturating_add(1);
            page.next_x = 0;
            page.next_y = 0;
            page.row_height = 0;
            page.content_area = 0;
            page.allocated_area = 0;
        }
        Some(victim)
    }
}

/// One renderer-local atlas slot: the retained binding resources plus the
/// content generation they were built for. The generation is what lets
/// host reclamation and bind-time validation tell current content from a
/// superseded epoch of the same page index.
#[derive(Clone, Debug)]
pub struct GlyphPageSlot<T> {
    pub resource: T,
    pub generation: u64,
}

/// Renderer-local atlas page table: index-stable slots validated by
/// content generation. A slot whose generation no longer matches the
/// shared page is never sampled — it is replaced on next ensure or
/// released by [`Self::reclaim`] — so reusing a page index can never
/// display a different glyph through an old reference.
///
/// Frame atomicity makes this sound: lowering, binding, and submission
/// run synchronously within one render call on the owning thread, so no
/// slot changes between a bind and its draw. Replacement always goes
/// through [`Self::ensure`] with a freshly resolved generation, and
/// release only happens between frames (host maintenance) or at frame end
/// (see below).
#[derive(Clone, Debug)]
pub struct RendererGlyphPages<T> {
    slots: Vec<Option<GlyphPageSlot<T>>>,
}

impl<T> Default for RendererGlyphPages<T> {
    fn default() -> Self {
        Self { slots: Vec::new() }
    }
}

impl<T> RendererGlyphPages<T> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Retained (non-empty) slot count.
    #[must_use]
    pub fn len(&self) -> usize {
        self.slots.iter().flatten().count()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(Option::is_none)
    }

    /// Returns the retained resource for `page` without checking its
    /// generation. Callers bind only what [`Self::ensure`] validated in
    /// the same frame, which is sound by frame atomicity (see above).
    #[must_use]
    pub fn get(&self, page: u16) -> Option<&T> {
        self.slots
            .get(usize::from(page))?
            .as_ref()
            .map(|slot| &slot.resource)
    }

    /// Retained generation for `page`, if the slot is occupied. Lets
    /// callers detect a superseded binding without rebuilding it.
    #[must_use]
    pub fn generation(&self, page: u16) -> Option<u64> {
        self.slots
            .get(usize::from(page))?
            .as_ref()
            .map(|slot| slot.generation)
    }

    /// Returns the retained resource for `page`, replacing the slot when
    /// its generation differs (or the slot is empty). Replacement drops
    /// the old texture and bind group; submitted work stays valid through
    /// the wgpu lifetime contract. Callers must pass a freshly resolved
    /// generation — and, while lowering, only for pages outside the
    /// frame-pinned set — so a replacement can never strand an
    /// already-emitted batch of the same frame.
    pub fn ensure(&mut self, page: u16, generation: u64, create: impl FnOnce() -> T) -> &T {
        let index = usize::from(page);
        if self.slots.len() <= index {
            self.slots.resize_with(index + 1, || None);
        }
        let stale = !matches!(
            &self.slots[index],
            Some(slot) if slot.generation == generation
        );
        if stale {
            self.slots[index] = Some(GlyphPageSlot {
                resource: create(),
                generation,
            });
        }
        &self.slots[index]
            .as_ref()
            .expect("slot just ensured")
            .resource
    }

    /// Releases slots whose generation differs from the live shared
    /// generations (or which have none), returning how many went. Safe
    /// between frames; the next `ensure` rebuilds on demand.
    pub fn reclaim(&mut self, current_generation: &dyn Fn(u16) -> Option<u64>) -> usize {
        let mut dropped = 0;
        for (index, slot) in self.slots.iter_mut().enumerate() {
            let stale = match slot {
                Some(entry) => {
                    u16::try_from(index).ok().and_then(current_generation) != Some(entry.generation)
                }
                None => false,
            };
            if stale {
                *slot = None;
                dropped += 1;
            }
        }
        dropped
    }
}
