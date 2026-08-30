use crate::compositor::LayerId;
use crate::effects::{BlendMode, ColorFilter, DropShadowEffect, GaussianBlur};
use crate::geometry::RRect;
use crate::glyphs::GlyphRun;
use crate::gradients::Brush;
use crate::paint::{Border, FillRule, Paint, PaintStyle, Stroke};
use crate::paths::Path;
use incular_core::{Color, Rect, Size, Transform};
use incular_image::ImageHandle;
use std::sync::Arc;

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
    /// Begins an isolated shader-mask stage. The shader is evaluated in the
    /// mask's logical coordinate space and blended over the child source.
    PushShaderMask {
        layer: LayerId,
        shader: Brush,
        blend_mode: BlendMode,
        mask_size: Size,
        mask_transform: Transform,
        generation: u64,
        bounds: Rect,
    },
    /// Begins a backdrop stage. The backend samples the already painted
    /// destination in `bounds`, filters it, and then composites the filtered
    /// pixels before the child source is painted.
    PushBackdropFilter {
        layer: LayerId,
        blur: GaussianBlur,
        blend_mode: BlendMode,
        enabled: bool,
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
    /// Lowers a [`Paint`] to the existing renderer-neutral path command.
    ///
    /// Blend and color-filter metadata remain available on the paint for a
    /// compositor/backend that needs them; the retained path geometry itself
    /// is never duplicated into a second command representation.
    pub fn draw_path(&mut self, path: Arc<Path>, paint: &Paint, fill_rule: FillRule) {
        match paint.style_value() {
            PaintStyle::Fill => self.fill_path(path, paint.brush_value().clone(), fill_rule),
            PaintStyle::Stroke => {
                self.stroke_path(path, paint.brush_value().clone(), paint.stroke_value())
            }
        }
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
