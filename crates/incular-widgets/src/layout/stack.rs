//! Flutter-style stack and positioning layout descriptors.

use incular_config::{Alignment, Clip, StackFit, TextDirection};

use crate::{Widget, WidgetKind};

/// Overlays children in paint order with alignment, sizing fit, and clipping.
#[derive(Clone, Debug, PartialEq)]
pub struct Stack {
    children: Vec<Widget>,
    alignment: Alignment,
    fit: StackFit,
    clip_behavior: Clip,
    text_direction: Option<TextDirection>,
}

impl Default for Stack {
    fn default() -> Self {
        Self::new(Vec::<Widget>::new())
    }
}

impl Stack {
    /// Creates a new Stack overlay.
    #[must_use]
    pub fn new(children: impl IntoIterator<Item = impl Into<Widget>>) -> Self {
        Self {
            children: children.into_iter().map(Into::into).collect(),
            alignment: Alignment::TOP_LEFT,
            fit: StackFit::Loose,
            clip_behavior: Clip::HardEdge,
            text_direction: None,
        }
    }

    /// Creates a Stack with explicit alignment.
    #[must_use]
    pub fn aligned(
        alignment: Alignment,
        children: impl IntoIterator<Item = impl Into<Widget>>,
    ) -> Self {
        Self::new(children).alignment(alignment)
    }

    /// Sets the alignment for non-positioned children.
    #[must_use]
    pub fn alignment(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }

    /// Sets the sizing policy for non-positioned children.
    #[must_use]
    pub fn fit(mut self, fit: StackFit) -> Self {
        self.fit = fit;
        self
    }

    /// Sets clipping behavior for children overflowing stack bounds.
    #[must_use]
    pub fn clip_behavior(mut self, clip: Clip) -> Self {
        self.clip_behavior = clip;
        self
    }

    /// Sets the text direction for resolving directional alignments.
    #[must_use]
    pub fn text_direction(mut self, direction: TextDirection) -> Self {
        self.text_direction = Some(direction);
        self
    }

    #[must_use]
    pub fn get_alignment(&self) -> Alignment {
        self.alignment
    }

    #[must_use]
    pub fn get_fit(&self) -> StackFit {
        self.fit
    }

    #[must_use]
    pub fn get_clip_behavior(&self) -> Clip {
        self.clip_behavior
    }
}

impl From<Stack> for Widget {
    fn from(value: Stack) -> Self {
        Widget::from_kind(WidgetKind::Stack {
            alignment: value.alignment,
            text_direction: value.text_direction.unwrap_or(TextDirection::Ltr),
            fit: value.fit,
            clip_behavior: value.clip_behavior,
            children: value.children,
        })
    }
}

/// Positions a child inside a [`Stack`].
#[derive(Clone, Debug, PartialEq)]
pub struct Positioned {
    left: Option<f32>,
    top: Option<f32>,
    right: Option<f32>,
    bottom: Option<f32>,
    width: Option<f32>,
    height: Option<f32>,
    child: Widget,
}

impl Positioned {
    /// Creates a new unconstrained positioned child.
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            left: None,
            top: None,
            right: None,
            bottom: None,
            width: None,
            height: None,
            child: child.into(),
        }
    }

    /// Creates a positioned child filled to all four edges.
    #[must_use]
    pub fn fill(child: impl Into<Widget>) -> Self {
        Self {
            left: Some(0.0),
            top: Some(0.0),
            right: Some(0.0),
            bottom: Some(0.0),
            width: None,
            height: None,
            child: child.into(),
        }
    }

    /// Sets the left edge offset.
    #[must_use]
    pub fn left(mut self, left: f32) -> Self {
        self.left = Some(left);
        self
    }

    /// Sets the top edge offset.
    #[must_use]
    pub fn top(mut self, top: f32) -> Self {
        self.top = Some(top);
        self
    }

    /// Sets the right edge offset.
    #[must_use]
    pub fn right(mut self, right: f32) -> Self {
        self.right = Some(right);
        self
    }

    /// Sets the bottom edge offset.
    #[must_use]
    pub fn bottom(mut self, bottom: f32) -> Self {
        self.bottom = Some(bottom);
        self
    }

    /// Sets an explicit width.
    #[must_use]
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Sets an explicit height.
    #[must_use]
    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }
}

impl From<Positioned> for Widget {
    fn from(value: Positioned) -> Self {
        Widget::from_kind(WidgetKind::Positioned {
            left: value.left,
            top: value.top,
            right: value.right,
            bottom: value.bottom,
            width: value.width,
            height: value.height,
            child: Box::new(value.child),
        })
    }
}

/// A Stack that shows only one child at a time while keeping all children laid out.
#[derive(Clone, Debug, PartialEq)]
pub struct IndexedStack {
    index: usize,
    alignment: Alignment,
    fit: StackFit,
    clip_behavior: Clip,
    children: Vec<Widget>,
}

impl Default for IndexedStack {
    fn default() -> Self {
        Self::new(Vec::<Widget>::new())
    }
}

impl IndexedStack {
    /// Creates a new IndexedStack.
    #[must_use]
    pub fn new(children: impl IntoIterator<Item = impl Into<Widget>>) -> Self {
        Self {
            index: 0,
            alignment: Alignment::TOP_LEFT,
            fit: StackFit::Loose,
            clip_behavior: Clip::HardEdge,
            children: children.into_iter().map(Into::into).collect(),
        }
    }

    /// Sets the active index.
    #[must_use]
    pub fn index(mut self, index: usize) -> Self {
        self.index = index;
        self
    }

    /// Sets alignment for children.
    #[must_use]
    pub fn alignment(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }

    /// Sets the stack fit policy.
    #[must_use]
    pub fn fit(mut self, fit: StackFit) -> Self {
        self.fit = fit;
        self
    }

    /// Sets the clipping behavior.
    #[must_use]
    pub fn clip_behavior(mut self, clip: Clip) -> Self {
        self.clip_behavior = clip;
        self
    }

    #[must_use]
    pub fn get_index(&self) -> usize {
        self.index
    }
}

impl From<IndexedStack> for Widget {
    fn from(value: IndexedStack) -> Self {
        Widget::from_kind(WidgetKind::IndexedStack {
            alignment: value.alignment,
            index: value.index,
            children: value.children,
        })
    }
}
