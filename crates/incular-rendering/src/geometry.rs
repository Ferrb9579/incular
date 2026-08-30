use incular_core::{Offset, Rect, Size};

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

pub(crate) fn union_rect(left: Rect, right: Rect) -> Rect {
    let x0 = left.origin.x.min(right.origin.x);
    let y0 = left.origin.y.min(right.origin.y);
    let x1 = (left.origin.x + left.size.width).max(right.origin.x + right.size.width);
    let y1 = (left.origin.y + left.size.height).max(right.origin.y + right.size.height);
    Rect::from_origin_size(Offset::new(x0, y0), Size::new(x1 - x0, y1 - y0))
}
