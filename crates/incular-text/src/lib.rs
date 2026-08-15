//! Renderer-independent font selection, shaping, and cached logical text layout.
//!
//! Incular uses `fontdb` for system-font discovery, `rustybuzz` for OpenType
//! shaping (including ligatures and complex scripts), and `unicode-linebreak`
//! for line-break opportunities. Rasterization intentionally lives in
//! `incular-wgpu`, not here.
use std::{
    collections::{HashMap, VecDeque, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    sync::Arc,
};

use fontdb::{Database, Family, Query, Style, Weight};
use incular_assets::FontHandle;
use incular_core::{Color, Offset, Size};
use incular_painting::{GlyphPosition, GlyphRun};
use rustybuzz::{Face, UnicodeBuffer, shape};
use unicode_linebreak::linebreaks;

pub use incular_assets::FontId;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum FontFamily {
    SansSerif,
    Serif,
    Monospace,
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

/// Paint color is deliberately separate from the metric fields used as a cache key.
#[derive(Clone, Debug, PartialEq)]
pub struct TextStyle {
    pub family: FontFamily,
    pub size: f32,
    pub weight: FontWeight,
    pub style: FontStyle,
    pub color: Color,
    pub line_height: Option<f32>,
    pub letter_spacing: f32,
}
impl Default for TextStyle {
    fn default() -> Self {
        Self {
            family: FontFamily::SansSerif,
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
    pub run: Arc<GlyphRun>,
    pub width: f32,
    pub baseline: f32,
}
#[derive(Clone, Debug)]
pub struct TextLayout {
    pub lines: Arc<[TextLine]>,
    pub metrics: TextMetrics,
}
impl TextLayout {
    #[must_use]
    pub fn glyph_count(&self) -> usize {
        self.lines.iter().map(|line| line.run.glyphs.len()).sum()
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextDiagnostics {
    pub layouts_requested: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
}

#[derive(Hash, PartialEq, Eq, Clone)]
struct LayoutKey {
    text: String,
    family: FontFamily,
    size: u32,
    weight: u16,
    style: FontStyle,
    line_height: Option<u32>,
    letter_spacing: u32,
    width: Option<u32>,
    align: TextAlign,
}
impl LayoutKey {
    fn new(text: &str, style: &TextStyle, width: Option<f32>, align: TextAlign) -> Self {
        Self {
            text: text.into(),
            family: style.family.clone(),
            size: style.size.to_bits(),
            weight: style.weight.0,
            style: style.style,
            line_height: style.line_height.map(f32::to_bits),
            letter_spacing: style.letter_spacing.to_bits(),
            width: width.map(f32::to_bits),
            align,
        }
    }
}

/// Bounded per-widget-tree layout cache. Entries are retained for 256 distinct
/// metric keys; oldest entries are evicted, while render objects retain live
/// layouts through `Arc`.
pub struct TextEngine {
    database: Database,
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
        let mut database = Database::new();
        database.load_system_fonts();
        Self {
            database,
            cache: HashMap::new(),
            order: VecDeque::new(),
            diagnostics: TextDiagnostics::default(),
        }
    }
    #[must_use]
    pub const fn diagnostics(&self) -> TextDiagnostics {
        self.diagnostics
    }
    pub fn layout(
        &mut self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f32>,
        align: TextAlign,
    ) -> Arc<TextLayout> {
        self.diagnostics.layouts_requested += 1;
        let key = LayoutKey::new(text, style, max_width, align);
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
    fn select_font(&self, style: &TextStyle) -> Option<FontHandle> {
        let family = match &style.family {
            FontFamily::SansSerif => Family::SansSerif,
            FontFamily::Serif => Family::Serif,
            FontFamily::Monospace => Family::Monospace,
            FontFamily::Named(name) => Family::Name(name),
        };
        let id = self.database.query(&Query {
            families: &[family],
            weight: Weight(style.weight.0),
            style: match style.style {
                FontStyle::Normal => Style::Normal,
                FontStyle::Italic => Style::Italic,
            },
            ..Query::default()
        })?;
        self.database.with_face_data(id, |bytes, _index| {
            let mut hasher = DefaultHasher::new();
            bytes.hash(&mut hasher);
            FontHandle::new(FontId(hasher.finish()), Arc::<[u8]>::from(bytes.to_vec()))
        })
    }
    fn shape_and_wrap(
        &self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f32>,
        align: TextAlign,
    ) -> TextLayout {
        let line_height = style.line_height.unwrap_or(style.size * 1.2);
        let baseline = style.size * 0.8;
        let Some(font) = self.select_font(style) else {
            return TextLayout {
                lines: Arc::new([]),
                metrics: TextMetrics {
                    size: Size::ZERO,
                    baseline,
                    line_height,
                },
            };
        };
        let Some(face) = Face::from_slice(font.bytes(), 0) else {
            return TextLayout {
                lines: Arc::new([]),
                metrics: TextMetrics {
                    size: Size::ZERO,
                    baseline,
                    line_height,
                },
            };
        };
        let mut buffer = UnicodeBuffer::new();
        buffer.push_str(text);
        let glyph_buffer = shape(&face, &[], buffer);
        let scale = style.size / face.units_per_em() as f32;
        let opportunities: Vec<u32> = linebreaks(text).map(|(index, _)| index as u32).collect();
        let mut opportunity = 0usize;
        let mut next_break = opportunities.first().copied().unwrap_or(u32::MAX);
        let mut lines: Vec<Vec<GlyphPosition>> = vec![Vec::new()];
        let mut widths = vec![0_f32];
        for (info, position) in glyph_buffer
            .glyph_infos()
            .iter()
            .zip(glyph_buffer.glyph_positions())
        {
            while info.cluster >= next_break {
                opportunity += 1;
                next_break = opportunities.get(opportunity).copied().unwrap_or(u32::MAX);
            }
            let advance = position.x_advance as f32 * scale + style.letter_spacing;
            let width = *widths.last().expect("line") + advance.max(0.);
            if max_width.is_some_and(|limit| width > limit)
                && !lines.last().expect("line").is_empty()
            {
                lines.push(Vec::new());
                widths.push(0.);
            }
            let current_width = *widths.last().expect("line");
            lines.last_mut().expect("line").push(GlyphPosition {
                id: info.glyph_id as u16,
                offset: Offset::new(
                    current_width + position.x_offset as f32 * scale,
                    position.y_offset as f32 * scale,
                ),
                advance,
                cluster: info.cluster,
            });
            *widths.last_mut().expect("line") += advance.max(0.);
        }
        if lines.len() == 1 && lines[0].is_empty() {
            lines.clear();
            widths.clear();
        }
        let max_line = widths.iter().copied().fold(0., f32::max);
        let runs: Vec<_> = lines
            .into_iter()
            .zip(widths.iter().copied())
            .enumerate()
            .map(|(line_index, (glyphs, width))| {
                let x = match align {
                    TextAlign::Start => 0.,
                    TextAlign::Center => (max_width.unwrap_or(max_line) - width).max(0.) / 2.,
                    TextAlign::End => (max_width.unwrap_or(max_line) - width).max(0.),
                };
                TextLine {
                    run: Arc::new(GlyphRun {
                        font: font.clone(),
                        font_size: style.size,
                        origin: Offset::new(x, line_index as f32 * line_height + baseline),
                        glyphs: glyphs.into(),
                    }),
                    width,
                    baseline: line_index as f32 * line_height + baseline,
                }
            })
            .collect();
        TextLayout {
            lines: runs.into(),
            metrics: TextMetrics {
                size: Size::new(max_line, line_height * widths.len() as f32),
                baseline,
                line_height,
            },
        }
    }
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
    fn handles_empty_unicode_and_combining_input() {
        let mut engine = TextEngine::new();
        for text in ["", "ASCII", "नमस्ते 世界", "e\u{301}"] {
            let layout = engine.layout(text, &TextStyle::default(), Some(50.), TextAlign::Start);
            assert!(layout.metrics.size.width.is_finite());
        }
    }
}
