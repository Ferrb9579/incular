//! Declarative render configuration updates and their phase invalidation.

use super::{RenderKind, RenderObjectPayload};

use incular_core::Invalidation;

impl RenderObjectPayload {
    /// Applies new declarative configuration and returns the minimum phase
    /// work required to make the retained object observable as that value.
    pub(crate) fn update_kind(&mut self, kind: RenderKind) -> Invalidation {
        if self.kind == kind {
            return Invalidation::NONE;
        }
        let invalidation = invalidation_for_change(&self.kind, &kind);
        self.replace_kind(kind);
        invalidation
    }
}

fn invalidation_for_change(old: &RenderKind, new: &RenderKind) -> Invalidation {
    use Invalidation as I;

    if same_compositor_family(old, new) {
        // Geometry consumers read the current retained transform directly; they
        // do not require measurement or picture recording to observe movement.
        return if matches!(
            new,
            RenderKind::Transform { .. }
                | RenderKind::Scale { .. }
                | RenderKind::Rotation { .. }
                | RenderKind::Leader { .. }
                | RenderKind::Follower { .. }
        ) {
            I::COMPOSITE | I::SEMANTICS | I::HIT_TEST
        } else {
            I::COMPOSITE
        };
    }
    if paint_only_change(old, new) {
        return I::PAINT;
    }

    // Layout changes also invalidate local painting and semantic geometry.
    I::LAYOUT | I::PAINT | I::SEMANTICS | I::HIT_TEST
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
