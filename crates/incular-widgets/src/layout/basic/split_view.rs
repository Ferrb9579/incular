//! Dual-pane split-view layout primitives.

use incular_config::{Alignment, Axis};
use incular_core::Color;
use typed_builder::TypedBuilder;

use super::SizedBox;
use crate::Widget;

/// Explicit positioning strategy for dual-pane splits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SplitPosition {
    /// Fixed extent for the first (left/top) pane from start.
    FromStart(f32),
    /// Fixed extent for the second (right/bottom) pane from end.
    FromEnd(f32),
    /// Proportional fraction (0.0 .. 1.0) between panes.
    Fraction(f32),
}

/// A resizable dual-pane container separated by an interactive divider.
#[derive(Clone, TypedBuilder)]
pub struct SplitView {
    axis: Axis,
    #[builder(setter(into))]
    first: Widget,
    #[builder(setter(into))]
    second: Widget,
    #[builder(default = SplitPosition::Fraction(0.5))]
    pub(super) position: SplitPosition,
    #[builder(default)]
    min_first: f32,
    #[builder(default = f32::INFINITY)]
    max_first: f32,
    #[builder(default)]
    min_second: f32,
    #[builder(default = f32::INFINITY)]
    max_second: f32,
    #[builder(default = 8.0)]
    pub(super) divider_hit_extent: f32,
    #[builder(default = 1.0)]
    pub(super) divider_visual_extent: f32,
    #[builder(default, setter(strip_option))]
    divider_color: Option<Color>,
    #[builder(default, setter(skip))]
    pub(super) on_split_changed: Option<std::rc::Rc<dyn Fn(f32)>>,
}

impl SplitView {
    #[must_use]
    pub fn horizontal(first: impl Into<Widget>, second: impl Into<Widget>) -> Self {
        Self {
            axis: Axis::Horizontal,
            first: first.into(),
            second: second.into(),
            position: SplitPosition::Fraction(0.5),
            min_first: 0.0,
            max_first: f32::INFINITY,
            min_second: 0.0,
            max_second: f32::INFINITY,
            divider_hit_extent: 8.0,
            divider_visual_extent: 1.0,
            divider_color: None,
            on_split_changed: None,
        }
    }

    #[must_use]
    pub fn vertical(first: impl Into<Widget>, second: impl Into<Widget>) -> Self {
        Self {
            axis: Axis::Vertical,
            first: first.into(),
            second: second.into(),
            position: SplitPosition::Fraction(0.5),
            min_first: 0.0,
            max_first: f32::INFINITY,
            min_second: 0.0,
            max_second: f32::INFINITY,
            divider_hit_extent: 8.0,
            divider_visual_extent: 1.0,
            divider_color: None,
            on_split_changed: None,
        }
    }

    /// Sets explicit fixed extent for the first pane.
    #[must_use]
    pub fn first_extent(mut self, extent: f32) -> Self {
        self.position = SplitPosition::FromStart(extent.max(0.0));
        self
    }

    /// Sets explicit fixed extent for the second pane.
    #[must_use]
    pub fn second_extent(mut self, extent: f32) -> Self {
        self.position = SplitPosition::FromEnd(extent.max(0.0));
        self
    }

    /// Sets explicit split positioning strategy.
    #[must_use]
    pub fn split_position(mut self, position: SplitPosition) -> Self {
        self.position = position;
        self
    }

    /// Convenience alias for `first_extent`.
    #[must_use]
    pub fn split_offset(self, offset: f32) -> Self {
        self.first_extent(offset)
    }

    /// Sets proportional split fraction between 0.0 and 1.0.
    #[must_use]
    pub fn split_fraction(self, fraction: f32) -> Self {
        self.split_position(SplitPosition::Fraction(fraction.clamp(0.0, 1.0)))
    }

    #[must_use]
    pub fn min_first(mut self, min: f32) -> Self {
        self.min_first = min.max(0.0);
        self
    }

    #[must_use]
    pub fn max_first(mut self, max: f32) -> Self {
        self.max_first = max.max(0.0);
        self
    }

    #[must_use]
    pub fn min_second(mut self, min: f32) -> Self {
        self.min_second = min.max(0.0);
        self
    }

    #[must_use]
    pub fn max_second(mut self, max: f32) -> Self {
        self.max_second = max.max(0.0);
        self
    }

    #[must_use]
    pub fn divider_thickness(mut self, thickness: f32) -> Self {
        self.divider_hit_extent = thickness.max(1.0);
        self.divider_visual_extent = thickness.max(0.0);
        self
    }

