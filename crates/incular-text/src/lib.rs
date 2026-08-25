//! Renderer-independent text primitives and cached logical text layout.
//!
//! The public surface is split into small modules so consumers can use the
//! text model (`spans`), editor state (`editing`), and shaping engine
//! independently. Existing font/layout exports remain available at the crate
//! root for backwards compatibility.
mod editing;
mod spans;
mod style;

pub use editing::{
    ComposingRange, EditableText, SelectionChangedCause, TextAffinity, TextEditingController,
    TextEditingDelta, TextEditingValue, TextField, TextRange, TextSelection,
};
pub use spans::{
    InlineSpan, RichText, Text, TextSpan, TextSpanVisitor, WidgetSpan, WidgetSpanAlignment,
};
pub use style::{
    FontFamily, FontFeature, FontStyle, FontVariation, FontWeight, LineHeight, StrutStyle,
    TextAlign, TextBaseline, TextDecoration, TextDecorationStyle, TextHeightBehavior,
    TextLeadingDistribution, TextOverflow, TextScaler, TextScalerKind, TextShadow, TextStyle,
    TextWidthBasis,
};

mod engine;
pub use engine::{
    FontId, FontRunDebug, TextDiagnostics, TextEngine, TextLayout, TextLayoutOptions, TextLine,
    TextMetrics,
};
#[cfg(test)]
mod tests {
    use super::*;
    use incular_core::Color;
    use std::sync::Arc;
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

    #[test]
    fn soft_wrapping_and_max_lines_bound_retained_layout() {
        let mut engine = TextEngine::new();
        let layout = engine.layout_with_options(
            "one two three four five six seven eight",
            &TextStyle::default().font_size(18.),
            TextLayoutOptions::new(Some(70.), TextAlign::Start)
                .max_lines(Some(2))
                .overflow(TextOverflow::Clip),
        );
        assert_eq!(layout.lines.len(), 2);
        assert!(layout.overflowed);

        let unwrapped = engine.layout_with_options(
            "one two three four",
            &TextStyle::default().font_size(18.),
            TextLayoutOptions::new(Some(20.), TextAlign::Start).soft_wrap(false),
        );
        assert_eq!(unwrapped.lines.len(), 1);
        assert!(unwrapped.overflowed);
    }

    #[test]
    fn ellipsis_is_shaped_and_never_splits_bidi_graphemes() {
        let mut engine = TextEngine::new();
        let source = "עברית café 👩\u{200d}💻 mixed English words";
        let clipped = engine.layout_with_options(
            source,
            &TextStyle::default().font_size(20.),
            TextLayoutOptions::new(Some(95.), TextAlign::Start)
                .max_lines(Some(1))
                .overflow(TextOverflow::Clip),
        );
        let ellipsized = engine.layout_with_options(
            source,
            &TextStyle::default().font_size(20.),
            TextLayoutOptions::new(Some(95.), TextAlign::Start)
                .max_lines(Some(1))
                .overflow(TextOverflow::Ellipsis),
        );
        assert_eq!(ellipsized.lines.len(), 1);
        assert!(ellipsized.overflowed);
        // The final source is re-shaped with U+2026, so it has at least one
        // renderer glyph even when the visible source prefix is empty.
        assert!(ellipsized.glyph_count() > 0);
        assert!(ellipsized.glyph_count() >= clipped.glyph_count());
        assert!(
            ellipsized
                .font_runs
                .iter()
                .any(|run| run.direction == "rtl")
        );
    }
}
