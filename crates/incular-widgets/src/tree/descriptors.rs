//! Concrete retained-widget descriptors.
//!
//! Public descriptor types lower into the opaque `Widget` transport. Mutable mounted state continues to live exclusively in `WidgetTree`.

use super::*;

/// Renderer-neutral image sizing policy (maps to Flutter's `BoxFit`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ImageFit {
    Fill,
    #[default]
    Contain,
    Cover,
    FitWidth,
    FitHeight,
    None,
    ScaleDown,
}

/// Alias for [`ImageFit`], matching Flutter naming.
pub type BoxFit = ImageFit;

/// Repetition policy for raster image painting inside its allocated bounds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ImageRepeat {
    #[default]
    NoRepeat,
    RepeatX,
    RepeatY,
    Repeat,
}
/// A declarative raster image. It retains only a shared asset handle.
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    image: ImageHandle,
    width: Option<f32>,
    height: Option<f32>,
    fit: ImageFit,
    repeat: ImageRepeat,
    alignment: Alignment,
    sampling: ImageSampling,
}
impl Image {
    #[must_use]
    pub fn new(image: ImageHandle) -> Self {
        Self {
            image,
            width: None,
            height: None,
            fit: ImageFit::Contain,
            repeat: ImageRepeat::NoRepeat,
            alignment: Alignment::CENTER,
            sampling: ImageSampling::Linear,
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
    pub fn fit(mut self, fit: ImageFit) -> Self {
        self.fit = fit;
        self
    }
    #[must_use]
    pub fn repeat(mut self, repeat: ImageRepeat) -> Self {
        self.repeat = repeat;
        self
    }
    #[must_use]
    pub fn alignment(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }
    #[must_use]
    pub fn sampling(mut self, sampling: ImageSampling) -> Self {
        self.sampling = sampling;
        self
    }

    #[must_use]
    pub fn filter_quality(self, quality: FilterQuality) -> Self {
        self.sampling(quality.into())
    }
}
impl From<Image> for Widget {
    fn from(value: Image) -> Self {
        Widget::image(
            value.image,
            value.width,
            value.height,
            value.fit,
            value.repeat,
            value.alignment,
            value.sampling,
        )
    }
}

/// Immutable declarative vector shape. Its coordinates remain local; normal
/// layout and compositor translation position it without changing PathId.
#[derive(Clone, Debug, PartialEq)]
pub struct PathView {
    path: Arc<Path>,
    fill: Option<Brush>,
    stroke: Option<(Brush, Stroke)>,
    size: Option<Size>,
}
impl PathView {
    #[must_use]
    pub fn new(path: impl Into<Arc<Path>>) -> Self {
        Self {
            path: path.into(),
            fill: None,
            stroke: None,
            size: None,
        }
    }
    #[must_use]
    pub fn fill(mut self, brush: impl Into<Brush>) -> Self {
        self.fill = Some(brush.into());
        self
    }
    #[must_use]
    pub fn stroke(mut self, brush: impl Into<Brush>, stroke: Stroke) -> Self {
        self.stroke = Some((brush.into(), stroke));
        self
    }
    #[must_use]
    pub fn size(mut self, size: Size) -> Self {
        self.size = Some(size);
        self
    }
}
impl From<PathView> for Widget {
    fn from(value: PathView) -> Self {
        Widget::shape(value.path, value.fill, value.stroke, value.size)
    }
}

/// Reusable vector icon backed by the same retained Path renderer as PathView.
#[derive(Clone, Debug, PartialEq)]
pub struct Icon {
    path: Arc<Path>,
    size: f32,
    brush: Brush,
}
impl Icon {
    #[must_use]
    pub fn new(path: impl Into<Arc<Path>>) -> Self {
        Self {
            path: path.into(),
            size: 24.,
            brush: Color::WHITE.into(),
        }
    }
    #[must_use]
    pub fn size(mut self, size: f32) -> Self {
        self.size = size.max(0.);
        self
    }
    #[must_use]
    pub fn brush(mut self, brush: impl Into<Brush>) -> Self {
        self.brush = brush.into();
        self
    }
}
impl From<Icon> for Widget {
    fn from(value: Icon) -> Self {
        // Icon paths use a canonical coordinate system (the built-in paths
        // are authored around a 24px viewport).  A PathView's `size` controls
        // layout only, so fit the geometry itself as well; otherwise a 12px
        // check would still paint at coordinates 3..21 and appear offset or
        // clipped inside its indicator.
        let path = value
            .path
            .bounds()
            .filter(|bounds| bounds.size.width > 0. && bounds.size.height > 0.)
            .map_or_else(
                || value.path.clone(),
                |bounds| {
                    let scale =
                        (value.size / bounds.size.width).min(value.size / bounds.size.height);
                    let fitted = Size::new(bounds.size.width * scale, bounds.size.height * scale);
                    let offset = Offset::new(
                        (value.size - fitted.width) * 0.5 - bounds.origin.x * scale,
                        (value.size - fitted.height) * 0.5 - bounds.origin.y * scale,
                    );
                    Arc::new(
                        value.path.transformed(
                            CoreTransform::translation(offset)
                                .then(CoreTransform::scale_non_uniform(scale, scale)),
                        ),
                    )
                },
            );
        PathView::new(path)
            .fill(value.brush)
            .size(Size::new(value.size, value.size))
            .into()
    }
}

/// Small shared demo icons. Each function returns the same immutable `Path`,
/// so repeated icons share PathId and retained tessellation/GPU meshes.
pub mod icons {
    use super::*;
    use std::sync::OnceLock;
    fn path(build: impl FnOnce(&mut incular_painting::PathBuilder)) -> Arc<Path> {
        let mut builder = Path::builder();
        build(&mut builder);
        Arc::new(builder.build())
    }
    #[must_use]
    pub fn check() -> Arc<Path> {
        static PATH: OnceLock<Arc<Path>> = OnceLock::new();
        PATH.get_or_init(|| {
            path(|p| {
                p.move_to(Offset::new(3., 12.))
                    .line_to(Offset::new(9., 18.))
                    .line_to(Offset::new(21., 4.))
                    .line_to(Offset::new(18., 2.))
                    .line_to(Offset::new(9., 14.))
                    .line_to(Offset::new(5., 10.))
                    .close();
            })
        })
        .clone()
    }
    #[must_use]
    pub fn close() -> Arc<Path> {
        static PATH: OnceLock<Arc<Path>> = OnceLock::new();
        PATH.get_or_init(|| {
            path(|p| {
                p.move_to(Offset::new(3., 5.))
                    .line_to(Offset::new(5., 3.))
                    .line_to(Offset::new(12., 10.))
                    .line_to(Offset::new(19., 3.))
                    .line_to(Offset::new(21., 5.))
                    .line_to(Offset::new(14., 12.))
                    .line_to(Offset::new(21., 19.))
                    .line_to(Offset::new(19., 21.))
                    .line_to(Offset::new(12., 14.))
                    .line_to(Offset::new(5., 21.))
                    .line_to(Offset::new(3., 19.))
                    .line_to(Offset::new(10., 12.))
                    .close();
            })
        })
        .clone()
    }
    #[must_use]
    pub fn plus() -> Arc<Path> {
        static PATH: OnceLock<Arc<Path>> = OnceLock::new();
        PATH.get_or_init(|| {
            path(|p| {
                p.move_to(Offset::new(10., 3.))
                    .line_to(Offset::new(14., 3.))
                    .line_to(Offset::new(14., 10.))
                    .line_to(Offset::new(21., 10.))
                    .line_to(Offset::new(21., 14.))
                    .line_to(Offset::new(14., 14.))
                    .line_to(Offset::new(14., 21.))
                    .line_to(Offset::new(10., 21.))
                    .line_to(Offset::new(10., 14.))
                    .line_to(Offset::new(3., 14.))
                    .line_to(Offset::new(3., 10.))
                    .line_to(Offset::new(10., 10.))
                    .close();
            })
        })
        .clone()
    }
    #[must_use]
    pub fn minus() -> Arc<Path> {
        static PATH: OnceLock<Arc<Path>> = OnceLock::new();
        PATH.get_or_init(|| {
            path(|p| {
                p.move_to(Offset::new(3., 10.))
                    .line_to(Offset::new(21., 10.))
                    .line_to(Offset::new(21., 14.))
                    .line_to(Offset::new(3., 14.))
                    .close();
            })
        })
        .clone()
    }
    #[must_use]
    pub fn chevron_right() -> Arc<Path> {
        static PATH: OnceLock<Arc<Path>> = OnceLock::new();
        PATH.get_or_init(|| {
            path(|p| {
                p.move_to(Offset::new(7., 3.))
                    .line_to(Offset::new(10., 0.))
                    .line_to(Offset::new(22., 12.))
                    .line_to(Offset::new(10., 24.))
                    .line_to(Offset::new(7., 21.))
                    .line_to(Offset::new(16., 12.))
                    .close();
            })
        })
        .clone()
    }

    /// A compact downward chevron used by select, disclosure, and menu
    /// controls.  Keeping each direction as a shared immutable path avoids
    /// per-control path construction and makes the icon independent of font
    /// fallback or glyph metrics.
    #[must_use]
    pub fn chevron_down() -> Arc<Path> {
        static PATH: OnceLock<Arc<Path>> = OnceLock::new();
        PATH.get_or_init(|| {
            path(|p| {
                p.move_to(Offset::new(3., 7.))
                    .line_to(Offset::new(6., 4.))
                    .line_to(Offset::new(12., 10.))
                    .line_to(Offset::new(18., 4.))
                    .line_to(Offset::new(21., 7.))
                    .line_to(Offset::new(12., 16.))
                    .close();
            })
        })
        .clone()
    }

    /// A compact upward chevron.
    #[must_use]
    pub fn chevron_up() -> Arc<Path> {
        static PATH: OnceLock<Arc<Path>> = OnceLock::new();
        PATH.get_or_init(|| {
            path(|p| {
                p.move_to(Offset::new(3., 17.))
                    .line_to(Offset::new(6., 20.))
                    .line_to(Offset::new(12., 14.))
                    .line_to(Offset::new(18., 20.))
                    .line_to(Offset::new(21., 17.))
                    .line_to(Offset::new(12., 8.))
                    .close();
            })
        })
        .clone()
    }

    /// A compact left-pointing chevron.
    #[must_use]
    pub fn chevron_left() -> Arc<Path> {
        static PATH: OnceLock<Arc<Path>> = OnceLock::new();
        PATH.get_or_init(|| {
            path(|p| {
                p.move_to(Offset::new(17., 3.))
                    .line_to(Offset::new(20., 6.))
                    .line_to(Offset::new(14., 12.))
                    .line_to(Offset::new(20., 18.))
                    .line_to(Offset::new(17., 21.))
                    .line_to(Offset::new(8., 12.))
                    .close();
            })
        })
        .clone()
    }
}

/// A small background/border wrapper. It intentionally does not clip children;
/// true rounded clipping belongs to Phase 9.1C.
#[derive(Clone, Debug, PartialEq)]
pub struct DecoratedBox {
    child: Widget,
    size: Option<Size>,
    background: Option<Brush>,
    border: Option<Border>,
    radius: CornerRadii,
}
impl DecoratedBox {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            size: None,
            background: None,
            border: None,
            radius: CornerRadii::default(),
        }
    }
    #[must_use]
    pub fn size(mut self, size: Size) -> Self {
        self.size = Some(size);
        self
    }
    #[must_use]
    pub fn background(mut self, brush: impl Into<Brush>) -> Self {
        self.background = Some(brush.into());
        self
    }
    #[must_use]
    pub fn border(mut self, border: impl Into<Border>) -> Self {
        self.border = Some(border.into());
        self
    }
    #[must_use]
    pub fn radius(mut self, radius: f32) -> Self {
        self.radius = CornerRadii::uniform(radius);
        self
    }
    #[must_use]
    pub fn decoration(mut self, decoration: crate::BoxDecoration) -> Self {
        if let Some(color) = decoration.color {
            self.background = Some(Brush::Solid(color));
        }
        if let Some(border) = decoration.border {
            self.border = Some(incular_rendering::Border::new(
                border.top.width,
                border.top.color,
            ));
        }
        if let Some(radius) = decoration.border_radius {
            self.radius = radius.to_corner_radii();
        }
        self
    }
}
impl From<DecoratedBox> for Widget {
    fn from(value: DecoratedBox) -> Self {
        Widget::decorated(
            value.size,
            value.background,
            value.border,
            value.radius,
            value.child,
        )
    }
}

