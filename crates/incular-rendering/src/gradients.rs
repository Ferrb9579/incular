use crate::display_list::ImageSampling;
use incular_core::{Color, Offset};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

static NEXT_GRADIENT_ID: AtomicU64 = AtomicU64::new(1);

/// Stable identity for immutable normalized gradient stop data.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct GradientId(u64);

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
impl GradientId {
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
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

/// A renderer-neutral shader value.
///
/// Incular's gradients are already immutable, cacheable shader descriptions;
/// keeping this alias means widgets and text do not need a second shader
/// hierarchy that would eventually drift from the renderer's brush model.
pub type Shader = Brush;

/// Image filtering policy matching Flutter's public quality vocabulary.
///
/// The current display-list backend exposes nearest-neighbour and linear
/// sampling. `Low`, `Medium`, and `High` intentionally lower to linear until
/// a backend grows a more expensive filter; callers still retain a stable,
/// renderer-independent policy value.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FilterQuality {
    None,
    #[default]
    Low,
    Medium,
    High,
}

impl FilterQuality {
    #[must_use]
    pub const fn sampling(self) -> ImageSampling {
        match self {
            Self::None => ImageSampling::Nearest,
            Self::Low | Self::Medium | Self::High => ImageSampling::Linear,
        }
    }
}

impl From<FilterQuality> for ImageSampling {
    fn from(value: FilterQuality) -> Self {
        value.sampling()
    }
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
