//! Flutter-style stack and positioning layout descriptors.

use incular_config::{Alignment, Clip, StackFit, TextDirection};
use typed_builder::TypedBuilder;

use crate::{Widget, WidgetKind};

/// Overlays children in paint order with alignment, sizing fit, and clipping.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct Stack {
    #[builder(default, setter(into))]
    children: Vec<Widget>,
    #[builder(default = Alignment::TOP_LEFT)]
    alignment: Alignment,
    #[builder(default = StackFit::Loose)]
    fit: StackFit,
    #[builder(default = Clip::HardEdge)]
    clip_behavior: Clip,
    #[builder(default, setter(strip_option))]
    text_direction: Option<TextDirection>,
}

impl Default for Stack {
    fn default() -> Self {
        Self {
            children: Vec::new(),
            alignment: Alignment::TOP_LEFT,
            fit: StackFit::Loose,
            clip_behavior: Clip::HardEdge,
            text_direction: None,
        }
    }
}

impl Stack {
    /// Creates a new Stack overlay.
    #[must_use]
    pub fn new(children: impl IntoIterator<Item = impl Into<Widget>>) -> Self {
        Self {
            children: children.into_iter().map(Into::into).collect(),
            ..Self::default()
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
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct Positioned {
    #[builder(default, setter(strip_option))]
    left: Option<f32>,
    #[builder(default, setter(strip_option))]
    top: Option<f32>,
    #[builder(default, setter(strip_option))]
    right: Option<f32>,
    #[builder(default, setter(strip_option))]
    bottom: Option<f32>,
    #[builder(default, setter(strip_option))]
    width: Option<f32>,
    #[builder(default, setter(strip_option))]
    height: Option<f32>,
    #[builder(setter(into))]
    child: Widget,
}

impl Positioned {
    /// Creates a new unconstrained positioned child.
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self::builder().child(child).build()
    }

    /// Creates a positioned child filled to all four edges.
    #[must_use]
    pub fn fill(child: impl Into<Widget>) -> Self {
        Self::builder()
            .left(0.0)
            .top(0.0)
            .right(0.0)
            .bottom(0.0)
            .child(child)
            .build()
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
            child: value.child,
        })
    }
}

/// A Stack that shows only one child at a time while keeping all children laid out.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct IndexedStack {
    #[builder(default)]
    index: usize,
    #[builder(default = Alignment::TOP_LEFT)]
    alignment: Alignment,
    #[builder(default = StackFit::Loose)]
    fit: StackFit,
    #[builder(default = Clip::HardEdge)]
    clip_behavior: Clip,
    #[builder(default, setter(into))]
    children: Vec<Widget>,
}

impl Default for IndexedStack {
    fn default() -> Self {
        Self {
            index: 0,
            alignment: Alignment::TOP_LEFT,
            fit: StackFit::Loose,
            clip_behavior: Clip::HardEdge,
            children: Vec::new(),
        }
    }
}

impl IndexedStack {
    /// Creates a new IndexedStack.
    #[must_use]
    pub fn new(children: impl IntoIterator<Item = impl Into<Widget>>) -> Self {
        Self {
            children: children.into_iter().map(Into::into).collect(),
            ..Self::default()
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
