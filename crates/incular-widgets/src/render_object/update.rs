//! Declarative render configuration updates and their phase invalidation.

use super::*;

/// Minimal retained work required after replacing declarative render
/// configuration.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct RenderInvalidation(u8);

impl RenderInvalidation {
    pub(crate) const NONE: Self = Self(0);
    pub(crate) const LAYOUT: Self = Self(1 << 0);
    pub(crate) const PAINT: Self = Self(1 << 1);
    pub(crate) const COMPOSITE: Self = Self(1 << 2);
    pub(crate) const SEMANTICS: Self = Self(1 << 3);

    pub(crate) const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

impl std::ops::BitOr for RenderInvalidation {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        self.union(rhs)
    }
}

impl RenderObjectPayload {
    /// Applies new declarative configuration and returns the minimum phase
    /// work required to make the retained object observable as that value.
    pub(crate) fn update_kind(&mut self, kind: RenderKind) -> RenderInvalidation {
        if self.kind == kind {
            return RenderInvalidation::NONE;
        }
        let invalidation = invalidation_for_change(&self.kind, &kind);
        self.replace_kind(kind);
        invalidation
    }
}

fn invalidation_for_change(old: &RenderKind, new: &RenderKind) -> RenderInvalidation {
    use RenderInvalidation as I;

    if same_compositor_family(old, new) {
        return I::COMPOSITE;
    }
    if paint_only_change(old, new) {
        return I::PAINT;
    }

    // Layout changes also invalidate local painting and semantic geometry.
    I::LAYOUT | I::PAINT | I::SEMANTICS
}

fn paint_only_change(old: &RenderKind, new: &RenderKind) -> bool {
    text_paint_only_change(old, new)
        || custom_paint_only_change(old, new)
        || visual_paint_only_change(old, new)
        || image_paint_only_change(old, new)
}

fn visual_paint_only_change(old: &RenderKind, new: &RenderKind) -> bool {
    match (old, new) {
        (RenderKind::Box { desired: a, .. }, RenderKind::Box { desired: b, .. })
        | (RenderKind::Shape { desired: a, .. }, RenderKind::Shape { desired: b, .. }) => a == b,
        (RenderKind::Decorated { desired: a, .. }, RenderKind::Decorated { desired: b, .. }) => {
            a == b
        }
        (
            RenderKind::Button {
                desired: old_size,
                enabled: old_enabled,
                focusable_when_disabled: old_focusable,
                ..
            },
            RenderKind::Button {
                desired: new_size,
                enabled: new_enabled,
                focusable_when_disabled: new_focusable,
                ..
            },
        ) => old_size == new_size && old_enabled == new_enabled && old_focusable == new_focusable,
        _ => false,
    }
}

fn image_paint_only_change(old: &RenderKind, new: &RenderKind) -> bool {
    matches!(
        (old, new),
        (
            RenderKind::Image {
                image: old_image,
                width: old_width,
                height: old_height,
                ..
            },
            RenderKind::Image {
                image: new_image,
                width: new_width,
                height: new_height,
                ..
            }
        ) if old_image == new_image && old_width == new_width && old_height == new_height
    )
}

fn text_paint_only_change(old: &RenderKind, new: &RenderKind) -> bool {
    let (
        RenderKind::Text {
            text: old_text,
            style: old_style,
            align: old_align,
            soft_wrap: old_soft_wrap,
            max_lines: old_max_lines,
            overflow: old_overflow,
        },
        RenderKind::Text {
            text: new_text,
            style: new_style,
            align: new_align,
            soft_wrap: new_soft_wrap,
            max_lines: new_max_lines,
            overflow: new_overflow,
        },
    ) = (old, new)
    else {
        return false;
    };
    old_text == new_text
        && old_align == new_align
        && old_soft_wrap == new_soft_wrap
        && old_max_lines == new_max_lines
        && old_overflow == new_overflow
        && old_style.family == new_style.family
        && old_style.size == new_style.size
        && old_style.weight == new_style.weight
        && old_style.style == new_style.style
        && old_style.line_height == new_style.line_height
        && old_style.letter_spacing == new_style.letter_spacing
}

fn custom_paint_only_change(old: &RenderKind, new: &RenderKind) -> bool {
    matches!(
        (old, new),
        (
            RenderKind::CustomPaint {
                desired: old_size,
                ..
            },
            RenderKind::CustomPaint {
                desired: new_size,
                ..
            }
        ) if old_size == new_size
    )
}

fn same_compositor_family(old: &RenderKind, new: &RenderKind) -> bool {
    matches!(
        (old, new),
        (RenderKind::Opacity { .. }, RenderKind::Opacity { .. })
            | (RenderKind::Blur { .. }, RenderKind::Blur { .. })
            | (RenderKind::DropShadow { .. }, RenderKind::DropShadow { .. })
            | (
                RenderKind::ColorFiltered { .. },
                RenderKind::ColorFiltered { .. }
            )
            | (RenderKind::Blend { .. }, RenderKind::Blend { .. })
            | (RenderKind::ShaderMask { .. }, RenderKind::ShaderMask { .. })
            | (
                RenderKind::BackdropFilter { .. },
                RenderKind::BackdropFilter { .. }
            )
            | (
                RenderKind::AnnotatedRegion { .. },
                RenderKind::AnnotatedRegion { .. }
            )
            | (RenderKind::Leader { .. }, RenderKind::Leader { .. })
            | (RenderKind::Follower { .. }, RenderKind::Follower { .. })
            | (RenderKind::Transform { .. }, RenderKind::Transform { .. })
            | (RenderKind::Scale { .. }, RenderKind::Scale { .. })
            | (RenderKind::Rotation { .. }, RenderKind::Rotation { .. })
    )
}