/// Public text description. Its font metrics are resolved during layout, not
/// paint.  A plain [`Text::new`] uses [`TextStyle::default`] (16 logical px,
/// system UI family, normal weight) and reports its natural shaped size; it
/// does not expand to the parent width unless a parent layout allocates that
/// width explicitly.
#[derive(Clone, Debug, PartialEq)]
pub struct Text {
    text: String,
    style: TextStyle,
    align: TextAlign,
    soft_wrap: bool,
    max_lines: Option<usize>,
    overflow: TextOverflow,
}
impl Text {
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style: TextStyle::default(),
            align: TextAlign::Start,
            soft_wrap: true,
            max_lines: None,
            overflow: TextOverflow::Clip,
        }
    }
    #[must_use]
    pub fn style(mut self, style: TextStyle) -> Self {
        self.style = style;
        self
    }
    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.style.color = color;
        self
    }
    #[must_use]
    pub fn align(mut self, align: TextAlign) -> Self {
        self.align = align;
        self
    }
    /// Enables or disables Parley's soft line breaking at the allocated width.
    #[must_use]
    pub fn soft_wrap(mut self, soft_wrap: bool) -> Self {
        self.soft_wrap = soft_wrap;
        self
    }
    #[must_use]
    pub fn max_lines(mut self, max_lines: Option<usize>) -> Self {
        self.max_lines = max_lines;
        self
    }
    #[must_use]
    pub fn overflow(mut self, overflow: TextOverflow) -> Self {
        self.overflow = overflow;
        self
    }
}
impl From<Text> for Widget {
    fn from(value: Text) -> Self {
        Widget::text_configured(
            value.text,
            value.style,
            value.align,
            value.soft_wrap,
            value.max_lines,
            value.overflow,
        )
    }
}

