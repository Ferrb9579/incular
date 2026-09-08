use super::super::*;

/// Computes the constrained shrink-wrap size and viewport main-axis extent
/// from content in one place. Initial sizing and convergence corrections
/// below share this calculation so the reported size, the sliver
/// constraints, the controller metrics, and the scroll geometry cannot drift
/// apart. The nonconvergence fallback intentionally keeps the last published
/// size instead (see below).
fn shrink_wrap_viewport_geometry(
    axis: Axis,
    content_extent: f32,
    constraints: Constraints,
    cross_extent: f32,
) -> (Size, f32) {
    let content_extent = content_extent.max(0.);
    let main = if axis.is_vertical() {
        content_extent.clamp(constraints.min_height(), constraints.max_height())
    } else {
        content_extent.clamp(constraints.min_width(), constraints.max_width())
    };
    let size = constraints.constrain(axis.size(main, cross_extent));
    (size, scroll_viewport_extent(axis, size))
}

/// One published sliver-viewport attempt: the size and viewport extent a
/// sliver layout was produced from, the layout it returned, and the content
/// extent published to the controller for it.
///
/// Performing a layout, publishing its metrics, and applying its children
/// happen at different points below, so those values can momentarily describe
/// different attempts. Recording each publication lets the bounded fallback
/// retain a single attempt instead of mixing state across attempts.
#[derive(Clone, Debug)]
struct PublishedViewportAttempt {
    size: Size,
    viewport_extent: f32,
    content_extent: f32,
    layout: crate::scrolling::SliverViewportLayout,
}

impl WidgetTree {
    /// Reconciles materialized sliver children against `sliver_layout`,
    /// measures each under its constraints, applies offsets, and reports
    /// whether any retained measurement changed. When `record_measurements`
    /// the measured extents are fed back into the delegate so estimates keep
    /// learning; the nonconvergence fallback applies a retained snapshot with
    /// recording disabled so the snapshot's geometry stays authoritative for
    /// the delegate. Either way children are positioned from identical code.
    fn measure_sliver_children(
        &mut self,
        id: RenderObjectId,
        config: &crate::scrolling::SliverViewportConfig,
        sliver_layout: &crate::scrolling::SliverViewportLayout,
        record_measurements: bool,
    ) -> Result<bool, TreeError> {
        self.reconcile_sliver_children(id, config, sliver_layout)?;
        let materialized = self
            .render_live(id, "sliver viewport render must remain live")
            .children
            .clone();
        let mut pass_changed = false;
        for (child, layout) in materialized.into_iter().zip(&sliver_layout.children) {
            self.layout_render(child, layout.constraints)?;
            let measured = config.axis.main_extent(
                self.render_live(child, "sliver child render must remain live")
                    .size,
            );
            if record_measurements {
                pass_changed |= config.delegate.set_child_extent(layout.id, measured);
            }
            let offset = config.axis.offset(layout.offset, layout.cross_offset);
            let layer = {
                let child = self.render_live_mut(child, "sliver child render must remain live");
                child.offset = offset;
                child.object.layers.root
            };
            self.compositor
                .update_transform(layer, CoreTransform::translation(offset));
        }
        Ok(pass_changed)
    }

