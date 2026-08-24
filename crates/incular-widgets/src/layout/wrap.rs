//! Flutter-style multi-run Wrap layout descriptor.

use incular_config::{Axis, TextDirection, VerticalDirection, WrapAlignment, WrapCrossAlignment};

use crate::{Widget, WidgetKind};

/// A multi-run layout widget that places children sequentially, wrapping onto
/// new runs when exceeding available main-axis extent.
#[derive(Clone, Debug, PartialEq)]
pub struct Wrap {
    children: Vec<Widget>,
    direction: Axis,
    alignment: WrapAlignment,
    spacing: f32,
    run_alignment: WrapAlignment,
    run_spacing: f32,
    cross_axis_alignment: WrapCrossAlignment,
    text_direction: Option<TextDirection>,
    vertical_direction: VerticalDirection,
}

impl Default for Wrap {
    fn default() -> Self {
        Self::new(Vec::<Widget>::new())
    }
}

impl Wrap {
    /// Creates a new Wrap layout.
    #[must_use]
    pub fn new(children: impl IntoIterator<Item = impl Into<Widget>>) -> Self {
        Self {
            children: children.into_iter().map(Into::into).collect(),
            direction: Axis::Horizontal,
            alignment: WrapAlignment::Start,
            spacing: 0.0,
            run_alignment: WrapAlignment::Start,
            run_spacing: 0.0,
            cross_axis_alignment: WrapCrossAlignment::Start,
            text_direction: None,
            vertical_direction: VerticalDirection::Down,
        }
    }

    /// Sets the wrapping main axis.
    #[must_use]
    pub fn direction(mut self, direction: Axis) -> Self {
        self.direction = direction;
        self
    }

    /// Sets how children within a single run align on the main axis.
    #[must_use]
    pub fn alignment(mut self, alignment: WrapAlignment) -> Self {
        self.alignment = alignment;
        self
    }

    /// Sets the spacing between children in the same run.
    #[must_use]
    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing.max(0.0);
        self
    }

    /// Sets how runs align along the cross axis.
    #[must_use]
    pub fn run_alignment(mut self, alignment: WrapAlignment) -> Self {
        self.run_alignment = alignment;
        self
    }

    /// Sets the spacing between runs.
    #[must_use]
    pub fn run_spacing(mut self, spacing: f32) -> Self {
        self.run_spacing = spacing.max(0.0);
        self
    }

    /// Sets how children within a run align relative to each other on the cross axis.
    #[must_use]
    pub fn cross_axis_alignment(mut self, alignment: WrapCrossAlignment) -> Self {
        self.cross_axis_alignment = alignment;
        self
    }

    /// Sets the text direction for horizontal runs.
    #[must_use]
    pub fn text_direction(mut self, direction: TextDirection) -> Self {
        self.text_direction = Some(direction);
        self
    }

    /// Sets the vertical direction for vertical runs.
    #[must_use]
    pub fn vertical_direction(mut self, direction: VerticalDirection) -> Self {
        self.vertical_direction = direction;
        self
    }

    #[must_use]
    pub fn get_direction(&self) -> Axis {
        self.direction
    }

    #[must_use]
    pub fn get_spacing(&self) -> f32 {
        self.spacing
    }

    #[must_use]
    pub fn get_run_spacing(&self) -> f32 {
        self.run_spacing
    }
}

impl From<Wrap> for Widget {
    fn from(value: Wrap) -> Self {
        Widget::from_kind(WidgetKind::Wrap {
            axis: value.direction,
            alignment: value.alignment,
            spacing: value.spacing,
            run_alignment: value.run_alignment,
            run_spacing: value.run_spacing,
            cross_axis_alignment: value.cross_axis_alignment,
            text_direction: value.text_direction.unwrap_or(TextDirection::Ltr),
            vertical_direction: value.vertical_direction,
            children: value.children,
        })
    }
}
