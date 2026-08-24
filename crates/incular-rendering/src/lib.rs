//! Renderer-neutral scene, canvas, display-list, and compositor primitives.
use incular_assets::FontHandle;
use incular_core::{Arena, ArenaId, Color, DirtyFlags, Offset, Rect, Size, Transform};
use incular_image::ImageHandle;
use kurbo::{BezPath, Point, Shape};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

static NEXT_PATH_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_GRADIENT_ID: AtomicU64 = AtomicU64::new(1);

/// Stable identity for immutable normalized gradient stop data.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct GradientId(u64);

/// Per-corner radii in logical pixels, ordered clockwise from the top left.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CornerRadii {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_right: f32,
    pub bottom_left: f32,
}
impl CornerRadii {
    pub const ZERO: Self = Self::uniform(0.0);

    #[must_use]
    pub const fn uniform(radius: f32) -> Self {
        Self {
            top_left: radius,
            top_right: radius,
            bottom_right: radius,
            bottom_left: radius,
        }
    }

    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.top_left <= 0.0
            && self.top_right <= 0.0
            && self.bottom_right <= 0.0
            && self.bottom_left <= 0.0
    }

    /// CSS-compatible normalization: invalid values become zero and all four
    /// radii scale together when an opposing pair exceeds an edge.
    #[must_use]
    pub fn normalized(self, size: incular_core::Size) -> Self {
        let clean = |v: f32| if v.is_finite() { v.max(0.) } else { 0. };
        let mut r = Self {
            top_left: clean(self.top_left),
            top_right: clean(self.top_right),
            bottom_right: clean(self.bottom_right),
            bottom_left: clean(self.bottom_left),
        };
        let ratio = |edge: f32, sum: f32| if sum > 0. { edge / sum } else { 1. };
        let scale = ratio(size.width, r.top_left + r.top_right)
            .min(ratio(size.width, r.bottom_left + r.bottom_right))
            .min(ratio(size.height, r.top_left + r.bottom_left))
            .min(ratio(size.height, r.top_right + r.bottom_right))
            .min(1.);
        r.top_left *= scale;
        r.top_right *= scale;
        r.bottom_right *= scale;
        r.bottom_left *= scale;
        r
    }
    #[must_use]
    pub fn inset(self, amount: f32) -> Self {
        Self {
            top_left: (self.top_left - amount).max(0.),
            top_right: (self.top_right - amount).max(0.),
            bottom_right: (self.bottom_right - amount).max(0.),
            bottom_left: (self.bottom_left - amount).max(0.),
        }
    }
}

