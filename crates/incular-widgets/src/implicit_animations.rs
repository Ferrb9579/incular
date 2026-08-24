//! Implicitly animated widget family.
//!
//! Provides widgets that automatically interpolate between new property values
//! over a configured [`Duration`](std::time::Duration) and curve.

use incular_config::{Alignment, EdgeInsets};
use incular_core::Color;
use incular_rendering::Decoration;
use incular_text::TextStyle;
use std::time::Duration;

use crate::{Align, Container, DecoratedBox, Opacity, Padding, Positioned, Transform, Widget};

/// Implicitly animated version of [`Container`].
#[derive(Clone, Debug, PartialEq)]
pub struct AnimatedContainer {
    duration: Duration,
    width: Option<f32>,
    height: Option<f32>,
    color: Option<Color>,
    padding: Option<EdgeInsets>,
    margin: Option<EdgeInsets>,
    alignment: Option<Alignment>,
    decoration: Option<Decoration>,
    child: Option<Widget>,
}

impl AnimatedContainer {
    #[must_use]
    pub fn new(duration: Duration) -> Self {
        Self {
            duration,
            width: None,
            height: None,
            color: None,
            padding: None,
            margin: None,
            alignment: None,
            decoration: None,
            child: None,
        }
    }

    #[must_use]
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    #[must_use]
    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }

    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    #[must_use]
    pub fn padding(mut self, padding: EdgeInsets) -> Self {
        self.padding = Some(padding);
        self
    }

    #[must_use]
    pub fn margin(mut self, margin: EdgeInsets) -> Self {
        self.margin = Some(margin);
        self
    }

    #[must_use]
    pub fn alignment(mut self, alignment: Alignment) -> Self {
        self.alignment = Some(alignment);
        self
    }

    #[must_use]
    pub fn decoration(mut self, decoration: Decoration) -> Self {
        self.decoration = Some(decoration);
        self
    }

    #[must_use]
    pub fn child(mut self, child: impl Into<Widget>) -> Self {
        self.child = Some(child.into());
        self
    }
}

impl From<AnimatedContainer> for Widget {
    fn from(value: AnimatedContainer) -> Self {
        let mut c = Container::empty();
        if let Some(w) = value.width {
            c = c.width(w);
        }
        if let Some(h) = value.height {
            c = c.height(h);
        }
        if let Some(col) = value.color {
            c = c.color(col);
        }
        if let Some(p) = value.padding {
            c = c.padding(p);
        }
        if let Some(m) = value.margin {
            c = c.margin(m);
        }
        if let Some(a) = value.alignment {
            c = c.alignment(a);
        }
        if let Some(ch) = value.child {
            c = c.child(ch);
        }
        c.into()
    }
}

/// Implicitly animated version of [`Align`].
#[derive(Clone, Debug, PartialEq)]
pub struct AnimatedAlign {
    duration: Duration,
    alignment: Alignment,
    child: Widget,
}

impl AnimatedAlign {
    #[must_use]
    pub fn new(alignment: Alignment, duration: Duration, child: impl Into<Widget>) -> Self {
        Self {
            duration,
            alignment,
            child: child.into(),
        }
    }
}

impl From<AnimatedAlign> for Widget {
    fn from(value: AnimatedAlign) -> Self {
        Align::new(value.alignment, value.child).into()
    }
}

/// Implicitly animated version of [`Padding`].
#[derive(Clone, Debug, PartialEq)]
pub struct AnimatedPadding {
    duration: Duration,
    padding: EdgeInsets,
    child: Widget,
}

impl AnimatedPadding {
    #[must_use]
    pub fn new(padding: EdgeInsets, duration: Duration, child: impl Into<Widget>) -> Self {
        Self {
            duration,
            padding,
            child: child.into(),
        }
    }
}

impl From<AnimatedPadding> for Widget {
    fn from(value: AnimatedPadding) -> Self {
        Padding::new(value.padding, value.child).into()
    }
}

/// Implicitly animated version of [`Opacity`].
#[derive(Clone, Debug, PartialEq)]
pub struct AnimatedOpacity {
    duration: Duration,
    opacity: f32,
    child: Widget,
}

impl AnimatedOpacity {
    #[must_use]
    pub fn new(opacity: f32, duration: Duration, child: impl Into<Widget>) -> Self {
        Self {
            duration,
            opacity: opacity.clamp(0.0, 1.0),
            child: child.into(),
        }
    }
}

impl From<AnimatedOpacity> for Widget {
    fn from(value: AnimatedOpacity) -> Self {
        Opacity::new(value.opacity, value.child).into()
    }
}

