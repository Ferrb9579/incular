//! Element mounting, reconciliation, lazy materialization, and retained lifecycle.

use super::*;

use super::rendering::{carry_replaced_transition, render_kind};

impl WidgetTree {
    pub fn update(&mut self, id: ElementId, widget: Widget) -> Result<(), TreeError> {
        if !self.elements.contains(id.0) {
            return Err(TreeError::MissingElement(id));
        }
        let result = self.update_existing(id, &widget);
        if result.is_ok() {
            self.refresh_notification_listeners();
        }
        result
    }

    pub(super) fn mount_element(
        &mut self,
        parent: Option<ElementId>,
        widget: Widget,
    ) -> Result<ElementId, TreeError> {
        enum MountWork {
            Create {
                parent: Option<ElementId>,
                widget: Box<Widget>,
            },
            Finish(ElementId),
        }

        let mut work = vec![MountWork::Create {
            parent,
            widget: Box::new(widget),
        }];
        let mut root = None;
        while let Some(next) = work.pop() {
            match next {
                MountWork::Create { parent, widget } => {
                    let children = widget
                        .children_refs()
                        .into_iter()
                        .cloned()
                        .collect::<Vec<_>>();
                    let id = self.mount_element_node(parent, *widget)?;
                    if root.is_none() {
                        root = Some(id);
                    }
                    if let Some(parent) = parent {
                        self.elements
                            .get_mut(parent.0)
                            .expect("mount parent remains live")
                            .children
                            .push(id);
                    }
                    work.push(MountWork::Finish(id));
                    for child in children.into_iter().rev() {
                        work.push(MountWork::Create {
                            parent: Some(id),
                            widget: Box::new(child),
                        });
                    }
                }
                MountWork::Finish(id) => {
                    self.sync_render_children(id);
                    self.sync_raw_input_state(id);
                    if self
                        .elements
                        .get(id.0)
                        .is_some_and(|element| element.parent.is_none())
                    {
                        let layer = self
                            .renders
                            .get(self.render_id(id).expect("mounted").0)
                            .expect("mounted render")
                            .object
                            .layers
                            .root;
                        self.compositor.set_root(layer);
                    }
                }
            }
        }
        Ok(root.expect("mount work always contains the root widget"))
    }