/// Makes the renderer-independent rich paragraph description available in a
/// retained widget tree. Paragraph policy is preserved; inline style flattening
/// follows the existing `incular-text::RichText` contract.
impl From<RichText> for Widget {
    fn from(value: RichText) -> Self {
        let style = value
            .flatten()
            .first()
            .map_or_else(TextStyle::default, |run| run.style.clone());
        Widget::text_configured(
            value.plain_text(),
            style,
            value.text_align,
            value.soft_wrap,
            value.max_lines,
            value.overflow,
        )
    }
}

/// Core editable text widget.
///
/// This is Incular's renderer-neutral counterpart to Flutter's
/// `widgets/EditableText`.  Material `TextField` and multiline field chrome
/// belong to a higher-level component library. Keep the controller outside a
/// rebuilt widget description so text, selection, and active composition
/// survive rebuilds.
pub struct EditableText {
    controller: TextEditingController,
    size: Size,
    style: TextStyle,
    placeholder: String,
    on_submit: Option<Rc<dyn Fn(String)>>,
    multiline: bool,
    min_lines: Option<usize>,
    max_lines: Option<usize>,
    expands: bool,
    text_align: TextAlign,
    enabled: bool,
    read_only: bool,
    obscure_text: bool,
    cursor_width: f32,
    cursor_height: Option<f32>,
    cursor_radius: f32,
    show_cursor: bool,
    cursor_color: Color,
    selection_color: Color,
    input_type: TextInputTypeHint,
    input_action: TextInputActionHint,
}
impl EditableText {
    #[must_use]
    pub fn new(controller: TextEditingController) -> Self {
        Self {
            controller,
            size: Size::ZERO,
            style: TextStyle::default(),
            placeholder: String::new(),
            on_submit: None,
            multiline: false,
            min_lines: None,
            max_lines: Some(1),
            expands: false,
            text_align: TextAlign::Start,
            enabled: true,
            read_only: false,
            obscure_text: false,
            cursor_width: 1.0,
            cursor_height: None,
            cursor_radius: 0.0,
            show_cursor: true,
            cursor_color: Color::WHITE,
            selection_color: Color::rgba(72, 120, 220, 150),
            input_type: TextInputTypeHint::Text,
            input_action: TextInputActionHint::Unspecified,
        }
    }
    #[must_use]
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
    #[must_use]
    pub fn style(mut self, style: TextStyle) -> Self {
        self.style = style;
        self
    }
    #[must_use]
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }
    #[must_use]
    pub fn on_submit(mut self, callback: impl Fn(String) + 'static) -> Self {
        self.on_submit = Some(Rc::new(callback));
        self
    }
    /// Enables multiline editing. Material text-field wrappers should use
    /// this core primitive rather than introducing a separate `TextArea`
    /// widget name.
    #[must_use]
    pub fn multiline(mut self, multiline: bool) -> Self {
        self.multiline = multiline;
        if multiline && self.max_lines == Some(1) {
            self.max_lines = None;
        } else if !multiline {
            self.max_lines = Some(1);
        }
        self
    }
    /// Sets an explicit multiline height while preserving the current width.
    #[must_use]
    pub fn height(mut self, height: f32) -> Self {
        self.size = Size::new(self.size.width, height);
        self
    }

    #[must_use]
    pub fn min_lines(mut self, lines: Option<usize>) -> Self {
        self.min_lines = lines.map(|value| value.max(1));
        self
    }

    #[must_use]
    pub fn max_lines(mut self, lines: Option<usize>) -> Self {
        self.max_lines = lines.map(|value| value.max(1));
        self.multiline = !matches!(self.max_lines, Some(1));
        self
    }

    #[must_use]
    pub fn expands(mut self, expands: bool) -> Self {
        self.expands = expands;
        self
    }

    #[must_use]
    pub fn text_align(mut self, align: TextAlign) -> Self {
        self.text_align = align;
        self
    }

    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    #[must_use]
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    #[must_use]
    pub fn obscure_text(mut self, obscure_text: bool) -> Self {
        self.obscure_text = obscure_text;
        self
    }

    /// Sets the caret width in logical pixels.
    #[must_use]
    pub fn cursor_width(mut self, width: f32) -> Self {
        self.cursor_width = width.max(0.0);
        self
    }

    /// Sets an optional caret height. `None` uses the shaped line height.
    #[must_use]
    pub fn cursor_height(mut self, height: Option<f32>) -> Self {
        self.cursor_height = height.map(|value| value.max(0.0));
        self
    }

    /// Sets the caret corner radius. A zero radius keeps the caret rectangular.
    #[must_use]
    pub fn cursor_radius(mut self, radius: f32) -> Self {
        self.cursor_radius = radius.max(0.0);
        self
    }

    /// Controls whether the caret is painted while this editor is focused.
    #[must_use]
    pub fn show_cursor(mut self, show: bool) -> Self {
        self.show_cursor = show;
        self
    }

    /// Sets the caret paint color.
    #[must_use]
    pub fn cursor_color(mut self, color: Color) -> Self {
        self.cursor_color = color;
        self
    }

    /// Sets the selection highlight paint color.
    #[must_use]
    pub fn selection_color(mut self, color: Color) -> Self {
        self.selection_color = color;
        self
    }

    /// Selects the native keyboard/input method hint for this editor. The
    /// runtime still validates and owns the committed text value.
    #[must_use]
    pub fn input_type(mut self, input_type: TextInputTypeHint) -> Self {
        self.input_type = input_type;
        self
    }

    /// Selects the action shown by a native software keyboard.
    #[must_use]
    pub fn input_action(mut self, input_action: TextInputActionHint) -> Self {
        self.input_action = input_action;
        self
    }
}
impl From<EditableText> for Widget {
    fn from(value: EditableText) -> Self {
        Widget::editable_text_configured_with_cursor(
            value.controller,
            value.size,
            value.style,
            value.placeholder,
            value.on_submit,
            value.multiline,
            value.min_lines,
            value.max_lines,
            value.expands,
            value.text_align,
            value.enabled,
            value.read_only,
            value.obscure_text,
            value.cursor_width,
            value.cursor_height,
            value.cursor_radius,
            value.show_cursor,
            value.cursor_color,
            value.selection_color,
        )
        .with_text_input_hints(value.input_type, value.input_action)
    }
}

