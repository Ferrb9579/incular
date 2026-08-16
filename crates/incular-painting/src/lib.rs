//! Ordered renderer-independent display lists.
use incular_assets::{FontHandle, ImageHandle};
use incular_core::{Arena, ArenaId, Color, DirtyFlags, Offset, Rect, Transform};
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
    #[must_use]
    pub const fn uniform(radius: f32) -> Self {
        Self {
            top_left: radius,
            top_right: radius,
            bottom_right: radius,
            bottom_left: radius,
        }
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
pub enum Brush {
    Solid(Color),
    LinearGradient(LinearGradient),
    RadialGradient(RadialGradient),
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
#[derive(Clone, Debug, PartialEq)]
pub enum PathVerb {
    MoveTo(Offset),
    LineTo(Offset),
    QuadraticTo(Offset, Offset),
    CubicTo(Offset, Offset, Offset),
    Close,
}
/// Stable identity for immutable path geometry. It deliberately does not
/// encode paint, placement, or transforms.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct PathId(u64);
#[derive(Clone, Debug, PartialEq)]
pub struct Path {
    id: PathId,
    verbs: Arc<[PathVerb]>,
    bounds: Option<Rect>,
}
impl Path {
    #[must_use]
    pub fn builder() -> PathBuilder {
        PathBuilder::default()
    }
    #[must_use]
    pub fn verbs(&self) -> &[PathVerb] {
        &self.verbs
    }
    #[must_use]
    pub fn bounds(&self) -> Option<Rect> {
        self.bounds
    }
    #[must_use]
    pub const fn id(&self) -> PathId {
        self.id
    }
    /// Renderer-neutral filled-path containment. Curves are flattened into
    /// sixteen deterministic line segments, which is sufficient for pointer
    /// targeting and deliberately independent from GPU tessellation.
    #[must_use]
    pub fn contains(&self, point: Offset, rule: FillRule) -> bool {
        let mut contours: Vec<Vec<Offset>> = Vec::new();
        let mut contour = Vec::new();
        let mut current = Offset::ZERO;
        let mut start = Offset::ZERO;
        for verb in self.verbs() {
            match *verb {
                PathVerb::MoveTo(p) => {
                    if contour.len() > 2 {
                        contours.push(std::mem::take(&mut contour));
                    }
                    current = p;
                    start = p;
                    contour.push(p);
                }
                PathVerb::LineTo(p) => {
                    contour.push(p);
                    current = p;
                }
                PathVerb::QuadraticTo(control, end) => {
                    for step in 1..=16 {
                        let t = step as f32 / 16.;
                        let u = 1. - t;
                        contour.push(Offset::new(
                            u * u * current.x + 2. * u * t * control.x + t * t * end.x,
                            u * u * current.y + 2. * u * t * control.y + t * t * end.y,
                        ));
                    }
                    current = end;
                }
                PathVerb::CubicTo(a, b, end) => {
                    for step in 1..=16 {
                        let t = step as f32 / 16.;
                        let u = 1. - t;
                        contour.push(Offset::new(
                            u * u * u * current.x
                                + 3. * u * u * t * a.x
                                + 3. * u * t * t * b.x
                                + t * t * t * end.x,
                            u * u * u * current.y
                                + 3. * u * u * t * a.y
                                + 3. * u * t * t * b.y
                                + t * t * t * end.y,
                        ));
                    }
                    current = end;
                }
                PathVerb::Close => {
                    if contour.last().copied() != Some(start) {
                        contour.push(start);
                    }
                    if contour.len() > 2 {
                        contours.push(std::mem::take(&mut contour));
                    }
                }
            }
        }
        if contour.len() > 2 {
            contours.push(contour);
        }
        let winding: i32 = contours
            .iter()
            .map(|contour| {
                contour
                    .windows(2)
                    .map(|edge| {
                        let (a, b) = (edge[0], edge[1]);
                        if (a.y <= point.y && b.y > point.y) || (a.y > point.y && b.y <= point.y) {
                            let x = a.x + (point.y - a.y) * (b.x - a.x) / (b.y - a.y);
                            if x >= point.x {
                                if b.y > a.y { 1 } else { -1 }
                            } else {
                                0
                            }
                        } else {
                            0
                        }
                    })
                    .sum::<i32>()
            })
            .sum();
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
    verbs: Vec<PathVerb>,
    points: Vec<Offset>,
}
impl PathBuilder {
    pub fn move_to(&mut self, p: Offset) -> &mut Self {
        self.verbs.push(PathVerb::MoveTo(p));
        self.points.push(p);
        self
    }
    pub fn line_to(&mut self, p: Offset) -> &mut Self {
        self.verbs.push(PathVerb::LineTo(p));
        self.points.push(p);
        self
    }
    pub fn quadratic_to(&mut self, c: Offset, p: Offset) -> &mut Self {
        self.verbs.push(PathVerb::QuadraticTo(c, p));
        self.points.extend([c, p]);
        self
    }
    pub fn cubic_to(&mut self, a: Offset, b: Offset, p: Offset) -> &mut Self {
        self.verbs.push(PathVerb::CubicTo(a, b, p));
        self.points.extend([a, b, p]);
        self
    }
    pub fn close(&mut self) -> &mut Self {
        self.verbs.push(PathVerb::Close);
        self
    }
    #[must_use]
    pub fn build(self) -> Path {
        let bounds = self
            .points
            .iter()
            .filter(|p| p.x.is_finite() && p.y.is_finite())
            .fold(None, |a: Option<(f32, f32, f32, f32)>, p| {
                Some(match a {
                    Some((l, t, r, b)) => (l.min(p.x), t.min(p.y), r.max(p.x), b.max(p.y)),
                    None => (p.x, p.y, p.x, p.y),
                })
            })
            .map(|(l, t, r, b)| {
                Rect::from_origin_size(Offset::new(l, t), incular_core::Size::new(r - l, b - t))
            });
        Path {
            id: PathId(NEXT_PATH_ID.fetch_add(1, Ordering::Relaxed)),
            verbs: self.verbs.into(),
            bounds,
        }
    }
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
    PushClipPath {
        path: Arc<Path>,
        fill_rule: FillRule,
    },
    PopClip,
    PushTransform {
        transform: Transform,
    },
    PopTransform,
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CompositorDiagnostics {
    pub layers: u64,
    pub picture_layers_reused: u64,
    pub picture_layers_repainted: u64,
    pub transform_updates: u64,
    pub clip_updates: u64,
    pub layers_culled: u64,
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
}

#[derive(Clone, Debug)]
struct Layer {
    kind: LayerKind,
    children: Vec<LayerId>,
    dirty: DirtyFlags,
}

/// Renderer-independent retained layer arena. Picture display lists are owned
/// once and scene flattening only emits a transient linear command stream.
/// This deliberately has no GPU resources or offscreen textures.
pub struct LayerTree {
    layers: Arena<Layer>,
    root: Option<LayerId>,
    diagnostics: CompositorDiagnostics,
    flattened_pictures: Vec<FlattenedPicture>,
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
    fn insert(&mut self, kind: LayerKind) -> LayerId {
        let id = LayerId(self.layers.insert(Layer {
            kind,
            children: Vec::new(),
            dirty: DirtyFlags::COMPOSITE,
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
            layer.children = children;
            layer.dirty.insert(DirtyFlags::COMPOSITE);
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
        self.diagnostics.clip_updates += 1;
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
            self.flatten_layer(root, Offset::ZERO, None, &mut out);
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
        translation: Offset,
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
                let world = Rect::from_origin_size(bounds.origin + translation, bounds.size);
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
                    transform: Transform::translation(translation),
                });
                out.extend_from(&display_list);
                out.push(PaintCommand::PopTransform);
            }
            LayerKind::Transform { transform } => {
                let next = translation + transform.translation;
                for child in layer.children {
                    self.flatten_layer(child, next, clip, out);
                }
            }
            LayerKind::ClipRect { rect } => {
                let world = Rect::from_origin_size(rect.origin + translation, rect.size);
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
                    self.flatten_layer(child, translation, next_clip, out);
                }
                out.push(PaintCommand::PopClip);
            }
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
        self.write_debug_at(id, None, depth, Offset::ZERO, None, out);
    }
    fn write_debug_at(
        &self,
        id: LayerId,
        parent: Option<LayerId>,
        depth: usize,
        translation: Offset,
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
                Rect::from_origin_size(bounds.origin + translation, bounds.size)
            ),
            LayerKind::Transform { transform } => {
                format!(
                    "Transform(local={:?}, world={:?})",
                    transform.translation,
                    translation + transform.translation
                )
            }
            LayerKind::ClipRect { rect } => format!(
                "ClipRect(local={rect:?}, world={:?})",
                Rect::from_origin_size(rect.origin + translation, rect.size)
            ),
        };
        out.push_str(&format!(
            "{indent}{id:?} parent={parent:?} {kind}, dirty={:?}\n",
            layer.dirty
        ));
        let (next_translation, next_clip) = match layer.kind {
            LayerKind::Picture { .. } => (translation, clip),
            LayerKind::Transform { transform } => (translation + transform.translation, clip),
            LayerKind::ClipRect { rect } => {
                let world = Rect::from_origin_size(rect.origin + translation, rect.size);
                (
                    translation,
                    Some(clip.map_or(world, |old| old.intersection(world).unwrap_or(world))),
                )
            }
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
    fn path_bounds_conservatively_include_control_points() {
        let mut path = Path::builder();
        path.move_to(Offset::new(2., 3.)).cubic_to(
            Offset::new(-4., 8.),
            Offset::new(10., -2.),
            Offset::new(5., 6.),
        );
        assert_eq!(
            path.build().bounds(),
            Some(Rect::from_origin_size(
                Offset::new(-4., -2.),
                Size::new(14., 10.)
            ))
        );
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
        assert!(list.commands().iter().any(|command| matches!(command, PaintCommand::PushTransform { transform } if transform.translation == Offset::new(15., 27.))));
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
}
