//! Parley-backed text shaping, fallback, bidi ordering, and line breaking.
//!
//! Incular deliberately keeps its renderer-facing glyph protocol small. Parley
//! owns all font discovery, fallback selection and shaping; this module only
//! translates its positioned glyph runs into `incular-rendering` commands.

use super::{FontFamily, FontStyle, TextAlign, TextOverflow, TextStyle};
use icu_segmenter::GraphemeClusterSegmenter;
use incular_assets::FontHandle;
pub use incular_assets::FontId;
use incular_core::{Offset, Size};
use incular_rendering::{GlyphPosition, GlyphRun};
use parley::{
    Alignment, AlignmentOptions, FontContext, FontStack, FontWeight as ParleyFontWeight, Layout,
    LayoutContext, LineHeight, PositionedLayoutItem, StyleProperty,
};
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet, VecDeque, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    sync::Arc,
};

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
    /// End of selectable/caret text on this visual line. This excludes a
    /// trailing hard line-break which has no shaped glyph of its own.
    pub caret_end: usize,
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
    /// Whether the source had content outside the retained layout's visible
    /// bounds. `Visible` layouts deliberately leave this false.
    pub overflowed: bool,
}

/// Paragraph-level layout controls. They deliberately describe behavior in
/// Incular terms and do not expose Parley builder types.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextLayoutOptions {
    pub max_width: Option<f32>,
    pub align: TextAlign,
    pub soft_wrap: bool,
    pub max_lines: Option<usize>,
    pub overflow: TextOverflow,
}

impl TextLayoutOptions {
    #[must_use]
    pub const fn new(max_width: Option<f32>, align: TextAlign) -> Self {
        Self {
            max_width,
            align,
            soft_wrap: true,
            max_lines: None,
            overflow: TextOverflow::Clip,
        }
    }

    #[must_use]
    pub const fn soft_wrap(mut self, soft_wrap: bool) -> Self {
        self.soft_wrap = soft_wrap;
        self
    }

    #[must_use]
    pub const fn max_lines(mut self, max_lines: Option<usize>) -> Self {
        self.max_lines = max_lines;
        self
    }

    #[must_use]
    pub const fn overflow(mut self, overflow: TextOverflow) -> Self {
        self.overflow = overflow;
        self
    }
}

impl Default for TextLayoutOptions {
    fn default() -> Self {
        Self::new(None, TextAlign::Start)
    }
}

impl TextLayout {
    #[must_use]
    pub fn glyph_count(&self) -> usize {
        self.lines.iter().map(|line| line.glyphs.len()).sum()
    }
}

/// Layout/cache counters. Legacy field names are retained for source
/// compatibility; Parley performs fallback and coverage resolution internally.
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
    font_variations: Option<Arc<str>>,
    font_features: Option<Arc<str>>,
    width: Option<u32>,
    align: TextAlign,
    soft_wrap: bool,
    max_lines: Option<usize>,
    overflow: TextOverflow,
    generation: u64,
}

impl LayoutKey {
    fn new(text: &str, style: &TextStyle, options: TextLayoutOptions, generation: u64) -> Self {
        Self {
            text: text.into(),
            family: style.family.clone(),
            fallbacks: style.fallback_families.clone(),
            size: style.size.to_bits(),
            weight: style.weight.0,
            style: style.style,
            line_height: style.line_height.map(f32::to_bits),
            letter_spacing: style.letter_spacing.to_bits(),
            font_variations: style.font_variations.clone(),
            font_features: style.font_features.clone(),
            width: options.max_width.map(f32::to_bits),
            align: options.align,
            soft_wrap: options.soft_wrap,
            max_lines: options.max_lines,
            overflow: options.overflow,
            generation,
        }
    }
}