impl From<f32> for CornerRadii {
    fn from(v: f32) -> Self {
        Self::uniform(v)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RRect {
    pub rect: Rect,
    pub radii: CornerRadii,
}
impl RRect {
    #[must_use]
    pub fn new(rect: Rect, radii: CornerRadii) -> Self {
        Self {
            rect,
            radii: radii.normalized(rect.size),
        }
    }
    #[must_use]
    pub fn uniform(rect: Rect, radius: f32) -> Self {
        Self::new(rect, CornerRadii::uniform(radius))
    }
    #[must_use]
    pub fn inset(self, amount: f32) -> Self {
        let rect = Rect::from_origin_size(
            Offset::new(self.rect.origin.x + amount, self.rect.origin.y + amount),
            incular_core::Size::new(
                (self.rect.size.width - 2. * amount).max(0.),
                (self.rect.size.height - 2. * amount).max(0.),
            ),
        );
        Self::new(rect, self.radii.inset(amount))
    }
    /// Exact containment for the same normalized corner geometry used by the
    /// analytic renderer. Boundary points are included.
    #[must_use]
    pub fn contains(self, point: Offset) -> bool {
        if !self.rect.contains(point) {
            return false;
        }
        let local = point - self.rect.origin;
        let size = self.rect.size;
        let corner = if local.x < self.radii.top_left && local.y < self.radii.top_left {
            Some((
                self.radii.top_left,
                self.radii.top_left,
                self.radii.top_left,
            ))
        } else if local.x > size.width - self.radii.top_right && local.y < self.radii.top_right {
            Some((
                size.width - self.radii.top_right,
                self.radii.top_right,
                self.radii.top_right,
            ))
        } else if local.x > size.width - self.radii.bottom_right
            && local.y > size.height - self.radii.bottom_right
        {
            Some((
                size.width - self.radii.bottom_right,
                size.height - self.radii.bottom_right,
                self.radii.bottom_right,
            ))
        } else if local.x < self.radii.bottom_left && local.y > size.height - self.radii.bottom_left
        {
            Some((
                self.radii.bottom_left,
                size.height - self.radii.bottom_left,
                self.radii.bottom_left,
            ))
        } else {
            None
        };
        corner.is_none_or(|(cx, cy, radius)| {
            radius <= 0.
                || (local.x - cx).mul_add(local.x - cx, (local.y - cy) * (local.y - cy))
                    <= radius * radius
        })
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GradientStop {
    pub offset: f32,
    pub color: Color,
}
#[derive(Clone, Debug)]
pub struct GradientStops {
    id: GradientId,
    stops: Arc<[GradientStop]>,
}
impl PartialEq for GradientStops {
    fn eq(&self, other: &Self) -> bool {
        self.stops == other.stops
    }
}

/// Samples normalized stops using pad spread and premultiplied-linear color
/// interpolation. At a duplicate offset the last duplicate wins, allowing a
/// hard transition without division by zero.
#[must_use]
pub fn sample_gradient_stops(stops: &GradientStops, t: f32) -> [f32; 4] {
    let stops = stops.as_slice();
    let t = if t.is_finite() { t } else { 0. };
    if t <= stops[0].offset {
        return premultiplied(stops[0].color);
    }
    let last = stops.last().expect("normalized stops are non-empty");
    if t >= last.offset {
        return premultiplied(last.color);
    }
    let mut left = 0;
    for index in 1..stops.len() {
        if stops[index].offset <= t {
            left = index;
        } else {
            let a = stops[left];
            let b = stops[index];
            let amount = ((t - a.offset) / (b.offset - a.offset)).clamp(0., 1.);
            let a = premultiplied(a.color);
            let b = premultiplied(b.color);
            return [
                a[0] + (b[0] - a[0]) * amount,
                a[1] + (b[1] - a[1]) * amount,
                a[2] + (b[2] - a[2]) * amount,
                a[3] + (b[3] - a[3]) * amount,
            ];
        }
    }
    premultiplied(last.color)
}
fn premultiplied(color: Color) -> [f32; 4] {
    let [r, g, b, a] = color.to_linear_rgba();
    [r * a, g * a, b * a, a]
}
/// Pad-spread linear parameter in local logical coordinates. A zero-length
/// axis deterministically chooses the last stop (`t = 1`).
#[must_use]
pub fn linear_gradient_t(start: Offset, end: Offset, point: Offset) -> f32 {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let length_squared = dx * dx + dy * dy;
    if !length_squared.is_finite() || length_squared <= f32::EPSILON {
        return 1.;
    }
    (((point.x - start.x) * dx + (point.y - start.y) * dy) / length_squared).clamp(0., 1.)
}
/// Pad-spread radial parameter in local logical coordinates. A non-positive
/// radius deterministically chooses the last stop (`t = 1`).
#[must_use]
pub fn radial_gradient_t(center: Offset, radius: f32, point: Offset) -> f32 {
    if !radius.is_finite() || radius <= 0. {
        return 1.;
    }
    (((point.x - center.x).hypot(point.y - center.y)) / radius).clamp(0., 1.)
}
impl GradientStops {
    /// Stops are clamped to 0..=1 and stable-sorted. Empty becomes transparent;
    /// one stop is duplicated so shader sampling is always defined.
    #[must_use]
    pub fn new(mut stops: Vec<GradientStop>) -> Self {
        for s in &mut stops {
            s.offset = if s.offset.is_finite() {
                s.offset.clamp(0., 1.)
            } else {
                0.
            };
        }
        stops.sort_by(|a, b| a.offset.total_cmp(&b.offset));
        if stops.is_empty() {
            stops.push(GradientStop {
                offset: 0.,
                color: Color::TRANSPARENT,
            });
        }
        if stops.len() == 1 {
            stops.push(GradientStop {
                offset: 1.,
                color: stops[0].color,
            });
        }
        Self {
            id: GradientId(NEXT_GRADIENT_ID.fetch_add(1, Ordering::Relaxed)),
            stops: stops.into(),
        }
    }
    #[must_use]
    pub fn as_slice(&self) -> &[GradientStop] {
        &self.stops
    }
    /// Identity is computed when normalized stops are constructed, never while
    /// rendering. Clones intentionally retain the same cached GPU resource.
    #[must_use]
    pub const fn id(&self) -> GradientId {
        self.id
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct LinearGradient {
    pub start: Offset,
    pub end: Offset,
    pub stops: GradientStops,
}
#[derive(Clone, Debug, PartialEq)]
pub struct RadialGradient {
    pub center: Offset,
    pub radius: f32,
    pub stops: GradientStops,
}
#[derive(Clone, Debug, PartialEq)]
pub struct SweepGradient {
    pub center: Offset,
    pub start_angle: f32,
    pub stops: GradientStops,
}
#[derive(Clone, Debug, PartialEq)]
pub enum Brush {
    Solid(Color),
    LinearGradient(LinearGradient),
    RadialGradient(RadialGradient),
    SweepGradient(SweepGradient),
}
impl From<Color> for Brush {
    fn from(color: Color) -> Self {
        Self::Solid(color)
    }
}
impl From<LinearGradient> for Brush {
    fn from(gradient: LinearGradient) -> Self {
        Self::LinearGradient(gradient)
    }
}
impl From<RadialGradient> for Brush {
    fn from(gradient: RadialGradient) -> Self {
        Self::RadialGradient(gradient)
    }
}
impl From<SweepGradient> for Brush {
    fn from(gradient: SweepGradient) -> Self {
        Self::SweepGradient(gradient)
    }
}
#[must_use]
pub fn sweep_gradient_t(center: Offset, start_angle: f32, point: Offset) -> f32 {
    ((point.y - center.y).atan2(point.x - center.x) - start_angle).rem_euclid(std::f32::consts::TAU)
        / std::f32::consts::TAU
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Border {
    pub width: f32,
    pub color: Color,
}
impl Border {
    #[must_use]
    pub fn new(width: f32, color: Color) -> Self {
        Self {
            width: width.max(0.),
            color,
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Decoration {
    pub background: Option<Brush>,
    pub border: Option<Border>,
    pub border_radius: CornerRadii,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FillRule {
    NonZero,
    EvenOdd,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LineCap {
    Butt,
    Round,
    Square,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LineJoin {
    Miter,
    Round,
    Bevel,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stroke {
    pub width: f32,
    pub cap: LineCap,
    pub join: LineJoin,
    pub miter_limit: f32,
}
impl Default for Stroke {
    fn default() -> Self {
        Self {
            width: 1.,
            cap: LineCap::Butt,
            join: LineJoin::Miter,
            miter_limit: 4.,
        }
    }
}
/// Stable identity for immutable path geometry. It deliberately does not
/// encode paint, placement, or transforms.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct PathId(u64);
#[derive(Clone, Debug, PartialEq)]
pub struct Path {
    id: PathId,
    path: Arc<BezPath>,
    bounds: Option<Rect>,
}
impl Path {
    #[must_use]
    pub fn builder() -> PathBuilder {
        PathBuilder::default()
    }
    #[must_use]
    pub fn bez_path(&self) -> &BezPath {
        &self.path
    }
    #[must_use]
    pub fn bounds(&self) -> Option<Rect> {
        self.bounds
    }
    #[must_use]
    pub const fn id(&self) -> PathId {
        self.id
    }
    /// Renderer-neutral filled-path containment delegated to Kurbo's exact
    /// segment winding implementation. It is intentionally independent from
    /// GPU tessellation so pointer targeting follows the retained geometry.
    #[must_use]
    pub fn contains(&self, point: Offset, rule: FillRule) -> bool {
        let winding = self
            .path
            .winding(Point::new(f64::from(point.x), f64::from(point.y)));
        match rule {
            FillRule::NonZero => winding != 0,
            FillRule::EvenOdd => winding.unsigned_abs() % 2 == 1,
        }
    }
}
impl Default for Path {
    fn default() -> Self {
        PathBuilder::default().build()
    }
}
#[derive(Default)]
pub struct PathBuilder {
    path: BezPath,
}
impl PathBuilder {
    pub fn move_to(&mut self, p: Offset) -> &mut Self {
        self.path.move_to(point(p));
        self
    }
    pub fn line_to(&mut self, p: Offset) -> &mut Self {
        self.path.line_to(point(p));
        self
    }
    pub fn quadratic_to(&mut self, c: Offset, p: Offset) -> &mut Self {
        self.path.quad_to(point(c), point(p));
        self
    }
    pub fn cubic_to(&mut self, a: Offset, b: Offset, p: Offset) -> &mut Self {
        self.path.curve_to(point(a), point(b), point(p));
        self
    }
    pub fn close(&mut self) -> &mut Self {
        if !self.path.elements().is_empty() {
            self.path.close_path();
        }
        self
    }
    #[must_use]
    pub fn build(self) -> Path {
        let bounds = (!self.path.is_empty() && self.path.is_finite()).then(|| {
            let bounds = self.path.bounding_box();
            Rect::from_origin_size(
                Offset::new(bounds.x0 as f32, bounds.y0 as f32),
                incular_core::Size::new(
                    (bounds.x1 - bounds.x0) as f32,
                    (bounds.y1 - bounds.y0) as f32,
                ),
            )
        });
        Path {
            id: PathId(NEXT_PATH_ID.fetch_add(1, Ordering::Relaxed)),
            path: Arc::new(self.path),
            bounds,
        }
    }
}

fn point(offset: Offset) -> Point {
    Point::new(f64::from(offset.x), f64::from(offset.y))
}

/// A positioned glyph produced by a text shaper. It is intentionally not a
/// character: a glyph can represent multiple Unicode scalars or none.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlyphPosition {
    pub id: u16,
    pub offset: Offset,
    pub advance: f32,
    pub cluster: u32,
}
/// Compact, contiguous shaped glyph data shared by display-list commands.
#[derive(Clone, Debug, PartialEq)]
pub struct GlyphRun {
    pub font: FontHandle,
    pub font_size: f32,
    pub origin: Offset,
    pub glyphs: Arc<[GlyphPosition]>,
}

/// Renderer-neutral separable Gaussian blur description. Sigma is expressed
/// in logical pixels; a backend converts it to physical pixels at render
/// time. Negative and non-finite values are normalized to zero at this
/// boundary so every backend observes the same effect semantics.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GaussianBlur {
    pub sigma_x: f32,
    pub sigma_y: f32,
}
impl GaussianBlur {
    #[must_use]
    pub fn new(sigma_x: f32, sigma_y: f32) -> Self {
        Self {
            sigma_x: normalize_sigma(sigma_x),
            sigma_y: normalize_sigma(sigma_y),
        }
    }
    #[must_use]
    pub fn uniform(sigma: f32) -> Self {
        Self::new(sigma, sigma)
    }
}

/// Renderer-neutral arbitrary-subtree shadow description. The source is
/// isolated first, then its alpha is blurred and colorized by the compositor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DropShadowEffect {
    pub offset: Offset,
    pub sigma_x: f32,
    pub sigma_y: f32,
    pub color: Color,
}

/// A renderer-independent 4x5 color matrix. Matrix inputs and outputs are
/// straight (unpremultiplied) RGBA in the normalized 0..=1 range. The GPU
/// backend converts its retained premultiplied texture to straight color,
/// applies this matrix, clamps the result, and premultiplies it again.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorFilter {
    matrix: [f32; 20],
}
/// Compatibility spelling for callers that model a 4×5 filter as a color
/// matrix.  The alias keeps one canonical representation and one set of
/// straight/premultiplied semantics.
pub type ColorMatrix = ColorFilter;
impl ColorFilter {
    #[must_use]
    pub fn new(matrix: [f32; 20]) -> Self {
        Self::matrix(matrix)
    }
    #[must_use]
    pub fn matrix(matrix: [f32; 20]) -> Self {
        Self {
            matrix: matrix.map(|value| if value.is_finite() { value } else { 0. }),
        }
    }
    #[must_use]
    pub fn identity() -> Self {
        Self::matrix([
            1., 0., 0., 0., 0., 0., 1., 0., 0., 0., 0., 0., 1., 0., 0., 0., 0., 0., 1., 0.,
        ])
    }
    #[must_use]
    pub fn to_matrix(self) -> [f32; 20] {
        self.matrix
    }
    #[must_use]
    pub fn is_identity(self) -> bool {
        self == Self::identity()
    }
    /// Applies the matrix to straight normalized RGBA and clamps every output
    /// component. Zero-alpha input has zero straight RGB to avoid undefined
    /// premultiplied-to-straight division at transparent texels.
    #[must_use]
    pub fn apply(self, rgba: [f32; 4]) -> [f32; 4] {
        let alpha = finite_unit(rgba[3]);
        let input = [
            if alpha > f32::EPSILON {
                finite_unit(rgba[0])
            } else {
                0.
            },
            if alpha > f32::EPSILON {
                finite_unit(rgba[1])
            } else {
                0.
            },
            if alpha > f32::EPSILON {
                finite_unit(rgba[2])
            } else {
                0.
            },
            alpha,
        ];
        let mut out = [0.; 4];
        for (row, value) in out.iter_mut().enumerate() {
            let base = row * 5;
            *value = self.matrix[base] * input[0]
                + self.matrix[base + 1] * input[1]
                + self.matrix[base + 2] * input[2]
                + self.matrix[base + 3] * input[3]
                + self.matrix[base + 4];
        }
        let out = out.map(finite_unit);
        if out[3] <= f32::EPSILON {
            [0., 0., 0., 0.]
        } else {
            out
        }
    }
    /// Returns a matrix for `self` followed by `next` (`next * self`). The
    /// affine bias column is composed as part of the same multiplication.
    #[must_use]
    pub fn then(self, next: Self) -> Self {
        let a = self.matrix;
        let b = next.matrix;
        let mut out = [0.; 20];
        for row in 0..4 {
            for column in 0..4 {
                out[row * 5 + column] = (0..4)
                    .map(|index| b[row * 5 + index] * a[index * 5 + column])
                    .sum();
            }
            out[row * 5 + 4] = b[row * 5 + 4]
                + (0..4)
                    .map(|index| b[row * 5 + index] * a[index * 5 + 4])
                    .sum::<f32>();
        }
        Self::matrix(out)
    }
    /// Alias for [`Self::then`] that reads naturally at call sites building a
    /// filter pipeline.
    #[must_use]
    pub fn compose(self, next: Self) -> Self {
        self.then(next)
    }
    #[must_use]
    pub fn grayscale(amount: f32) -> Self {
        let amount = finite_unit(amount);
        let inv = 1. - amount;
        let r = 0.2126 * amount;
        let g = 0.7152 * amount;
        let b = 0.0722 * amount;
        Self::matrix([
            r + inv,
            g,
            b,
            0.,
            0.,
            r,
            g + inv,
            b,
            0.,
            0.,
            r,
            g,
            b + inv,
            0.,
            0.,
            0.,
            0.,
            0.,
            1.,
            0.,
        ])
    }
    #[must_use]
    pub fn sepia(amount: f32) -> Self {
        let amount = finite_unit(amount);
        let inv = 1. - amount;
        Self::matrix([
            0.393 * amount + inv,
            0.769 * amount,
            0.189 * amount,
            0.,
            0.,
            0.349 * amount,
            0.686 * amount + inv,
            0.168 * amount,
            0.,
            0.,
            0.272 * amount,
            0.534 * amount,
            0.131 * amount + inv,
            0.,
            0.,
            0.,
            0.,
            0.,
            1.,
            0.,
        ])
    }
    /// Brightness uses one as identity and zero as black.
    #[must_use]
    pub fn brightness(amount: f32) -> Self {
        let amount = if amount.is_finite() {
            amount.max(0.)
        } else {
            0.
        };
        Self::matrix([
            amount, 0., 0., 0., 0., 0., amount, 0., 0., 0., 0., 0., amount, 0., 0., 0., 0., 0., 1.,
            0.,
        ])
    }
    /// Contrast uses one as identity; the bias keeps middle gray fixed.
    #[must_use]
    pub fn contrast(amount: f32) -> Self {
        let amount = if amount.is_finite() {
            amount.max(0.)
        } else {
            0.
        };
        let bias = 0.5 * (1. - amount);
        Self::matrix([
            amount, 0., 0., 0., bias, 0., amount, 0., 0., bias, 0., 0., amount, 0., bias, 0., 0.,
            0., 1., 0.,
        ])
    }
    /// Saturation uses one as identity and zero as grayscale.
    #[must_use]
    pub fn saturate(amount: f32) -> Self {
        let amount = if amount.is_finite() {
            amount.max(0.)
        } else {
            0.
        };
        let inv = 1. - amount;
        let r = 0.2126 * inv;
        let g = 0.7152 * inv;
        let b = 0.0722 * inv;
        Self::matrix([
            r + amount,
            g,
            b,
            0.,
            0.,
            r,
            g + amount,
            b,
            0.,
            0.,
            r,
            g,
            b + amount,
            0.,
            0.,
            0.,
            0.,
            0.,
            1.,
            0.,
        ])
    }
    /// Invert uses one as the fully inverted result and zero as identity.
    #[must_use]
    pub fn invert(amount: f32) -> Self {
        let amount = finite_unit(amount);
        Self::matrix([
            1. - 2. * amount,
            0.,
            0.,
            0.,
            amount,
            0.,
            1. - 2. * amount,
            0.,
            0.,
            amount,
            0.,
            0.,
            1. - 2. * amount,
            0.,
            amount,
            0.,
            0.,
            0.,
            1.,
            0.,
        ])
    }
    /// Opacity changes alpha only; RGB remains straight and is premultiplied
    /// by the resulting alpha by the renderer.
    #[must_use]
    pub fn opacity(amount: f32) -> Self {
        Self::matrix([
            1.,
            0.,
            0.,
            0.,
            0.,
            0.,
            1.,
            0.,
            0.,
            0.,
            0.,
            0.,
            1.,
            0.,
            0.,
            0.,
            0.,
            0.,
            finite_unit(amount),
            0.,
        ])
    }
}
impl Default for ColorFilter {
    fn default() -> Self {
        Self::identity()
    }
}

fn finite_unit(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0., 1.)
    } else {
        0.
    }
}

/// A blend operation defined over premultiplied linear RGBA values. The GPU
/// compositor uses the same equations as [`blend_premultiplied`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BlendMode {
    #[default]
    SrcOver,
    Src,
    DstOver,
    SrcIn,
    DstIn,
    SrcOut,
    DstOut,
    SrcAtop,
    DstAtop,
    Xor,
    Plus,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
    ColorDodge,
    ColorBurn,
    HardLight,
    SoftLight,
    Difference,
    Exclusion,
}
impl BlendMode {
    #[must_use]
    pub const fn requires_destination_read(self) -> bool {
        !matches!(
            self,
            Self::SrcOver
                | Self::Src
                | Self::DstOver
                | Self::SrcIn
                | Self::DstIn
                | Self::SrcOut
                | Self::DstOut
                | Self::SrcAtop
                | Self::DstAtop
                | Self::Xor
                | Self::Plus
        )
    }
    #[must_use]
    pub const fn code(self) -> u32 {
        match self {
            Self::SrcOver => 0,
            Self::Src => 1,
            Self::DstOver => 2,
            Self::SrcIn => 3,
            Self::DstIn => 4,
            Self::SrcOut => 5,
            Self::DstOut => 6,
            Self::SrcAtop => 7,
            Self::DstAtop => 8,
            Self::Xor => 9,
            Self::Plus => 10,
            Self::Multiply => 11,
            Self::Screen => 12,
            Self::Overlay => 13,
            Self::Darken => 14,
            Self::Lighten => 15,
            Self::ColorDodge => 16,
            Self::ColorBurn => 17,
            Self::HardLight => 18,
            Self::SoftLight => 19,
            Self::Difference => 20,
            Self::Exclusion => 21,
        }
    }
}

/// Reference blend implementation for premultiplied normalized RGBA. It is
/// public so tests and alternate backends can verify their shader formulas.
#[must_use]
pub fn blend_premultiplied(mode: BlendMode, source: [f32; 4], destination: [f32; 4]) -> [f32; 4] {
    let src = sanitize_premultiplied(source);
    let dst = sanitize_premultiplied(destination);
    let as_ = src[3];
    let ad = dst[3];
    let ao = (as_ + ad - as_ * ad).clamp(0., 1.);
    let result = match mode {
        BlendMode::SrcOver => [
            src[0] + dst[0] * (1. - as_),
            src[1] + dst[1] * (1. - as_),
            src[2] + dst[2] * (1. - as_),
            ao,
        ],
        BlendMode::Src => src,
        BlendMode::DstOver => [
            dst[0] + src[0] * (1. - ad),
            dst[1] + src[1] * (1. - ad),
            dst[2] + src[2] * (1. - ad),
            ao,
        ],
        BlendMode::SrcIn => [src[0] * ad, src[1] * ad, src[2] * ad, as_ * ad],
        BlendMode::DstIn => [dst[0] * as_, dst[1] * as_, dst[2] * as_, ad * as_],
        BlendMode::SrcOut => [
            src[0] * (1. - ad),
            src[1] * (1. - ad),
            src[2] * (1. - ad),
            as_ * (1. - ad),
        ],
        BlendMode::DstOut => [
            dst[0] * (1. - as_),
            dst[1] * (1. - as_),
            dst[2] * (1. - as_),
            ad * (1. - as_),
        ],
        BlendMode::SrcAtop => [
            src[0] * ad + dst[0] * (1. - as_),
            src[1] * ad + dst[1] * (1. - as_),
            src[2] * ad + dst[2] * (1. - as_),
            ad,
        ],
        BlendMode::DstAtop => [
            dst[0] * as_ + src[0] * (1. - ad),
            dst[1] * as_ + src[1] * (1. - ad),
            dst[2] * as_ + src[2] * (1. - ad),
            as_,
        ],
        BlendMode::Xor => [
            src[0] * (1. - ad) + dst[0] * (1. - as_),
            src[1] * (1. - ad) + dst[1] * (1. - as_),
            src[2] * (1. - ad) + dst[2] * (1. - as_),
            (as_ + ad - 2. * as_ * ad).clamp(0., 1.),
        ],
        BlendMode::Plus => [
            (src[0] + dst[0]).min(1.),
            (src[1] + dst[1]).min(1.),
            (src[2] + dst[2]).min(1.),
            (as_ + ad).min(1.),
        ],
        artistic => {
            let cs = straight_rgb(src);
            let cd = straight_rgb(dst);
            let blended = artistic_blend_rgb(artistic, cs, cd);
            [
                (src[0] * (1. - ad) + dst[0] * (1. - as_) + as_ * ad * blended[0]).clamp(0., 1.),
                (src[1] * (1. - ad) + dst[1] * (1. - as_) + as_ * ad * blended[1]).clamp(0., 1.),
                (src[2] * (1. - ad) + dst[2] * (1. - as_) + as_ * ad * blended[2]).clamp(0., 1.),
                ao,
            ]
        }
    };
    sanitize_premultiplied(result)
}

fn sanitize_premultiplied(value: [f32; 4]) -> [f32; 4] {
    let alpha = finite_unit(value[3]);
    [
        if alpha > 0. {
            value[0].finite_or_zero().clamp(0., alpha)
        } else {
            0.
        },
        if alpha > 0. {
            value[1].finite_or_zero().clamp(0., alpha)
        } else {
            0.
        },
        if alpha > 0. {
            value[2].finite_or_zero().clamp(0., alpha)
        } else {
            0.
        },
        alpha,
    ]
}
trait FiniteOrZero {
    fn finite_or_zero(self) -> Self;
}
impl FiniteOrZero for f32 {
    fn finite_or_zero(self) -> Self {
        if self.is_finite() { self } else { 0. }
    }
}
fn straight_rgb(value: [f32; 4]) -> [f32; 3] {
    if value[3] <= f32::EPSILON {
        [0.; 3]
    } else {
        [
            (value[0] / value[3]).clamp(0., 1.),
            (value[1] / value[3]).clamp(0., 1.),
            (value[2] / value[3]).clamp(0., 1.),
        ]
    }
}
fn artistic_blend_rgb(mode: BlendMode, source: [f32; 3], destination: [f32; 3]) -> [f32; 3] {
    let mut result = [0.; 3];
    for index in 0..3 {
        let s = source[index];
        let d = destination[index];
        result[index] = match mode {
            BlendMode::Multiply => s * d,
            BlendMode::Screen => s + d - s * d,
            BlendMode::Overlay => {
                if d <= 0.5 {
                    2. * s * d
                } else {
                    1. - 2. * (1. - s) * (1. - d)
                }
            }
            BlendMode::Darken => s.min(d),
            BlendMode::Lighten => s.max(d),
            BlendMode::ColorDodge => {
                if s >= 1. {
                    1.
                } else {
                    (d / (1. - s)).min(1.)
                }
            }
            BlendMode::ColorBurn => {
                if s <= 0. {
                    0.
                } else {
                    (1. - (1. - d) / s).max(0.)
                }
            }
            BlendMode::HardLight => {
                if s <= 0.5 {
                    2. * s * d
                } else {
                    1. - 2. * (1. - s) * (1. - d)
                }
            }
            BlendMode::SoftLight => {
                if s <= 0.5 {
                    d - (1. - 2. * s) * d * (1. - d)
                } else {
                    let g = if d <= 0.25 {
                        ((16. * d - 12.) * d + 4.) * d
                    } else {
                        d.sqrt()
                    };
                    d + (2. * s - 1.) * (g - d)
                }
            }
            BlendMode::Difference => (d - s).abs(),
            BlendMode::Exclusion => s + d - 2. * s * d,
            _ => s,
        };
    }
    result.map(|value| value.clamp(0., 1.))
}

/// Ordered single-input effect descriptions. Drop shadows remain a separate
/// layer because they intentionally produce both a shadow and the original
/// source output.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Effect {
    GaussianBlur(GaussianBlur),
    ColorMatrix(ColorFilter),
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EffectChain {
    effects: Vec<Effect>,
}
impl EffectChain {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            effects: Vec::new(),
        }
    }
    #[must_use]
    pub fn from_effects(effects: Vec<Effect>) -> Self {
        Self { effects }
    }
    #[must_use]
    pub fn effects(&self) -> &[Effect] {
        &self.effects
    }
    pub fn push(&mut self, effect: Effect) {
        self.effects.push(effect);
    }
    #[must_use]
    pub fn with_effect(mut self, effect: Effect) -> Self {
        self.push(effect);
        self
    }
    #[must_use]
    pub fn color_filter(self, filter: ColorFilter) -> Self {
        self.with_effect(Effect::ColorMatrix(filter))
    }
    #[must_use]
    pub fn blur(self, sigma: f32) -> Self {
        self.with_effect(Effect::GaussianBlur(GaussianBlur::uniform(sigma)))
    }
    /// Fuses only adjacent color matrices. Spatial effects remain explicit so
    /// their ordering and cache dependencies cannot be changed accidentally.
    #[must_use]
    pub fn optimized(&self) -> Self {
        let mut out = Self::new();
        for effect in &self.effects {
            if let (Some(Effect::ColorMatrix(previous)), Effect::ColorMatrix(next)) =
                (out.effects.last_mut(), effect)
            {
                *previous = previous.then(*next);
            } else {
                out.effects.push(*effect);
            }
        }
        out
    }
    #[must_use]
    pub fn fusion_count(&self) -> usize {
        self.effects
            .len()
            .saturating_sub(self.optimized().effects.len())
    }
}
impl DropShadowEffect {
    #[must_use]
    pub fn new(offset: Offset, sigma: f32, color: Color) -> Self {
        Self {
            offset: finite_offset(offset),
            sigma_x: normalize_sigma(sigma),
            sigma_y: normalize_sigma(sigma),
            color,
        }
    }
    #[must_use]
    pub fn asymmetric(offset: Offset, sigma_x: f32, sigma_y: f32, color: Color) -> Self {
        Self {
            offset: finite_offset(offset),
            sigma_x: normalize_sigma(sigma_x),
            sigma_y: normalize_sigma(sigma_y),
            color,
        }
    }
    #[must_use]
    pub fn blur(self) -> GaussianBlur {
        GaussianBlur::new(self.sigma_x, self.sigma_y)
    }
}

/// Canonical sigma normalization used by painting, widgets, and GPU
/// lowering. Non-finite values follow the existing "invalid numeric input is
/// transparent/zero" policy.
#[must_use]
pub fn normalize_sigma(sigma: f32) -> f32 {
    if sigma.is_finite() { sigma.max(0.) } else { 0. }
}

fn finite_offset(offset: Offset) -> Offset {
    Offset::new(
        if offset.x.is_finite() { offset.x } else { 0. },
        if offset.y.is_finite() { offset.y } else { 0. },
    )
}

/// Conservative logical blur margin for the finite 3-sigma cutoff.
#[must_use]
pub fn blur_margin(sigma: f32) -> f32 {
    (3. * normalize_sigma(sigma)).max(0.)
}

/// Expands source bounds by the finite Gaussian support in logical pixels.
#[must_use]
pub fn blur_bounds(source: Rect, sigma_x: f32, sigma_y: f32) -> Rect {
    let mx = blur_margin(sigma_x);
    let my = blur_margin(sigma_y);
    Rect::from_origin_size(
        Offset::new(source.origin.x - mx, source.origin.y - my),
        Size::new(source.size.width + 2. * mx, source.size.height + 2. * my),
    )
}

/// Bounds of a shadow plus the original source. The union is deliberately
/// conservative and works for positive, negative, and fractional offsets.
#[must_use]
pub fn drop_shadow_bounds(source: Rect, offset: Offset, sigma_x: f32, sigma_y: f32) -> Rect {
    union_rect(
        source,
        blur_bounds(
            Rect::from_origin_size(source.origin + finite_offset(offset), source.size),
            sigma_x,
            sigma_y,
        ),
    )
}

/// A normalized, symmetric one-dimensional Gaussian kernel represented in
/// order `[-radius, ..., 0, ..., +radius]`. The cutoff is exactly three
/// physical sigma, and sigma zero is the identity kernel.
#[must_use]
pub fn gaussian_kernel_weights(sigma_physical: f32) -> Vec<f32> {
    let sigma = normalize_sigma(sigma_physical);
    if sigma <= f32::EPSILON {
        return vec![1.];
    }
    let radius = (3. * sigma).ceil().max(1.) as i32;
    let denominator = 2. * sigma * sigma;
    let mut weights = (-radius..=radius)
        .map(|i| (-(i * i) as f32 / denominator).exp())
        .collect::<Vec<_>>();
    let sum: f32 = weights.iter().sum();
    if sum.is_finite() && sum > 0. {
        for weight in &mut weights {
            *weight /= sum;
        }
    } else {
        weights.fill(0.);
        weights[radius as usize] = 1.;
    }
    weights
}

/// Premultiplied shadow color for one blurred alpha sample. This helper is
/// shared by deterministic CPU tests and documents the exact GPU equation.
#[must_use]
pub fn premultiplied_shadow_sample(color: Color, mask: f32) -> [f32; 4] {
    let alpha = (f32::from(color.alpha) / 255.) * mask.clamp(0., 1.);
    let [r, g, b, _] = color.to_linear_rgba();
    [r * alpha, g * alpha, b * alpha, alpha]
}

#[derive(Clone, Debug, PartialEq)]
pub enum PaintCommand {
    Rect {
        rect: Rect,
        color: Color,
    },
    RRect {
        rrect: RRect,
        brush: Brush,
    },
    Border {
        rrect: RRect,
        border: Border,
    },
    FillPath {
        path: Arc<Path>,
        brush: Brush,
        fill_rule: FillRule,
    },
    StrokePath {
        path: Arc<Path>,
        brush: Brush,
        stroke: Stroke,
    },
    /// Source is image pixels; destination is local logical geometry.
    Image {
        image: ImageHandle,
        source: Rect,
        destination: Rect,
        sampling: ImageSampling,
    },
    GlyphRun {
        run: Arc<GlyphRun>,
        color: Color,
    },
    PushClip {
        rect: Rect,
    },
    PushClipRRect {
        rrect: RRect,
    },
    PushClipOval {
        rect: Rect,
    },
    PushClipPath {
        path: Arc<Path>,
        fill_rule: FillRule,
    },
    PopClip,
    PushTransform {
        transform: Transform,
    },
    PopTransform,
    /// Begins an isolated retained opacity group. The compositor renders the
    /// enclosed commands into a transparent target and applies `alpha` once
    /// when that target is composited back into its parent.
    PushOpacity {
        layer: LayerId,
        alpha: f32,
        generation: u64,
        bounds: Rect,
    },
    PopOpacity,
    /// Begins an isolated Gaussian-filtered subtree. The generation and
    /// bounds describe source pixels only; sigma is a compositor property.
    PushBlur {
        layer: LayerId,
        blur: GaussianBlur,
        generation: u64,
        bounds: Rect,
    },
    /// Begins an isolated arbitrary-subtree drop shadow. The child source is
    /// emitted once and the compositor draws the blurred alpha behind it.
    PushDropShadow {
        layer: LayerId,
        shadow: DropShadowEffect,
        generation: u64,
        bounds: Rect,
    },
    /// Begins an isolated straight-RGBA color-matrix stage.
    PushColorFilter {
        layer: LayerId,
        filter: ColorFilter,
        generation: u64,
        bounds: Rect,
    },
    /// Begins an isolated blend group. The source is emitted once and the
    /// compositor combines it with the destination in painter order.
    PushBlend {
        layer: LayerId,
        mode: BlendMode,
        generation: u64,
        bounds: Rect,
    },
    PopEffect,
}
/// Renderer-neutral raster filtering policy. Ordinary UI images default to
/// linear sampling; pixel art can opt into nearest-neighbour sampling.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ImageSampling {
    #[default]
    Linear,
    Nearest,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DisplayList {
    commands: Vec<PaintCommand>,
}
impl DisplayList {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }
    pub fn push(&mut self, command: PaintCommand) {
        self.commands.push(command);
    }
    pub fn extend_from(&mut self, other: &Self) {
        self.commands.extend(other.commands.iter().cloned());
    }
    #[must_use]
    pub fn commands(&self) -> &[PaintCommand] {
        &self.commands
    }
    #[must_use]
    pub fn len(&self) -> usize {
        self.commands.len()
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}
#[derive(Default)]
pub struct Canvas {
    list: DisplayList,
    saves: Vec<SaveKind>,
}
#[derive(Clone, Copy)]
enum SaveKind {
    Transform,
    Clip,
}
impl Canvas {
    pub fn rect(&mut self, rect: Rect, color: Color) {
        self.list.push(PaintCommand::Rect { rect, color });
    }
    pub fn rrect(&mut self, rrect: RRect, brush: impl Into<Brush>) {
        self.list.push(PaintCommand::RRect {
            rrect,
            brush: brush.into(),
        });
    }
    pub fn border(&mut self, rrect: RRect, border: Border) {
        self.list.push(PaintCommand::Border { rrect, border });
    }
    pub fn fill_path(&mut self, path: Arc<Path>, brush: impl Into<Brush>, fill_rule: FillRule) {
        self.list.push(PaintCommand::FillPath {
            path,
            brush: brush.into(),
            fill_rule,
        });
    }
    pub fn stroke_path(&mut self, path: Arc<Path>, brush: impl Into<Brush>, stroke: Stroke) {
        self.list.push(PaintCommand::StrokePath {
            path,
            brush: brush.into(),
            stroke,
        });
    }
    pub fn image(&mut self, image: ImageHandle, source: Rect, destination: Rect) {
        self.image_with_sampling(image, source, destination, ImageSampling::Linear);
    }
    pub fn image_with_sampling(
        &mut self,
        image: ImageHandle,
        source: Rect,
        destination: Rect,
        sampling: ImageSampling,
    ) {
        self.list.push(PaintCommand::Image {
            image,
            source,
            destination,
            sampling,
        });
    }
    pub fn glyph_run(&mut self, run: Arc<GlyphRun>, color: Color) {
        self.list.push(PaintCommand::GlyphRun { run, color });
    }
    pub fn save_transform(&mut self, transform: Transform) {
        self.saves.push(SaveKind::Transform);
        self.list.push(PaintCommand::PushTransform { transform });
    }
    pub fn save_clip(&mut self, rect: Rect) {
        self.saves.push(SaveKind::Clip);
        self.list.push(PaintCommand::PushClip { rect });
    }
    pub fn save_clip_rrect(&mut self, rrect: RRect) {
        self.saves.push(SaveKind::Clip);
        self.list.push(PaintCommand::PushClipRRect { rrect });
    }
    pub fn save_clip_oval(&mut self, rect: Rect) {
        self.saves.push(SaveKind::Clip);
        self.list.push(PaintCommand::PushClipOval { rect });
    }
    pub fn save_clip_path(&mut self, path: Arc<Path>, fill_rule: FillRule) {
        self.saves.push(SaveKind::Clip);
        self.list
            .push(PaintCommand::PushClipPath { path, fill_rule });
    }
    pub fn restore(&mut self) {
        match self.saves.pop().expect("unbalanced canvas restore") {
            SaveKind::Transform => self.list.push(PaintCommand::PopTransform),
            SaveKind::Clip => self.list.push(PaintCommand::PopClip),
        }
    }
    #[must_use]
    pub fn finish(self) -> DisplayList {
        assert!(self.saves.is_empty(), "unbalanced display list");
        self.list
    }
}

/// Opaque retained compositor identity. It uses the core generational arena so
/// an ID for a removed layer can never select a later, unrelated layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LayerId(ArenaId);

/// Canonical opacity representation shared by widgets, retained layers, and
/// GPU lowering. Non-finite values become transparent and finite values are
/// clamped to the physically meaningful range.
#[must_use]
pub fn normalize_opacity(alpha: f32) -> f32 {
    if alpha.is_finite() {
        alpha.clamp(0., 1.)
    } else {
        0.
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CompositorDiagnostics {
    pub layers: u64,
    pub picture_layers_reused: u64,
    pub picture_layers_repainted: u64,
    pub transform_updates: u64,
    pub clip_updates: u64,
    pub layers_culled: u64,
    pub opacity_updates: u64,
}

/// A picture placement observed while flattening. All rectangles here are in
/// logical coordinates; this is intentionally useful for CPU-side scene tests
/// and diagnostics without involving a surface or DPI scale factor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlattenedPicture {
    pub layer: LayerId,
    pub local_bounds: Rect,
    pub world_bounds: Rect,
    pub active_clip: Option<Rect>,
}

#[derive(Clone, Debug)]
pub enum LayerKind {
    Picture {
        display_list: Arc<DisplayList>,
        bounds: Rect,
    },
    Transform {
        transform: Transform,
    },
    ClipRect {
        rect: Rect,
    },
    Opacity {
        alpha: f32,
    },
    Blur {
        blur: GaussianBlur,
    },
    DropShadow {
        shadow: DropShadowEffect,
    },
    ColorFilter {
        filter: ColorFilter,
    },
    Blend {
        mode: BlendMode,
    },
}

#[derive(Clone, Debug)]
struct Layer {
    kind: LayerKind,
    children: Vec<LayerId>,
    dirty: DirtyFlags,
    generation: u64,
}

/// Renderer-independent retained layer arena. Picture display lists are owned
/// once and scene flattening only emits a transient linear command stream.
/// This deliberately has no GPU resources or offscreen textures.
pub struct LayerTree {
    layers: Arena<Layer>,
    root: Option<LayerId>,
    diagnostics: CompositorDiagnostics,
    flattened_pictures: Vec<FlattenedPicture>,
    next_generation: u64,
}
impl Default for LayerTree {
    fn default() -> Self {
        Self::new()
    }
}
impl LayerTree {
    #[must_use]
    pub fn new() -> Self {
        Self {
            layers: Arena::new(),
            root: None,
            diagnostics: CompositorDiagnostics::default(),
            flattened_pictures: Vec::new(),
            next_generation: 1,
        }
    }
    #[must_use]
    pub fn diagnostics(&self) -> CompositorDiagnostics {
        self.diagnostics
    }
    #[must_use]
    pub fn root(&self) -> Option<LayerId> {
        self.root
    }
    pub fn set_root(&mut self, root: LayerId) {
        self.root = Some(root);
    }
    pub fn create_picture(&mut self, display_list: DisplayList, bounds: Rect) -> LayerId {
        self.insert(LayerKind::Picture {
            display_list: Arc::new(display_list),
            bounds,
        })
    }
    pub fn create_transform(&mut self, transform: Transform) -> LayerId {
        self.insert(LayerKind::Transform { transform })
    }
    pub fn create_clip_rect(&mut self, rect: Rect) -> LayerId {
        self.insert(LayerKind::ClipRect { rect })
    }
    /// Creates an isolated compositor group. Alpha is normalized at the
    /// renderer-neutral boundary so every backend observes identical values.
    pub fn create_opacity(&mut self, alpha: f32) -> LayerId {
        self.insert(LayerKind::Opacity {
            alpha: normalize_opacity(alpha),
        })
    }
    /// Creates an isolated Gaussian-filtered compositor group.
    pub fn create_blur(&mut self, blur: GaussianBlur) -> LayerId {
        self.insert(LayerKind::Blur {
            blur: GaussianBlur::new(blur.sigma_x, blur.sigma_y),
        })
    }
    /// Creates an isolated arbitrary-subtree drop-shadow compositor group.
    pub fn create_drop_shadow(&mut self, shadow: DropShadowEffect) -> LayerId {
        self.insert(LayerKind::DropShadow {
            shadow: DropShadowEffect::asymmetric(
                shadow.offset,
                shadow.sigma_x,
                shadow.sigma_y,
                shadow.color,
            ),
        })
    }
    /// Creates an isolated color-matrix stage. Matrix changes are compositor
    /// updates and deliberately do not change the source generation.
    pub fn create_color_filter(&mut self, filter: ColorFilter) -> LayerId {
        self.insert(LayerKind::ColorFilter { filter })
    }
    /// Creates a retained blend group whose source remains cached when only
    /// the discrete blend mode changes.
    pub fn create_blend(&mut self, mode: BlendMode) -> LayerId {
        self.insert(LayerKind::Blend { mode })
    }
    fn insert(&mut self, kind: LayerKind) -> LayerId {
        let generation = self.next_generation;
        self.next_generation = self.next_generation.wrapping_add(1).max(1);
        let id = LayerId(self.layers.insert(Layer {
            kind,
            children: Vec::new(),
            dirty: DirtyFlags::COMPOSITE,
            generation,
        }));
        self.diagnostics.layers += 1;
        id
    }
    pub fn remove(&mut self, id: LayerId) {
        if self.layers.remove(id.0).is_some() {
            self.diagnostics.layers -= 1;
            if self.root == Some(id) {
                self.root = None;
            }
            for (_, layer) in self.layers.iter() {
                // Children are pruned by their owners before removal in normal
                // widget unmounting; stale entries are ignored by flattening.
                let _ = layer;
            }
        }
    }
    pub fn set_children(&mut self, id: LayerId, children: Vec<LayerId>) {
        if let Some(layer) = self.layers.get_mut(id.0) {
            if layer.children == children {
                return;
            }
            layer.children = children;
            layer.dirty.insert(DirtyFlags::COMPOSITE);
            layer.generation = self.next_generation;
            self.next_generation = self.next_generation.wrapping_add(1).max(1);
        }
    }
    pub fn update_picture(&mut self, id: LayerId, display_list: DisplayList, bounds: Rect) {
        if let Some(layer) = self.layers.get_mut(id.0)
            && let LayerKind::Picture {
                display_list: current,
                bounds: current_bounds,
            } = &mut layer.kind
        {
            *current = Arc::new(display_list);
            *current_bounds = bounds;
            layer
                .dirty
                .insert(DirtyFlags::PAINT | DirtyFlags::COMPOSITE);
            layer.generation = self.next_generation;
            self.next_generation = self.next_generation.wrapping_add(1).max(1);
            self.diagnostics.picture_layers_repainted += 1;
        }
    }
    pub fn update_transform(&mut self, id: LayerId, transform: Transform) -> bool {
        let Some(layer) = self.layers.get_mut(id.0) else {
            return false;
        };
        let LayerKind::Transform { transform: current } = &mut layer.kind else {
            return false;
        };
        if *current == transform {
            return false;
        }
        *current = transform;
        layer.dirty.insert(DirtyFlags::COMPOSITE);
        layer.generation = self.next_generation;
        self.next_generation = self.next_generation.wrapping_add(1).max(1);
        self.diagnostics.transform_updates += 1;
        true
    }
    pub fn update_clip(&mut self, id: LayerId, rect: Rect) -> bool {
        let Some(layer) = self.layers.get_mut(id.0) else {
            return false;
        };
        let LayerKind::ClipRect { rect: current } = &mut layer.kind else {
            return false;
        };
        if *current == rect {
            return false;
        }
        *current = rect;
        layer.dirty.insert(DirtyFlags::COMPOSITE);
        layer.generation = self.next_generation;
        self.next_generation = self.next_generation.wrapping_add(1).max(1);
        self.diagnostics.clip_updates += 1;
        true
    }
    /// Updates only the group alpha. This deliberately does not change the
    /// content generation, allowing a cached offscreen texture to be reused
    /// while an opacity animation is ticking.
    pub fn update_opacity(&mut self, id: LayerId, alpha: f32) -> bool {
        let Some(layer) = self.layers.get_mut(id.0) else {
            return false;
        };
        let LayerKind::Opacity { alpha: current } = &mut layer.kind else {
            return false;
        };
        let alpha = normalize_opacity(alpha);
        if *current == alpha {
            return false;
        }
        *current = alpha;
        layer.dirty.insert(DirtyFlags::COMPOSITE);
        self.diagnostics.opacity_updates += 1;
        true
    }
    /// Updates Gaussian parameters without changing the retained source
    /// generation. A backend recomputes only the filtered result.
    pub fn update_blur(&mut self, id: LayerId, blur: GaussianBlur) -> bool {
        let Some(layer) = self.layers.get_mut(id.0) else {
            return false;
        };
        let LayerKind::Blur { blur: current } = &mut layer.kind else {
            return false;
        };
        let blur = GaussianBlur::new(blur.sigma_x, blur.sigma_y);
        if *current == blur {
            return false;
        }
        *current = blur;
        layer.dirty.insert(DirtyFlags::COMPOSITE);
        true
    }
    /// Updates shadow presentation properties without changing source pixels.
    /// Sigma invalidates the filtered mask; offset/color remain composite-only
    /// in GPU backends.
    pub fn update_drop_shadow(&mut self, id: LayerId, shadow: DropShadowEffect) -> bool {
        let Some(layer) = self.layers.get_mut(id.0) else {
            return false;
        };
        let LayerKind::DropShadow { shadow: current } = &mut layer.kind else {
            return false;
        };
        let shadow = DropShadowEffect::asymmetric(
            shadow.offset,
            shadow.sigma_x,
            shadow.sigma_y,
            shadow.color,
        );
        if *current == shadow {
            return false;
        }
        *current = shadow;
        layer.dirty.insert(DirtyFlags::COMPOSITE);
        true
    }
    pub fn update_color_filter(&mut self, id: LayerId, filter: ColorFilter) -> bool {
        let Some(layer) = self.layers.get_mut(id.0) else {
            return false;
        };
        let LayerKind::ColorFilter { filter: current } = &mut layer.kind else {
            return false;
        };
        if *current == filter {
            return false;
        }
        *current = filter;
        layer.dirty.insert(DirtyFlags::COMPOSITE);
        true
    }
    pub fn update_blend(&mut self, id: LayerId, mode: BlendMode) -> bool {
        let Some(layer) = self.layers.get_mut(id.0) else {
            return false;
        };
        let LayerKind::Blend { mode: current } = &mut layer.kind else {
            return false;
        };
        if *current == mode {
            return false;
        }
        *current = mode;
        layer.dirty.insert(DirtyFlags::COMPOSITE);
        true
    }
    /// World-space logical bounds captured by the most recent [`Self::flatten`]
    /// call. This keeps coordinate diagnostics out of the renderer hot path.
    #[must_use]
    pub fn flattened_pictures(&self) -> &[FlattenedPicture] {
        &self.flattened_pictures
    }
    #[must_use]
    pub fn flatten(&mut self) -> DisplayList {
        let mut out = DisplayList::new();
        self.flattened_pictures.clear();
        if let Some(root) = self.root {
            self.flatten_layer(root, Transform::IDENTITY, None, &mut out);
        }
        for (_, layer) in self.layers.iter() {
            if matches!(layer.kind, LayerKind::Picture { .. })
                && !layer.dirty.contains(DirtyFlags::PAINT)
            {
                self.diagnostics.picture_layers_reused += 1;
            }
        }
        for (_, layer) in self.layers.iter() {
            // Cleared after submission: retained data stays intact.
            let _ = layer;
        }
        for (_, layer) in self.layers.iter() {
            let _ = layer;
        }
        // Arena does not expose mutable iteration intentionally; dirty state is
        // advisory/debug-only and property writes remain coalesced by value.
        out
    }
    fn flatten_layer(
        &mut self,
        id: LayerId,
        world_transform: Transform,
        clip: Option<Rect>,
        out: &mut DisplayList,
    ) {
        let Some(layer) = self.layers.get(id.0).cloned() else {
            return;
        };
        match layer.kind {
            LayerKind::Picture {
                display_list,
                bounds,
            } => {
                let world = world_transform.transform_rect_bbox(bounds);
                if clip.is_some_and(|active| !active.intersects(world)) {
                    self.diagnostics.layers_culled += 1;
                    return;
                }
                self.flattened_pictures.push(FlattenedPicture {
                    layer: id,
                    local_bounds: bounds,
                    world_bounds: world,
                    active_clip: clip,
                });
                out.push(PaintCommand::PushTransform {
                    transform: world_transform,
                });
                out.extend_from(&display_list);
                out.push(PaintCommand::PopTransform);
            }
            LayerKind::Transform { transform: local } => {
                let next = world_transform.then(local);
                for child in layer.children {
                    self.flatten_layer(child, next, clip, out);
                }
            }
            LayerKind::ClipRect { rect } => {
                let world = world_transform.transform_rect_bbox(rect);
                let next_clip = match clip {
                    Some(old) => old.intersection(world),
                    None => Some(world),
                };
                if next_clip.is_none() {
                    self.diagnostics.layers_culled += 1;
                    return;
                }
                out.push(PaintCommand::PushClip { rect: world });
                for child in layer.children {
                    self.flatten_layer(child, world_transform, next_clip, out);
                }
                out.push(PaintCommand::PopClip);
            }
            LayerKind::Opacity { alpha } => {
                let bounds = self
                    .subtree_bounds(id, world_transform)
                    .unwrap_or_else(|| Rect::from_origin_size(Offset::ZERO, Size::ZERO));
                out.push(PaintCommand::PushOpacity {
                    layer: id,
                    alpha,
                    generation: self.subtree_generation(id),
                    bounds,
                });
                for child in layer.children {
                    self.flatten_layer(child, world_transform, clip, out);
                }
                out.push(PaintCommand::PopOpacity);
            }
            LayerKind::Blur { blur } => {
                let bounds = self
                    .subtree_bounds(id, world_transform)
                    .unwrap_or_else(|| Rect::from_origin_size(Offset::ZERO, Size::ZERO));
                out.push(PaintCommand::PushBlur {
                    layer: id,
                    blur,
                    generation: self.subtree_generation(id),
                    bounds,
                });
                for child in layer.children {
                    self.flatten_layer(child, world_transform, clip, out);
                }
                out.push(PaintCommand::PopEffect);
            }
            LayerKind::DropShadow { shadow } => {
                let bounds = self
                    .subtree_bounds(id, world_transform)
                    .unwrap_or_else(|| Rect::from_origin_size(Offset::ZERO, Size::ZERO));
                out.push(PaintCommand::PushDropShadow {
                    layer: id,
                    shadow,
                    generation: self.subtree_generation(id),
                    bounds,
                });
                for child in layer.children {
                    self.flatten_layer(child, world_transform, clip, out);
                }
                out.push(PaintCommand::PopEffect);
            }
            LayerKind::ColorFilter { filter } => {
                let bounds = self
                    .subtree_bounds(id, world_transform)
                    .unwrap_or_else(|| Rect::from_origin_size(Offset::ZERO, Size::ZERO));
                out.push(PaintCommand::PushColorFilter {
                    layer: id,
                    filter,
                    generation: self.subtree_generation(id),
                    bounds,
                });
                for child in layer.children {
                    self.flatten_layer(child, world_transform, clip, out);
                }
                out.push(PaintCommand::PopEffect);
            }
            LayerKind::Blend { mode } => {
                let bounds = self
                    .subtree_bounds(id, world_transform)
                    .unwrap_or_else(|| Rect::from_origin_size(Offset::ZERO, Size::ZERO));
                out.push(PaintCommand::PushBlend {
                    layer: id,
                    mode,
                    generation: self.subtree_generation(id),
                    bounds,
                });
                for child in layer.children {
                    self.flatten_layer(child, world_transform, clip, out);
                }
                out.push(PaintCommand::PopEffect);
            }
        }
    }
    fn subtree_generation(&self, id: LayerId) -> u64 {
        let Some(layer) = self.layers.get(id.0) else {
            return 0;
        };
        let mut value = layer.generation.wrapping_mul(0x9e37_79b9_7f4a_7c15);
        for child in &layer.children {
            // An opacity layer's own alpha is intentionally excluded from
            // its cache key: changing it only changes the final composite.
            // When that layer is nested inside another isolated group,
            // however, its composite is part of the parent's pixels. Include
            // the child's alpha at this boundary so an inner fade invalidates
            // the outer group's cached texture without invalidating the
            // inner group's own content target.
            if let Some(child_layer) = self.layers.get(child.0)
                && let LayerKind::Opacity { alpha } = &child_layer.kind
            {
                value = value.rotate_left(11) ^ u64::from(alpha.to_bits());
            }
            if let Some(child_layer) = self.layers.get(child.0) {
                match &child_layer.kind {
                    LayerKind::Blur { blur } => {
                        value = value.rotate_left(13)
                            ^ u64::from(blur.sigma_x.to_bits())
                            ^ u64::from(blur.sigma_y.to_bits()).rotate_left(17);
                    }
                    LayerKind::DropShadow { shadow } => {
                        value = value.rotate_left(13)
                            ^ u64::from(shadow.offset.x.to_bits())
                            ^ u64::from(shadow.offset.y.to_bits()).rotate_left(7)
                            ^ u64::from(shadow.sigma_x.to_bits()).rotate_left(17)
                            ^ u64::from(shadow.sigma_y.to_bits()).rotate_left(23)
                            ^ u64::from(shadow.color.red)
                            ^ (u64::from(shadow.color.green) << 8)
                            ^ (u64::from(shadow.color.blue) << 16)
                            ^ (u64::from(shadow.color.alpha) << 24);
                    }
                    LayerKind::ColorFilter { filter } => {
                        for (index, bits) in filter.to_matrix().map(f32::to_bits).iter().enumerate()
                        {
                            value ^= u64::from(*bits).rotate_left((index as u32 % 31) + 1);
                        }
                    }
                    LayerKind::Blend { mode } => {
                        value = value.rotate_left(13) ^ u64::from(mode.code());
                    }
                    _ => {}
                }
            }
            value = value.rotate_left(7) ^ self.subtree_generation(*child);
        }
        value
    }
    fn subtree_bounds(&self, id: LayerId, world_transform: Transform) -> Option<Rect> {
        let layer = self.layers.get(id.0)?;
        match &layer.kind {
            LayerKind::Picture { bounds, .. } => Some(world_transform.transform_rect_bbox(*bounds)),
            LayerKind::Transform { transform: local } => layer
                .children
                .iter()
                .filter_map(|child| self.subtree_bounds(*child, world_transform.then(*local)))
                .reduce(union_rect),
            LayerKind::ClipRect { rect } => {
                let world = world_transform.transform_rect_bbox(*rect);
                layer
                    .children
                    .iter()
                    .filter_map(|child| self.subtree_bounds(*child, world_transform))
                    .reduce(union_rect)
                    .and_then(|bounds| bounds.intersection(world))
            }
            LayerKind::Opacity { .. } => layer
                .children
                .iter()
                .filter_map(|child| self.subtree_bounds(*child, world_transform))
                .reduce(union_rect),
            LayerKind::Blur { .. } | LayerKind::DropShadow { .. } => layer
                .children
                .iter()
                .filter_map(|child| self.subtree_bounds(*child, world_transform))
                .reduce(union_rect),
            LayerKind::ColorFilter { .. } | LayerKind::Blend { .. } => layer
                .children
                .iter()
                .filter_map(|child| self.subtree_bounds(*child, world_transform))
                .reduce(union_rect),
        }
    }
    #[must_use]
    pub fn debug_tree(&self) -> String {
        let mut out = String::new();
        if let Some(root) = self.root {
            self.write_debug(root, 0, &mut out);
        }
        out
    }
    fn write_debug(&self, id: LayerId, depth: usize, out: &mut String) {
        self.write_debug_at(id, None, depth, Transform::IDENTITY, None, out);
    }
    fn write_debug_at(
        &self,
        id: LayerId,
        parent: Option<LayerId>,
        depth: usize,
        world_transform: Transform,
        clip: Option<Rect>,
        out: &mut String,
    ) {
        let Some(layer) = self.layers.get(id.0) else {
            return;
        };
        let indent = "  ".repeat(depth);
        let kind = match &layer.kind {
            LayerKind::Picture { bounds, .. } => format!(
                "Picture(local_bounds={bounds:?}, world_bounds={:?}, clip={clip:?})",
                world_transform.transform_rect_bbox(*bounds)
            ),
            LayerKind::Transform { transform } => {
                format!(
                    "Transform(local={:?}, world={:?})",
                    transform,
                    world_transform.then(*transform)
                )
            }
            LayerKind::ClipRect { rect } => format!(
                "ClipRect(local={rect:?}, world={:?})",
                world_transform.transform_rect_bbox(*rect)
            ),
            LayerKind::Opacity { alpha } => format!(
                "Opacity(alpha={alpha:.3}, bounds={:?}, generation={})",
                self.subtree_bounds(id, world_transform),
                self.subtree_generation(id)
            ),
            LayerKind::Blur { blur } => format!(
                "Blur(sigma=({:.3},{:.3}), bounds={:?}, generation={})",
                blur.sigma_x,
                blur.sigma_y,
                self.subtree_bounds(id, world_transform),
                self.subtree_generation(id)
            ),
            LayerKind::DropShadow { shadow } => format!(
                "DropShadow(sigma=({:.3},{:.3}), offset={:?}, bounds={:?}, generation={})",
                shadow.sigma_x,
                shadow.sigma_y,
                shadow.offset,
                self.subtree_bounds(id, world_transform),
                self.subtree_generation(id)
            ),
            LayerKind::ColorFilter { filter } => format!(
                "ColorFilter(matrix={:?}, bounds={:?}, generation={})",
                filter.to_matrix(),
                self.subtree_bounds(id, world_transform),
                self.subtree_generation(id)
            ),
            LayerKind::Blend { mode } => format!(
                "Blend(mode={mode:?}, bounds={:?}, generation={})",
                self.subtree_bounds(id, world_transform),
                self.subtree_generation(id)
            ),
        };
        out.push_str(&format!(
            "{indent}{id:?} parent={parent:?} {kind}, dirty={:?}\n",
            layer.dirty
        ));
        let (next_translation, next_clip) = match layer.kind {
            LayerKind::Picture { .. } => (world_transform, clip),
            LayerKind::Transform { transform: local } => (world_transform.then(local), clip),
            LayerKind::ClipRect { rect } => {
                let world = world_transform.transform_rect_bbox(rect);
                (
                    world_transform,
                    Some(clip.map_or(world, |old| old.intersection(world).unwrap_or(world))),
                )
            }
            LayerKind::Opacity { .. } => (world_transform, clip),
            LayerKind::Blur { .. } | LayerKind::DropShadow { .. } => (world_transform, clip),
            LayerKind::ColorFilter { .. } | LayerKind::Blend { .. } => (world_transform, clip),
        };
        for child in &layer.children {
            self.write_debug_at(
                *child,
                Some(id),
                depth + 1,
                next_translation,
                next_clip,
                out,
            );
        }
    }
}

fn union_rect(left: Rect, right: Rect) -> Rect {
    let x0 = left.origin.x.min(right.origin.x);
    let y0 = left.origin.y.min(right.origin.y);
    let x1 = (left.origin.x + left.size.width).max(right.origin.x + right.size.width);
    let y1 = (left.origin.y + left.size.height).max(right.origin.y + right.size.height);
    Rect::from_origin_size(Offset::new(x0, y0), Size::new(x1 - x0, y1 - y0))
}
#[cfg(test)]
mod tests {
    use super::*;
    use incular_core::{Offset, Size};
    #[test]
    fn commands_keep_painter_order() {
        let mut c = Canvas::default();
        c.rect(
            Rect::from_origin_size(Offset::ZERO, Size::new(1., 1.)),
            Color::WHITE,
        );
        assert!(matches!(
            c.finish().commands()[0],
            PaintCommand::Rect { .. }
        ));
    }
    #[test]
    fn radii_normalize_coherently_across_opposing_edges() {
        let r = CornerRadii {
            top_left: 80.,
            top_right: 80.,
            bottom_right: 20.,
            bottom_left: 20.,
        }
        .normalized(Size::new(100., 60.));
        assert!(r.top_left + r.top_right <= 100.);
        assert_eq!(r.top_left + r.bottom_left, 60.);
    }
    #[test]
    fn gradient_stops_are_defined_for_empty_unsorted_and_single_inputs() {
        assert_eq!(GradientStops::new(Vec::new()).as_slice().len(), 2);
        let stops = GradientStops::new(vec![
            GradientStop {
                offset: 2.,
                color: Color::WHITE,
            },
            GradientStop {
                offset: -1.,
                color: Color::BLACK,
            },
        ]);
        assert_eq!(stops.as_slice()[0].offset, 0.);
        assert_eq!(stops.as_slice()[1].offset, 1.);
    }
    #[test]
    fn path_bounds_are_tight_bezier_geometry_not_control_boxes() {
        let mut path = Path::builder();
        path.move_to(Offset::new(2., 3.)).cubic_to(
            Offset::new(-4., 8.),
            Offset::new(10., -2.),
            Offset::new(5., 6.),
        );
        let bounds = path.build().bounds().expect("curve has bounds");
        assert!(bounds.origin.x > -4. && bounds.origin.y > -2.);
        assert!(bounds.size.width < 14. && bounds.size.height < 10.);
    }
    #[test]
    fn path_identity_is_stable_for_clones_and_unique_for_new_geometry() {
        let mut builder = Path::builder();
        builder.move_to(Offset::ZERO).line_to(Offset::new(1., 1.));
        let path = builder.build();
        assert_eq!(path.id(), path.clone().id());
        let mut replacement = Path::builder();
        replacement
            .move_to(Offset::ZERO)
            .line_to(Offset::new(1., 1.));
        assert_ne!(path.id(), replacement.build().id());
    }
    #[test]
    fn retained_transform_reuses_picture_payload_and_accumulates_offsets() {
        let mut tree = LayerTree::new();
        let mut picture = DisplayList::new();
        picture.push(PaintCommand::Rect {
            rect: Rect::from_origin_size(Offset::ZERO, Size::new(5., 5.)),
            color: Color::WHITE,
        });
        let leaf = tree.create_picture(
            picture,
            Rect::from_origin_size(Offset::ZERO, Size::new(5., 5.)),
        );
        let child = tree.create_transform(Transform::translation(Offset::new(5., 7.)));
        let parent = tree.create_transform(Transform::translation(Offset::new(10., 20.)));
        tree.set_children(child, vec![leaf]);
        tree.set_children(parent, vec![child]);
        tree.set_root(parent);
        let before = tree.diagnostics().picture_layers_repainted;
        let list = tree.flatten();
        assert_eq!(tree.diagnostics().picture_layers_repainted, before);
        assert!(list.commands().iter().any(|command| matches!(command, PaintCommand::PushTransform { transform } if transform.translation_offset() == Offset::new(15., 27.))));
        assert!(tree.update_transform(parent, Transform::translation(Offset::new(11., 20.))));
    }
    #[test]
    fn nested_clips_cull_fully_outside_picture() {
        let mut tree = LayerTree::new();
        let picture = tree.create_picture(
            DisplayList::new(),
            Rect::from_origin_size(Offset::new(20., 20.), Size::new(2., 2.)),
        );
        let clip = tree.create_clip_rect(Rect::from_origin_size(Offset::ZERO, Size::new(10., 10.)));
        tree.set_children(clip, vec![picture]);
        tree.set_root(clip);
        let _ = tree.flatten();
        assert_eq!(tree.diagnostics().layers_culled, 1);
    }
    #[test]
    fn world_bounds_and_clips_use_the_same_accumulated_coordinates() {
        let mut tree = LayerTree::new();
        let picture = tree.create_picture(
            DisplayList::new(),
            Rect::from_origin_size(Offset::new(2., 3.), Size::new(10., 10.)),
        );
        let placement = tree.create_transform(Transform::translation(Offset::new(20., 30.)));
        let clip = tree.create_clip_rect(Rect::from_origin_size(
            Offset::new(15., 25.),
            Size::new(20., 20.),
        ));
        tree.set_children(clip, vec![placement]);
        tree.set_children(placement, vec![picture]);
        tree.set_root(clip);
        let _ = tree.flatten();
        assert_eq!(
            tree.flattened_pictures(),
            &[FlattenedPicture {
                layer: picture,
                local_bounds: Rect::from_origin_size(Offset::new(2., 3.), Size::new(10., 10.)),
                world_bounds: Rect::from_origin_size(Offset::new(22., 33.), Size::new(10., 10.)),
                active_clip: Some(Rect::from_origin_size(
                    Offset::new(15., 25.),
                    Size::new(20., 20.),
                )),
            }]
        );
    }

    #[test]
    fn opacity_normalization_is_finite_and_bounded() {
        assert_eq!(normalize_opacity(f32::NAN), 0.);
        assert_eq!(normalize_opacity(f32::NEG_INFINITY), 0.);
        assert_eq!(normalize_opacity(f32::INFINITY), 0.);
        assert_eq!(normalize_opacity(-1.), 0.);
        assert_eq!(normalize_opacity(2.), 1.);
        assert_eq!(normalize_opacity(0.35), 0.35);
    }

    #[test]
    fn opacity_layer_isolated_group_is_retained_and_updates_composite_only() {
        let mut tree = LayerTree::new();
        let mut picture = DisplayList::new();
        picture.push(PaintCommand::Rect {
            rect: Rect::from_origin_size(Offset::ZERO, Size::new(20., 20.)),
            color: Color::WHITE,
        });
        let leaf = tree.create_picture(
            picture,
            Rect::from_origin_size(Offset::ZERO, Size::new(20., 20.)),
        );
        let opacity = tree.create_opacity(0.5);
        tree.set_children(opacity, vec![leaf]);
        tree.set_root(opacity);
        let first = tree.flatten();
        assert!(matches!(
            first.commands().first(),
            Some(PaintCommand::PushOpacity { alpha, .. }) if *alpha == 0.5
        ));
        assert!(matches!(
            first.commands().last(),
            Some(PaintCommand::PopOpacity)
        ));
        let first_generation = match first.commands().first() {
            Some(PaintCommand::PushOpacity { generation, .. }) => *generation,
            _ => unreachable!("opacity command missing"),
        };
        let before = tree.diagnostics().opacity_updates;
        assert!(tree.update_opacity(opacity, 0.75));
        assert_eq!(tree.diagnostics().opacity_updates, before + 1);
        let second = tree.flatten();
        assert!(matches!(
            second.commands().first(),
            Some(PaintCommand::PushOpacity { alpha, .. }) if *alpha == 0.75
        ));
        let second_generation = match second.commands().first() {
            Some(PaintCommand::PushOpacity { generation, .. }) => *generation,
            _ => unreachable!("opacity command missing"),
        };
        assert_eq!(first_generation, second_generation);
    }

    #[test]
    fn nested_opacity_alpha_invalidates_only_the_outer_content_key() {
        let mut tree = LayerTree::new();
        let leaf = tree.create_picture(
            DisplayList::new(),
            Rect::from_origin_size(Offset::ZERO, Size::new(20., 20.)),
        );
        let inner = tree.create_opacity(0.5);
        let outer = tree.create_opacity(0.5);
        tree.set_children(inner, vec![leaf]);
        tree.set_children(outer, vec![inner]);
        tree.set_root(outer);

        let first = tree.flatten();
        let generations = |list: &DisplayList| {
            list.commands()
                .iter()
                .filter_map(|command| match command {
                    PaintCommand::PushOpacity {
                        layer, generation, ..
                    } => Some((*layer, *generation)),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        let first_generations = generations(&first);
        assert_eq!(first_generations.len(), 2);

        assert!(tree.update_opacity(inner, 0.75));
        let second_generations = generations(&tree.flatten());
        assert_eq!(second_generations.len(), 2);
        assert_eq!(first_generations[1].0, second_generations[1].0);
        assert_eq!(first_generations[1].1, second_generations[1].1);
        assert_ne!(first_generations[0].1, second_generations[0].1);
    }

    #[test]
    fn gaussian_kernel_is_normalized_and_symmetric() {
        for sigma in [0., 0.5, 2., 8., 32.] {
            let weights = gaussian_kernel_weights(sigma);
            let sum: f32 = weights.iter().sum();
            assert!((sum - 1.).abs() < 1e-5, "sigma={sigma} sum={sum}");
            for (left, right) in weights.iter().zip(weights.iter().rev()) {
                assert!((left - right).abs() < 1e-6);
            }
        }
    }

    #[test]
    fn effect_bounds_expand_outward_for_asymmetric_sigma_and_offsets() {
        let source = Rect::from_origin_size(Offset::new(10., 20.), Size::new(30., 40.));
        let blur = blur_bounds(source, 2., 4.);
        assert_eq!(blur.origin, Offset::new(4., 8.));
        assert_eq!(blur.size, Size::new(42., 64.));
        let shadow = drop_shadow_bounds(source, Offset::new(-9., 7.), 2., 4.);
        assert_eq!(shadow.origin, Offset::new(-5., 15.));
        assert_eq!(shadow.size, Size::new(45., 64.));
    }

    #[test]
    fn effect_bounds_remain_conservative_for_fractional_dpi_equivalents() {
        let source = Rect::from_origin_size(Offset::new(0.25, 1.75), Size::new(12.5, 9.25));
        let logical_sigma = 2.25;
        let physical_sigma = logical_sigma * 1.5;
        let physical_margin = 3. * physical_sigma;
        let logical_margin = physical_margin / 1.5;
        assert_eq!(
            blur_bounds(source, logical_sigma, logical_sigma),
            Rect::from_origin_size(
                Offset::new(
                    source.origin.x - logical_margin,
                    source.origin.y - logical_margin
                ),
                Size::new(
                    source.size.width + 2. * logical_margin,
                    source.size.height + 2. * logical_margin,
                ),
            )
        );
    }

    #[test]
    fn blur_and_shadow_layers_keep_source_generation_stable_for_parameter_updates() {
        let mut tree = LayerTree::new();
        let leaf = tree.create_picture(
            DisplayList::new(),
            Rect::from_origin_size(Offset::ZERO, Size::new(20., 20.)),
        );
        let blur = tree.create_blur(GaussianBlur::uniform(4.));
        tree.set_children(blur, vec![leaf]);
        tree.set_root(blur);
        let first = tree.flatten();
        let first_generation = match first.commands().first() {
            Some(PaintCommand::PushBlur { generation, .. }) => *generation,
            _ => panic!("blur command missing"),
        };
        assert!(tree.update_blur(blur, GaussianBlur::uniform(8.)));
        let second = tree.flatten();
        assert_eq!(
            first_generation,
            match second.commands().first() {
                Some(PaintCommand::PushBlur { generation, .. }) => *generation,
                _ => panic!("blur command missing"),
            }
        );

        let shadow = tree.create_drop_shadow(DropShadowEffect::new(
            Offset::new(0., 4.),
            5.,
            Color::rgba(0, 0, 0, 128),
        ));
        tree.set_children(shadow, vec![blur]);
        tree.set_root(shadow);
        let first_shadow = tree.flatten();
        let first_shadow_generation = match first_shadow.commands().first() {
            Some(PaintCommand::PushDropShadow { generation, .. }) => *generation,
            _ => panic!("shadow command missing"),
        };
        assert!(tree.update_drop_shadow(
            shadow,
            DropShadowEffect::new(Offset::new(8., -2.), 5., Color::rgba(200, 20, 40, 90),)
        ));
        let second_shadow = tree.flatten();
        assert_eq!(
            first_shadow_generation,
            match second_shadow.commands().first() {
                Some(PaintCommand::PushDropShadow { generation, .. }) => *generation,
                _ => panic!("shadow command missing"),
            }
        );
    }

    #[test]
    fn rounded_rect_contains_uses_normalized_corner_arcs() {
        let rect = RRect::uniform(
            Rect::from_origin_size(Offset::ZERO, Size::new(20., 20.)),
            8.,
        );
        assert!(rect.contains(Offset::new(10., 10.)));
        assert!(rect.contains(Offset::new(0., 10.)));
        assert!(rect.contains(Offset::new(3., 3.)));
        assert!(!rect.contains(Offset::new(1., 1.)));
        assert!(rect.contains(Offset::new(8., 0.)));
    }

    #[test]
    fn path_contains_honors_even_odd_holes() {
        let mut b = Path::builder();
        b.move_to(Offset::new(0., 0.))
            .line_to(Offset::new(20., 0.))
            .line_to(Offset::new(20., 20.))
            .line_to(Offset::new(0., 20.))
            .close();
        b.move_to(Offset::new(5., 5.))
            .line_to(Offset::new(15., 5.))
            .line_to(Offset::new(15., 15.))
            .line_to(Offset::new(5., 15.))
            .close();
        let path = b.build();
        assert!(path.contains(Offset::new(2., 2.), FillRule::EvenOdd));
        assert!(!path.contains(Offset::new(10., 10.), FillRule::EvenOdd));
    }
    #[test]
    fn gradient_sampling_preserves_middle_stops_duplicate_edges_and_alpha() {
        let stops = GradientStops::new(vec![
            GradientStop {
                offset: 0.,
                color: Color::rgba(255, 0, 0, 255),
            },
            GradientStop {
                offset: 0.5,
                color: Color::rgba(0, 255, 0, 0),
            },
            GradientStop {
                offset: 1.,
                color: Color::rgba(0, 0, 255, 255),
            },
        ]);
        assert_eq!(sample_gradient_stops(&stops, 0.), [1., 0., 0., 1.]);
        assert_eq!(sample_gradient_stops(&stops, 1.), [0., 0., 1., 1.]);
        let quarter = sample_gradient_stops(&stops, 0.25);
        assert!(
            quarter[0] > 0.4 && quarter[3] > 0.4,
            "premultiplied transparent transition"
        );
        let hard = GradientStops::new(vec![
            GradientStop {
                offset: 0.,
                color: Color::rgba(255, 0, 0, 255),
            },
            GradientStop {
                offset: 0.5,
                color: Color::rgba(255, 0, 0, 255),
            },
            GradientStop {
                offset: 0.5,
                color: Color::rgba(0, 0, 255, 255),
            },
            GradientStop {
                offset: 1.,
                color: Color::rgba(0, 0, 255, 255),
            },
        ]);
        assert_eq!(sample_gradient_stops(&hard, 0.5), [0., 0., 1., 1.]);
        assert_eq!(sample_gradient_stops(&hard, -1.), [1., 0., 0., 1.]);
        assert_eq!(sample_gradient_stops(&hard, 2.), [0., 0., 1., 1.]);
    }
    #[test]
    fn gradient_local_geometry_handles_regular_and_degenerate_cases() {
        assert_eq!(
            linear_gradient_t(Offset::ZERO, Offset::new(10., 0.), Offset::new(2.5, 4.)),
            0.25
        );
        assert_eq!(
            linear_gradient_t(Offset::ZERO, Offset::ZERO, Offset::new(2., 2.)),
            1.
        );
        assert_eq!(
            radial_gradient_t(Offset::ZERO, 10., Offset::new(3., 4.)),
            0.5
        );
        assert_eq!(radial_gradient_t(Offset::ZERO, 0., Offset::ZERO), 1.);
    }

    #[test]
    fn sweep_gradient_wraps_angles_and_canvas_records_oval_clips() {
        assert!((sweep_gradient_t(Offset::ZERO, 0., Offset::new(1., 0.))).abs() < 1e-6);
        assert!((sweep_gradient_t(Offset::ZERO, 0., Offset::new(0., 1.)) - 0.25).abs() < 1e-6);
        let mut canvas = Canvas::default();
        canvas.save_clip_oval(Rect::from_origin_size(Offset::ZERO, Size::new(40., 20.)));
        canvas.restore();
        assert!(matches!(
            canvas.finish().commands()[0],
            PaintCommand::PushClipOval { .. }
        ));
    }

    #[test]
    fn color_filter_helpers_cover_identity_alpha_and_common_adjustments() {
        let sample = [0.2, 0.4, 0.8, 0.5];
        assert_eq!(ColorFilter::identity().apply(sample), sample);
        assert!(ColorFilter::grayscale(0.).is_identity());
        let gray = ColorFilter::grayscale(1.).apply(sample);
        assert!((gray[0] - gray[1]).abs() < 1e-6);
        assert!((gray[1] - gray[2]).abs() < 1e-6);
        assert_eq!(ColorFilter::brightness(0.).apply(sample), [0., 0., 0., 0.5]);
        assert!(ColorFilter::contrast(1.).is_identity());
        assert!(ColorFilter::saturate(1.).is_identity());
        let inverted = ColorFilter::invert(1.).apply(sample);
        assert!((inverted[0] - 0.8).abs() < 1e-6);
        assert!((inverted[1] - 0.6).abs() < 1e-6);
        assert!((inverted[2] - 0.2).abs() < 1e-6);
        assert_eq!(ColorFilter::opacity(0.).apply(sample), [0., 0., 0., 0.]);
        let transparent = ColorFilter::invert(1.).apply([1., 0.5, 0.25, 0.]);
        assert_eq!(transparent, [0., 0., 0., 0.]);
        for value in ColorFilter::matrix([f32::NAN; 20]).apply(sample) {
            assert!(value.is_finite());
        }
    }

    #[test]
    fn color_filter_composition_preserves_followed_by_order_and_bias() {
        let first = ColorFilter::matrix([
            0.5, 0., 0., 0., 0.1, 0., 0.5, 0., 0., 0.2, 0., 0., 0.5, 0., 0.3, 0., 0., 0., 1., 0.,
        ]);
        let second = ColorFilter::matrix([
            1., 0., 0., 0., 0.05, 0., 1., 0., 0., 0.06, 0., 0., 1., 0., 0.07, 0., 0., 0., 1., 0.,
        ]);
        let input = [0.4, 0.5, 0.6, 0.8];
        let sequential = second.apply(first.apply(input));
        let composed = first.then(second).apply(input);
        for (left, right) in sequential.into_iter().zip(composed) {
            assert!((left - right).abs() < 1e-6, "{left} != {right}");
        }
        assert_eq!(first.compose(second), first.then(second));
    }

    #[test]
    fn effect_chain_fuses_only_adjacent_color_matrices() {
        let chain = EffectChain::new()
            .color_filter(ColorFilter::grayscale(1.))
            .color_filter(ColorFilter::sepia(0.5))
            .blur(8.)
            .color_filter(ColorFilter::contrast(1.2));
        let optimized = chain.optimized();
        assert_eq!(chain.fusion_count(), 1);
        assert_eq!(optimized.effects().len(), 3);
        assert!(matches!(optimized.effects()[0], Effect::ColorMatrix(_)));
        assert!(matches!(optimized.effects()[1], Effect::GaussianBlur(_)));
        assert!(matches!(optimized.effects()[2], Effect::ColorMatrix(_)));
    }

    #[test]
    fn blend_reference_is_finite_for_transparent_and_semitransparent_pixels() {
        let modes = [
            BlendMode::SrcOver,
            BlendMode::Src,
            BlendMode::DstOver,
            BlendMode::SrcIn,
            BlendMode::DstIn,
            BlendMode::SrcOut,
            BlendMode::DstOut,
            BlendMode::SrcAtop,
            BlendMode::DstAtop,
            BlendMode::Xor,
            BlendMode::Plus,
            BlendMode::Multiply,
            BlendMode::Screen,
            BlendMode::Overlay,
            BlendMode::Darken,
            BlendMode::Lighten,
            BlendMode::ColorDodge,
            BlendMode::ColorBurn,
            BlendMode::HardLight,
            BlendMode::SoftLight,
            BlendMode::Difference,
            BlendMode::Exclusion,
        ];
        for mode in modes {
            for (source, destination) in [
                ([0., 0., 0., 0.], [0., 0., 0., 0.]),
                ([0.2, 0.1, 0.05, 0.5], [0.3, 0.2, 0.1, 0.5]),
                ([0.1, 0.3, 0.2, 1.], [0.4, 0.1, 0.7, 1.]),
            ] {
                let output = blend_premultiplied(mode, source, destination);
                assert!(output.iter().all(|value| value.is_finite()));
                assert!(output[..3].iter().all(|value| *value <= output[3] + 1e-6));
            }
        }
    }

    #[test]
    fn color_filter_and_blend_updates_keep_source_generations_stable() {
        let mut tree = LayerTree::new();
        let leaf = tree.create_picture(
            DisplayList::new(),
            Rect::from_origin_size(Offset::ZERO, Size::new(20., 20.)),
        );
        let filter = tree.create_color_filter(ColorFilter::grayscale(1.));
        let blend = tree.create_blend(BlendMode::Multiply);
        tree.set_children(filter, vec![leaf]);
        tree.set_children(blend, vec![filter]);
        tree.set_root(blend);
        let first = tree.flatten();
        let generations = first
            .commands()
            .iter()
            .filter_map(|command| match command {
                PaintCommand::PushColorFilter {
                    layer, generation, ..
                }
                | PaintCommand::PushBlend {
                    layer, generation, ..
                } => Some((*layer, *generation)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(generations.len(), 2);
        assert!(tree.update_color_filter(filter, ColorFilter::sepia(1.)));
        assert!(tree.update_blend(blend, BlendMode::Screen));
        let second = tree.flatten();
        let updated = second
            .commands()
            .iter()
            .filter_map(|command| match command {
                PaintCommand::PushColorFilter {
                    layer, generation, ..
                }
                | PaintCommand::PushBlend {
                    layer, generation, ..
                } => Some((*layer, *generation)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(generations[1], updated[1]);
        assert_ne!(generations[0], updated[0]);
    }

    #[test]
    fn ordered_effect_generations_invalidate_only_downstream_stages() {
        fn generations(list: &DisplayList) -> Vec<(LayerId, u64)> {
            list.commands()
                .iter()
                .filter_map(|command| match command {
                    PaintCommand::PushBlur {
                        layer, generation, ..
                    }
                    | PaintCommand::PushColorFilter {
                        layer, generation, ..
                    } => Some((*layer, *generation)),
                    _ => None,
                })
                .collect()
        }

        let make_picture = |tree: &mut LayerTree| {
            tree.create_picture(
                DisplayList::new(),
                Rect::from_origin_size(Offset::ZERO, Size::new(20., 20.)),
            )
        };

        // Color -> blur: the blur's input generation includes the color stage.
        let mut color_then_blur = LayerTree::new();
        let source = make_picture(&mut color_then_blur);
        let color = color_then_blur.create_color_filter(ColorFilter::grayscale(0.));
        let blur = color_then_blur.create_blur(GaussianBlur::uniform(8.));
        color_then_blur.set_children(color, vec![source]);
        color_then_blur.set_children(blur, vec![color]);
        color_then_blur.set_root(blur);
        let before = generations(&color_then_blur.flatten());
        assert!(color_then_blur.update_color_filter(color, ColorFilter::grayscale(1.)));
        let after = generations(&color_then_blur.flatten());
        assert_ne!(before[0].1, after[0].1);
        assert_eq!(before[1].1, after[1].1);

        // Blur -> color: changing the downstream matrix does not invalidate
        // the upstream blur source generation.
        let mut blur_then_color = LayerTree::new();
        let source = make_picture(&mut blur_then_color);
        let blur = blur_then_color.create_blur(GaussianBlur::uniform(8.));
        let color = blur_then_color.create_color_filter(ColorFilter::grayscale(0.));
        blur_then_color.set_children(blur, vec![source]);
        blur_then_color.set_children(color, vec![blur]);
        blur_then_color.set_root(color);
        let before = generations(&blur_then_color.flatten());
        assert!(blur_then_color.update_color_filter(color, ColorFilter::grayscale(1.)));
        let after = generations(&blur_then_color.flatten());
        assert_eq!(before[0].1, after[0].1);
        assert_eq!(before[1].1, after[1].1);
    }

    #[test]
    fn kurbo_provides_tight_bezier_bounds_and_winding() {
        let mut builder = Path::builder();
        builder
            .move_to(Offset::new(0., 0.))
            .quadratic_to(Offset::new(10., 20.), Offset::new(20., 0.))
            .line_to(Offset::new(0., 0.))
            .close();
        let path = builder.build();
        let bounds = path.bounds().expect("non-empty path");
        // The control point reaches y=20, while the actual quadratic maximum
        // is y=10. This guards against restoring the old control-point box.
        assert!((bounds.size.height - 10.).abs() < 0.0001, "{bounds:?}");
        assert!(path.contains(Offset::new(10., 5.), FillRule::NonZero));
        assert!(!path.contains(Offset::new(10., 12.), FillRule::NonZero));
    }

    #[test]
    fn retained_layers_compose_affines_for_world_bounds() {
        let mut tree = LayerTree::new();
        let picture = tree.create_picture(
            DisplayList::new(),
            Rect::from_origin_size(Offset::ZERO, Size::new(10., 20.)),
        );
        let rotate = tree.create_transform(Transform::rotation(std::f32::consts::FRAC_PI_2));
        tree.set_children(rotate, vec![picture]);
        tree.set_root(rotate);
        let _ = tree.flatten();
        let bounds = tree.flattened_pictures()[0].world_bounds;
        assert!((bounds.size.width - 20.).abs() < 0.0001);
        assert!((bounds.size.height - 10.).abs() < 0.0001);
    }
}
