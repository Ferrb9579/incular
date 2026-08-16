use crate::{
    Alignment, Axis, Constraints, CrossAxisAlignment, FlexFit, MainAxisAlignment, MainAxisSize,
    TextDirection, VerticalDirection, WrapAlignment, WrapCrossAlignment,
};

/// Shared options used by a pure layout pass. It intentionally has no child
/// or widget references; callers provide measured children to an algorithm.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayoutOptions {
    pub text_direction: TextDirection,
    pub vertical_direction: VerticalDirection,
}

impl Default for LayoutOptions {
    fn default() -> Self {
        Self {
            text_direction: TextDirection::Ltr,
            vertical_direction: VerticalDirection::Down,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flex {
    pub direction: Axis,
    pub main_axis_size: MainAxisSize,
    pub main_axis_alignment: MainAxisAlignment,
    pub cross_axis_alignment: CrossAxisAlignment,
    pub text_direction: TextDirection,
    pub vertical_direction: VerticalDirection,
    pub spacing: f32,
}

impl Default for Flex {
    fn default() -> Self {
        Self::new(Axis::Horizontal)
    }
}

impl Flex {
    #[must_use]
    pub const fn new(direction: Axis) -> Self {
        Self {
            direction,
            main_axis_size: MainAxisSize::Max,
            main_axis_alignment: MainAxisAlignment::Start,
            cross_axis_alignment: CrossAxisAlignment::Center,
            text_direction: TextDirection::Ltr,
            vertical_direction: VerticalDirection::Down,
            spacing: 0.0,
        }
    }

    #[must_use]
    pub const fn row() -> Self {
        Self::new(Axis::Horizontal)
    }

    #[must_use]
    pub const fn column() -> Self {
        Self::new(Axis::Vertical)
    }

    #[must_use]
    pub const fn with_main_axis_size(mut self, value: MainAxisSize) -> Self {
        self.main_axis_size = value;
        self
    }

    #[must_use]
    pub const fn with_main_axis_alignment(mut self, value: MainAxisAlignment) -> Self {
        self.main_axis_alignment = value;
        self
    }

    #[must_use]
    pub const fn with_cross_axis_alignment(mut self, value: CrossAxisAlignment) -> Self {
        self.cross_axis_alignment = value;
        self
    }

    #[must_use]
    pub const fn with_text_direction(mut self, value: TextDirection) -> Self {
        self.text_direction = value;
        self
    }

    #[must_use]
    pub const fn with_vertical_direction(mut self, value: VerticalDirection) -> Self {
        self.vertical_direction = value;
        self
    }

    #[must_use]
    pub const fn with_spacing(mut self, value: f32) -> Self {
        self.spacing = value;
        self
    }
}

/// Flutter-compatible row descriptor. It carries policy only; children are
/// passed to [`crate::layout_flex`] as measured values.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Row {
    pub flex: Flex,
}

impl Default for Row {
    fn default() -> Self {
        Self::new()
    }
}

impl Row {
    #[must_use]
    pub const fn new() -> Self {
        Self { flex: Flex::row() }
    }

    #[must_use]
    pub const fn with_flex(mut self, flex: Flex) -> Self {
        self.flex = Flex {
            direction: Axis::Horizontal,
            ..flex
        };
        self
    }

    #[must_use]
    pub const fn policy(self) -> Flex {
        self.flex
    }
}

/// Flutter-compatible column descriptor. It carries policy only; children are
/// passed to [`crate::layout_flex`] as measured values.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Column {
    pub flex: Flex,
}

impl Default for Column {
    fn default() -> Self {
        Self::new()
    }
}

impl Column {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            flex: Flex::column(),
        }
    }

    #[must_use]
    pub const fn with_flex(mut self, flex: Flex) -> Self {
        self.flex = Flex {
            direction: Axis::Vertical,
            ..flex
        };
        self
    }

    #[must_use]
    pub const fn policy(self) -> Flex {
        self.flex
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stack {
    pub alignment: Alignment,
    pub text_direction: TextDirection,
    pub fit: StackFit,
    pub clip_behavior: Clip,
}

impl Default for Stack {
    fn default() -> Self {
        Self::new()
    }
}

impl Stack {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            alignment: Alignment::TOP_LEFT,
            text_direction: TextDirection::Ltr,
            fit: StackFit::Loose,
            clip_behavior: Clip::HardEdge,
        }
    }

    #[must_use]
    pub const fn with_alignment(mut self, value: Alignment) -> Self {
        self.alignment = value;
        self
    }

    #[must_use]
    pub const fn with_fit(mut self, value: StackFit) -> Self {
        self.fit = value;
        self
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum StackFit {
    #[default]
    Loose,
    Expand,
    Passthrough,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Clip {
    None,
    #[default]
    HardEdge,
    AntiAlias,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wrap {
    pub direction: Axis,
    pub spacing: f32,
    pub run_spacing: f32,
    pub alignment: WrapAlignment,
    pub run_alignment: WrapAlignment,
    pub cross_axis_alignment: WrapCrossAlignment,
    pub text_direction: TextDirection,
    pub vertical_direction: VerticalDirection,
}

impl Default for Wrap {
    fn default() -> Self {
        Self::new(Axis::Horizontal)
    }
}

impl Wrap {
    #[must_use]
    pub const fn new(direction: Axis) -> Self {
        Self {
            direction,
            spacing: 0.0,
            run_spacing: 0.0,
            alignment: WrapAlignment::Start,
            run_alignment: WrapAlignment::Start,
            cross_axis_alignment: WrapCrossAlignment::Start,
            text_direction: TextDirection::Ltr,
            vertical_direction: VerticalDirection::Down,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Table {
    pub columns: usize,
    pub column_spacing: f32,
    pub row_spacing: f32,
    pub alignment: Alignment,
}

impl Default for Table {
    fn default() -> Self {
        Self::new(1)
    }
}

impl Table {
    #[must_use]
    pub const fn new(columns: usize) -> Self {
        Self {
            columns,
            column_spacing: 0.0,
            row_spacing: 0.0,
            alignment: Alignment::TOP_LEFT,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Align {
    pub alignment: Alignment,
    pub width_factor: Option<f32>,
    pub height_factor: Option<f32>,
}

impl Align {
    #[must_use]
    pub const fn new(alignment: Alignment) -> Self {
        Self {
            alignment,
            width_factor: None,
            height_factor: None,
        }
    }

    #[must_use]
    pub const fn with_width_factor(mut self, value: Option<f32>) -> Self {
        self.width_factor = value;
        self
    }

    #[must_use]
    pub const fn with_height_factor(mut self, value: Option<f32>) -> Self {
        self.height_factor = value;
        self
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Center {
    pub width_factor: Option<f32>,
    pub height_factor: Option<f32>,
}

impl Center {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            width_factor: None,
            height_factor: None,
        }
    }

    #[must_use]
    pub const fn alignment(self) -> Align {
        Align {
            alignment: Alignment::CENTER,
            width_factor: self.width_factor,
            height_factor: self.height_factor,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Padding {
    pub padding: crate::EdgeInsets,
}

impl Default for Padding {
    fn default() -> Self {
        Self::new(crate::EdgeInsets::ZERO)
    }
}

impl Padding {
    #[must_use]
    pub const fn new(padding: crate::EdgeInsets) -> Self {
        Self { padding }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SizedBox {
    pub width: Option<f32>,
    pub height: Option<f32>,
}

impl SizedBox {
    #[must_use]
    pub const fn new(width: Option<f32>, height: Option<f32>) -> Self {
        Self { width, height }
    }

    #[must_use]
    pub const fn from_size(size: crate::Size) -> Self {
        Self::new(Some(size.width), Some(size.height))
    }

    #[must_use]
    pub const fn square(value: f32) -> Self {
        Self::new(Some(value), Some(value))
    }

    #[must_use]
    pub const fn shrink() -> Self {
        Self::new(Some(0.0), Some(0.0))
    }

    #[must_use]
    pub const fn expand() -> Self {
        Self::new(None, None)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConstrainedBox {
    pub constraints: Constraints,
}

impl ConstrainedBox {
    #[must_use]
    pub const fn new(constraints: Constraints) -> Self {
        Self { constraints }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UnconstrainedBox {
    pub alignment: Alignment,
    pub constrained_axis: Option<Axis>,
}

impl Default for UnconstrainedBox {
    fn default() -> Self {
        Self::new()
    }
}

impl UnconstrainedBox {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            alignment: Alignment::CENTER,
            constrained_axis: None,
        }
    }

    #[must_use]
    pub const fn with_alignment(mut self, value: Alignment) -> Self {
        self.alignment = value;
        self
    }

    #[must_use]
    pub const fn with_constrained_axis(mut self, value: Option<Axis>) -> Self {
        self.constrained_axis = value;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AspectRatio {
    pub aspect_ratio: f32,
}

impl AspectRatio {
    #[must_use]
    pub const fn new(aspect_ratio: f32) -> Self {
        Self { aspect_ratio }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BaselineType {
    #[default]
    Alphabetic,
    Ideographic,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Baseline {
    pub baseline: f32,
    pub baseline_type: BaselineType,
}

impl Baseline {
    #[must_use]
    pub const fn new(baseline: f32, baseline_type: BaselineType) -> Self {
        Self {
            baseline,
            baseline_type,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FractionallySizedBox {
    pub width_factor: Option<f32>,
    pub height_factor: Option<f32>,
    pub alignment: Alignment,
}

impl Default for FractionallySizedBox {
    fn default() -> Self {
        Self::new(None, None)
    }
}

impl FractionallySizedBox {
    #[must_use]
    pub const fn new(width_factor: Option<f32>, height_factor: Option<f32>) -> Self {
        Self {
            width_factor,
            height_factor,
            alignment: Alignment::CENTER,
        }
    }

    #[must_use]
    pub const fn with_alignment(mut self, value: Alignment) -> Self {
        self.alignment = value;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Positioned {
    pub left: Option<f32>,
    pub top: Option<f32>,
    pub right: Option<f32>,
    pub bottom: Option<f32>,
    pub width: Option<f32>,
    pub height: Option<f32>,
}

impl Default for Positioned {
    fn default() -> Self {
        Self::new()
    }
}

impl Positioned {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            left: None,
            top: None,
            right: None,
            bottom: None,
            width: None,
            height: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Visibility {
    pub visible: bool,
    pub maintain_state: bool,
    pub maintain_animation: bool,
    pub maintain_size: bool,
    pub maintain_semantics: bool,
    pub maintain_interactivity: bool,
}

impl Visibility {
    #[must_use]
    pub const fn new(visible: bool) -> Self {
        Self {
            visible,
            maintain_state: false,
            maintain_animation: false,
            maintain_size: false,
            maintain_semantics: false,
            maintain_interactivity: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Offstage {
    pub offstage: bool,
}

impl Offstage {
    #[must_use]
    pub const fn new(offstage: bool) -> Self {
        Self { offstage }
    }
}

// Keep this import in the module so the public API documents that flex fit is
// part of descriptor configuration even when no child is owned here.
#[allow(dead_code)]
const _DEFAULT_FLEX_FIT: FlexFit = FlexFit::Tight;
