//! Transforming layout primitives.

use typed_builder::TypedBuilder;

use crate::Widget;

/// Rotates its child by an integral number of quarter turns (90 degrees each).
///
/// Layout measures the child normally: allocated dimensions never swap,
/// including odd quarter turns. The rotation applies at the compositor
/// about the child's center, observed by paint, hit testing, and
/// semantics — exactly like an equivalent presentation transform.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct RotatedBox {
    quarter_turns: i32,
    #[builder(setter(into))]
    child: Widget,
}

impl RotatedBox {
    #[must_use]
    pub fn new(quarter_turns: i32, child: impl Into<Widget>) -> Self {
        Self {
            quarter_turns,
            child: child.into(),
        }
    }
}

impl From<RotatedBox> for Widget {
    fn from(value: RotatedBox) -> Self {
        let radians = (value.quarter_turns as f32) * std::f32::consts::FRAC_PI_2;
        crate::Transform::rotation(radians, value.child).into()
    }
}

/// Translates its child by a fraction of the child's size.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct FractionalTranslation {
    translation: incular_core::Offset,
    #[builder(default = true)]
    transform_hit_tests: bool,
    #[builder(setter(into))]
    child: Widget,
}

impl FractionalTranslation {
    #[must_use]
    pub fn new(translation: incular_core::Offset, child: impl Into<Widget>) -> Self {
        Self {
            translation,
            transform_hit_tests: true,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn transform_hit_tests(mut self, transform: bool) -> Self {
        self.transform_hit_tests = transform;
        self
    }
}

impl From<FractionalTranslation> for Widget {
    fn from(value: FractionalTranslation) -> Self {
        Widget::fractional_translation(value.translation, value.transform_hit_tests, value.child)
    }
}