/// Declarative retained group opacity. The child is painted into an isolated
/// compositor target when alpha is between zero and one, so overlapping
/// descendants are attenuated exactly once.
#[derive(Clone, Debug, PartialEq)]
pub struct Opacity {
    alpha: f32,
    controller: Option<OpacityController>,
    child: Widget,
}
impl Opacity {
    #[must_use]
    pub fn new(alpha: f32, child: impl Into<Widget>) -> Self {
        Self {
            alpha: normalize_opacity(alpha),
            controller: None,
            child: child.into(),
        }
    }
    #[must_use]
    pub fn controlled(controller: OpacityController, child: impl Into<Widget>) -> Self {
        Self {
            alpha: controller.opacity(),
            controller: Some(controller),
            child: child.into(),
        }
    }
    #[must_use]
    pub fn controller(mut self, controller: OpacityController) -> Self {
        self.alpha = controller.opacity();
        self.controller = Some(controller);
        self
    }
    #[must_use]
    pub fn alpha(mut self, alpha: f32) -> Self {
        self.alpha = normalize_opacity(alpha);
        self
    }
}
impl From<Opacity> for Widget {
    fn from(value: Opacity) -> Self {
        match value.controller {
            Some(controller) => Widget::controlled_opacity(controller, value.child),
            None => Widget::opacity(value.alpha, value.child),
        }
    }
}

