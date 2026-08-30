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
    FontFamily, FontFeature, FontStyle, FontVariation, FontWeight, IconData, LineHeight,
    StrutStyle, TextAlign, TextBaseline, TextDecoration, TextDecorationStyle, TextHeightBehavior,
    TextLeadingDistribution, TextOverflow, TextScaler, TextScalerKind, TextShadow, TextStyle,
    TextWidthBasis,
};

mod engine;
pub use engine::{
    FontId, FontRunDebug, TextCaretPosition, TextDiagnostics, TextEngine, TextLayout,
    TextLayoutOptions, TextLine, TextMetrics,
};
