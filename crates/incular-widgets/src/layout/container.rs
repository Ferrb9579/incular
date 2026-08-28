//! Flutter-style Container composition widget.
//!
//! [`Container`] is a convenience composition widget that combines common
//! painting, positioning, sizing, padding, margins, and transforms into a single
//! ergonomic descriptor. It lowers to existing primitive widgets without introducing
//! a duplicate layout or rendering engine.

use incular_config::{Alignment, Clip, Constraints, EdgeInsets};
use incular_core::{Color, Transform as CoreTransform};
use incular_rendering::{Border, Brush, CornerRadii};
use typed_builder::TypedBuilder;

use crate::layout::basic::{Align, ClipRRect, ClipRect, ConstrainedBox, Padding, SizedBox};
use crate::{DecoratedBox, Transform, Widget};

/// A convenience composition widget combining sizing, constraints, margin,
/// padding, background color/decoration, alignment, transforms, and clipping.
#[derive(Clone, Debug, Default, PartialEq, TypedBuilder)]
pub struct Container {
    #[builder(default, setter(strip_option, into))]
    pub child: Option<Widget>,
    #[builder(default, setter(strip_option))]
    pub alignment: Option<Alignment>,
    #[builder(default, setter(strip_option))]
    pub padding: Option<EdgeInsets>,
    #[builder(default, setter(strip_option))]
    pub margin: Option<EdgeInsets>,
    #[builder(default, setter(strip_option))]
    pub color: Option<Color>,
    #[builder(default, setter(strip_option, into))]
    pub background: Option<Brush>,
    #[builder(default, setter(strip_option, into))]
    pub border: Option<Border>,
    #[builder(default, setter(into))]
    pub radius: CornerRadii,
    #[builder(default, setter(strip_option))]
    pub constraints: Option<Constraints>,
    #[builder(default, setter(strip_option))]
    pub width: Option<f32>,
    #[builder(default, setter(strip_option))]
    pub height: Option<f32>,
    #[builder(default, setter(strip_option))]
    pub transform: Option<CoreTransform>,
    #[builder(default)]
    pub clip_behavior: Clip,
}

impl Container {
    /// Creates a new empty Container.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new Container wrapping a child widget.
    #[must_use]
    pub fn with_child(child: impl Into<Widget>) -> Self {
        Self {
            child: Some(child.into()),
            ..Self::default()
        }
    }

    /// Creates an empty Container.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Sets the child widget.
    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = Some(child.into());
        self
    }

    /// Sets inner alignment of the child within container bounds.
    #[must_use]
    pub fn alignment(mut self, alignment: Alignment) -> Self {
        self.alignment = Some(alignment);
        self
    }

    /// Sets inner padding.
    #[must_use]
    pub fn padding(mut self, padding: EdgeInsets) -> Self {
        self.padding = Some(padding);
        self
    }

    /// Sets outer margin.
    #[must_use]
    pub fn margin(mut self, margin: EdgeInsets) -> Self {
        self.margin = Some(margin);
        self
    }

    /// Sets background solid color.
    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// Sets background brush (color, gradient, etc.).
    #[must_use]
    pub fn background(mut self, brush: impl Into<Brush>) -> Self {
        self.background = Some(brush.into());
        self
    }

    /// Sets container border.
    #[must_use]
    pub fn border(mut self, border: impl Into<Border>) -> Self {
        self.border = Some(border.into());
        self
    }

    /// Sets container visual decoration (color, border, radius).
    #[must_use]
    pub fn decoration(mut self, decoration: crate::BoxDecoration) -> Self {
        if let Some(c) = decoration.color {
            self.color = Some(c);
        }
        if let Some(b) = decoration.border {
            self.border = Some(b.into());
        }
        if let Some(r) = decoration.border_radius {
            self.radius = r.into();
        }
        self
    }

    /// Sets container corner radii.
    #[must_use]
    pub fn radius(mut self, radius: impl Into<CornerRadii>) -> Self {
        self.radius = radius.into();
        self
    }

    /// Sets explicit fixed width.
    #[must_use]
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Sets explicit fixed height.
    #[must_use]
    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }

    /// Sets additional layout constraints.
    #[must_use]
    pub fn constraints(mut self, constraints: Constraints) -> Self {
        self.constraints = Some(constraints);
        self
    }

    /// Sets transform matrix applied to the container.
    #[must_use]
    pub fn transform(mut self, transform: CoreTransform) -> Self {
        self.transform = Some(transform);
        self
    }

    /// Sets clipping behavior.
    #[must_use]
    pub fn clip_behavior(mut self, clip: Clip) -> Self {
        self.clip_behavior = clip;
        self
    }
}