/// A retained opacity transition driven directly by an [`OpacityController`].
#[derive(Clone, Debug, PartialEq)]
pub struct FadeTransition {
    controller: OpacityController,
    child: Widget,
}
impl FadeTransition {
    #[must_use]
    pub fn new(controller: OpacityController, child: impl Into<Widget>) -> Self {
        Self {
            controller,
            child: child.into(),
        }
    }
}
impl From<FadeTransition> for Widget {
    fn from(value: FadeTransition) -> Self {
        Widget::controlled_opacity(value.controller, value.child)
    }
}

/// A retained translation transition driven directly by a
/// [`TranslationController`].
#[derive(Clone, Debug, PartialEq)]
pub struct SlideTransition {
    controller: TranslationController,
    child: Widget,
}
impl SlideTransition {
    #[must_use]
    pub fn new(controller: TranslationController, child: impl Into<Widget>) -> Self {
        Self {
            controller,
            child: child.into(),
        }
    }
}
impl From<SlideTransition> for Widget {
    fn from(value: SlideTransition) -> Self {
        Widget::translate(value.controller, value.child)
    }
}

/// An arbitrary retained affine transform. Layout remains the child's normal
/// layout; only the compositor, pointer coordinate conversion, and semantic
/// bounds observe the affine transform.
#[derive(Clone, Debug, PartialEq)]
pub struct Transform {
    transform: CoreTransform,
    origin: Option<Offset>,
    child: Widget,
}
impl Transform {
    #[must_use]
    pub fn new(transform: CoreTransform, child: impl Into<Widget>) -> Self {
        Self {
            transform,
            origin: None,
            child: child.into(),
        }
    }
    #[must_use]
    pub fn translation(offset: Offset, child: impl Into<Widget>) -> Self {
        Self::new(CoreTransform::translation(offset), child)
    }
    #[must_use]
    pub fn scale(scale: f32, child: impl Into<Widget>) -> Self {
        Self::new(CoreTransform::scale(scale), child)
    }
    #[must_use]
    pub fn rotation(radians: f32, child: impl Into<Widget>) -> Self {
        Self::new(CoreTransform::rotation(radians), child)
    }
    #[must_use]
    pub fn skew(x: f32, y: f32, child: impl Into<Widget>) -> Self {
        Self::new(CoreTransform::skew(x, y), child)
    }
    /// Selects the local pivot. The default is the child's center.
    #[must_use]
    pub fn origin(mut self, origin: Offset) -> Self {
        self.origin = Some(finite_offset(origin));
        self
    }
}
impl From<Transform> for Widget {
    fn from(value: Transform) -> Self {
        match value.origin {
            Some(origin) => Widget::transform_around(value.transform, origin, value.child),
            None => Widget::transform(value.transform, value.child),
        }
    }
}

