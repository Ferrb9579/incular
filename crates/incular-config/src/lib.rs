//! Renderer-independent configuration values shared by Incular layers.
//!
//! The types in this crate describe policy and bounds only. Layout algorithms
//! live in `incular-layout`; widgets, runtime, and platform adapters can use
//! these values without depending on each other.

mod alignment;
mod constraints;
mod defaults;
mod environment;
mod insets;
mod localization;

pub use alignment::{
    Alignment, AlignmentDirectional, Axis, AxisDirection, Clip, CrossAxisAlignment, FlexFit,
    FractionalOffset, MainAxisAlignment, MainAxisSize, StackFit, TextDirection, VerticalDirection,
    WrapAlignment, WrapCrossAlignment,
};
pub use constraints::{ConstraintError, Constraints};
pub use defaults::{ApplicationDefaults, WidgetDefaults};
pub use environment::{Brightness, InputCapabilities, RuntimeEnvironment};
pub use icu_locale::Locale;
pub use insets::{EdgeInsets, EdgeInsetsDirectional};
pub use localization::{
    LocaleResolver, LocalizationCatalog, LocalizationError, LocalizedMessage, PluralCategory,
    PluralForms,
};