impl From<Container> for Widget {
    fn from(value: Container) -> Self {
        let has_explicit_dimensions =
            value.width.is_some() || value.height.is_some() || value.constraints.is_some();

        let mut current = if let Some(child) = value.child {
            child
        } else if value.alignment.is_some() || has_explicit_dimensions {
            SizedBox::shrink().into()
        } else {
            // Flutter Container semantics: when no child and no alignment/explicit constraints,
            // expand to fill bounded constraints, and shrink to zero in unbounded constraints.
            Align::new(Alignment::CENTER, SizedBox::shrink()).into()
        };

        // 1. Inner Alignment
        if let Some(alignment) = value.alignment {
            current = Align::new(alignment, current).into();
        }

        // 2. Padding
        if let Some(padding) = value.padding {
            if !padding.is_zero() {
                current = Padding::new(padding, current).into();
            }
        }

        // 3. Decoration (Color, Brush, Border, Radius)
        let has_decoration = value.color.is_some()
            || value.background.is_some()
            || value.border.is_some()
            || !value.radius.is_zero();
        if has_decoration {
            let mut decorated = DecoratedBox::new(current).radius(value.radius.top_left);
            if let Some(color) = value.color {
                decorated = decorated.background(color);
            } else if let Some(bg) = value.background {
                decorated = decorated.background(bg);
            }
            if let Some(border) = value.border {
                decorated = decorated.border(border);
            }
            current = decorated.into();
        }

        // 4. Sizing / Constraints
        let mut explicit_constraints = value.constraints.unwrap_or_else(Constraints::unbounded);
        if let Some(w) = value.width {
            explicit_constraints = Constraints::new(
                w,
                w,
                explicit_constraints.min_height,
                explicit_constraints.max_height,
            );
        }
        if let Some(h) = value.height {
            explicit_constraints = Constraints::new(
                explicit_constraints.min_width,
                explicit_constraints.max_width,
                h,
                h,
            );
        }
        if explicit_constraints != Constraints::unbounded() {
            current = ConstrainedBox::new(explicit_constraints, current).into();
        }

        // 5. Margin (outer padding)
        if let Some(margin) = value.margin {
            if !margin.is_zero() {
                current = Padding::new(margin, current).into();
            }
        }

        // 6. Transform
        if let Some(transform) = value.transform {
            current = Transform::new(transform, current).into();
        }

        // 7. Clipping
        if value.clip_behavior != Clip::None {
            if !value.radius.is_zero() {
                current = ClipRRect::new(value.radius, current)
                    .clip_behavior(value.clip_behavior)
                    .into();
            } else {
                current = ClipRect::new(current)
                    .clip_behavior(value.clip_behavior)
                    .into();
            }
        }

        current
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DecoratedBox, Text, WidgetKind};
    use incular_core::{Size, Transform};

    #[test]
    fn constructors_and_empty_builder_share_the_same_defaults() {
        let default = Container::default();

        assert_eq!(Container::new(), default);
        assert_eq!(Container::empty(), default);
        assert_eq!(Container::builder().build(), default);
        assert_eq!(default.child, None);
        assert_eq!(default.alignment, None);
        assert_eq!(default.padding, None);
        assert_eq!(default.margin, None);
        assert_eq!(default.color, None);
        assert_eq!(default.background, None);
        assert_eq!(default.border, None);
        assert_eq!(default.radius, CornerRadii::default());
        assert_eq!(default.constraints, None);
        assert_eq!(default.width, None);
        assert_eq!(default.height, None);
        assert_eq!(default.transform, None);
        assert_eq!(default.clip_behavior, Clip::default());
    }

