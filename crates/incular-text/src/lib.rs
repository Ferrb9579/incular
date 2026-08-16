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
    FontFamily, FontStyle, FontWeight, TextAlign, TextScaler, TextScalerKind, TextStyle,
};

mod engine;
pub use engine::{
    FontId, FontRunDebug, TextDiagnostics, TextEngine, TextLayout, TextLine, TextMetrics,
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
}
