//! Specialized utility widgets built from the basic layout primitives.

use typed_builder::TypedBuilder;

use super::Center;
use crate::Widget;

/// Wraps a child subtree with an explicit retained key.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct KeyedSubtree {
    #[builder(setter(into))]
    key: crate::tree::Key,
    #[builder(setter(into))]
    child: Widget,
}

impl KeyedSubtree {
    #[must_use]
    pub fn new(key: impl Into<crate::tree::Key>, child: impl Into<Widget>) -> Self {
        Self {
            key: key.into(),
            child: child.into(),
        }
    }
}

impl From<KeyedSubtree> for Widget {
    fn from(value: KeyedSubtree) -> Self {
        let mut child = value.child;
        child.set_key(Some(value.key));
        child
    }
}

/// A cell in a Table widget.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct TableCell {
    #[builder(default, setter(strip_option))]
    vertical_alignment: Option<incular_config::CrossAxisAlignment>,
    #[builder(setter(into))]
    child: Widget,
}

impl TableCell {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            vertical_alignment: None,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn vertical_alignment(mut self, alignment: incular_config::CrossAxisAlignment) -> Self {
        self.vertical_alignment = Some(alignment);
        self
    }
}

impl From<TableCell> for Widget {
    fn from(value: TableCell) -> Self {
        value.child
    }
}

/// Lays out children in a row with spacing; wraps or flows when width is constrained.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct OverflowBar {
    #[builder(default = 0.0)]
    spacing: f32,
    #[builder(default = 0.0)]
    overflow_spacing: f32,
    #[builder(default = incular_config::WrapCrossAlignment::Start)]
    overflow_alignment: incular_config::WrapCrossAlignment,
    #[builder(
        default = Vec::new(),
        setter(transform = |children: impl IntoIterator<Item = impl Into<Widget>>| {
            children.into_iter().map(Into::into).collect::<Vec<Widget>>()
        })
    )]
    pub(super) children: Vec<Widget>,
}

impl Default for OverflowBar {
    fn default() -> Self {
        Self::new(Vec::<Widget>::new())
    }
}

impl OverflowBar {
    #[must_use]
    pub fn new(children: impl IntoIterator<Item = impl Into<Widget>>) -> Self {
        Self {
            spacing: 0.0,
            overflow_spacing: 0.0,
            overflow_alignment: incular_config::WrapCrossAlignment::Start,
            children: children.into_iter().map(Into::into).collect(),
        }
    }

    #[must_use]
    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing;
        self
    }

    #[must_use]
    pub fn overflow_spacing(mut self, spacing: f32) -> Self {
        self.overflow_spacing = spacing;
        self
    }

    #[must_use]
    pub fn overflow_alignment(mut self, alignment: incular_config::WrapCrossAlignment) -> Self {
        self.overflow_alignment = alignment;
        self
    }
}

impl From<OverflowBar> for Widget {
    fn from(value: OverflowBar) -> Self {
        crate::layout::Wrap::new(value.children)
            .spacing(value.spacing)
            .run_spacing(value.overflow_spacing)
            .cross_axis_alignment(value.overflow_alignment)
            .into()
    }
}

/// App bar navigation toolbar with leading, middle, and trailing slots.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct NavigationToolbar {
    #[builder(default, setter(strip_option, into))]
    leading: Option<Widget>,
    #[builder(default, setter(strip_option, into))]
    middle: Option<Widget>,
    #[builder(default, setter(strip_option, into))]
    trailing: Option<Widget>,
    #[builder(default = true)]
    center_middle: bool,
    #[builder(default = 16.0)]
    middle_spacing: f32,
}

impl Default for NavigationToolbar {
    fn default() -> Self {
        Self::new()
    }
}

impl NavigationToolbar {
    #[must_use]
    pub fn new() -> Self {
        Self {
            leading: None,
            middle: None,
            trailing: None,
            center_middle: true,
            middle_spacing: 16.0,
        }
    }

    #[must_use]
    pub fn leading(mut self, widget: impl Into<Widget>) -> Self {
        self.leading = Some(widget.into());
        self
    }

    #[must_use]
    pub fn middle(mut self, widget: impl Into<Widget>) -> Self {
        self.middle = Some(widget.into());
        self
    }

    #[must_use]
    pub fn trailing(mut self, widget: impl Into<Widget>) -> Self {
        self.trailing = Some(widget.into());
        self
    }

    #[must_use]
    pub fn center_middle(mut self, center: bool) -> Self {
        self.center_middle = center;
        self
    }
}

impl From<NavigationToolbar> for Widget {
    fn from(value: NavigationToolbar) -> Self {
        let mut row_children = Vec::new();
        if let Some(leading) = value.leading {
            row_children.push(leading);
        }
        if let Some(middle) = value.middle {
            if value.center_middle {
                row_children.push(crate::layout::Expanded::new(Center::new(middle)).into());
            } else {
                row_children.push(middle);
            }
        } else if value.center_middle {
            row_children.push(crate::layout::Spacer::new().into());
        }
        if let Some(trailing) = value.trailing {
            row_children.push(trailing);
        }
        crate::layout::Row::new(row_children)
            .main_axis_alignment(incular_config::MainAxisAlignment::SpaceBetween)
            .cross_axis_alignment(incular_config::CrossAxisAlignment::Center)
            .into()
    }
}

/// An interactive viewer enabling panning and zooming on a child widget.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct InteractiveViewer {
    #[builder(default = 0.8)]
    min_scale: f32,
    #[builder(default = 2.5)]
    max_scale: f32,
    #[builder(default = true)]
    pan_enabled: bool,
    #[builder(default = true)]
    scale_enabled: bool,
    #[builder(setter(into))]
    child: Widget,
}

impl InteractiveViewer {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            min_scale: 0.8,
            max_scale: 2.5,
            pan_enabled: true,
            scale_enabled: true,
            child: child.into(),
        }
    }

    #[must_use]
    pub fn min_scale(mut self, scale: f32) -> Self {
        self.min_scale = scale.max(0.1);
        self
    }

    #[must_use]
    pub fn max_scale(mut self, scale: f32) -> Self {
        self.max_scale = scale.max(self.min_scale);
        self
    }

    #[must_use]
    pub fn pan_enabled(mut self, enabled: bool) -> Self {
        self.pan_enabled = enabled;
        self
    }

    #[must_use]
    pub fn scale_enabled(mut self, enabled: bool) -> Self {
        self.scale_enabled = enabled;
        self
    }
}

impl From<InteractiveViewer> for Widget {
    fn from(value: InteractiveViewer) -> Self {
        value.child
    }
}