    #[test]
    fn builder_accepts_text_and_existing_widget_values() {
        let text = Container::builder().child(Text::new("hello")).build();
        assert_eq!(
            text.child.as_ref().and_then(Widget::text_if_any),
            Some(String::from("hello"))
        );

        let widget = Widget::box_(Size::ZERO, Color::TRANSPARENT);
        let existing_widget = Container::builder().child(widget.clone()).build();
        assert_eq!(existing_widget.child, Some(widget));
    }

    #[test]
    fn builder_accepts_arbitrary_into_widget_children() {
        let decorated = DecoratedBox::new(Text::new("decorated"));
        let container = Container::builder().child(decorated).build();

        assert!(container.child.is_some());
    }

    #[test]
    fn struct_literals_require_explicit_widget_conversion() {
        let container = Container {
            child: Some(Text::new("literal").into()),
            ..Default::default()
        };

        assert_eq!(
            container.child.as_ref().and_then(Widget::text_if_any),
            Some(String::from("literal"))
        );
    }

    #[test]
    fn fluent_child_apis_accept_widgets() {
        let with_child = Container::with_child(Text::new("fluent"));
        let fluent = Container::empty().child(Text::new("fluent"));

        assert_eq!(
            with_child.child.as_ref().and_then(Widget::text_if_any),
            Some(String::from("fluent"))
        );
        assert_eq!(
            fluent.child.as_ref().and_then(Widget::text_if_any),
            Some(String::from("fluent"))
        );
    }

    #[test]
    fn lowering_preserves_the_outer_wrapper_order() {
        let decorated: Widget = Container::builder()
            .child(Text::new("child"))
            .padding(EdgeInsets::all(4.0))
            .color(Color::WHITE)
            .clip_behavior(Clip::None)
            .build()
            .into();
        assert!(matches!(decorated.kind, WidgetKind::Decorated { .. }));

        let constrained: Widget = Container::builder()
            .child(Text::new("child"))
            .padding(EdgeInsets::all(4.0))
            .width(120.0)
            .clip_behavior(Clip::None)
            .build()
            .into();
        assert!(matches!(constrained.kind, WidgetKind::Constrained { .. }));

        let margin: Widget = Container::builder()
            .child(Text::new("child"))
            .width(120.0)
            .margin(EdgeInsets::all(8.0))
            .clip_behavior(Clip::None)
            .build()
            .into();
        assert!(matches!(margin.kind, WidgetKind::Padding { .. }));

        let transformed: Widget = Container::builder()
            .child(Text::new("child"))
            .margin(EdgeInsets::all(8.0))
            .transform(Transform::IDENTITY)
            .clip_behavior(Clip::None)
            .build()
            .into();
        assert!(matches!(transformed.kind, WidgetKind::Transform { .. }));

        let clipped: Widget = Container::builder()
            .child(Text::new("child"))
            .clip_behavior(Clip::AntiAlias)
            .build()
            .into();
        assert!(matches!(clipped.kind, WidgetKind::ClipRect { .. }));

        let rounded: Widget = Container::builder()
            .child(Text::new("child"))
            .radius(CornerRadii::uniform(8.0))
            .clip_behavior(Clip::AntiAlias)
            .build()
            .into();
        assert!(matches!(rounded.kind, WidgetKind::ClipRRect { .. }));
    }

    #[test]
    fn lowering_without_clipping_keeps_content_sized_child_path() {
        let widget: Widget = Container::builder()
            .child(Text::new("content"))
            .clip_behavior(Clip::None)
            .build()
            .into();

        assert!(matches!(widget.kind, WidgetKind::Text { .. }));
    }

    #[test]
    fn empty_container_lowering_keeps_fallback_alignment_behavior() {
        let widget: Widget = Container::builder()
            .clip_behavior(Clip::None)
            .build()
            .into();

        assert!(matches!(widget.kind, WidgetKind::Align { .. }));
    }
}