/// The sole owner of font discovery and app-registered fonts. Parley owns the
/// actual font collection and resolves scripts, bidi direction and fallback.
pub struct TextEngine {
    font_context: FontContext,
    layout_context: LayoutContext<()>,
    generation: u64,
    cache: HashMap<LayoutKey, Arc<TextLayout>>,
    order: VecDeque<LayoutKey>,
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
        Self {
            font_context: FontContext::new(),
            layout_context: LayoutContext::new(),
            generation: 1,
            cache: HashMap::new(),
            order: VecDeque::new(),
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

    /// Registers all faces in a TrueType/OpenType byte buffer with Parley's
    /// Fontique collection. Invalid buffers are harmless: Fontique ignores
    /// them, while the generation still invalidates stale cached layouts.
    pub fn register_font_bytes(&mut self, bytes: impl Into<Vec<u8>>) {
        self.font_context
            .collection
            .register_fonts(parley::fontique::Blob::from(bytes.into()), None);
        self.generation += 1;
        self.cache.clear();
        self.order.clear();
    }

    pub fn layout(
        &mut self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f32>,
        align: TextAlign,
    ) -> Arc<TextLayout> {
        self.layout_with_options(text, style, TextLayoutOptions::new(max_width, align))
    }

    /// Shapes a paragraph with retained wrapping and overflow policy.
    pub fn layout_with_options(
        &mut self,
        text: &str,
        style: &TextStyle,
        options: TextLayoutOptions,
    ) -> Arc<TextLayout> {
        self.diagnostics.layouts_requested += 1;
        let key = LayoutKey::new(text, style, options, self.generation);
        if let Some(layout) = self.cache.get(&key) {
            self.diagnostics.cache_hits += 1;
            return layout.clone();
        }

        self.diagnostics.cache_misses += 1;
        let layout = Arc::new(self.shape_and_wrap(text, style, options));
        if self.order.len() == 256 {
            if let Some(old) = self.order.pop_front() {
                self.cache.remove(&old);
            }
        }
        self.order.push_back(key.clone());
        self.cache.insert(key, layout.clone());
        layout
    }

    fn font_stack(style: &TextStyle) -> FontStack<'static> {
        let mut names = vec![family_name(&style.family)];
        names.extend(style.fallback_families.iter().map(family_name));
        FontStack::Source(names.join(", ").into())
    }

    fn shape_and_wrap(
        &mut self,
        text: &str,
        style: &TextStyle,
        options: TextLayoutOptions,
    ) -> TextLayout {
        let wrap_width = options.soft_wrap.then_some(options.max_width).flatten();
        let layout = self.shape_raw(text, style, wrap_width, options.align);
        let horizontal_overflow = !options.soft_wrap
            && options
                .max_width
                .is_some_and(|width| layout.metrics.size.width > width);
        let line_limit = options
            .max_lines
            .or_else(|| horizontal_overflow.then_some(1));
        let line_overflow = line_limit.is_some_and(|limit| layout.lines.len() > limit);
        if (!horizontal_overflow && !line_overflow) || options.overflow == TextOverflow::Visible {
            return layout;
        }

        let limit = line_limit.unwrap_or(layout.lines.len());
        match options.overflow {
            TextOverflow::Visible => layout,
            TextOverflow::Clip => clipped_layout(layout, limit),
            TextOverflow::Ellipsis => self.ellipsized_layout(text, style, options, limit),
        }
    }

    fn ellipsized_layout(
        &mut self,
        text: &str,
        style: &TextStyle,
        options: TextLayoutOptions,
        limit: usize,
    ) -> TextLayout {
        if limit == 0 {
            return clipped_layout(self.shape_raw("", style, None, options.align), 0);
        }
        let wrap_width = options.soft_wrap.then_some(options.max_width).flatten();
        let initial = self.shape_raw(text, style, wrap_width, options.align);
        let end = initial
            .lines
            .get(limit.saturating_sub(1))
            .map_or(0, |line| line.caret_end.min(text.len()));
        let mut end = grapheme_boundary_at_or_before(text, end);

        loop {
            let mut candidate = text[..end].to_owned();
            candidate.push('\u{2026}');
            // The ellipsis always enters Parley as source text and is shaped
            // with the selected/fallback font stack, never appended as a raw
            // renderer glyph or a byte-truncated source suffix.
            let shaped = self.shape_raw(&candidate, style, wrap_width, options.align);
            let fits_width = options.max_width.is_none_or(|width| {
                shaped
                    .lines
                    .iter()
                    .all(|line| line.width <= width + f32::EPSILON)
            });
            if shaped.lines.len() <= limit && fits_width {
                return TextLayout {
                    overflowed: true,
                    ..shaped
                };
            }
            let previous = previous_grapheme_boundary(text, end);
            if previous == end {
                return TextLayout {
                    overflowed: true,
                    ..shaped
                };
            }
            end = previous;
        }
    }