/// A compositor-only scale transition driven by [`ScaleController`].
#[derive(Clone, Debug, PartialEq)]
pub struct ScaleTransition {
    controller: ScaleController,
    child: Widget,
}
impl ScaleTransition {
    #[must_use]
    pub fn new(controller: ScaleController, child: impl Into<Widget>) -> Self {
        Self {
            controller,
            child: child.into(),
        }
    }
}
impl From<ScaleTransition> for Widget {
    fn from(value: ScaleTransition) -> Self {
        Widget::controlled_scale(value.controller, value.child)
    }
}

/// A compositor-only clockwise rotation transition driven by
/// [`RotationController`].
#[derive(Clone, Debug, PartialEq)]
pub struct RotationTransition {
    controller: RotationController,
    alignment: Alignment,
    child: Widget,
}
impl RotationTransition {
    #[must_use]
    pub fn new(controller: RotationController, child: impl Into<Widget>) -> Self {
        Self {
            controller,
            alignment: Alignment::CENTER,
            child: child.into(),
        }
    }

    /// Creates a rotation from a normalized turns value without exposing the
    /// retained controller implementation. One turn is a full revolution.
    #[must_use]
    pub fn from_turns(turns: f32, child: impl Into<Widget>) -> Self {
        let controller = RotationController::new();
        controller.set_radians(if turns.is_finite() {
            turns * std::f32::consts::TAU
        } else {
            0.
        });
        Self::new(controller, child)
    }

    /// Selects the normalized pivot for the compositor transform. The
    /// default is the child's center, matching Flutter's transition.
    #[must_use]
    pub fn alignment(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }
}
impl From<RotationTransition> for Widget {
    fn from(value: RotationTransition) -> Self {
        // Resolve alignment against the retained child size at compositor
        // update time; a turns change therefore does not relayout or repaint
        // the child subtree.
        Widget::controlled_rotation_with_alignment(
            value.controller,
            Some(value.alignment),
            value.child,
        )
    }
}