    pub(super) fn mount_element_node(
        &mut self,
        parent: Option<ElementId>,
        widget: Widget,
    ) -> Result<ElementId, TreeError> {
        let inherited_environment = parent
            .and_then(|id| self.elements.get(id.0))
            .and_then(|element| element.environment.clone());
        let (environment_override, environment_boundary) = match &widget.kind {
            WidgetKind::LayoutBuilder {
                environment,
                environment_boundary,
                ..
            } => (environment.clone(), *environment_boundary),
            _ => (None, false),
        };
        let environment = if environment_boundary {
            None
        } else {
            compose_environment(environment_override.clone(), inherited_environment)
        };
        self.check_keys_borrowed(widget.children_refs())?;
        let layer = self
            .compositor
            .create_transform(CoreTransform::translation(Offset::ZERO));
        let picture = matches!(
            widget.kind,
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
        .then(|| {
            self.compositor.create_picture(
                DisplayList::new(),
                Rect::from_origin_size(Offset::ZERO, Size::ZERO),
            )
        });
        let focus_picture = match &widget.kind {
            WidgetKind::Button {
                color,
                focused_color: Some(_),
                ..
            } if color.alpha == 0 => Some(self.compositor.create_picture(
                DisplayList::new(),
                Rect::from_origin_size(Offset::ZERO, Size::ZERO),
            )),
            _ => None,
        };
        let (
            clip_layer,
            content_layer,
            opacity_layer,
            blur_layer,
            shadow_layer,
            color_filter_layer,
            blend_layer,
            shader_mask_layer,
            backdrop_filter_layer,
            annotation_layer,
            leader_layer,
            follower_layer,
        ) = match &widget.kind {
            WidgetKind::Scroll { .. } | WidgetKind::SliverViewport { .. } => {
                let clip = self
                    .compositor
                    .create_clip_rect(Rect::from_origin_size(Offset::ZERO, Size::ZERO));
                let content = self
                    .compositor
                    .create_transform(CoreTransform::translation(Offset::ZERO));
                self.compositor
                    .set_children(layer, std::iter::once(clip).chain(picture).collect());
                self.compositor.set_children(clip, vec![content]);
                (
                    Some(clip),
                    Some(content),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                )
            }
            WidgetKind::PersistentHeader { .. } => {
                // Keep the pinning transform below static flow placement. The
                // header still occupies its normal sliver extent in layout.
                let content = self
                    .compositor
                    .create_transform(CoreTransform::translation(Offset::ZERO));
                self.compositor.set_children(layer, vec![content]);
                (
                    None,
                    Some(content),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                )
            }
            WidgetKind::Translate { .. } => {
                // Keep dynamic movement structurally below static layout
                // placement: parent -> layout transform -> animated transform
                // -> normal child tree. This is also what lets animation avoid
                // repainting local picture commands.
                let content = self
                    .compositor
                    .create_transform(CoreTransform::translation(Offset::ZERO));
                self.compositor.set_children(layer, vec![content]);
                (
                    None,
                    Some(content),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                )
            }
            WidgetKind::Transform { .. }
            | WidgetKind::Scale { .. }
            | WidgetKind::Rotation { .. }
            | WidgetKind::FittedBox { .. } => {
                let content = self.compositor.create_transform(CoreTransform::IDENTITY);
                self.compositor.set_children(layer, vec![content]);
                (
                    None,
                    Some(content),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                )
            }
            WidgetKind::Opacity { alpha, .. } => {
                let opacity = self.compositor.create_opacity(*alpha);
                self.compositor.set_children(layer, vec![opacity]);
                (
                    None,
                    None,
                    Some(opacity),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                )
            }
            WidgetKind::Blur {
                sigma_x, sigma_y, ..
            } => {
                let blur = self
                    .compositor
                    .create_blur(GaussianBlur::new(*sigma_x, *sigma_y));
                self.compositor.set_children(layer, vec![blur]);
                (
                    None,
                    None,
                    None,
                    Some(blur),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                )
            }
            WidgetKind::DropShadow {
                offset,
                sigma_x,
                sigma_y,
                color,
                ..
            } => {
                let shadow = self
                    .compositor
                    .create_drop_shadow(DropShadowEffect::asymmetric(
                        *offset, *sigma_x, *sigma_y, *color,
                    ));
                self.compositor.set_children(layer, vec![shadow]);
                (
                    None,
                    None,
                    None,
                    None,
                    Some(shadow),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                )
            }
            WidgetKind::Banner { shadow, .. } => {
                let sigma = crate::utilities::banner_shadow_sigma(shadow.blur_radius);
                let shadow_layer =
                    self.compositor
                        .create_drop_shadow(DropShadowEffect::asymmetric(
                            shadow.offset,
                            sigma,
                            sigma,
                            shadow.color,
                        ));
                self.compositor.set_children(layer, vec![shadow_layer]);
                (
                    None,
                    None,
                    None,
                    None,
                    Some(shadow_layer),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                )
            }
            WidgetKind::ColorFiltered { filter, .. } => {
                let color_filter = self.compositor.create_color_filter(*filter);
                self.compositor.set_children(layer, vec![color_filter]);
                (
                    None,
                    None,
                    None,
                    None,
                    None,
                    Some(color_filter),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                )
            }
            WidgetKind::Blend { mode, .. } => {
                let blend = self.compositor.create_blend(*mode);
                self.compositor.set_children(layer, vec![blend]);
                (
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    Some(blend),
                    None,
                    None,
                    None,
                    None,
                    None,
                )
            }
            WidgetKind::ShaderMask { blend_mode, .. } => {
                let shader_mask = self.compositor.create_shader_mask(
                    Brush::Solid(Color::WHITE),
                    *blend_mode,
                    Size::ZERO,
                    CoreTransform::IDENTITY,
                );
                self.compositor.set_children(layer, vec![shader_mask]);
                (
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    Some(shader_mask),
                    None,
                    None,
                    None,
                    None,
                )
            }
            WidgetKind::BackdropFilter {
                blur,
                blend_mode,
                enabled,
                ..
            } => {
                let backdrop = self
                    .compositor
                    .create_backdrop_filter(*blur, *blend_mode, *enabled);
                self.compositor.set_children(layer, vec![backdrop]);
                (
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    Some(backdrop),
                    None,
                    None,
                    None,
                )
            }
            WidgetKind::AnnotatedRegion {
                annotation, sized, ..
            } => {
                let annotation_layer =
                    self.compositor
                        .create_annotated_region(annotation.clone(), *sized, Size::ZERO);
                self.compositor.set_children(layer, vec![annotation_layer]);
                (
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    Some(annotation_layer),
                    None,
                    None,
                )
            }
            WidgetKind::CompositedTransformTarget { link, .. } => {
                let leader = self.compositor.create_leader(link.clone(), Size::ZERO);
                self.compositor.set_children(layer, vec![leader]);
                (
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    Some(leader),
                    None,
                )
            }
            WidgetKind::CompositedTransformFollower {
                link,
                show_when_unlinked,
                offset,
                target_anchor,
                follower_anchor,
                ..
            } => {
                let follower = self.compositor.create_follower(
                    link.clone(),
                    *show_when_unlinked,
                    *offset,
                    *target_anchor,
                    *follower_anchor,
                    Size::ZERO,
                );
                self.compositor.set_children(layer, vec![follower]);
                (
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    Some(follower),
                )
            }
            _ => {
                let mut layers = picture.into_iter().collect::<Vec<_>>();
                layers.extend(focus_picture);
                self.compositor.set_children(layer, layers);
                (
                    None, None, None, None, None, None, None, None, None, None, None, None,
                )
            }
        };
        let kind = render_kind(&widget, environment.as_ref());
        let render = self.renders.insert(RenderNode {
            parent: None,
            children: Vec::new(),
            geometry: RenderGeometry {
                size: Size::ZERO,
                offset: Offset::ZERO,
                constraints: None,
                baseline: None,
            },
            dirty: DirtyFlags::LAYOUT | DirtyFlags::PAINT,
            object: RenderObjectPayload::new(
                kind,
                LegacyRenderLayers {
                    root: layer,
                    picture,
                    focus_picture,
                    clip: clip_layer,
                    content: content_layer,
                    opacity: opacity_layer,
                    blur: blur_layer,
                    shadow: shadow_layer,
                    color_filter: color_filter_layer,
                    blend: blend_layer,
                    shader_mask: shader_mask_layer,
                    backdrop_filter: backdrop_filter_layer,
                    annotation: annotation_layer,
                    leader: leader_layer,
                    follower: follower_layer,
                },
            ),
        });
        let id = ElementId(self.elements.insert(Element {
            parent,
            children: Vec::new(),
            widget: widget.clone(),
            render: RenderObjectId(render),
            dirty: DirtyFlags::NONE,
            sliver_child_ids: Vec::new(),
            sliver_child_semantic_indices: Vec::new(),
            sliver_pinned_ids: HashSet::new(),
            advanced_child_keys: Vec::new(),
            notification_subscriptions: Vec::new(),
            sliver_delegate_revision: 0,
            sliver_scroll_revision: 0,
            layout_builder_constraints: None,
            layout_builder_revision: 0,
            environment,
            environment_override,
            environment_boundary,
            #[cfg(feature = "devtools")]
            dev: ElementDevData::default(),
        }));
        self.elements.get_mut(id.0).expect("fresh element").children = Vec::new();
        if let WidgetKind::SelectionListener { notifier, .. } = &widget.kind {
            notifier.register();
        }
        self.diagnostics.mounts += 1;
        Ok(id)
    }

    pub(super) fn propagate_environment(&mut self, id: ElementId) {
        let (inherited, children) = {
            let element = self.elements.get(id.0).expect("live environment element");
            (element.environment.clone(), element.children.clone())
        };
        for child in children {
            let (override_value, boundary, previous, widget, render) = {
                let element = self.elements.get(child.0).expect("live child");
                (
                    element.environment_override.clone(),
                    element.environment_boundary,
                    element.environment.clone(),
                    element.widget.clone(),
                    element.render,
                )
            };
            let effective = if boundary {
                None
            } else {
                compose_environment(override_value, inherited.clone())
            };
            let changed = match (&previous, &effective) {
                (Some(a), Some(b)) => !Rc::ptr_eq(a, b),
                (None, None) => false,
                _ => true,
            };
            if changed {
                let new_kind = render_kind(&widget, effective.as_ref());
                let invalidation = self
                    .renders
                    .get_mut(render.0)
                    .expect("live child render")
                    .object
                    .update_kind(new_kind);
                {
                    let element = self.elements.get_mut(child.0).expect("live child");
                    element.environment = effective;
                    element.layout_builder_constraints = None;
                    element.layout_builder_revision = 0;
                }
                if invalidation.contains(RenderInvalidation::LAYOUT) {
                    self.mark_render_dirty(render, DirtyFlags::LAYOUT | DirtyFlags::PAINT, true);
                } else if invalidation.contains(RenderInvalidation::PAINT) {
                    self.mark_render_dirty(render, DirtyFlags::PAINT, false);
                }
            }
            self.propagate_environment(child);
        }
    }

    pub(super) fn update_existing(
        &mut self,
        id: ElementId,
        widget: &Widget,
    ) -> Result<(), TreeError> {
        stacker::maybe_grow(128 * 1024, 2 * 1024 * 1024, || {
            self.update_existing_inner(id, widget)
        })
    }

    pub(super) fn update_existing_inner(
        &mut self,
        id: ElementId,
        widget: &Widget,
    ) -> Result<(), TreeError> {
        let _node_guard = self.guard_element(FramePhase::Build, id);
        // Borrow-compare first: the unchanged-subtree bailout must not clone
        // either widget. This is the dominant hot path for wide static trees
        // under a rebuilding parent (Task 15 measured bottleneck).
        if self
            .elements
            .get(id.0)
            .ok_or(TreeError::MissingElement(id))?
            .widget
            == *widget
        {
            self.diagnostics.identical_child_bailouts += 1;
            return Ok(());
        }
        #[cfg(feature = "devtools")]
        let trace = self.devtools_trace_begin_element(id, TracePhase::Build);
        let old = self
            .elements
            .get(id.0)
            .ok_or(TreeError::MissingElement(id))?
            .widget
            .clone();
        if let WidgetKind::SelectionListener {
            notifier: old_notifier,
            ..
        } = &old.kind
            && let WidgetKind::SelectionListener {
                notifier: new_notifier,
                ..
            } = &widget.kind
            && old_notifier != new_notifier
        {
            old_notifier.unregister();
            new_notifier.register();
        }
        let old_environment = self
            .elements
            .get(id.0)
            .and_then(|element| element.environment.clone());
        let (new_override, new_boundary) = match &widget.kind {
            WidgetKind::LayoutBuilder {
                environment,
                environment_boundary,
                ..
            } => (environment.clone(), *environment_boundary),
            _ => (None, false),
        };
        let parent_environment = self
            .elements
            .get(id.0)
            .and_then(|element| element.parent)
            .and_then(|parent| self.elements.get(parent.0))
            .and_then(|parent| parent.environment.clone());
        let new_environment = if new_boundary {
            None
        } else {
            compose_environment(new_override.clone(), parent_environment.clone())
        };
        let environment_changed = match (&old_environment, &new_environment) {
            (Some(a), Some(b)) => !Rc::ptr_eq(a, b),
            (None, None) => false,
            _ => true,
        };
        #[cfg(feature = "devtools")]
        let property_changes = crate::devtools_props::diff_properties(&old.kind, &widget.kind);
        debug_assert_eq!(
            old.type_(),
            widget.type_(),
            "only compatible elements may update"
        );
        self.check_keys_borrowed(widget.children_refs())?;
        let render = self.elements.get(id.0).expect("present").render;
        let old_kind = render_kind(&old, old_environment.as_ref());
        let new_kind = render_kind(widget, new_environment.as_ref());
        carry_replaced_transition(&old_kind, &new_kind);
        #[cfg(feature = "devtools")]
        let mut work_reasons: (Option<String>, Option<String>, Option<String>) = (None, None, None);
        if old_kind != new_kind {
            let invalidation = self
                .renders
                .get_mut(render.0)
                .expect("present")
                .object
                .update_kind(new_kind.clone());
            if let RenderKind::Banner { shadow, .. } = &new_kind
                && let Some(layer) = self
                    .renders
                    .get(render.0)
                    .and_then(|node| node.object.layers.shadow)
            {
                let sigma = crate::utilities::banner_shadow_sigma(shadow.blur_radius);
                self.compositor.update_drop_shadow(
                    layer,
                    DropShadowEffect::asymmetric(shadow.offset, sigma, sigma, shadow.color),
                );
            }
            if invalidation.contains(RenderInvalidation::COMPOSITE) {
                self.diagnostics.compositor_only_updates += 1;
                #[cfg(feature = "devtools")]
                {
                    work_reasons.2 = Some("retained compositor property changed".into());
                }
                match new_kind.clone() {
                    RenderKind::Opacity { alpha, .. } => {
                        if let Some(layer) = self
                            .renders
                            .get(render.0)
                            .and_then(|node| node.object.layers.opacity)
                        {
                            self.compositor.update_opacity(layer, alpha);
                        }
                    }
                    RenderKind::Blur {
                        sigma_x, sigma_y, ..
                    } => {
                        if let Some(layer) = self
                            .renders
                            .get(render.0)
                            .and_then(|n| n.object.layers.blur)
                        {
                            self.compositor
                                .update_blur(layer, GaussianBlur::new(sigma_x, sigma_y));
                        }
                    }
                    RenderKind::DropShadow {
                        offset,
                        sigma_x,
                        sigma_y,
                        color,
                        ..
                    } => {
                        if let Some(layer) = self
                            .renders
                            .get(render.0)
                            .and_then(|n| n.object.layers.shadow)
                        {
                            self.compositor.update_drop_shadow(
                                layer,
                                DropShadowEffect::asymmetric(offset, sigma_x, sigma_y, color),
                            );
                        }
                    }
                    RenderKind::ColorFiltered { filter, .. } => {
                        if let Some(layer) = self
                            .renders
                            .get(render.0)
                            .and_then(|node| node.object.layers.color_filter)
                        {
                            self.compositor.update_color_filter(layer, filter);
                        }
                    }
                    RenderKind::Blend { mode } => {
                        if let Some(layer) = self
                            .renders
                            .get(render.0)
                            .and_then(|node| node.object.layers.blend)
                        {
                            self.compositor.update_blend(layer, mode);
                        }
                    }
                    RenderKind::ShaderMask { shader, blend_mode } => {
                        if let Some(layer) = self
                            .renders
                            .get(render.0)
                            .and_then(|node| node.object.layers.shader_mask)
                        {
                            let size = self.renders.get(render.0).expect("present").size;
                            self.compositor.update_shader_mask(
                                layer,
                                shader.call(Rect::from_origin_size(Offset::ZERO, size)),
                                blend_mode,
                                size,
                                self.render_world_transform(render),
                            );
                        }
                    }
                    RenderKind::BackdropFilter {
                        blur,
                        blend_mode,
                        enabled,
                    } => {
                        if let Some(layer) = self
                            .renders
                            .get(render.0)
                            .and_then(|node| node.object.layers.backdrop_filter)
                        {
                            self.compositor
                                .update_backdrop_filter(layer, blur, blend_mode, enabled);
                        }
                    }
                    RenderKind::AnnotatedRegion { annotation, sized } => {
                        if let Some(layer) = self
                            .renders
                            .get(render.0)
                            .and_then(|node| node.object.layers.annotation)
                        {
                            let size = self.renders.get(render.0).expect("present").size;
                            self.compositor
                                .update_annotated_region(layer, annotation, sized, size);
                        }
                    }
                    RenderKind::Leader { link } => {
                        if let Some(layer) = self
                            .renders
                            .get(render.0)
                            .and_then(|node| node.object.layers.leader)
                        {
                            let size = self.renders.get(render.0).expect("present").size;
                            self.compositor.update_leader(layer, link, size);
                        }
                    }
                    RenderKind::Follower {
                        link,
                        show_when_unlinked,
                        offset,
                        target_anchor,
                        follower_anchor,
                    } => {
                        if let Some(layer) = self
                            .renders
                            .get(render.0)
                            .and_then(|node| node.object.layers.follower)
                        {
                            let size = self.renders.get(render.0).expect("present").size;
                            self.compositor.update_follower(
                                layer,
                                link,
                                show_when_unlinked,
                                offset,
                                target_anchor,
                                follower_anchor,
                                size,
                            );
                        }
                    }
                    RenderKind::Transform { .. }
                    | RenderKind::Scale { .. }
                    | RenderKind::Rotation { .. } => {
                        if let (Some(layer), Some(transform)) = (
                            self.renders
                                .get(render.0)
                                .and_then(|node| node.object.layers.content),
                            self.content_transform(render),
                        ) {
                            self.compositor.update_transform(layer, transform);
                        }
                    }
                    _ => {}
                }
            } else if invalidation.contains(RenderInvalidation::PAINT)
                && !invalidation.contains(RenderInvalidation::LAYOUT)
            {
                #[cfg(feature = "devtools")]
                {
                    work_reasons.1 = Some("paint-only configuration changed".into());
                }
                self.mark_render_dirty(render, DirtyFlags::PAINT, false);
            } else if invalidation.contains(RenderInvalidation::LAYOUT) {
                #[cfg(feature = "devtools")]
                {
                    work_reasons.0 = Some("layout-affecting configuration changed".into());
                    work_reasons.1 = Some("layout result invalidated paint".into());
                }
                self.mark_render_dirty(render, DirtyFlags::LAYOUT | DirtyFlags::PAINT, true);
            }
        }
        {
            let element = self.elements.get_mut(id.0).expect("present");
            element.widget = widget.clone();
            element.environment_override = new_override;
            element.environment_boundary = new_boundary;
            element.environment = new_environment;
            if environment_changed {
                element.layout_builder_constraints = None;
            }
            element.dirty.remove(DirtyFlags::BUILD);
        }
        if environment_changed {
            self.propagate_environment(id);
        }
        if matches!(widget.kind, WidgetKind::LayoutBuilder { .. }) {
            // A new descriptor may carry a different builder closure while
            // retaining the same constraints/revision value. Force one
            // materialization so updates cannot leave the old child mounted.
            if let Some(element) = self.elements.get_mut(id.0) {
                element.layout_builder_constraints = None;
                element.layout_builder_revision = 0;
            }
            // LayoutBuilder has a stable render kind, so the normal
            // old-kind/new-kind invalidation above does not run. Propagate the
            // dirty bit explicitly; otherwise an unchanged parent can return
            // from the layout cache before this builder gets a chance to
            // materialize the new closure output.
            self.mark_render_dirty(render, DirtyFlags::LAYOUT | DirtyFlags::PAINT, true);
        }
        self.diagnostics.rebuilds += 1;
        #[cfg(feature = "devtools")]
        {
            let element = self.elements.get_mut(id.0).expect("present");
            element.dev.builds += 1;
            element.dev.revision += 1;
            element.dev.property_changes = property_changes;
            element.dev.layout_reason = work_reasons.0;
            element.dev.paint_reason = work_reasons.1;
            element.dev.composite_reason = work_reasons.2;
        }
        // Lazy children are owned by the viewport's indexed materialization
        // map, not by `Widget::children()`. Recreating a sliver viewport
        // description must preserve every still-valid mounted child; the
        // next layout pass will add/drop only indices required by the delegate.
        if matches!(widget.kind, WidgetKind::SliverViewport { .. }) {
            #[cfg(feature = "devtools")]
            self.devtools_trace_end(trace);
            return Ok(());
        }
        if matches!(widget.kind, WidgetKind::LayoutBuilder { .. }) {
            #[cfg(feature = "devtools")]
            self.devtools_trace_end(trace);
            return Ok(());
        }
        let previous = self.elements.get(id.0).expect("present").children.clone();
        let desired = widget.children_refs();
        let reconciled = self.reconcile_children(id, previous.clone(), &desired)?;
        if reconciled != previous {
            self.elements.get_mut(id.0).expect("present").children = reconciled;
            self.sync_render_children(id);
        }
        self.sync_raw_input_state(id);
        #[cfg(feature = "devtools")]
        self.devtools_trace_end(trace);
        Ok(())
    }
    pub(super) fn compatible(&mut self, id: ElementId, widget: &Widget) -> bool {
        match self.elements.get(id.0) {
            Some(element) => {
                self.diagnostics.widget_type_comparisons += 1;
                element.widget.type_() == widget.type_() && {
                    self.diagnostics.key_comparisons += 1;
                    element.widget.key == widget.key
                }
            }
            None => false,
        }
    }
    pub(super) fn reconcile_children(
        &mut self,
        parent: ElementId,
        previous: Vec<ElementId>,
        desired: &[&Widget],
    ) -> Result<Vec<ElementId>, TreeError> {
        self.check_keys_borrowed(desired.iter().copied())?;
        self.diagnostics.child_list_scans += 1;
        let mut start = 0;
        let mut old_end = previous.len();
        let mut new_end = desired.len();
        let mut next = Vec::with_capacity(new_end);
        while start < old_end && start < new_end && self.compatible(previous[start], desired[start])
        {
            let id = previous[start];
            self.update_existing(id, desired[start])?;
            next.push(id);
            start += 1;
            self.diagnostics.reconciliation_fast_paths += 1;
            self.diagnostics.elements_reused += 1;
        }
        while start < old_end
            && start < new_end
            && self.compatible(previous[old_end - 1], desired[new_end - 1])
        {
            old_end -= 1;
            new_end -= 1;
            self.diagnostics.reconciliation_fast_paths += 1;
            self.diagnostics.elements_reused += 1;
        }
        let old_middle = &previous[start..old_end];
        let desired_middle = &desired[start..new_end];
        // Track original positions to detect genuine moves (Task 15 counter).
        let mut keyed: HashMap<Key, (ElementId, usize)> = HashMap::new();
        let mut unkeyed_ids: Vec<(ElementId, usize)> = Vec::new();
        let any_keys = old_middle.iter().any(|id| {
            self.elements
                .get(id.0)
                .is_some_and(|e| e.widget.key.is_some())
        }) || desired_middle.iter().any(|w| w.key.is_some());
        if any_keys {
            self.diagnostics.key_maps_built += 1;
            for (position, id) in old_middle.iter().enumerate() {
                if let Some(key) = self.elements.get(id.0).and_then(|e| e.widget.key.clone()) {
                    keyed.insert(key, (*id, position));
                    self.diagnostics.key_map_entries += 1;
                } else {
                    unkeyed_ids.push((*id, position));
                }
            }
        } else {
            for (position, id) in old_middle.iter().enumerate() {
                unkeyed_ids.push((*id, position));
            }
        }
        let mut unkeyed = unkeyed_ids.into_iter();
        let mut used = std::collections::HashSet::new();
        for widget in desired_middle {
            let candidate = if let Some(key) = widget.key() {
                self.diagnostics.key_lookups += 1;
                keyed.get(key).copied()
            } else {
                unkeyed.next()
            };
            self.diagnostics.widget_type_comparisons += 1;
            if let Some((id, old_position)) = candidate.filter(|(id, _)| {
                self.elements
                    .get(id.0)
                    .is_some_and(|e| e.widget.type_() == widget.type_())
            }) {
                self.update_existing(id, widget)?;
                used.insert(id);
                // A reused child is "moved" when its previous middle position
                // differs from the position it is emitted at now.
                let emitted_index = next.len();
                let expected_index = start + old_position;
                if emitted_index != expected_index {
                    self.diagnostics.elements_moved += 1;
                }
                next.push(id);
                self.diagnostics.elements_reused += 1;
            } else {
                self.diagnostics.elements_created += 1;
                next.push(self.mount_element(Some(parent), (*widget).clone())?);
            }
        }
        for id in old_middle {
            if !used.contains(id) {
                self.unmount_element(*id);
                self.diagnostics.elements_removed += 1;
            }
        }
        let mut suffix = Vec::new();
        for index in new_end..desired.len() {
            let id = previous[old_end + (index - new_end)];
            self.update_existing(id, desired[index])?;
            suffix.push(id);
            self.diagnostics.elements_reused += 1;
        }
        next.extend(suffix);
        Ok(next)
    }
    /// Reconciles the indexed child window produced by the retained sliver
    /// protocol. Unlike a box list, each child carries a viewport-scoped
    /// identity and an explicit sliver placement/constraint record.
    pub(super) fn materialize_sliver_children(
        &mut self,
        id: RenderObjectId,
        config: &SliverViewportConfig,
        layout: &SliverViewportLayout,
    ) {
        let element_id = self
            .element_for_render(id)
            .expect("sliver viewport element");
        let (old_ids, old_children) = {
            let element = self
                .elements
                .get(element_id.0)
                .expect("sliver viewport element");
            (element.sliver_child_ids.clone(), element.children.clone())
        };
        let existing = old_ids
            .into_iter()
            .zip(old_children.iter().copied())
            .collect::<HashMap<_, _>>();
        let mut next_ids = Vec::with_capacity(layout.children.len());
        let mut next_semantic_indices = Vec::with_capacity(layout.children.len());
        let mut next_children = Vec::with_capacity(layout.children.len());
        let mut pinned = HashSet::new();
        for child in &layout.children {
            let child_id = child.id;
            let retained = if let Some(existing) = existing
                .get(&child_id)
                .copied()
                .filter(|existing| self.compatible(*existing, &child.widget))
            {
                self.update_existing(existing, &child.widget)
                    .expect("sliver child update must remain valid");
                existing
            } else {
                // A stable sliver item ID does not guarantee that the widget
                // shape is unchanged. For example, a list row may gain a
                // GestureDetector when a workload changes. Retained element
                // identity is only reusable across compatible widget types;
                // leave incompatible old children for the cleanup pass below
                // and mount a fresh element for the same sliver slot.
                let mut widget = child.widget.clone();
                let handlers = &mut self.pending_handlers;
                let next = &mut self.next_action;
                widget.bind_callbacks(&mut |callback| {
                    let action = ActionId(*next);
                    *next += 1;
                    handlers.push((action, callback));
                    action
                });
                self.diagnostics.items_built += 1;
                match self.mount_element(Some(element_id), widget) {
                    Ok(child) => {
                        self.diagnostics.items_mounted += 1;
                        child
                    }
                    Err(error) => panic!("sliver child {child_id:?} could not mount: {error:?}"),
                }
            };
            if child.pinned {
                pinned.insert(child_id);
            }
            next_ids.push(child_id);
            next_semantic_indices.push(
                child
                    .semantic_index
                    .or_else(|| child.widget.semantic_index())
                    .or_else(|| child_id.item_index()),
            );
            next_children.push(retained);
        }
        let retained = next_children.iter().copied().collect::<HashSet<_>>();
        for child in old_children {
            if !retained.contains(&child) {
                self.unmount_element(child);
                self.diagnostics.items_unmounted += 1;
            } else {
                self.diagnostics.items_reused += 1;
            }
        }
        let element = self
            .elements
            .get_mut(element_id.0)
            .expect("sliver viewport element");
        element.children = next_children;
        element.sliver_child_ids = next_ids;
        element.sliver_child_semantic_indices = next_semantic_indices;
        element.sliver_pinned_ids = pinned;
        element.sliver_delegate_revision = config.delegate.revision();
        element.sliver_scroll_revision = config.controller.revision();
        self.sync_render_children(element_id);
    }

    /// Reconciles children materialized by one of the renderer-independent
    /// advanced scrolling algorithms. The algorithm-owned key is kept
    /// separately from the widget key because a lazy child can move in and
    /// out of the cache window without changing its declarative identity.
    pub(super) fn materialize_advanced_children(
        &mut self,
        element_id: ElementId,
        desired: Vec<(AdvancedChildKey, Widget)>,
    ) {
        let (old_keys, old_children) = {
            let element = self
                .elements
                .get(element_id.0)
                .expect("advanced scrolling element");
            (
                element.advanced_child_keys.clone(),
                element.children.clone(),
            )
        };
        let existing = old_keys
            .into_iter()
            .zip(old_children.iter().copied())
            .collect::<HashMap<_, _>>();
        let mut retained = HashSet::new();
        let mut next_keys = Vec::with_capacity(desired.len());
        let mut next_children = Vec::with_capacity(desired.len());

        for (key, mut widget) in desired {
            let child = if let Some(existing) = existing
                .get(&key)
                .copied()
                .filter(|existing| self.compatible(*existing, &widget))
            {
                self.update_existing(existing, &widget)
                    .expect("advanced child update must remain valid");
                existing
            } else {
                let handlers = &mut self.pending_handlers;
                let next = &mut self.next_action;
                widget.bind_callbacks(&mut |callback| {
                    let action = ActionId(*next);
                    *next += 1;
                    handlers.push((action, callback));
                    action
                });
                self.diagnostics.items_built += 1;
                match self.mount_element(Some(element_id), widget) {
                    Ok(child) => {
                        self.diagnostics.items_mounted += 1;
                        child
                    }
                    Err(error) => {
                        panic!("advanced child {key:?} could not mount: {error:?}")
                    }
                }
            };
            retained.insert(child);
            next_keys.push(key);
            next_children.push(child);
        }

        for child in old_children {
            if !retained.contains(&child) {
                self.unmount_element(child);
                self.diagnostics.items_unmounted += 1;
            } else {
                self.diagnostics.items_reused += 1;
            }
        }

        let element = self
            .elements
            .get_mut(element_id.0)
            .expect("advanced scrolling element");
        element.children = next_children;
        element.advanced_child_keys = next_keys;
        element.sliver_child_ids.clear();
        element.sliver_child_semantic_indices.clear();
        element.sliver_pinned_ids.clear();
        self.sync_render_children(element_id);
    }

    pub(super) fn materialize_layout_builder(
        &mut self,
        id: RenderObjectId,
        constraints: Constraints,
    ) {
        stacker::maybe_grow(128 * 1024, 2 * 1024 * 1024, || {
            self.materialize_layout_builder_inner(id, constraints);
        });
    }

    pub(super) fn materialize_layout_builder_inner(
        &mut self,
        id: RenderObjectId,
        constraints: Constraints,
    ) {
        let element_id = self.element_for_render(id).expect("layout builder element");
        let _build_guard = self.guard_element(FramePhase::Build, element_id);
        let (
            builder,
            environment_boundary,
            revision,
            previous_constraints,
            previous_revision,
            previous_children,
        ) = {
            let element = self
                .elements
                .get(element_id.0)
                .expect("layout builder element");
            let WidgetKind::LayoutBuilder {
                builder,
                environment_boundary,
                revision,
                ..
            } = &element.widget.kind
            else {
                return;
            };
            (
                builder.clone(),
                *environment_boundary,
                revision.clone(),
                element.layout_builder_constraints,
                element.layout_builder_revision,
                element.children.clone(),
            )
        };
        let revision_value = revision.as_ref().map_or(0, |revision| revision.get());
        if previous_constraints == Some(constraints)
            && previous_revision == revision_value
            && previous_children.len() == 1
        {
            return;
        }
        let environment = self
            .elements
            .get(element_id.0)
            .and_then(|element| element.environment.clone());
        let mut child = if environment_boundary {
            with_build_environment_boundary(|| builder(constraints))
        } else {
            with_build_environment(environment, || builder(constraints))
        };
        let handlers = &mut self.pending_handlers;
        let next = &mut self.next_action;
        child.bind_callbacks(&mut |callback| {
            let action = ActionId(*next);
            *next += 1;
            handlers.push((action, callback));
            action
        });
        let children = self
            .reconcile_children(element_id, previous_children, &[&child])
            .expect("layout builder child must have unique sibling keys");
        let element = self
            .elements
            .get_mut(element_id.0)
            .expect("layout builder element");
        element.children = children;
        element.layout_builder_constraints = Some(constraints);
        element.layout_builder_revision = revision_value;
        self.sync_render_children(element_id);
    }
    /// Returns whether the current retained sliver children still cover the
    /// viewport's requested cache window. A small scroll can therefore stay a
    /// compositor-only transform; layout is needed only when a cache boundary
    /// is crossed or the delegate itself changed.
    pub(super) fn sliver_cache_window_is_covered(
        &self,
        render_id: RenderObjectId,
        element: &Element,
        config: &SliverViewportConfig,
    ) -> bool {
        let Some(render) = self.renders.get(render_id.0) else {
            return false;
        };
        let viewport = config.axis.main_extent(render.size).max(0.);
        let cache = config.cache_extent.max(0.);
        let content_end = (config.controller.max_offset() + viewport).max(0.);
        let physical =
            physical_scroll_offset(&config.controller, config.reverse).clamp(0., content_end);
        if !physical.is_finite()
            || !viewport.is_finite()
            || !cache.is_finite()
            || !content_end.is_finite()
        {
            return false;
        }
        let wanted_start = (physical - cache).max(0.);
        let wanted_end = (physical + viewport + cache)
            .min(content_end)
            .max(wanted_start);
        let mut materialized_start = f32::INFINITY;
        let mut materialized_end = f32::NEG_INFINITY;
        for child in &element.children {
            let Some(child_element) = self.elements.get(child.0) else {
                return false;
            };
            let Some(child_render) = self.renders.get(child_element.render.0) else {
                return false;
            };
            let child_start = match config.axis {
                Axis::Horizontal => child_render.offset.x,
                Axis::Vertical => child_render.offset.y,
            };
            let child_extent = config.axis.main_extent(child_render.size).max(0.);
            if !child_start.is_finite() || !child_extent.is_finite() {
                return false;
            }
            materialized_start = materialized_start.min(child_start);
            materialized_end = materialized_end.max(child_start + child_extent);
        }
        if !materialized_start.is_finite() || !materialized_end.is_finite() {
            return content_end == 0.;
        }
        const RANGE_EPSILON: f32 = 0.001;
        materialized_start <= wanted_start + RANGE_EPSILON
            && materialized_end + RANGE_EPSILON >= wanted_end
    }

    /// A controller or sliver delegate can change independently of widget
    /// BUILD. Mark the retained viewport dirty only when its sliver protocol
    /// needs a new cache window or its delegate changed.
    pub(super) fn refresh_sliver_ranges(&mut self) {
        let pending = self
            .renders
            .iter()
            .filter_map(|(raw, render)| {
                let RenderKind::SliverViewport { config } = &render.object.kind else {
                    return None;
                };
                let element = self.element_for_render(RenderObjectId(raw))?;
                let element = self.elements.get(element.0)?;
                let delegate_changed =
                    element.sliver_delegate_revision != config.delegate.revision();
                let scroll_changed = element.sliver_scroll_revision != config.controller.revision();
                (delegate_changed
                    || (scroll_changed
                        && !self.sliver_cache_window_is_covered(
                            RenderObjectId(raw),
                            element,
                            config,
                        )))
                .then_some(RenderObjectId(raw))
            })
            .collect::<Vec<_>>();
        for render in pending {
            self.mark_render_dirty(render, DirtyFlags::LAYOUT, true);
        }
    }

    /// Rebuilds the retained notification subscriptions after layout. A
    /// sliver viewport owns its materialized children, so installing these
    /// subscriptions after materialization lets an ancestor listener observe
    /// both ordinary and newly visible nested viewports without rebuilding the
    /// application widget description.
    pub(super) fn refresh_notification_listeners(&mut self) {
        let mut listeners = self
            .elements
            .iter()
            .filter_map(|(raw, element)| {
                let WidgetKind::NotificationListener {
                    callback: Some(callback),
                    ..
                } = &element.widget.kind
                else {
                    return None;
                };
                let id = ElementId(raw);
                Some((id, callback.clone(), self.element_depth(id)))
            })
            .collect::<Vec<_>>();

        // Drop old registrations first. This also removes listeners for
        // children that left a lazy cache window.
        for (_, element) in self.elements.iter_mut() {
            element.notification_subscriptions.clear();
        }

        // Controller dispatch is stop-on-true. Register the deepest wrapper
        // first so an inner listener gets first refusal, matching bubbling up
        // through Flutter's notification tree.
        listeners.sort_by_key(|(_, _, depth)| std::cmp::Reverse(*depth));
        for (listener, callback, _) in listeners {
            let mut sources = Vec::new();
            self.collect_scroll_sources(listener, 0, &mut sources);
            let subscriptions = sources
                .into_iter()
                .map(|(controller, depth)| {
                    let callback = callback.clone();
                    controller.add_listener(move |mut notification| {
                        notification.depth = depth;
                        callback(notification)
                    })
                })
                .collect::<Vec<_>>();
            if let Some(element) = self.elements.get_mut(listener.0) {
                element.notification_subscriptions = subscriptions;
            }
        }
    }

    pub(super) fn element_depth(&self, mut id: ElementId) -> usize {
        let mut depth = 0;
        while let Some(parent) = self.elements.get(id.0).and_then(|element| element.parent) {
            depth += 1;
            id = parent;
        }
        depth
    }

    pub(super) fn scroll_controller_for_element(&self, id: ElementId) -> Option<ScrollController> {
        let render = self.elements.get(id.0)?.render;
        match &self.renders.get(render.0)?.object.kind {
            RenderKind::Scroll { controller, .. } => Some(controller.clone()),
            RenderKind::SliverViewport { config } => Some(config.controller.clone()),
            _ => None,
        }
    }

    pub(super) fn collect_scroll_sources(
        &self,
        id: ElementId,
        viewport_depth: usize,
        out: &mut Vec<(ScrollController, usize)>,
    ) {
        let Some(element) = self.elements.get(id.0) else {
            return;
        };
        let next_depth = if let Some(controller) = self.scroll_controller_for_element(id) {
            if !out.iter().any(|(existing, _)| *existing == controller) {
                out.push((controller, viewport_depth));
            }
            viewport_depth + 1
        } else {
            viewport_depth
        };
        for child in &element.children {
            self.collect_scroll_sources(*child, next_depth, out);
        }
    }

    pub(super) fn unmount(&mut self, id: ElementId) -> Result<(), TreeError> {
        if !self.elements.contains(id.0) {
            return Err(TreeError::MissingElement(id));
        }
        self.unmount_element(id);
        if self.root == Some(id) {
            self.root = None;
        }
        self.refresh_selection_states();
        Ok(())
    }
    pub(super) fn unmount_element(&mut self, id: ElementId) {
        self.raw_input_unmounted(id);
        let Some(element) = self.elements.remove(id.0) else {
            return;
        };
        match &element.widget.kind {
            WidgetKind::SelectionListener {
                notifier, delegate, ..
            } => {
                notifier.unregister();
                self.static_selections.remove(&id);
                let controller = delegate.controller();
                controller.set_registered_child_count(0);
                controller.clear_selection();
            }
            WidgetKind::SelectionArea { controller, .. } => {
                self.static_selections.remove(&id);
                controller.set_registered_child_count(0);
                controller.clear_selection();
            }
            WidgetKind::SelectionContainer { delegate, .. } => {
                self.static_selections.remove(&id);
                let controller = delegate.controller();
                controller.set_registered_child_count(0);
                controller.clear_selection();
            }
            _ => {}
        }
        self.active_gestures.retain(|_, active| {
            active
                .members
                .iter()
                .all(|candidate| candidate.element != id)
        });
        self.pointer_captures.retain(|_, target| *target != id);
        self.scale_gestures.remove(&id);
        for child in element.children {
            self.unmount_element(child);
        }
        if let Some(render) = self.renders.remove(element.render.0) {
            if let Some(picture) = render.object.layers.picture {
                self.compositor.remove(picture);
            }
            if let Some(picture) = render.object.layers.focus_picture {
                self.compositor.remove(picture);
            }
            if let Some(layer) = render.object.layers.clip {
                self.compositor.remove(layer);
            }
            if let Some(layer) = render.object.layers.content {
                self.compositor.remove(layer);
            }
            if let Some(layer) = render.object.layers.opacity {
                self.compositor.remove(layer);
            }
            if let Some(layer) = render.object.layers.blur {
                self.compositor.remove(layer);
            }
            if let Some(layer) = render.object.layers.shadow {
                self.compositor.remove(layer);
            }
            if let Some(layer) = render.object.layers.color_filter {
                self.compositor.remove(layer);
            }
            if let Some(layer) = render.object.layers.blend {
                self.compositor.remove(layer);
            }
            if let Some(layer) = render.object.layers.shader_mask {
                self.compositor.remove(layer);
            }
            if let Some(layer) = render.object.layers.backdrop_filter {
                self.compositor.remove(layer);
            }
            if let Some(layer) = render.object.layers.annotation {
                self.compositor.remove(layer);
            }
            if let Some(layer) = render.object.layers.leader {
                self.compositor.remove(layer);
            }
            if let Some(layer) = render.object.layers.follower {
                self.compositor.remove(layer);
            }
            self.compositor.remove(render.object.layers.root);
        }
        self.unmounted.push(id);
        self.diagnostics.unmounts += 1;
    }
    pub(super) fn check_keys_borrowed<'a>(
        &self,
        widgets: impl IntoIterator<Item = &'a Widget>,
    ) -> Result<(), TreeError> {
        let mut keys: HashSet<&Key> = HashSet::new();
        for key in widgets.into_iter().filter_map(Widget::key) {
            if !keys.insert(key) {
                return Err(TreeError::DuplicateKey(key.clone()));
            }
        }
        Ok(())
    }
    pub(super) fn sync_render_children(&mut self, id: ElementId) {
        let (render, children) = {
            let e = self.elements.get(id.0).expect("mounted");
            (e.render, e.children.clone())
        };
        let render_children: Vec<_> = children
            .into_iter()
            .filter_map(|child| self.render_id(child))
            .collect();
        let children_changed = {
            let node = self.renders.get_mut(render.0).expect("mounted");
            let changed = node.children != render_children;
            node.children = render_children.clone();
            if changed {
                node.dirty.insert(DirtyFlags::LAYOUT | DirtyFlags::PAINT);
            }
            changed
        };
        let (
            layer,
            picture,
            focus_picture,
            content_layer,
            opacity_layer,
            blur_layer,
            shadow_layer,
            color_filter_layer,
            blend_layer,
            shader_mask_layer,
            backdrop_filter_layer,
            annotation_layer,
            leader_layer,
            follower_layer,
            kind,
        ) = {
            let node = self.renders.get(render.0).expect("mounted");
            (
                node.object.layers.root,
                node.object.layers.picture,
                node.object.layers.focus_picture,
                node.object.layers.content,
                node.object.layers.opacity,
                node.object.layers.blur,
                node.object.layers.shadow,
                node.object.layers.color_filter,
                node.object.layers.blend,
                node.object.layers.shader_mask,
                node.object.layers.backdrop_filter,
                node.object.layers.annotation,
                node.object.layers.leader,
                node.object.layers.follower,
                node.object.kind.clone(),
            )
        };
        let mut child_layers = render_children
            .iter()
            .filter_map(|child| {
                self.renders
                    .get(child.0)
                    .map(|render| render.object.layers.root)
            })
            .collect::<Vec<_>>();
        if let RenderKind::IndexedStack { index, .. } = kind {
            child_layers = child_layers.get(index).copied().into_iter().collect();
        } else if matches!(kind, RenderKind::SliverViewport { .. }) {
            let pinned = self
                .element_for_render(render)
                .and_then(|element| self.elements.get(element.0))
                .map(|element| element.sliver_pinned_ids.clone())
                .unwrap_or_default();
            let mut order = (0..render_children.len()).collect::<Vec<_>>();
            order.sort_by_key(|index| {
                let child_id = self
                    .element_for_render(render)
                    .and_then(|element| self.elements.get(element.0))
                    .and_then(|element| element.sliver_child_ids.get(*index))
                    .copied();
                usize::from(child_id.is_some_and(|id| pinned.contains(&id)))
            });
            child_layers = order
                .into_iter()
                .filter_map(|index| child_layers.get(index).copied())
                .collect();
        }
        if let RenderKind::Banner { .. } = kind {
            let mut banner_layers =
                Vec::with_capacity(child_layers.len() + usize::from(picture.is_some()));
            banner_layers.extend(child_layers);
            if let (Some(shadow), Some(picture)) = (shadow_layer, picture) {
                self.compositor.set_children(shadow, vec![picture]);
                banner_layers.push(shadow);
            } else {
                banner_layers.extend(picture);
            }
            self.compositor.set_children(layer, banner_layers);
        } else if let Some(opacity) = opacity_layer {
            self.compositor.set_children(opacity, child_layers);
            self.compositor.set_children(layer, vec![opacity]);
        } else if let Some(blur) = blur_layer {
            self.compositor.set_children(blur, child_layers);
            self.compositor.set_children(layer, vec![blur]);
        } else if let Some(shadow) = shadow_layer {
            self.compositor.set_children(shadow, child_layers);
            self.compositor.set_children(layer, vec![shadow]);
        } else if let Some(color_filter) = color_filter_layer {
            self.compositor.set_children(color_filter, child_layers);
            self.compositor.set_children(layer, vec![color_filter]);
        } else if let Some(blend) = blend_layer {
            self.compositor.set_children(blend, child_layers);
            self.compositor.set_children(layer, vec![blend]);
        } else if let Some(shader_mask) = shader_mask_layer {
            self.compositor.set_children(shader_mask, child_layers);
            self.compositor.set_children(layer, vec![shader_mask]);
        } else if let Some(backdrop) = backdrop_filter_layer {
            self.compositor.set_children(backdrop, child_layers);
            self.compositor.set_children(layer, vec![backdrop]);
        } else if let Some(annotation) = annotation_layer {
            self.compositor.set_children(annotation, child_layers);
            self.compositor.set_children(layer, vec![annotation]);
        } else if let Some(leader) = leader_layer {
            self.compositor.set_children(leader, child_layers);
            self.compositor.set_children(layer, vec![leader]);
        } else if let Some(follower) = follower_layer {
            self.compositor.set_children(follower, child_layers);
            self.compositor.set_children(layer, vec![follower]);
        } else if let Some(content) = content_layer {
            self.compositor.set_children(content, child_layers);
        } else {
            let mut layers =
                Vec::with_capacity(child_layers.len() + 1 + usize::from(focus_picture.is_some()));
            layers.extend(picture);
            layers.extend(child_layers);
            layers.extend(focus_picture);
            self.compositor.set_children(layer, layers);
        }
        for child in render_children {
            self.renders.get_mut(child.0).expect("mounted").parent = Some(render);
        }
        if children_changed {
            self.mark_render_dirty(render, DirtyFlags::LAYOUT | DirtyFlags::PAINT, true);
        }
    }
}