    fn shape_raw(
        &mut self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f32>,
        align: TextAlign,
    ) -> TextLayout {
        let mut builder =
            self.layout_context
                .ranged_builder(&mut self.font_context, text, 1.0, true);
        builder.push_default(StyleProperty::FontStack(Self::font_stack(style)));
        builder.push_default(StyleProperty::FontSize(style.size.max(0.0)));
        builder.push_default(StyleProperty::FontWeight(ParleyFontWeight::new(
            style.weight.0 as f32,
        )));
        builder.push_default(StyleProperty::FontStyle(match style.style {
            FontStyle::Normal => parley::FontStyle::Normal,
            FontStyle::Italic => parley::FontStyle::Italic,
        }));
        builder.push_default(StyleProperty::LetterSpacing(style.letter_spacing));
        if let Some(settings) = &style.font_variations {
            builder.push_default(StyleProperty::FontVariations(parley::FontSettings::Source(
                Cow::Owned(settings.to_string()),
            )));
        }
        if let Some(settings) = &style.font_features {
            builder.push_default(StyleProperty::FontFeatures(parley::FontSettings::Source(
                Cow::Owned(settings.to_string()),
            )));
        }
        if let Some(line_height) = style.line_height {
            builder.push_default(StyleProperty::LineHeight(LineHeight::Absolute(line_height)));
        }

        let mut layout: Layout<()> = builder.build(text);
        layout.break_all_lines(max_width);
        layout.align(
            max_width,
            match align {
                TextAlign::Start => Alignment::Start,
                TextAlign::Center => Alignment::Center,
                TextAlign::End => Alignment::End,
            },
            AlignmentOptions::default(),
        );

        self.translate_layout(&layout)
    }

    fn translate_layout(&mut self, layout: &Layout<()>) -> TextLayout {
        let mut lines = Vec::new();
        let mut font_runs = Vec::new();
        let mut used_fonts = HashSet::new();
        let primary = layout
            .lines()
            .flat_map(|line| line.runs())
            .next()
            .map(|run| font_id(run.font()));

        for line in layout.lines() {
            let metrics = line.metrics();
            let range = line.text_range();
            let mut runs = Vec::new();
            let mut glyphs = Vec::new();
            let mut caret_end = range.start;
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(parley_run) = item else {
                    continue;
                };
                let run = parley_run.run();
                let font = font_handle(run.font());
                let font_id = font.id();
                if Some(font_id) != primary {
                    self.diagnostics.fallback_runs += 1;
                }
                used_fonts.insert(font_id);
                self.diagnostics.shaping_runs += 1;

                // `PositionedLayoutItem::GlyphRun` is a line-local slice of
                // the underlying shaping run. Parley's renderer-facing Glyph
                // intentionally has no byte index, so recover each glyph's
                // cluster from the authoritative visual-cluster sequence
                // before translating it. This keeps caret movement and
                // selection on shaped cluster boundaries instead of assigning
                // every glyph to the run's first byte.
                let visible_glyphs = parley_run.glyphs().collect::<Vec<_>>();
                let all_clustered_glyphs = run
                    .visual_clusters()
                    .flat_map(|cluster| {
                        let cluster_range = cluster.text_range();
                        cluster
                            .glyphs()
                            .map(move |glyph| (glyph, cluster_range.clone()))
                    })
                    .collect::<Vec<_>>();
                let first_glyph = all_clustered_glyphs
                    .windows(visible_glyphs.len())
                    .position(|candidate| {
                        candidate
                            .iter()
                            .zip(&visible_glyphs)
                            .all(|((glyph, _), visible)| glyph == visible)
                    })
                    .unwrap_or(0);
                let clusters = all_clustered_glyphs
                    .iter()
                    .skip(first_glyph)
                    .map(|(_, range)| range.clone())
                    .take(visible_glyphs.len())
                    .collect::<Vec<_>>();
                caret_end = caret_end.max(
                    clusters
                        .iter()
                        .map(|range| range.end)
                        .max()
                        .unwrap_or(range.start),
                );
                let positions: Vec<_> = parley_run
                    .positioned_glyphs()
                    .zip(
                        clusters
                            .iter()
                            .map(|range| range.start as u32)
                            .chain(std::iter::repeat(run.text_range().start as u32)),
                    )
                    .map(|(glyph, cluster)| GlyphPosition {
                        id: glyph.id as u16,
                        // Incular's renderer uses `origin.y - offset.y`; Parley
                        // exposes glyph y from its top-left layout coordinate.
                        offset: Offset::new(glyph.x, -glyph.y),
                        advance: glyph.advance,
                        cluster,
                    })
                    .collect();
                glyphs.extend(positions.iter().copied());
                runs.push(Arc::new(GlyphRun {
                    font,
                    font_size: run.font_size(),
                    origin: Offset::ZERO,
                    glyphs: positions.into(),
                }));
                font_runs.push(FontRunDebug {
                    range: run.text_range(),
                    font: font_id,
                    family: "Parley/Fontique resolved face".into(),
                    script: "Parley resolved".into(),
                    direction: if run.is_rtl() { "rtl" } else { "ltr" },
                });
            }
            lines.push(TextLine {
                runs: runs.into(),
                glyphs: glyphs.into(),
                width: metrics.advance,
                baseline: metrics.baseline,
                start: range.start,
                caret_end,
                end: range.end,
            });
        }

