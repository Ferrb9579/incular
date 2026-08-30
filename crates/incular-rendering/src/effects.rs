use crate::geometry::union_rect;
use incular_core::{Color, Offset, Rect, Size};

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
