use crate::paint::FillRule;
use incular_core::{Offset, Rect, Size, Transform};
use kurbo::{BezPath, Point, Shape};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

static NEXT_PATH_ID: AtomicU64 = AtomicU64::new(1);

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

    /// Returns a copy of this path with an affine transform applied to its
    /// geometry.  Path identity is intentionally not reused: the transformed
    /// geometry is a distinct immutable mesh candidate.  This is used by
    /// higher-level icon primitives to fit a canonical 24px path into the
    /// requested logical size without changing pointer/layout coordinates.
    #[must_use]
    pub fn transformed(&self, transform: Transform) -> Self {
        let mut path = (*self.path).clone();
        path.apply_affine(transform.to_kurbo());
        let bounds = (!path.is_empty() && path.is_finite()).then(|| {
            let bounds = path.bounding_box();
            Rect::from_origin_size(
                Offset::new(bounds.x0 as f32, bounds.y0 as f32),
                Size::new(
                    (bounds.x1 - bounds.x0) as f32,
                    (bounds.y1 - bounds.y0) as f32,
                ),
            )
        });
        Self {
            id: PathId(NEXT_PATH_ID.fetch_add(1, Ordering::Relaxed)),
            path: Arc::new(path),
            bounds,
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