/// A compact composition surface for the retained transition primitives.
/// It intentionally merges fade and slide behavior instead of creating a
/// large hierarchy of narrowly different animation widget types.
#[derive(Clone, Debug, PartialEq)]
pub struct Transition {
    opacity: Option<OpacityController>,
    translation: Option<TranslationController>,
    child: Widget,
}
impl Transition {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            opacity: None,
            translation: None,
            child: child.into(),
        }
    }
    #[must_use]
    pub fn fade(mut self, controller: OpacityController) -> Self {
        self.opacity = Some(controller);
        self
    }
    #[must_use]
    pub fn slide(mut self, controller: TranslationController) -> Self {
        self.translation = Some(controller);
        self
    }
}
impl From<Transition> for Widget {
    fn from(value: Transition) -> Self {
        let child = match value.translation {
            Some(controller) => Widget::translate(controller, value.child),
            None => value.child,
        };
        match value.opacity {
            Some(controller) => Widget::controlled_opacity(controller, child),
            None => child,
        }
    }
}

/// Declarative Gaussian blur isolation. Sigma is in logical pixels and is
/// converted to physical pixels by the renderer at the current DPI.
#[derive(Clone, Debug, PartialEq)]
pub struct Blur {
    sigma_x: f32,
    sigma_y: f32,
    controller: Option<BlurController>,
    child: Widget,
}
impl Blur {
    #[must_use]
    pub fn new(sigma: f32, child: impl Into<Widget>) -> Self {
        Self {
            sigma_x: normalize_sigma(sigma),
            sigma_y: normalize_sigma(sigma),
            controller: None,
            child: child.into(),
        }
    }
    #[must_use]
    pub fn asymmetric(sigma_x: f32, sigma_y: f32, child: impl Into<Widget>) -> Self {
        Self {
            sigma_x: normalize_sigma(sigma_x),
            sigma_y: normalize_sigma(sigma_y),
            controller: None,
            child: child.into(),
        }
    }
    #[must_use]
    pub fn controlled(controller: BlurController, child: impl Into<Widget>) -> Self {
        Self {
            sigma_x: controller.sigma(),
            sigma_y: controller.sigma(),
            controller: Some(controller),
            child: child.into(),
        }
    }
    #[must_use]
    pub fn sigma_x(mut self, sigma: f32) -> Self {
        self.sigma_x = normalize_sigma(sigma);
        self.controller = None;
        self
    }
    #[must_use]
    pub fn sigma_y(mut self, sigma: f32) -> Self {
        self.sigma_y = normalize_sigma(sigma);
        self.controller = None;
        self
    }
}
impl From<Blur> for Widget {
    fn from(value: Blur) -> Self {
        match value.controller {
            Some(controller) => Widget::controlled_blur(controller, value.child),
            None => Widget::asymmetric_blur(value.sigma_x, value.sigma_y, value.child),
        }
    }
}

/// Declarative arbitrary-subtree drop shadow. The source subtree is isolated
/// and its alpha is blurred, so text, images, gradients, and paths all share
/// the same shadow semantics.
#[derive(Clone, Debug, PartialEq)]
pub struct DropShadow {
    offset: Offset,
    sigma_x: f32,
    sigma_y: f32,
    color: Color,
    controller: Option<DropShadowController>,
    child: Widget,
}
impl DropShadow {
    #[must_use]
    pub fn new(offset: Offset, sigma: f32, color: Color, child: impl Into<Widget>) -> Self {
        Self {
            offset: finite_offset(offset),
            sigma_x: normalize_sigma(sigma),
            sigma_y: normalize_sigma(sigma),
            color,
            controller: None,
            child: child.into(),
        }
    }
    #[must_use]
    pub fn asymmetric(
        offset: Offset,
        sigma_x: f32,
        sigma_y: f32,
        color: Color,
        child: impl Into<Widget>,
    ) -> Self {
        Self {
            offset: finite_offset(offset),
            sigma_x: normalize_sigma(sigma_x),
            sigma_y: normalize_sigma(sigma_y),
            color,
            controller: None,
            child: child.into(),
        }
    }
    #[must_use]
    pub fn controlled(controller: DropShadowController, child: impl Into<Widget>) -> Self {
        Self {
            offset: controller.offset(),
            sigma_x: controller.sigma(),
            sigma_y: controller.sigma(),
            color: controller.color(),
            controller: Some(controller),
            child: child.into(),
        }
    }
    #[must_use]
    pub fn offset(mut self, offset: Offset) -> Self {
        self.offset = finite_offset(offset);
        self.controller = None;
        self
    }
    #[must_use]
    pub fn sigma(mut self, sigma: f32) -> Self {
        self.sigma_x = normalize_sigma(sigma);
        self.sigma_y = normalize_sigma(sigma);
        self.controller = None;
        self
    }
    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self.controller = None;
        self
    }
}
impl From<DropShadow> for Widget {
    fn from(value: DropShadow) -> Self {
        match value.controller {
            Some(controller) => Widget::controlled_drop_shadow(controller, value.child),
            None => Widget::drop_shadow(value.offset, value.sigma_x, value.color, value.child),
        }
    }
}

