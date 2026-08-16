//! Renderer-independent font resolution, cluster-safe fallback shaping, and cached logical text layout.
use fontdb::{Database, Family, ID, Query, Style, Weight};
use incular_assets::FontHandle;
pub use incular_assets::FontId;
use incular_core::{Color, Offset, Size};
use incular_painting::{GlyphPosition, GlyphRun};
use rustybuzz::{Direction, Face, UnicodeBuffer, shape};
use std::{
    collections::{HashMap, HashSet, VecDeque, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    sync::Arc,
};
use ttf_parser::Face as TableFace;
use unicode_bidi::BidiInfo;
use unicode_script::{Script, UnicodeScript};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum FontFamily {
    SansSerif,
    Serif,
    Monospace,
    Cursive,
    SystemUi,
    Named(String),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FontWeight(pub u16);
impl FontWeight {
    pub const NORMAL: Self = Self(400);
    pub const BOLD: Self = Self(700);
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FontStyle {
    #[default]
    Normal,
    Italic,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextAlign {
    #[default]
    Start,
    Center,
    End,
}
#[derive(Clone, Debug, PartialEq)]
pub struct TextStyle {
    pub family: FontFamily,
    pub fallback_families: Arc<[FontFamily]>,
    pub size: f32,
    pub weight: FontWeight,
    pub style: FontStyle,
    pub color: Color,
    pub line_height: Option<f32>,
    pub letter_spacing: f32,
}
impl TextStyle {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    #[must_use]
    pub fn font_family(mut self, family: impl Into<String>) -> Self {
        self.family = FontFamily::Named(family.into());
        self
    }
    #[must_use]
    pub fn fallback_families<I, S>(mut self, families: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.fallback_families = families
            .into_iter()
            .map(|name| FontFamily::Named(name.into()))
            .collect();
        self
    }
}
impl Default for TextStyle {
    fn default() -> Self {
        Self {
            family: FontFamily::SystemUi,
            fallback_families: Arc::new([]),
            size: 16.,
            weight: FontWeight::NORMAL,
            style: FontStyle::Normal,
            color: Color::WHITE,
            line_height: None,
            letter_spacing: 0.,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextMetrics {
    pub size: Size,
    pub baseline: f32,
    pub line_height: f32,
}
#[derive(Clone, Debug)]
pub struct TextLine {
    pub runs: Arc<[Arc<GlyphRun>]>,
    pub glyphs: Arc<[GlyphPosition]>,
    pub width: f32,
    pub baseline: f32,
    pub start: usize,
    pub end: usize,
}
#[derive(Clone, Debug)]
pub struct FontRunDebug {
    pub range: std::ops::Range<usize>,
    pub font: FontId,
    pub family: String,
    pub script: String,
    pub direction: &'static str,
}
#[derive(Clone, Debug)]
pub struct TextLayout {
    pub lines: Arc<[TextLine]>,
    pub metrics: TextMetrics,
    pub font_runs: Arc<[FontRunDebug]>,
}
impl TextLayout {
    #[must_use]
    pub fn glyph_count(&self) -> usize {
        self.lines.iter().map(|line| line.glyphs.len()).sum()
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextDiagnostics {
    pub layouts_requested: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub font_resolution_requests: u64,
    pub font_resolution_cache_hits: u64,
    pub font_resolution_cache_misses: u64,
    pub coverage_queries: u64,
    pub fallback_runs: u64,
    pub fallback_fonts_used: u64,
    pub missing_clusters: u64,
    pub shaping_runs: u64,
}

#[derive(Hash, PartialEq, Eq, Clone)]
struct LayoutKey {
    text: String,
    family: FontFamily,
    fallbacks: Arc<[FontFamily]>,
    size: u32,
    weight: u16,
    style: FontStyle,
    line_height: Option<u32>,
    letter_spacing: u32,
    width: Option<u32>,
    align: TextAlign,
    generation: u64,
}
impl LayoutKey {
    fn new(
        text: &str,
        style: &TextStyle,
        width: Option<f32>,
        align: TextAlign,
        generation: u64,
    ) -> Self {
        Self {
            text: text.into(),
            family: style.family.clone(),
            fallbacks: style.fallback_families.clone(),
            size: style.size.to_bits(),
            weight: style.weight.0,
            style: style.style,
            line_height: style.line_height.map(f32::to_bits),
            letter_spacing: style.letter_spacing.to_bits(),
            width: width.map(f32::to_bits),
            align,
            generation,
        }
    }
}
#[derive(Hash, PartialEq, Eq, Clone)]
struct ResolutionKey {
    cluster: String,
    family: FontFamily,
    fallbacks: Arc<[FontFamily]>,
    weight: u16,
    style: FontStyle,
    generation: u64,
}
#[derive(Clone)]
struct ResolvedFace {
    handle: FontHandle,
    family: String,
}
#[derive(Clone)]
struct Cluster {
    start: usize,
    end: usize,
    text: String,
    script: Script,
}
#[derive(Clone)]
struct ResolvedCluster {
    cluster: Cluster,
    face: Option<ResolvedFace>,
}

/// The sole owner of system and application font discovery. Font registration is immutable;
/// adding faces advances the database generation and clears layout/resolution caches.
pub struct TextEngine {
    database: Database,
    generation: u64,
    cache: HashMap<LayoutKey, Arc<TextLayout>>,
    order: VecDeque<LayoutKey>,
    resolution_cache: HashMap<ResolutionKey, Option<ResolvedFace>>,
    script_candidates: HashMap<Script, Vec<ID>>,
    diagnostics: TextDiagnostics,
}
impl Default for TextEngine {
    fn default() -> Self {
        Self::new()
    }
}
impl TextEngine {
    #[must_use]
    pub fn new() -> Self {
        let mut database = Database::new();
        database.load_system_fonts();
        Self {
            database,
            generation: 1,
            cache: HashMap::new(),
            order: VecDeque::new(),
            resolution_cache: HashMap::new(),
            script_candidates: HashMap::new(),
            diagnostics: TextDiagnostics::default(),
        }
    }
    #[must_use]
    pub const fn diagnostics(&self) -> TextDiagnostics {
        self.diagnostics
    }
    #[must_use]
    pub const fn font_database_generation(&self) -> u64 {
        self.generation
    }
    pub fn register_font_bytes(&mut self, bytes: impl Into<Vec<u8>>) {
        self.database.load_font_data(bytes.into());
        self.generation += 1;
        self.cache.clear();
        self.order.clear();
        self.resolution_cache.clear();
        self.script_candidates.clear();
    }
    pub fn layout(
        &mut self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f32>,
        align: TextAlign,
    ) -> Arc<TextLayout> {
        self.diagnostics.layouts_requested += 1;
        let key = LayoutKey::new(text, style, max_width, align, self.generation);
        if let Some(layout) = self.cache.get(&key) {
            self.diagnostics.cache_hits += 1;
            return layout.clone();
        }
        self.diagnostics.cache_misses += 1;
        let layout = Arc::new(self.shape_and_wrap(text, style, max_width, align));
        if self.order.len() == 256 {
            if let Some(old) = self.order.pop_front() {
                self.cache.remove(&old);
            }
        }
        self.order.push_back(key.clone());
        self.cache.insert(key, layout.clone());
        layout
    }
    fn families<'a>(&self, family: &'a FontFamily) -> Vec<Family<'a>> {
        match family {
            FontFamily::Named(name) => vec![Family::Name(name)],
            FontFamily::Serif => vec![Family::Serif],
            FontFamily::Monospace => vec![Family::Monospace],
            FontFamily::Cursive => vec![Family::Cursive],
            FontFamily::SansSerif => vec![
                Family::Name("Noto Sans"),
                Family::Name("DejaVu Sans"),
                Family::Name("Liberation Sans"),
                Family::SansSerif,
            ],
            FontFamily::SystemUi => vec![
                Family::Name("Noto Sans"),
                Family::Name("Cantarell"),
                Family::Name("Inter"),
                Family::Name("DejaVu Sans"),
                Family::Name("Liberation Sans"),
                Family::SansSerif,
            ],
        }
    }
    fn query_family(&self, family: &FontFamily, style: &TextStyle) -> Option<ID> {
        let families = self.families(family);
        self.database.query(&Query {
            families: &families,
            weight: Weight(style.weight.0),
            style: match style.style {
                FontStyle::Normal => Style::Normal,
                FontStyle::Italic => Style::Italic,
            },
            ..Query::default()
        })
    }
    fn face_handle(&self, id: ID) -> Option<ResolvedFace> {
        let info = self.database.face(id)?;
        let family = info
            .families
            .first()
            .map_or_else(|| info.post_script_name.clone(), |entry| entry.0.clone());
        let index = info.index;
        self.database.with_face_data(id, |bytes, _| {
            let mut hash = DefaultHasher::new();
            bytes.hash(&mut hash);
            index.hash(&mut hash);
            ResolvedFace {
                handle: FontHandle::with_face_index(
                    FontId(hash.finish()),
                    Arc::<[u8]>::from(bytes.to_vec()),
                    index,
                ),
                family,
            }
        })
    }
    fn supports_cluster(&mut self, id: ID, cluster: &str) -> bool {
        self.diagnostics.coverage_queries += 1;
        self.database
            .with_face_data(id, |bytes, index| {
                let Ok(face) = TableFace::parse(bytes, index) else {
                    return false;
                };
                cluster
                    .chars()
                    .filter(|c| !is_cluster_control(*c))
                    .all(|c| face.glyph_index(c).is_some())
            })
            .unwrap_or(false)
    }
    fn script_candidates(&mut self, script: Script, probe: char) -> Vec<ID> {
        if let Some(ids) = self.script_candidates.get(&script) {
            return ids.clone();
        }
        let ids: Vec<_> = self.database.faces().map(|face| face.id).collect();
        let mut matches: Vec<ID> = ids
            .into_iter()
            .filter(|id| self.supports_cluster(*id, &probe.to_string()))
            .collect();
        matches.sort_by_key(|id| {
            self.database
                .face(*id)
                .map_or_else(String::new, |face| face.post_script_name.clone())
        });
        self.script_candidates.insert(script, matches.clone());
        matches
    }
    fn resolve_cluster(&mut self, cluster: Cluster, style: &TextStyle) -> Option<ResolvedFace> {
        self.diagnostics.font_resolution_requests += 1;
        let key = ResolutionKey {
            cluster: cluster.text.clone(),
            family: style.family.clone(),
            fallbacks: style.fallback_families.clone(),
            weight: style.weight.0,
            style: style.style,
            generation: self.generation,
        };
        if let Some(resolved) = self.resolution_cache.get(&key) {
            self.diagnostics.font_resolution_cache_hits += 1;
            return resolved.clone();
        }
        self.diagnostics.font_resolution_cache_misses += 1;
        let primary = self.query_family(&style.family, style);
        let mut candidates = primary.into_iter().collect::<Vec<_>>();
        candidates.extend(
            style
                .fallback_families
                .iter()
                .filter_map(|family| self.query_family(family, style)),
        );
        if let Some(probe) = cluster.text.chars().find(|c| !is_cluster_control(*c)) {
            candidates.extend(self.script_candidates(cluster.script, probe));
        }
        let mut ordered = Vec::with_capacity(candidates.len());
        for candidate in candidates {
            if !ordered.contains(&candidate) {
                ordered.push(candidate);
            }
        }
        let resolved = ordered
            .into_iter()
            .find(|id| self.supports_cluster(*id, &cluster.text))
            .and_then(|id| self.face_handle(id));
        if resolved.is_none() {
            self.diagnostics.missing_clusters += 1;
        }
        self.resolution_cache.insert(key, resolved.clone());
        resolved
    }
    fn shape_and_wrap(
        &mut self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f32>,
        align: TextAlign,
    ) -> TextLayout {
        let line_height = style.line_height.unwrap_or(style.size * 1.2);
        let baseline = style.size * 0.8;
        let mut builds = Vec::new();
        let mut debug = Vec::new();
        let primary_font = self
            .query_family(&style.family, style)
            .and_then(|id| self.face_handle(id))
            .map(|face| face.handle.id());
        let mut fallback_fonts = HashSet::new();
        let mut paragraph_start = 0;
        for segment in text.split_inclusive('\n') {
            let source = segment.strip_suffix('\n').unwrap_or(segment);
            let source_start = paragraph_start;
            let mut clusters: Vec<Cluster> = source
                .grapheme_indices(true)
                .map(|(offset, value)| Cluster {
                    start: source_start + offset,
                    end: source_start + offset + value.len(),
                    text: value.into(),
                    script: cluster_script(value),
                })
                .collect();
            let mut previous = Script::Common;
            for cluster in &mut clusters {
                if is_weak_script(cluster.script) {
                    cluster.script = previous;
                } else {
                    previous = cluster.script;
                }
            }
            let resolved: Vec<_> = clusters
                .into_iter()
                .map(|cluster| {
                    let face = self.resolve_cluster(cluster.clone(), style);
                    ResolvedCluster { cluster, face }
                })
                .collect();
            let mut logical_runs: Vec<Vec<ResolvedCluster>> = Vec::new();
            for item in resolved {
                if logical_runs.last().is_some_and(|last| {
                    last.last()
                        .and_then(|entry| entry.face.as_ref())
                        .map(|face| face.handle.id())
                        == item.face.as_ref().map(|face| face.handle.id())
                }) {
                    logical_runs.last_mut().unwrap().push(item);
                } else {
                    logical_runs.push(vec![item]);
                }
            }
            let mut built = LineBuild::new(source_start);
            let mut pen = 0.;
            for entries in logical_runs {
                let Some(face) = entries.first().and_then(|entry| entry.face.clone()) else {
                    continue;
                };
                if Some(face.handle.id()) != primary_font {
                    self.diagnostics.fallback_runs += 1;
                    fallback_fonts.insert(face.handle.id());
                }
                let range_start = entries.first().unwrap().cluster.start;
                let range_end = entries.last().unwrap().cluster.end;
                let contents: String = entries
                    .iter()
                    .map(|entry| entry.cluster.text.as_str())
                    .collect();
                let Some(rb_face) = Face::from_slice(face.handle.bytes(), face.handle.face_index())
                else {
                    continue;
                };
                let scale = style.size / rb_face.units_per_em() as f32;
                let mut buffer = UnicodeBuffer::new();
                buffer.push_str(&contents);
                // Resolve direction before this font-specific shaping call;
                // fallback changes the face, never the paragraph's bidi text.
                let bidi = BidiInfo::new(&contents, None);
                if bidi
                    .paragraphs
                    .first()
                    .is_some_and(|paragraph| paragraph.level.is_rtl())
                {
                    buffer.set_direction(Direction::RightToLeft);
                } else {
                    buffer.set_direction(Direction::LeftToRight);
                }
                let glyph_buffer = shape(&rb_face, &[], buffer);
                self.diagnostics.shaping_runs += 1;
                let mut glyphs = Vec::new();
                let mut run_width = 0.;
                for (info, position) in glyph_buffer
                    .glyph_infos()
                    .iter()
                    .zip(glyph_buffer.glyph_positions())
                {
                    let advance = position.x_advance as f32 * scale + style.letter_spacing;
                    glyphs.push(GlyphPosition {
                        id: info.glyph_id as u16,
                        offset: Offset::new(
                            run_width + position.x_offset as f32 * scale,
                            position.y_offset as f32 * scale,
                        ),
                        advance,
                        cluster: info.cluster + range_start as u32,
                    });
                    run_width += advance.max(0.);
                }
                if max_width.is_some_and(|limit| pen > 0. && pen + run_width > limit) {
                    builds.push(built);
                    built = LineBuild::new(range_start);
                    pen = 0.;
                }
                let origin = Offset::new(pen, 0.);
                for glyph in &glyphs {
                    let mut global = *glyph;
                    global.offset.x += pen;
                    built.glyphs.push(global);
                }
                built.runs.push(Arc::new(GlyphRun {
                    font: face.handle.clone(),
                    font_size: style.size,
                    origin,
                    glyphs: glyphs.into(),
                }));
                built.width = pen + run_width;
                built.end = range_end;
                pen += run_width;
                let script = entries.first().unwrap().cluster.script;
                debug.push(FontRunDebug {
                    range: range_start..range_end,
                    font: face.handle.id(),
                    family: face.family,
                    script: format!("{script:?}"),
                    direction: if is_rtl(script) { "rtl" } else { "ltr" },
                });
            }
            builds.push(built);
            paragraph_start += segment.len();
        }
        self.diagnostics.fallback_fonts_used += fallback_fonts.len() as u64;
        if text.is_empty() {
            builds.push(LineBuild::new(0));
        }
        let max_line = builds.iter().map(|line| line.width).fold(0., f32::max);
        let lines: Vec<_> = builds
            .into_iter()
            .enumerate()
            .map(|(index, build)| {
                let x = match align {
                    TextAlign::Start => 0.,
                    TextAlign::Center => (max_width.unwrap_or(max_line) - build.width).max(0.) / 2.,
                    TextAlign::End => (max_width.unwrap_or(max_line) - build.width).max(0.),
                };
                let runs = build
                    .runs
                    .into_iter()
                    .map(|run| {
                        Arc::new(GlyphRun {
                            origin: Offset::new(
                                run.origin.x + x,
                                index as f32 * line_height + baseline,
                            ),
                            ..(*run).clone()
                        })
                    })
                    .collect();
                TextLine {
                    runs,
                    glyphs: build.glyphs.into(),
                    width: build.width,
                    baseline: index as f32 * line_height + baseline,
                    start: build.start,
                    end: build.end,
                }
            })
            .collect();
        let count = lines.len();
        TextLayout {
            lines: lines.into(),
            metrics: TextMetrics {
                size: Size::new(max_line, line_height * count as f32),
                baseline,
                line_height,
            },
            font_runs: debug.into(),
        }
    }
}
struct LineBuild {
    runs: Vec<Arc<GlyphRun>>,
    glyphs: Vec<GlyphPosition>,
    width: f32,
    start: usize,
    end: usize,
}
impl LineBuild {
    fn new(start: usize) -> Self {
        Self {
            runs: Vec::new(),
            glyphs: Vec::new(),
            width: 0.,
            start,
            end: start,
        }
    }
}
fn is_cluster_control(c: char) -> bool {
    c == '\u{200d}'
        || ('\u{fe00}'..='\u{fe0f}').contains(&c)
        || ('\u{e0100}'..='\u{e01ef}').contains(&c)
}
fn cluster_script(value: &str) -> Script {
    value
        .chars()
        .map(|character| character.script())
        .find(|script| !is_weak_script(*script))
        .unwrap_or(Script::Common)
}
fn is_weak_script(script: Script) -> bool {
    matches!(script, Script::Common | Script::Inherited | Script::Unknown)
}
fn is_rtl(script: Script) -> bool {
    matches!(
        script,
        Script::Arabic | Script::Hebrew | Script::Syriac | Script::Thaana | Script::Nko
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cache_ignores_color_but_not_size() {
        let mut engine = TextEngine::new();
        let mut style = TextStyle::default();
        let a = engine.layout("hello", &style, Some(200.), TextAlign::Start);
        style.color = Color::BLACK;
        let b = engine.layout("hello", &style, Some(200.), TextAlign::Start);
        assert!(Arc::ptr_eq(&a, &b));
        style.size = 20.;
        let c = engine.layout("hello", &style, Some(200.), TextAlign::Start);
        assert!(!Arc::ptr_eq(&a, &c));
    }
    #[test]
    fn unicode_clusters_are_never_split_before_resolution() {
        let mut engine = TextEngine::new();
        for text in ["e\u{301}", "नमस्ते", "مرحبا", "👩\u{200d}💻", "✈\u{fe0f}"]
        {
            let layout = engine.layout(text, &TextStyle::default(), None, TextAlign::Start);
            assert!(layout.metrics.size.width.is_finite());
            assert!(
                layout
                    .font_runs
                    .iter()
                    .all(|run| run.range.start <= run.range.end)
            );
        }
    }
    #[test]
    fn registered_fonts_advance_generation_and_invalidate_layouts() {
        let mut engine = TextEngine::new();
        let first = engine.layout("hello", &TextStyle::default(), None, TextAlign::Start);
        let generation = engine.font_database_generation();
        engine.register_font_bytes(Vec::new());
        assert!(engine.font_database_generation() > generation);
        let second = engine.layout("hello", &TextStyle::default(), None, TextAlign::Start);
        assert!(!Arc::ptr_eq(&first, &second));
    }
}