        if lines.is_empty() {
            lines.push(TextLine {
                runs: Arc::new([]),
                glyphs: Arc::new([]),
                width: 0.0,
                baseline: 0.0,
                start: 0,
                caret_end: 0,
                end: 0,
            });
        }
        self.diagnostics.fallback_fonts_used += used_fonts.len().saturating_sub(1) as u64;
        let baseline = lines[0].baseline;
        let line_height = layout
            .lines()
            .next()
            .map_or(0.0, |line| line.metrics().line_height);
        TextLayout {
            lines: lines.into(),
            metrics: TextMetrics {
                size: Size::new(layout.width(), layout.height()),
                baseline,
                line_height,
            },
            font_runs: font_runs.into(),
            overflowed: false,
        }
    }
}

fn clipped_layout(mut layout: TextLayout, limit: usize) -> TextLayout {
    let lines = layout.lines.iter().take(limit).cloned().collect::<Vec<_>>();
    let width = lines
        .iter()
        .fold(0.0_f32, |width, line| width.max(line.width));
    layout.metrics.size = Size::new(width, layout.metrics.line_height * lines.len() as f32);
    layout.metrics.baseline = lines.first().map_or(0.0, |line| line.baseline);
    layout.lines = lines.into();
    layout.overflowed = true;
    layout
}

fn grapheme_boundary_at_or_before(text: &str, offset: usize) -> usize {
    GraphemeClusterSegmenter::new()
        .segment_str(text)
        .take_while(|boundary| *boundary <= offset)
        .last()
        .unwrap_or(0)
}

fn previous_grapheme_boundary(text: &str, offset: usize) -> usize {
    GraphemeClusterSegmenter::new()
        .segment_str(text)
        .take_while(|boundary| *boundary < offset)
        .last()
        .unwrap_or(0)
}

fn family_name(family: &FontFamily) -> String {
    match family {
        FontFamily::SansSerif => "sans-serif".into(),
        FontFamily::Serif => "serif".into(),
        FontFamily::Monospace => "monospace".into(),
        FontFamily::Cursive => "cursive".into(),
        FontFamily::SystemUi => "system-ui".into(),
        FontFamily::Named(name) => format!("'{name}'"),
    }
}

fn font_id(font: &parley::FontData) -> FontId {
    let mut hasher = DefaultHasher::new();
    font.data.as_ref().hash(&mut hasher);
    font.index.hash(&mut hasher);
    FontId(hasher.finish())
}

fn font_handle(font: &parley::FontData) -> FontHandle {
    FontHandle::with_face_index(
        font_id(font),
        Arc::<[u8]>::from(font.data.as_ref().to_vec()),
        font.index,
    )
}
