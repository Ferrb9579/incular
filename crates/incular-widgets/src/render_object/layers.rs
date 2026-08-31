use incular_core::{Color, Offset, Rect, Size, Transform as CoreTransform};
use incular_rendering::{Brush, DisplayList, DropShadowEffect, GaussianBlur, LayerId, LayerTree};

use crate::tree::{RenderKind, WidgetKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AttachmentSpec {
    Direct,
    Transform,
    Clip,
    Opacity,
    Blur,
    DropShadow,
    ColorFilter,
    Blend,
    ShaderMask,
    BackdropFilter,
    Annotation,
    Leader,
    Follower,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LayerSpec {
    picture: bool,
    focus_picture: bool,
    attachment: AttachmentSpec,
}

impl LayerSpec {
    fn for_widget(widget: &WidgetKind) -> Self {
        let focus_picture = matches!(
            widget,
            WidgetKind::Button {
                color,
                focused_color: Some(_),
                ..
            } if color.alpha == 0
        );
        let attachment = match widget {
            WidgetKind::Scroll { .. } | WidgetKind::SliverViewport { .. } => AttachmentSpec::Clip,
            WidgetKind::PersistentHeader { .. }
            | WidgetKind::Translate { .. }
            | WidgetKind::Transform { .. }
            | WidgetKind::Scale { .. }
            | WidgetKind::Rotation { .. }
            | WidgetKind::FittedBox { .. } => AttachmentSpec::Transform,
            WidgetKind::Opacity { .. } => AttachmentSpec::Opacity,
            WidgetKind::Blur { .. } => AttachmentSpec::Blur,
            WidgetKind::DropShadow { .. } | WidgetKind::Banner { .. } => AttachmentSpec::DropShadow,
            WidgetKind::ColorFiltered { .. } => AttachmentSpec::ColorFilter,
            WidgetKind::Blend { .. } => AttachmentSpec::Blend,
            WidgetKind::ShaderMask { .. } => AttachmentSpec::ShaderMask,
            WidgetKind::BackdropFilter { .. } => AttachmentSpec::BackdropFilter,
            WidgetKind::AnnotatedRegion { .. } => AttachmentSpec::Annotation,
            WidgetKind::CompositedTransformTarget { .. } => AttachmentSpec::Leader,
            WidgetKind::CompositedTransformFollower { .. } => AttachmentSpec::Follower,
            _ => AttachmentSpec::Direct,
        };
        Self {
            picture: needs_picture(widget),
            focus_picture,
            attachment,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum LayerAttachment {
    Direct,
    Transform { layer: LayerId },
    Clip { clip: LayerId, content: LayerId },
    Opacity { layer: LayerId },
    Blur { layer: LayerId },
    DropShadow { layer: LayerId },
    ColorFilter { layer: LayerId },
    Blend { layer: LayerId },
    ShaderMask { layer: LayerId },
    BackdropFilter { layer: LayerId },
    Annotation { layer: LayerId },
    Leader { layer: LayerId },
    Follower { layer: LayerId },
}

impl LayerAttachment {
    fn primary_layer(&self) -> Option<LayerId> {
        match self {
            Self::Direct => None,
            Self::Transform { layer }
            | Self::Opacity { layer }
            | Self::Blur { layer }
            | Self::DropShadow { layer }
            | Self::ColorFilter { layer }
            | Self::Blend { layer }
            | Self::ShaderMask { layer }
            | Self::BackdropFilter { layer }
            | Self::Annotation { layer }
            | Self::Leader { layer }
            | Self::Follower { layer } => Some(*layer),
            Self::Clip { clip, .. } => Some(*clip),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct RenderLayers {
    pub(crate) root: LayerId,
    pub(crate) picture: Option<LayerId>,
    pub(crate) focus_picture: Option<LayerId>,
    pub(crate) attachment: LayerAttachment,
}

impl RenderLayers {
    pub(crate) fn create(compositor: &mut LayerTree, widget: &WidgetKind) -> Self {
        let spec = LayerSpec::for_widget(widget);
        let root = compositor.create_transform(CoreTransform::translation(Offset::ZERO));
        let picture = spec.picture.then(|| {
            compositor.create_picture(
                DisplayList::new(),
                Rect::from_origin_size(Offset::ZERO, Size::ZERO),
            )
        });
        let focus_picture = spec.focus_picture.then(|| {
            compositor.create_picture(
                DisplayList::new(),
                Rect::from_origin_size(Offset::ZERO, Size::ZERO),
            )
        });

        let attachment = match widget {
            WidgetKind::Scroll { .. } | WidgetKind::SliverViewport { .. } => {
                let clip =
                    compositor.create_clip_rect(Rect::from_origin_size(Offset::ZERO, Size::ZERO));
                let content = compositor.create_transform(CoreTransform::translation(Offset::ZERO));
                compositor.set_children(root, std::iter::once(clip).chain(picture).collect());
                compositor.set_children(clip, vec![content]);
                LayerAttachment::Clip { clip, content }
            }
            WidgetKind::PersistentHeader { .. } | WidgetKind::Translate { .. } => {
                let layer = compositor.create_transform(CoreTransform::translation(Offset::ZERO));
                compositor.set_children(root, vec![layer]);
                LayerAttachment::Transform { layer }
            }
            WidgetKind::Transform { .. }
            | WidgetKind::Scale { .. }
            | WidgetKind::Rotation { .. }
            | WidgetKind::FittedBox { .. } => {
                let layer = compositor.create_transform(CoreTransform::IDENTITY);
                compositor.set_children(root, vec![layer]);
                LayerAttachment::Transform { layer }
            }
            WidgetKind::Opacity { alpha, .. } => {
                let layer = compositor.create_opacity(*alpha);
                compositor.set_children(root, vec![layer]);
                LayerAttachment::Opacity { layer }
            }
            WidgetKind::Blur {
                sigma_x, sigma_y, ..
            } => {
                let layer = compositor.create_blur(GaussianBlur::new(*sigma_x, *sigma_y));
                compositor.set_children(root, vec![layer]);
                LayerAttachment::Blur { layer }
            }
            WidgetKind::DropShadow {
                offset,
                sigma_x,
                sigma_y,
                color,
                ..
            } => {
                let layer = compositor.create_drop_shadow(DropShadowEffect::asymmetric(
                    *offset, *sigma_x, *sigma_y, *color,
                ));
                compositor.set_children(root, vec![layer]);
                LayerAttachment::DropShadow { layer }
            }
            WidgetKind::Banner { shadow, .. } => {
                let sigma = crate::utilities::banner_shadow_sigma(shadow.blur_radius);
                let layer = compositor.create_drop_shadow(DropShadowEffect::asymmetric(
                    shadow.offset,
                    sigma,
                    sigma,
                    shadow.color,
                ));
                compositor.set_children(root, vec![layer]);
                LayerAttachment::DropShadow { layer }
            }
            WidgetKind::ColorFiltered { filter, .. } => {
                let layer = compositor.create_color_filter(*filter);
                compositor.set_children(root, vec![layer]);
                LayerAttachment::ColorFilter { layer }
            }
            WidgetKind::Blend { mode, .. } => {
                let layer = compositor.create_blend(*mode);
                compositor.set_children(root, vec![layer]);
                LayerAttachment::Blend { layer }
            }
            WidgetKind::ShaderMask { blend_mode, .. } => {
                let layer = compositor.create_shader_mask(
                    Brush::Solid(Color::WHITE),
                    *blend_mode,
                    Size::ZERO,
                    CoreTransform::IDENTITY,
                );
                compositor.set_children(root, vec![layer]);
                LayerAttachment::ShaderMask { layer }
            }
            WidgetKind::BackdropFilter {
                blur,
                blend_mode,
                enabled,
                ..
            } => {
                let layer = compositor.create_backdrop_filter(*blur, *blend_mode, *enabled);
                compositor.set_children(root, vec![layer]);
                LayerAttachment::BackdropFilter { layer }
            }
            WidgetKind::AnnotatedRegion {
                annotation, sized, ..
            } => {
                let layer =
                    compositor.create_annotated_region(annotation.clone(), *sized, Size::ZERO);
                compositor.set_children(root, vec![layer]);
                LayerAttachment::Annotation { layer }
            }
            WidgetKind::CompositedTransformTarget { link, .. } => {
                let layer = compositor.create_leader(link.clone(), Size::ZERO);
                compositor.set_children(root, vec![layer]);
                LayerAttachment::Leader { layer }
            }
            WidgetKind::CompositedTransformFollower {
                link,
                show_when_unlinked,
                offset,
                target_anchor,
                follower_anchor,
                ..
            } => {
                let layer = compositor.create_follower(
                    link.clone(),
                    *show_when_unlinked,
                    *offset,
                    *target_anchor,
                    *follower_anchor,
                    Size::ZERO,
                );
                compositor.set_children(root, vec![layer]);
                LayerAttachment::Follower { layer }
            }
            _ => {
                let mut layers = picture.into_iter().collect::<Vec<_>>();
                layers.extend(focus_picture);
                compositor.set_children(root, layers);
                LayerAttachment::Direct
            }
        };
        Self {
            root,
            picture,
            focus_picture,
            attachment,
        }
    }

    fn spec(&self) -> LayerSpec {
        let attachment = match self.attachment {
            LayerAttachment::Direct => AttachmentSpec::Direct,
            LayerAttachment::Transform { .. } => AttachmentSpec::Transform,
            LayerAttachment::Clip { .. } => AttachmentSpec::Clip,
            LayerAttachment::Opacity { .. } => AttachmentSpec::Opacity,
            LayerAttachment::Blur { .. } => AttachmentSpec::Blur,
            LayerAttachment::DropShadow { .. } => AttachmentSpec::DropShadow,
            LayerAttachment::ColorFilter { .. } => AttachmentSpec::ColorFilter,
            LayerAttachment::Blend { .. } => AttachmentSpec::Blend,
            LayerAttachment::ShaderMask { .. } => AttachmentSpec::ShaderMask,
            LayerAttachment::BackdropFilter { .. } => AttachmentSpec::BackdropFilter,
            LayerAttachment::Annotation { .. } => AttachmentSpec::Annotation,
            LayerAttachment::Leader { .. } => AttachmentSpec::Leader,
            LayerAttachment::Follower { .. } => AttachmentSpec::Follower,
        };
        LayerSpec {
            picture: self.picture.is_some(),
            focus_picture: self.focus_picture.is_some(),
            attachment,
        }
    }

    /// Rebuilds only this render node's owned layer structure when the
    /// declarative compositor spec changes. The root transform ID remains
    /// stable so parent topology and render identity never need rewriting.
    pub(crate) fn reconcile_structure(
        &mut self,
        compositor: &mut LayerTree,
        widget: &WidgetKind,
    ) -> bool {
        if self.spec() == LayerSpec::for_widget(widget) {
            return false;
        }

        self.remove_owned_except_root(compositor);
        let fresh = Self::create(compositor, widget);
        compositor.remove(fresh.root);
        self.picture = fresh.picture;
        self.focus_picture = fresh.focus_picture;
        self.attachment = fresh.attachment;
        true
    }

    fn remove_owned_except_root(&mut self, compositor: &mut LayerTree) {
        if let Some(picture) = self.picture.take() {
            compositor.remove(picture);
        }
        if let Some(picture) = self.focus_picture.take() {
            compositor.remove(picture);
        }
        match std::mem::replace(&mut self.attachment, LayerAttachment::Direct) {
            LayerAttachment::Direct => {}
            LayerAttachment::Clip { clip, content } => {
                compositor.remove(content);
                compositor.remove(clip);
            }
            attachment => {
                compositor.remove(attachment.primary_layer().expect("attachment layer"));
            }
        }
    }

    #[cfg(feature = "devtools")]
    pub(crate) fn clip(&self) -> Option<LayerId> {
        match self.attachment {
            LayerAttachment::Clip { clip, .. } => Some(clip),
            _ => None,
        }
    }
    pub(crate) fn content(&self) -> Option<LayerId> {
        match self.attachment {
            LayerAttachment::Clip { content, .. } => Some(content),
            LayerAttachment::Transform { layer } => Some(layer),
            _ => None,
        }
    }
    pub(crate) fn opacity(&self) -> Option<LayerId> {
        match self.attachment {
            LayerAttachment::Opacity { layer } => Some(layer),
            _ => None,
        }
    }
    pub(crate) fn blur(&self) -> Option<LayerId> {
        match self.attachment {
            LayerAttachment::Blur { layer } => Some(layer),
            _ => None,
        }
    }
    pub(crate) fn shadow(&self) -> Option<LayerId> {
        match self.attachment {
            LayerAttachment::DropShadow { layer } => Some(layer),
            _ => None,
        }
    }
    pub(crate) fn color_filter(&self) -> Option<LayerId> {
        match self.attachment {
            LayerAttachment::ColorFilter { layer } => Some(layer),
            _ => None,
        }
    }
    pub(crate) fn blend(&self) -> Option<LayerId> {
        match self.attachment {
            LayerAttachment::Blend { layer } => Some(layer),
            _ => None,
        }
    }
    pub(crate) fn shader_mask(&self) -> Option<LayerId> {
        match self.attachment {
            LayerAttachment::ShaderMask { layer } => Some(layer),
            _ => None,
        }
    }
    pub(crate) fn backdrop_filter(&self) -> Option<LayerId> {
        match self.attachment {
            LayerAttachment::BackdropFilter { layer } => Some(layer),
            _ => None,
        }
    }
    pub(crate) fn update_from_kind(
        &self,
        compositor: &mut LayerTree,
        kind: &RenderKind,
        size: Size,
        world_transform: CoreTransform,
        content_transform: Option<CoreTransform>,
    ) {
        match (&self.attachment, kind) {
            (LayerAttachment::Opacity { layer }, RenderKind::Opacity { alpha, .. }) => {
                compositor.update_opacity(*layer, *alpha);
            }
            (
                LayerAttachment::Blur { layer },
                RenderKind::Blur {
                    sigma_x, sigma_y, ..
                },
            ) => {
                compositor.update_blur(*layer, GaussianBlur::new(*sigma_x, *sigma_y));
            }
            (
                LayerAttachment::DropShadow { layer },
                RenderKind::DropShadow {
                    offset,
                    sigma_x,
                    sigma_y,
                    color,
                    ..
                },
            ) => {
                compositor.update_drop_shadow(
                    *layer,
                    DropShadowEffect::asymmetric(*offset, *sigma_x, *sigma_y, *color),
                );
            }
            (LayerAttachment::DropShadow { layer }, RenderKind::Banner { shadow, .. }) => {
                let sigma = crate::utilities::banner_shadow_sigma(shadow.blur_radius);
                compositor.update_drop_shadow(
                    *layer,
                    DropShadowEffect::asymmetric(shadow.offset, sigma, sigma, shadow.color),
                );
            }
            (LayerAttachment::ColorFilter { layer }, RenderKind::ColorFiltered { filter, .. }) => {
                compositor.update_color_filter(*layer, *filter);
            }
            (LayerAttachment::Blend { layer }, RenderKind::Blend { mode }) => {
                compositor.update_blend(*layer, *mode);
            }
            (
                LayerAttachment::ShaderMask { layer },
                RenderKind::ShaderMask { shader, blend_mode },
            ) => {
                compositor.update_shader_mask(
                    *layer,
                    shader.call(Rect::from_origin_size(Offset::ZERO, size)),
                    *blend_mode,
                    size,
                    world_transform,
                );
            }
            (
                LayerAttachment::BackdropFilter { layer },
                RenderKind::BackdropFilter {
                    blur,
                    blend_mode,
                    enabled,
                },
            ) => {
                compositor.update_backdrop_filter(*layer, *blur, *blend_mode, *enabled);
            }
            (
                LayerAttachment::Annotation { layer },
                RenderKind::AnnotatedRegion { annotation, sized },
            ) => {
                compositor.update_annotated_region(*layer, annotation.clone(), *sized, size);
            }
            (LayerAttachment::Leader { layer }, RenderKind::Leader { link }) => {
                compositor.update_leader(*layer, link.clone(), size);
            }
            (
                LayerAttachment::Follower { layer },
                RenderKind::Follower {
                    link,
                    show_when_unlinked,
                    offset,
                    target_anchor,
                    follower_anchor,
                },
            ) => {
                compositor.update_follower(
                    *layer,
                    link.clone(),
                    *show_when_unlinked,
                    *offset,
                    *target_anchor,
                    *follower_anchor,
                    size,
                );
            }
            (LayerAttachment::Transform { layer }, _) => {
                if let Some(transform) = content_transform {
                    compositor.update_transform(*layer, transform);
                }
            }
            _ => {}
        }
    }

    pub(crate) fn update_layout_geometry(
        &self,
        compositor: &mut LayerTree,
        kind: &RenderKind,
        size: Size,
        content_transform: Option<CoreTransform>,
    ) {
        if let LayerAttachment::Clip { clip, .. } = self.attachment {
            compositor.update_clip(clip, Rect::from_origin_size(Offset::ZERO, size));
        }
        if let Some(content) = self.content()
            && let Some(transform) = content_transform
        {
            compositor.update_transform(content, transform);
        }
        match (&self.attachment, kind) {
            (
                LayerAttachment::Annotation { layer },
                RenderKind::AnnotatedRegion { annotation, sized },
            ) => {
                compositor.update_annotated_region(*layer, annotation.clone(), *sized, size);
            }
            (LayerAttachment::Leader { layer }, RenderKind::Leader { .. }) => {
                compositor.update_leader_size(*layer, size);
            }
            (
                LayerAttachment::Follower { layer },
                RenderKind::Follower {
                    link,
                    show_when_unlinked,
                    offset,
                    target_anchor,
                    follower_anchor,
                },
            ) => {
                compositor.update_follower(
                    *layer,
                    link.clone(),
                    *show_when_unlinked,
                    *offset,
                    *target_anchor,
                    *follower_anchor,
                    size,
                );
            }
            _ => {}
        }
    }

    pub(crate) fn set_render_children(
        &self,
        compositor: &mut LayerTree,
        kind: &RenderKind,
        mut child_layers: Vec<LayerId>,
    ) {
        if matches!(kind, RenderKind::Banner { .. }) {
            let mut layers = child_layers;
            if let (Some(shadow), Some(picture)) = (self.shadow(), self.picture) {
                compositor.set_children(shadow, vec![picture]);
                layers.push(shadow);
            } else {
                layers.extend(self.picture);
            }
            compositor.set_children(self.root, layers);
            return;
        }
        match self.attachment {
            LayerAttachment::Direct => {
                let mut layers = Vec::with_capacity(
                    child_layers.len() + 1 + usize::from(self.focus_picture.is_some()),
                );
                layers.extend(self.picture);
                layers.append(&mut child_layers);
                layers.extend(self.focus_picture);
                compositor.set_children(self.root, layers);
            }
            LayerAttachment::Clip { content, .. }
            | LayerAttachment::Transform { layer: content } => {
                compositor.set_children(content, child_layers);
            }
            _ => {
                let layer = self
                    .attachment
                    .primary_layer()
                    .expect("non-direct attachment has a layer");
                compositor.set_children(layer, child_layers);
                compositor.set_children(self.root, vec![layer]);
            }
        }
    }

    pub(crate) fn remove(mut self, compositor: &mut LayerTree) {
        let root = self.root;
        self.remove_owned_except_root(compositor);
        compositor.remove(root);
    }
}

fn needs_picture(widget: &WidgetKind) -> bool {
    matches!(
        widget,
        WidgetKind::Box { .. }
            | WidgetKind::Shape { .. }
            | WidgetKind::CustomPaint { .. }
            | WidgetKind::RepaintBoundary { .. }
            | WidgetKind::Decorated { .. }
            | WidgetKind::Banner { .. }
            | WidgetKind::Button { .. }
            | WidgetKind::Text { .. }
            | WidgetKind::SelectableText { .. }
            | WidgetKind::TextField { .. }
            | WidgetKind::Image { .. }
            | WidgetKind::Scroll { .. }
            | WidgetKind::SliverViewport { .. }
    )
}
