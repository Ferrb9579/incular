use incular_assets::FontHandle;
use incular_core::Offset;
use std::sync::Arc;

/// A positioned glyph produced by a text shaper. It is intentionally not a
/// character: a glyph can represent multiple Unicode scalars or none.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlyphPosition {
    pub id: u16,
    pub offset: Offset,
    pub advance: f32,
    pub cluster: u32,
}
/// Compact, contiguous shaped glyph data shared by display-list commands.
#[derive(Clone, Debug, PartialEq)]
pub struct GlyphRun {
    pub font: FontHandle,
    pub font_size: f32,
    pub origin: Offset,
    pub glyphs: Arc<[GlyphPosition]>,
}