    pub(super) fn layout_sliver_kind(
        &mut self,
        id: RenderObjectId,
        kind: RenderKind,
        _children: &[RenderObjectId],
        constraints: Constraints,
    ) -> Result<(Size, Vec<Offset>), TreeError> {
        Ok(match kind {
            RenderKind::SliverViewport { config } => {
                config
                    .controller
                    .set_metrics_context(config.axis, config.reverse);
                let mut size = sliver_viewport_size(config.axis, constraints, config.shrink_wrap);
                let mut viewport_extent = scroll_viewport_extent(config.axis, size);
                let cache_extent = config.cache_extent.max(0.);
                let cross_extent = config.axis.cross_extent(size);
                let make_constraints = |physical_offset: f32, viewport: f32| {
                    // Match RenderViewport's forward layout contract for
                    // overscroll: a negative physical position is represented
                    // as leading overlap and a reduced paint extent, while
                    // the sliver scroll offset itself stays non-negative.
                    let overlap = physical_offset.min(0.);
                    SliverConstraints::new(
                        config.axis,
                        config.reverse,
                        physical_offset.max(0.),
                        0.,
                        overlap,
                        (viewport + overlap).max(0.),
                        cross_extent,
                        viewport,
                        viewport + cache_extent * 2.,
                        -cache_extent,
                    )
                };
                // Records each metrics publication so the bounded fallback can
                // retain the last published attempt as one consistent frame.
                // Only shrink-wrapping viewports need it: fixed viewports
                // never resize from content, so they have no cross-attempt
                // size to reconcile.
                let mut last_published: Option<PublishedViewportAttempt> = None;
                let mut publish = |size: Size,
                                   viewport: f32,
                                   layout: crate::scrolling::SliverViewportLayout|
                 -> crate::scrolling::SliverViewportLayout {
                    let content = layout.geometry.scroll_extent.max(0.);
                    config.controller.update_extents_with_physics(
                        content,
                        viewport,
                        config.physics,
                    );
                    if config.shrink_wrap {
                        last_published = Some(PublishedViewportAttempt {
                            size,
                            viewport_extent: viewport,
                            content_extent: content,
                            layout: layout.clone(),
                        });
                    }
                    layout
                };
                let mut sliver_layout = publish(
                    size,
                    viewport_extent,
                    config.delegate.perform_layout(make_constraints(
                        physical_scroll_offset(&config.controller, config.reverse),
                        viewport_extent,
                    )),
                );
                // Content extent the current size/viewport were computed
                // from. Fixed-size viewports never consult it; shrink-wrap
                // viewports keep it in sync below as measurement converges.
                let mut sized_content = sliver_layout.geometry.scroll_extent.max(0.);

                // A shrink-wrapping viewport derives its own main-axis size
                // from the sliver geometry. The first pass supplies a
                // provisional zero/unbounded extent, then the real viewport
                // extent is laid out again before children are materialized.
                if config.shrink_wrap {
                    (size, viewport_extent) = shrink_wrap_viewport_geometry(
                        config.axis,
                        sized_content,
                        constraints,
                        cross_extent,
                    );
                    // Provisional: only its metrics publication matters; the
                    // corrective layout below supersedes its geometry.
                    let _ = publish(
                        size,
                        viewport_extent,
                        config.delegate.perform_layout(make_constraints(
                            physical_scroll_offset(&config.controller, config.reverse),
                            viewport_extent,
                        )),
                    );
                }

                // Extent updates can clamp a restored/programmatic offset or
                // establish the physical origin of a reversed viewport. One
                // corrective layout keeps geometry and the retained range in
                // the same coordinate space from the first frame.
                sliver_layout = publish(
                    size,
                    viewport_extent,
                    config.delegate.perform_layout(make_constraints(
                        physical_scroll_offset(&config.controller, config.reverse),
                        viewport_extent,
                    )),
                );

                let mut converged = false;
                for _ in 0..3 {
                    let physical_before =
                        physical_scroll_offset(&config.controller, config.reverse);
                    let anchor_before = sliver_anchor(&sliver_layout, physical_before);
                    let pass_changed =
                        self.measure_sliver_children(id, &config, &sliver_layout, true)?;
                    // Shrink-wrapping viewports derive size from content, so a
                    // layout whose content no longer matches the size must
                    // re-run with consistent constraints even when no child
                    // measurement changed (viewport-dependent content can move
                    // under a fixed child set). Child-measurement changes and
                    // viewport/geometry changes are distinct reasons for
                    // another pass; stability is established only when both
                    // are quiet, all within this loop's existing bound.
                    let mut geometry_synced = false;
                    if config.shrink_wrap {
                        let authoritative = sliver_layout.geometry.scroll_extent.max(0.);
                        if (authoritative - sized_content).abs() > f32::EPSILON {
                            sized_content = authoritative;
                            (size, viewport_extent) = shrink_wrap_viewport_geometry(
                                config.axis,
                                authoritative,
                                constraints,
                                cross_extent,
                            );
                            geometry_synced = true;
                        }
                    }
                    if !pass_changed && !geometry_synced {
                        converged = true;
                        break;
                    }
                    let mut next_layout = publish(
                        size,
                        viewport_extent,
                        config
                            .delegate
                            .perform_layout(make_constraints(physical_before, viewport_extent)),
                    );
                    // A reversed viewport's physical origin depends on the
                    // newly measured max offset. Recompute the layout after
                    // updating extents before comparing anchors; otherwise a
                    // first-frame estimate change can turn logical offset 0
                    // into an artificial scroll to the middle of the list.
                    let physical_after_extent =
                        physical_scroll_offset(&config.controller, config.reverse);
                    if physical_after_extent != physical_before {
                        next_layout = publish(
                            size,
                            viewport_extent,
                            config.delegate.perform_layout(make_constraints(
                                physical_after_extent,
                                viewport_extent,
                            )),
                        );
                    }
                    if let Some((anchor_id, anchor_offset)) = anchor_before
                        && let Some(updated) = next_layout
                            .children
                            .iter()
                            .find(|child| child.id == anchor_id)
                    {
                        // Compare visual positions, not raw content offsets.
                        // The physical origin may have changed when a reverse
                        // viewport learned its real content extent.
                        let delta = (updated.offset - physical_after_extent)
                            - (anchor_offset - physical_before);
                        if delta.is_finite() && delta.abs() > f32::EPSILON {
                            let corrected_physical = (physical_after_extent + delta).max(0.);
                            let corrected_logical = if config.reverse {
                                (config.controller.max_offset() - corrected_physical).max(0.)
                            } else {
                                corrected_physical
                            };
                            if config.controller.jump_to(corrected_logical) {
                                next_layout = publish(
                                    size,
                                    viewport_extent,
                                    config.delegate.perform_layout(make_constraints(
                                        physical_scroll_offset(&config.controller, config.reverse),
                                        viewport_extent,
                                    )),
                                );
                            }
                        }
                    }
                    sliver_layout = next_layout;
                }
                // Bounded fallback: retain the last published attempt as one
                // consistent frame instead of mixing state across attempts.
                // The loop above exhausts with content still moving when a
                // viewport-dependent sliver keeps answering each new viewport
                // extent with new content. The snapshot is the most recent
                // attempt: its size and viewport extent are the inputs the
                // layout was produced from, its geometry is computed from
                // delegate state after the last recorded measurement, and its
                // metrics are already published.
                //
                // Retaining it means the natural size stays at the snapshot's
                // input basis — left unresolved rather than recomputed from
                // newer content that no applied layout describes. Metrics are
                // republished from the snapshot (identical values, so no
                // notifications fire) before children are positioned, so a
                // clamped offset or a reversed origin cannot shift the
                // coordinate space under already-applied offsets; the scroll
                // correction below then follows the same path as converged
                // layouts. Children are reconciled and laid out under the
                // snapshot's constraints and offsets without recording
                // measurements back into the delegate, so the snapshot's
                // geometry stays authoritative and child coverage matches its
                // paint window. A later dirty layout re-measures from the
                // retained estimates.
                //
                // Measurement work stays bounded: at most three convergence
                // sweeps plus this one positioning sweep — four child
                // measurement sweeps per viewport layout (nested viewports
                // recurse under their own identical bound). No extra passes,
                // frames, or loops are scheduled.
                if config.shrink_wrap
                    && !converged
                    && let Some(snapshot) = last_published
                {
                    config.controller.update_extents_with_physics(
                        snapshot.content_extent,
                        snapshot.viewport_extent,
                        config.physics,
                    );
                    let _ = self.measure_sliver_children(id, &config, &snapshot.layout, false)?;
                    size = snapshot.size;
                    sliver_layout = snapshot.layout;
                }
                if let Some(correction) = sliver_layout.geometry.scroll_offset_correction {
                    let physical = (physical_scroll_offset(&config.controller, config.reverse)
                        + correction)
                        .max(0.);
                    let logical = if config.reverse {
                        (config.controller.max_offset() - physical).max(0.)
                    } else {
                        physical
                    };
                    config.controller.jump_to(logical);
                }
                self.diagnostics.lazy_layouts += 1;
                (size, Vec::new())
            }
            _ => unreachable!("sliver layout received a non-sliver render kind"),
        })
    }
}