/// Declarative 4x5 color-matrix stage. The matrix is evaluated in straight
/// RGBA and converted back to the retained premultiplied texture format.
#[derive(Clone, Debug, PartialEq)]
pub struct ColorFiltered {
    filter: ColorFilter,
    controller: Option<ColorFilterController>,
    child: Widget,
}
impl ColorFiltered {
    #[must_use]
    pub fn new(filter: ColorFilter, child: impl Into<Widget>) -> Self {
        Self {
            filter,
            controller: None,
            child: child.into(),
        }
    }
    #[must_use]
    pub fn controlled(controller: ColorFilterController, child: impl Into<Widget>) -> Self {
        Self {
            filter: controller.filter(),
            controller: Some(controller),
            child: child.into(),
        }
    }
    #[must_use]
    pub fn matrix(mut self, matrix: [f32; 20]) -> Self {
        self.filter = ColorFilter::matrix(matrix);
        self.controller = None;
        self
    }
    #[must_use]
    pub fn filter(mut self, filter: ColorFilter) -> Self {
        self.filter = filter;
        self.controller = None;
        self
    }
    #[must_use]
    pub fn controller(mut self, controller: ColorFilterController) -> Self {
        self.filter = controller.filter();
        self.controller = Some(controller);
        self
    }
}
impl From<ColorFiltered> for Widget {
    fn from(value: ColorFiltered) -> Self {
        match value.controller {
            Some(controller) => Widget::controlled_color_filtered(controller, value.child),
            None => Widget::color_filtered(value.filter, value.child),
        }
    }
}

/// Declarative retained blend group. Destination-dependent modes promote only
/// the required composition scope to a sampleable intermediate target.
#[derive(Clone, Debug, PartialEq)]
pub struct Blend {
    mode: BlendMode,
    child: Widget,
}
impl Blend {
    #[must_use]
    pub fn new(mode: BlendMode, child: impl Into<Widget>) -> Self {
        Self {
            mode,
            child: child.into(),
        }
    }
    #[must_use]
    pub fn mode(mut self, mode: BlendMode) -> Self {
        self.mode = mode;
        self
    }
}
impl From<Blend> for Widget {
    fn from(value: Blend) -> Self {
        Widget::blend(value.mode, value.child)
    }
}

/// Small composable builder for ordered retained effects. Each method wraps
/// the current child, so calls read in the same order as execution.
#[derive(Clone, Debug, PartialEq)]
pub struct Effects {
    child: Widget,
}
impl Effects {
    #[must_use]
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
        }
    }
    #[must_use]
    pub fn color_filter(mut self, filter: ColorFilter) -> Self {
        self.child = Widget::color_filtered(filter, self.child);
        self
    }
    #[must_use]
    pub fn color_matrix(self, filter: ColorFilter) -> Self {
        self.color_filter(filter)
    }
    #[must_use]
    pub fn blur(mut self, sigma: f32) -> Self {
        self.child = Widget::blur(sigma, self.child);
        self
    }
    #[must_use]
    pub fn asymmetric_blur(mut self, sigma_x: f32, sigma_y: f32) -> Self {
        self.child = Widget::asymmetric_blur(sigma_x, sigma_y, self.child);
        self
    }
    #[must_use]
    pub fn opacity(mut self, alpha: f32) -> Self {
        self.child = Widget::opacity(alpha, self.child);
        self
    }
    #[must_use]
    pub fn drop_shadow(mut self, offset: Offset, sigma: f32, color: Color) -> Self {
        self.child = Widget::drop_shadow(offset, sigma, color, self.child);
        self
    }
    #[must_use]
    pub fn blend(mut self, mode: BlendMode) -> Self {
        self.child = Widget::blend(mode, self.child);
        self
    }
    #[must_use]
    pub fn build(self) -> Widget {
        self.child
    }
}
impl From<Effects> for Widget {
    fn from(value: Effects) -> Self {
        value.build()
    }
}

/// Vertical retained viewport. Keep a [`ScrollController`] outside a rebuild
/// when application code needs the position to survive a recreated description.
pub struct ScrollView;
impl ScrollView {
    #[must_use]
    pub fn vertical(controller: ScrollController, child: impl Into<Widget>) -> Widget {
        Widget::scroll_view(controller, child.into())
    }
}