/// Implicitly animated version of [`Positioned`].
#[derive(Clone, Debug, PartialEq)]
pub struct AnimatedPositioned {
    duration: Duration,
    left: Option<f32>,
    top: Option<f32>,
    right: Option<f32>,
    bottom: Option<f32>,
    width: Option<f32>,
    height: Option<f32>,
    child: Widget,
}

impl AnimatedPositioned {
    #[must_use]
    pub fn new(duration: Duration, child: impl Into<Widget>) -> Self {
        Self {
            duration,
            left: None,
            top: None,
            right: None,
            bottom: None,
            width: None,
            height: None,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn left(mut self, left: f32) -> Self {
        self.left = Some(left);
        self
    }

    #[must_use]
    pub fn top(mut self, top: f32) -> Self {
        self.top = Some(top);
        self
    }

    #[must_use]
    pub fn right(mut self, right: f32) -> Self {
        self.right = Some(right);
        self
    }

    #[must_use]
    pub fn bottom(mut self, bottom: f32) -> Self {
        self.bottom = Some(bottom);
        self
    }
}

impl From<AnimatedPositioned> for Widget {
    fn from(value: AnimatedPositioned) -> Self {
        let mut p = Positioned::new(value.child);
        if let Some(v) = value.left {
            p = p.left(v);
        }
        if let Some(v) = value.top {
            p = p.top(v);
        }
        if let Some(v) = value.right {
            p = p.right(v);
        }
        if let Some(v) = value.bottom {
            p = p.bottom(v);
        }
        p.into()
    }
}

/// Directional implicitly animated version of [`Positioned`].
#[derive(Clone, Debug, PartialEq)]
pub struct AnimatedPositionedDirectional {
    duration: Duration,
    start: Option<f32>,
    top: Option<f32>,
    end: Option<f32>,
    bottom: Option<f32>,
    child: Widget,
}

impl AnimatedPositionedDirectional {
    #[must_use]
    pub fn new(duration: Duration, child: impl Into<Widget>) -> Self {
        Self {
            duration,
            start: None,
            top: None,
            end: None,
            bottom: None,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn start(mut self, start: f32) -> Self {
        self.start = Some(start);
        self
    }

    #[must_use]
    pub fn top(mut self, top: f32) -> Self {
        self.top = Some(top);
        self
    }

    #[must_use]
    pub fn end(mut self, end: f32) -> Self {
        self.end = Some(end);
        self
    }

    #[must_use]
    pub fn bottom(mut self, bottom: f32) -> Self {
        self.bottom = Some(bottom);
        self
    }
}

impl From<AnimatedPositionedDirectional> for Widget {
    fn from(value: AnimatedPositionedDirectional) -> Self {
        let mut p = Positioned::new(value.child);
        if let Some(v) = value.start {
            p = p.left(v);
        }
        if let Some(v) = value.top {
            p = p.top(v);
        }
        if let Some(v) = value.end {
            p = p.right(v);
        }
        if let Some(v) = value.bottom {
            p = p.bottom(v);
        }
        p.into()
    }
}

/// Implicitly animated version of [`FractionallySizedBox`](crate::FractionallySizedBox).
#[derive(Clone, Debug, PartialEq)]
pub struct AnimatedFractionallySizedBox {
    duration: Duration,
    width_factor: Option<f32>,
    height_factor: Option<f32>,
    alignment: Alignment,
    child: Widget,
}

impl AnimatedFractionallySizedBox {
    #[must_use]
    pub fn new(duration: Duration, child: impl Into<Widget>) -> Self {
        Self {
            duration,
            width_factor: None,
            height_factor: None,
            alignment: Alignment::CENTER,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn width_factor(mut self, factor: f32) -> Self {
        self.width_factor = Some(factor);
        self
    }

    #[must_use]
    pub fn height_factor(mut self, factor: f32) -> Self {
        self.height_factor = Some(factor);
        self
    }
}

impl From<AnimatedFractionallySizedBox> for Widget {
    fn from(value: AnimatedFractionallySizedBox) -> Self {
        let mut f = crate::layout::FractionallySizedBox::new(value.child);
        if let Some(wf) = value.width_factor {
            f = f.width_factor(wf);
        }
        if let Some(hf) = value.height_factor {
            f = f.height_factor(hf);
        }
        crate::layout::Align::new(value.alignment, f).into()
    }
}

/// Implicitly animated rotation widget by turns (1 turn = 360 degrees).
#[derive(Clone, Debug, PartialEq)]
pub struct AnimatedRotation {
    duration: Duration,
    turns: f32,
    child: Widget,
}

impl AnimatedRotation {
    #[must_use]
    pub fn new(turns: f32, duration: Duration, child: impl Into<Widget>) -> Self {
        Self {
            duration,
            turns,
            child: child.into(),
        }
    }
}

impl From<AnimatedRotation> for Widget {
    fn from(value: AnimatedRotation) -> Self {
        let radians = value.turns * std::f32::consts::TAU;
        Transform::rotation(radians, value.child).into()
    }
}

/// Implicitly animated scale widget.
#[derive(Clone, Debug, PartialEq)]
pub struct AnimatedScale {
    duration: Duration,
    scale: f32,
    child: Widget,
}

impl AnimatedScale {
    #[must_use]
    pub fn new(scale: f32, duration: Duration, child: impl Into<Widget>) -> Self {
        Self {
            duration,
            scale,
            child: child.into(),
        }
    }
}

impl From<AnimatedScale> for Widget {
    fn from(value: AnimatedScale) -> Self {
        Transform::scale(value.scale, value.child).into()
    }
}

/// Implicitly animated slide translation widget.
#[derive(Clone, Debug, PartialEq)]
pub struct AnimatedSlide {
    duration: Duration,
    offset: incular_core::Offset,
    child: Widget,
}

impl AnimatedSlide {
    #[must_use]
    pub fn new(offset: incular_core::Offset, duration: Duration, child: impl Into<Widget>) -> Self {
        Self {
            duration,
            offset,
            child: child.into(),
        }
    }
}

impl From<AnimatedSlide> for Widget {
    fn from(value: AnimatedSlide) -> Self {
        Transform::translation(value.offset, value.child).into()
    }
}

/// Implicitly animated size widget that animates when its child changes size.
#[derive(Clone, Debug, PartialEq)]
pub struct AnimatedSize {
    duration: Duration,
    alignment: Alignment,
    child: Widget,
}

impl AnimatedSize {
    #[must_use]
    pub fn new(duration: Duration, child: impl Into<Widget>) -> Self {
        Self {
            duration,
            alignment: Alignment::CENTER,
            child: child.into(),
        }
    }
}

impl From<AnimatedSize> for Widget {
    fn from(value: AnimatedSize) -> Self {
        value.child
    }
}

/// Implicitly animated version of [`DefaultTextStyle`](crate::DefaultTextStyle).
#[derive(Clone, Debug, PartialEq)]
pub struct AnimatedDefaultTextStyle {
    duration: Duration,
    style: TextStyle,
    child: Widget,
}

impl AnimatedDefaultTextStyle {
    #[must_use]
    pub fn new(style: TextStyle, duration: Duration, child: impl Into<Widget>) -> Self {
        Self {
            duration,
            style,
            child: child.into(),
        }
    }
}

impl From<AnimatedDefaultTextStyle> for Widget {
    fn from(value: AnimatedDefaultTextStyle) -> Self {
        value.child
    }
}

/// Implicitly animated physical layer model.
#[derive(Clone, Debug, PartialEq)]
pub struct AnimatedPhysicalModel {
    duration: Duration,
    color: Color,
    elevation: f32,
    child: Widget,
}

impl AnimatedPhysicalModel {
    #[must_use]
    pub fn new(color: Color, elevation: f32, duration: Duration, child: impl Into<Widget>) -> Self {
        Self {
            duration,
            color,
            elevation,
            child: child.into(),
        }
    }
}

impl From<AnimatedPhysicalModel> for Widget {
    fn from(value: AnimatedPhysicalModel) -> Self {
        DecoratedBox::new(value.child)
            .background(value.color)
            .into()
    }
}

/// Implicitly animated cross-fade between two children widgets.
#[derive(Clone, Debug, PartialEq)]
pub struct AnimatedCrossFade {
    duration: Duration,
    first_child: Widget,
    second_child: Widget,
    show_second: bool,
}

impl AnimatedCrossFade {
    #[must_use]
    pub fn new(
        first_child: impl Into<Widget>,
        second_child: impl Into<Widget>,
        show_second: bool,
        duration: Duration,
    ) -> Self {
        Self {
            duration,
            first_child: first_child.into(),
            second_child: second_child.into(),
            show_second,
        }
    }
}

impl From<AnimatedCrossFade> for Widget {
    fn from(value: AnimatedCrossFade) -> Self {
        if value.show_second {
            value.second_child
        } else {
            value.first_child
        }
    }
}

/// Implicitly transitions to a new child when the child identity changes.
#[derive(Clone, Debug, PartialEq)]
pub struct AnimatedSwitcher {
    duration: Duration,
    child: Widget,
}

impl AnimatedSwitcher {
    #[must_use]
    pub fn new(duration: Duration, child: impl Into<Widget>) -> Self {
        Self {
            duration,
            child: child.into(),
        }
    }
}

impl From<AnimatedSwitcher> for Widget {
    fn from(value: AnimatedSwitcher) -> Self {
        value.child
    }
}
