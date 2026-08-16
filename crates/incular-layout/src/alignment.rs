//! Backwards-compatible layout re-exports.
//!
//! Canonical ownership moved to `incular-config`; layout algorithms retain
//! these names in their historical module path so downstream code can migrate
//! without a type split.

pub use incular_config::{
    Alignment, AlignmentDirectional, Axis, AxisDirection, CrossAxisAlignment, FlexFit,
    MainAxisAlignment, MainAxisSize, TextDirection, VerticalDirection, WrapAlignment,
    WrapCrossAlignment,
};

#[cfg(test)]
mod tests {
    use super::*;
    use incular_core::{Offset, Size};

    #[test]
    fn alignment_places_child_in_remaining_space() {
        assert_eq!(
            Alignment::CENTER.within(Size::new(100.0, 80.0), Size::new(20.0, 10.0)),
            Offset::new(40.0, 35.0)
        );
        assert_eq!(
            AlignmentDirectional::CENTER_START.resolve(TextDirection::Rtl),
            Alignment::CENTER_RIGHT
        );
    }
}
