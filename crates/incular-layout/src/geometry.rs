use incular_core::{Offset, Rect, Size};

/// A measured child and its final position in a parent.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ChildLayout {
    pub offset: Offset,
    pub size: Size,
    /// Optional alphabetic baseline in the child's local coordinate space.
    pub baseline: Option<f32>,
}

impl ChildLayout {
    #[must_use]
    pub const fn new(offset: Offset, size: Size) -> Self {
        Self {
            offset,
            size,
            baseline: None,
        }
    }

    #[must_use]
    pub const fn with_baseline(mut self, baseline: f32) -> Self {
        self.baseline = Some(baseline);
        self
    }

    #[must_use]
    pub const fn with_optional_baseline(mut self, baseline: Option<f32>) -> Self {
        self.baseline = baseline;
        self
    }

    #[must_use]
    pub fn rect(self) -> Rect {
        Rect::from_origin_size(self.offset, self.size)
    }
}

/// Result returned by all pure layout algorithms in this crate.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LayoutResult {
    pub size: Size,
    pub children: Vec<ChildLayout>,
    /// `true` when the child is deliberately laid out outside the visual tree
    /// (for example an offstage descriptor).
    pub offstage: bool,
}

impl LayoutResult {
    #[must_use]
    pub fn new(size: Size, children: Vec<ChildLayout>) -> Self {
        Self {
            size,
            children,
            offstage: false,
        }
    }

    #[must_use]
    pub fn offstage(size: Size) -> Self {
        Self {
            size,
            children: Vec::new(),
            offstage: true,
        }
    }

    #[must_use]
    pub const fn empty(size: Size) -> Self {
        Self {
            size,
            children: Vec::new(),
            offstage: false,
        }
    }

    pub fn child_rects(&self) -> impl Iterator<Item = Rect> + '_ {
        self.children.iter().map(|child| child.rect())
    }
}
