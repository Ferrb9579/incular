//! Parse font tables once; decode outlines only for requested atlas glyphs.
use ab_glyph::{Font, FontRef, GlyphId, point};
use std::sync::Arc;

self_cell::self_cell! {
    struct SharedFont {
        owner: Arc<[u8]>,
        #[covariant]
        dependent: FontRef,
    }
}

pub(crate) struct RasterFont(SharedFont);

#[derive(Default)]
pub(crate) struct RasterMetrics {
    pub width: usize,
    pub height: usize,
    pub xmin: i32,
    pub ymin: i32,
}

impl RasterFont {
    pub fn new(bytes: Arc<[u8]>, face_index: u32) -> Option<Self> {
        // Keep the parsed tables tied to the original immutable allocation.
        // The cell moves safely and drops the parser before its shared owner.
        SharedFont::try_new(bytes, |bytes| {
            FontRef::try_from_slice_and_index(bytes, face_index)
        })
        .ok()
        .map(Self)
    }

    pub fn rasterize_indexed(
        &self,
        glyph: u16,
        ppem: f32,
        phase: [u8; 2],
    ) -> Option<(RasterMetrics, Vec<u8>)> {
        let font = self.0.borrow_dependent();
        if usize::from(glyph) >= font.glyph_count() {
            return None;
        }
        // ab_glyph's scale is ascent minus descent, whereas shaped runs and
        // the atlas key use pixels per em. Preserve the shaping engine's scale.
        let scale = ppem * font.height_unscaled() / font.units_per_em()?;
        let position = point(f32::from(phase[0]) / 4., f32::from(phase[1]) / 4.);
        let Some(outline) =
            font.outline_glyph(GlyphId(glyph).with_scale_and_position(scale, position))
        else {
            return Some((RasterMetrics::default(), Vec::new()));
        };
        let bounds = outline.px_bounds();
        let metrics = RasterMetrics {
            width: bounds.width() as usize,
            height: bounds.height() as usize,
            xmin: bounds.min.x as i32,
            // Atlas placement uses a bottom bearing in upward-positive font
            // coordinates; outline bounds use downward-positive screen space.
            ymin: -bounds.max.y as i32,
        };
        let len = metrics.width.checked_mul(metrics.height)?;
        if len > crate::MAX_GLYPH_BITMAP_BYTES {
            return None;
        }
        let mut bitmap = vec![0; len];
        outline.draw(|x, y, coverage| {
            bitmap[y as usize * metrics.width + x as usize] =
                (coverage.clamp(0.0, 1.0) * 255.0).round() as u8;
        });
        Some((metrics, bitmap))
    }
}