    #[must_use]
    pub fn divider_hit_extent(mut self, extent: f32) -> Self {
        self.divider_hit_extent = extent.max(1.0);
        self
    }

    #[must_use]
    pub fn divider_visual_extent(mut self, extent: f32) -> Self {
        self.divider_visual_extent = extent.max(0.0);
        self
    }

    #[must_use]
    pub fn divider_color(mut self, color: Color) -> Self {
        self.divider_color = Some(color);
        self
    }

    #[must_use]
    pub fn on_split_changed(mut self, callback: impl Fn(f32) + 'static) -> Self {
        self.on_split_changed = Some(std::rc::Rc::new(callback));
        self
    }
}

impl From<SplitView> for Widget {
    fn from(view: SplitView) -> Self {
        let callback = view.on_split_changed.clone();
        let axis = view.axis;
        let hit_thickness = view.divider_hit_extent.max(1.0);
        let visual_thickness = view.divider_visual_extent.min(hit_thickness).max(0.0);

        let divider_visual: Widget =
            if let Some(color) = view.divider_color.filter(|_| visual_thickness > 0.0) {
                crate::Container::new()
                    .width(if axis == Axis::Horizontal {
                        visual_thickness
                    } else {
                        0.0
                    })
                    .height(if axis == Axis::Vertical {
                        visual_thickness
                    } else {
                        0.0
                    })
                    .color(color)
                    .into()
            } else {
                SizedBox::new().into()
            };

        let divider = crate::GestureDetector::new(
            crate::Container::new()
                .width(if axis == Axis::Horizontal {
                    hit_thickness
                } else {
                    0.0
                })
                .height(if axis == Axis::Vertical {
                    hit_thickness
                } else {
                    0.0
                })
                .alignment(Alignment::CENTER)
                .child(divider_visual),
        )
        .on_pan_update(move |delta: incular_core::Offset| {
            if let Some(ref cb) = callback {
                cb(if axis == Axis::Horizontal {
                    delta.x
                } else {
                    delta.y
                });
            }
        });

        match view.position {
            SplitPosition::FromStart(extent) => {
                let clamped = extent.clamp(view.min_first, view.max_first);
                match axis {
                    Axis::Horizontal => {
                        let first = SizedBox::new().width(clamped).child(view.first);
                        let items: Vec<Widget> = vec![
                            first.into(),
                            divider.into(),
                            crate::Expanded::new(view.second).into(),
                        ];
                        crate::Row::new(items).into()
                    }
                    Axis::Vertical => {
                        let first = SizedBox::new().height(clamped).child(view.first);
                        let items: Vec<Widget> = vec![
                            first.into(),
                            divider.into(),
                            crate::Expanded::new(view.second).into(),
                        ];
                        crate::Column::new(items).into()
                    }
                }
            }
            SplitPosition::FromEnd(extent) => {
                let clamped = extent.clamp(view.min_second, view.max_second);
                match axis {
                    Axis::Horizontal => {
                        let second = SizedBox::new().width(clamped).child(view.second);
                        let items: Vec<Widget> = vec![
                            crate::Expanded::new(view.first).into(),
                            divider.into(),
                            second.into(),
                        ];
                        crate::Row::new(items).into()
                    }
                    Axis::Vertical => {
                        let second = SizedBox::new().height(clamped).child(view.second);
                        let items: Vec<Widget> = vec![
                            crate::Expanded::new(view.first).into(),
                            divider.into(),
                            second.into(),
                        ];
                        crate::Column::new(items).into()
                    }
                }
            }
            SplitPosition::Fraction(frac) => {
                let f1 = ((frac * 1000.0) as u32).max(1);
                let f2 = (((1.0 - frac) * 1000.0) as u32).max(1);
                match axis {
                    Axis::Horizontal => {
                        let items: Vec<Widget> = vec![
                            crate::Flexible::new(view.first).flex(f1).into(),
                            divider.into(),
                            crate::Flexible::new(view.second).flex(f2).into(),
                        ];
                        crate::Row::new(items).into()
                    }
                    Axis::Vertical => {
                        let items: Vec<Widget> = vec![
                            crate::Flexible::new(view.first).flex(f1).into(),
                            divider.into(),
                            crate::Flexible::new(view.second).flex(f2).into(),
                        ];
                        crate::Column::new(items).into()
                    }
                }
            }
        }
    }
}
