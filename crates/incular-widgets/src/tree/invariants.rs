//! Retained-tree invariant access and expensive consistency verification.
//!
//! Public/runtime boundary failures remain [`TreeError`]. The helpers here are
//! for states that can only arise from framework corruption after an identity
//! has already been proven live by the retained lifecycle.

use super::*;
use crate::render_object::{
    RenderButtonState, RenderDraggableSheetState, RenderRawScrollbarState, RenderScrollState,
    RenderSelectableTextState, RenderTextFieldState, RenderTwoDimensionalState, RenderWheelState,
};

pub(super) trait LiveElementId {
    fn live_element_id(self) -> ElementId;
}
impl LiveElementId for ElementId {
    fn live_element_id(self) -> ElementId {
        self
    }
}
impl LiveElementId for &ElementId {
    fn live_element_id(self) -> ElementId {
        *self
    }
}

pub(super) trait LiveRenderId {
    fn live_render_id(self) -> RenderObjectId;
}
impl LiveRenderId for RenderObjectId {
    fn live_render_id(self) -> RenderObjectId {
        self
    }
}
impl LiveRenderId for &RenderObjectId {
    fn live_render_id(self) -> RenderObjectId {
        *self
    }
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InvariantCategory {
    Identity,
    Topology,
    Ownership,
    FeatureState,
    Compositor,
    Semantics,
    Interaction,
    DynamicChildren,
}

#[doc(hidden)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvariantViolation {
    pub category: InvariantCategory,
    pub description: String,
    pub element: Option<ElementId>,
    pub render: Option<RenderObjectId>,
}

impl std::fmt::Display for InvariantViolation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{:?}: {}", self.category, self.description)?;
        if let Some(element) = self.element {
            write!(formatter, " element={element:?}")?;
        }
        if let Some(render) = self.render {
            write!(formatter, " render={render:?}")?;
        }
        Ok(())
    }
}

impl std::error::Error for InvariantViolation {}

impl WidgetTree {
    #[inline]
    pub(super) fn element_live(
        &self,
        id: impl LiveElementId,
        description: &'static str,
    ) -> &Element {
        let id = id.live_element_id();
        match self.elements.get(id.0) {
            Some(element) => element,
            None => self.panic_invariant(InvariantCategory::Identity, Some(id), None, description),
        }
    }

    #[inline]
    pub(super) fn element_live_mut(
        &mut self,
        id: impl LiveElementId,
        description: &'static str,
    ) -> &mut Element {
        let id = id.live_element_id();
        if !self.elements.contains(id.0) {
            self.panic_invariant(InvariantCategory::Identity, Some(id), None, description);
        }
        self.elements
            .get_mut(id.0)
            .expect("invariant access prechecked live element")
    }

    #[inline]
    pub(super) fn render_live(
        &self,
        id: impl LiveRenderId,
        description: &'static str,
    ) -> &RenderNode {
        let id = id.live_render_id();
        match self.renders.get(id.0) {
            Some(render) => render,
            None => self.panic_invariant(InvariantCategory::Identity, None, Some(id), description),
        }
    }

    #[inline]
    pub(super) fn render_live_mut(
        &mut self,
        id: impl LiveRenderId,
        description: &'static str,
    ) -> &mut RenderNode {
        let id = id.live_render_id();
        if !self.renders.contains(id.0) {
            self.panic_invariant(InvariantCategory::Identity, None, Some(id), description);
        }
        self.renders
            .get_mut(id.0)
            .expect("invariant access prechecked live render")
    }

    #[inline]
    pub(super) fn render_for_live_element(
        &self,
        id: ElementId,
        description: &'static str,
    ) -> &RenderNode {
        let render = self.element_live(id, description).render;
        match self.renders.get(render.0) {
            Some(node) => node,
            None => self.panic_invariant(
                InvariantCategory::Ownership,
                Some(id),
                Some(render),
                description,
            ),
        }
    }

    #[inline]
    pub(super) fn button_state_live(&self, id: impl LiveRenderId) -> &RenderButtonState {
        let id = id.live_render_id();
        match self
            .render_live(id, "button render must remain live")
            .button_state()
        {
            Some(state) => state,
            None => self.panic_invariant(
                InvariantCategory::FeatureState,
                self.element_for_render(id),
                Some(id),
                "button render must own button state",
            ),
        }
    }

    #[inline]
    pub(super) fn selectable_text_state_live(
        &self,
        id: impl LiveRenderId,
    ) -> &RenderSelectableTextState {
        let id = id.live_render_id();
        match self
            .render_live(id, "selectable-text render must remain live")
            .selectable_text_state()
        {
            Some(state) => state,
            None => self.panic_invariant(
                InvariantCategory::FeatureState,
                self.element_for_render(id),
                Some(id),
                "selectable-text render must own selectable state",
            ),
        }
    }

    #[inline]
    pub(super) fn text_field_state_live(&self, id: impl LiveRenderId) -> &RenderTextFieldState {
        let id = id.live_render_id();
        match self
            .render_live(id, "text-field render must remain live")
            .text_field_state()
        {
            Some(state) => state,
            None => self.panic_invariant(
                InvariantCategory::FeatureState,
                self.element_for_render(id),
                Some(id),
                "text-field render must own text-field state",
            ),
        }
    }

    #[inline]
    pub(super) fn text_field_state_live_mut(
        &mut self,
        id: impl LiveRenderId,
    ) -> &mut RenderTextFieldState {
        let id = id.live_render_id();
        if self
            .render_live(id, "text-field render must remain live")
            .text_field_state()
            .is_none()
        {
            self.panic_invariant(
                InvariantCategory::FeatureState,
                self.element_for_render(id),
                Some(id),
                "text-field render must own text-field state",
            );
        }
        self.render_live_mut(id, "text-field render must remain live")
            .text_field_state_mut()
            .expect("feature compatibility prechecked text-field state")
    }

    #[inline]
    pub(super) fn raw_scrollbar_state_live_mut(
        &mut self,
        id: impl LiveRenderId,
    ) -> &mut RenderRawScrollbarState {
        let id = id.live_render_id();
        if self
            .render_live(id, "raw-scrollbar render must remain live")
            .raw_scrollbar_state()
            .is_none()
        {
            self.panic_invariant(
                InvariantCategory::FeatureState,
                self.element_for_render(id),
                Some(id),
                "raw-scrollbar render must own scrollbar state",
            );
        }
        self.render_live_mut(id, "raw-scrollbar render must remain live")
            .raw_scrollbar_state_mut()
            .expect("feature compatibility prechecked raw-scrollbar state")
    }

    #[inline]
    pub(super) fn wheel_state_live(&self, id: impl LiveRenderId) -> &RenderWheelState {
        let id = id.live_render_id();
        match self
            .render_live(id, "wheel render must remain live")
            .wheel_state()
        {
            Some(state) => state,
            None => self.panic_invariant(
                InvariantCategory::FeatureState,
                self.element_for_render(id),
                Some(id),
                "wheel render must own wheel state",
            ),
        }
    }

    #[inline]
    pub(super) fn wheel_state_live_mut(&mut self, id: impl LiveRenderId) -> &mut RenderWheelState {
        let id = id.live_render_id();
        if self
            .render_live(id, "wheel render must remain live")
            .wheel_state()
            .is_none()
        {
            self.panic_invariant(
                InvariantCategory::FeatureState,
                self.element_for_render(id),
                Some(id),
                "wheel render must own wheel state",
            );
        }
        self.render_live_mut(id, "wheel render must remain live")
            .wheel_state_mut()
            .expect("feature compatibility prechecked wheel state")
    }

    #[inline]
    pub(super) fn draggable_sheet_state_live_mut(
        &mut self,
        id: impl LiveRenderId,
    ) -> &mut RenderDraggableSheetState {
        let id = id.live_render_id();
        if self
            .render_live(id, "draggable-sheet render must remain live")
            .draggable_sheet_state()
            .is_none()
        {
            self.panic_invariant(
                InvariantCategory::FeatureState,
                self.element_for_render(id),
                Some(id),
                "draggable-sheet render must own draggable state",
            );
        }
        self.render_live_mut(id, "draggable-sheet render must remain live")
            .draggable_sheet_state_mut()
            .expect("feature compatibility prechecked draggable-sheet state")
    }

    #[inline]
    pub(super) fn two_dimensional_state_live(
        &self,
        id: impl LiveRenderId,
    ) -> &RenderTwoDimensionalState {
        let id = id.live_render_id();
        match self
            .render_live(id, "two-dimensional render must remain live")
            .two_dimensional_state()
        {
            Some(state) => state,
            None => self.panic_invariant(
                InvariantCategory::FeatureState,
                self.element_for_render(id),
                Some(id),
                "two-dimensional render must own viewport state",
            ),
        }
    }

    #[inline]
    pub(super) fn two_dimensional_state_live_mut(
        &mut self,
        id: impl LiveRenderId,
    ) -> &mut RenderTwoDimensionalState {
        let id = id.live_render_id();
        if self
            .render_live(id, "two-dimensional render must remain live")
            .two_dimensional_state()
            .is_none()
        {
            self.panic_invariant(
                InvariantCategory::FeatureState,
                self.element_for_render(id),
                Some(id),
                "two-dimensional render must own viewport state",
            );
        }
        self.render_live_mut(id, "two-dimensional render must remain live")
            .two_dimensional_state_mut()
            .expect("feature compatibility prechecked two-dimensional state")
    }

    #[inline]
    pub(super) fn scroll_state_live(&self, id: impl LiveRenderId) -> &RenderScrollState {
        let id = id.live_render_id();
        match self
            .render_live(id, "scroll render must remain live")
            .scroll_state()
        {
            Some(state) => state,
            None => self.panic_invariant(
                InvariantCategory::FeatureState,
                self.element_for_render(id),
                Some(id),
                "scroll render must own scroll state",
            ),
        }
    }

    #[inline]
    pub(super) fn scroll_state_live_mut(
        &mut self,
        id: impl LiveRenderId,
    ) -> &mut RenderScrollState {
        let id = id.live_render_id();
        if self
            .render_live(id, "scroll render must remain live")
            .scroll_state()
            .is_none()
        {
            self.panic_invariant(
                InvariantCategory::FeatureState,
                self.element_for_render(id),
                Some(id),
                "scroll render must own scroll state",
            );
        }
        self.render_live_mut(id, "scroll render must remain live")
            .scroll_state_mut()
            .expect("feature compatibility prechecked scroll state")
    }

    #[cold]
    #[track_caller]
    pub(super) fn panic_invariant(
        &self,
        category: InvariantCategory,
        element: Option<ElementId>,
        render: Option<RenderObjectId>,
        description: &'static str,
    ) -> ! {
        let (phase, path) = self.recursion_diagnostics.invariant_context();
        let element_context = element.and_then(|id| {
            self.elements.get(id.0).map(|node| {
                format!(
                    "widget={:?} parent={:?} owned_render={:?}",
                    node.widget.type_(),
                    node.parent,
                    node.render
                )
            })
        });
        let render_context = render.and_then(|id| {
            self.renders
                .get(id.0)
                .map(|node| format!("kind={:?} parent={:?}", node.object.kind, node.parent))
        });
        panic!(
            "Incular retained-tree invariant violation [{category:?}]: {description}; phase={}; element={element:?} {}; render={render:?} {}; path=[{}]",
            phase.map_or_else(|| "idle".into(), |phase| phase.to_string()),
            element_context.as_deref().unwrap_or(""),
            render_context.as_deref().unwrap_or(""),
            path.join(" -> "),
        );
    }

    fn invariant_error(
        category: InvariantCategory,
        description: impl Into<String>,
        element: Option<ElementId>,
        render: Option<RenderObjectId>,
    ) -> InvariantViolation {
        InvariantViolation {
            category,
            description: description.into(),
            element,
            render,
        }
    }

    /// Performs an intentionally expensive, read-only consistency pass over
    /// the retained UI, render, compositor, semantic, and interaction graphs.
    /// It is a diagnostic/test tool and is never run automatically per frame.
    #[doc(hidden)]
    pub fn verify_invariants(&self) -> Result<(), InvariantViolation> {
        if let Some(root) = self.root {
            let element = self.elements.get(root.0).ok_or_else(|| {
                Self::invariant_error(
                    InvariantCategory::Identity,
                    "tree root does not name a live element",
                    Some(root),
                    None,
                )
            })?;
            if element.parent.is_some() {
                return Err(Self::invariant_error(
                    InvariantCategory::Topology,
                    "tree root has an element parent",
                    Some(root),
                    Some(element.render),
                ));
            }
        } else if !self.elements.is_empty() || !self.renders.is_empty() {
            return Err(Self::invariant_error(
                InvariantCategory::Ownership,
                "retained arenas are non-empty while tree root is absent",
                None,
                None,
            ));
        }

        let mut render_owners = HashMap::with_capacity(self.renders.len());
        for (raw, element) in self.elements.iter() {
            let id = ElementId(raw);
            let render = self.renders.get(element.render.0).ok_or_else(|| {
                Self::invariant_error(
                    InvariantCategory::Ownership,
                    "live element owns a dead render object",
                    Some(id),
                    Some(element.render),
                )
            })?;
            if let Some(previous) = render_owners.insert(element.render, id) {
                return Err(Self::invariant_error(
                    InvariantCategory::Ownership,
                    format!("render object is owned by both {previous:?} and {id:?}"),
                    Some(id),
                    Some(element.render),
                ));
            }

            match element.parent {
                Some(parent) => {
                    let parent_node = self.elements.get(parent.0).ok_or_else(|| {
                        Self::invariant_error(
                            InvariantCategory::Topology,
                            "element parent is not live",
                            Some(id),
                            Some(element.render),
                        )
                    })?;
                    if parent_node
                        .children
                        .iter()
                        .filter(|child| **child == id)
                        .count()
                        != 1
                    {
                        return Err(Self::invariant_error(
                            InvariantCategory::Topology,
                            "element parent/child relationship is not symmetric exactly once",
                            Some(id),
                            Some(element.render),
                        ));
                    }
                    let parent_render =
                        self.renders.get(parent_node.render.0).ok_or_else(|| {
                            Self::invariant_error(
                                InvariantCategory::Ownership,
                                "element parent owns a dead render object",
                                Some(parent),
                                Some(parent_node.render),
                            )
                        })?;
                    if render.parent != Some(parent_node.render)
                        || parent_render
                            .children
                            .iter()
                            .filter(|child| **child == element.render)
                            .count()
                            != 1
                    {
                        return Err(Self::invariant_error(
                            InvariantCategory::Topology,
                            "element and render parent relationships disagree",
                            Some(id),
                            Some(element.render),
                        ));
                    }
                }
                None if render.parent.is_some() => {
                    return Err(Self::invariant_error(
                        InvariantCategory::Topology,
                        "root-level element render has a parent",
                        Some(id),
                        Some(element.render),
                    ));
                }
                None => {}
            }

            if render.children.len() != element.children.len() {
                return Err(Self::invariant_error(
                    InvariantCategory::Topology,
                    "element and render child counts disagree",
                    Some(id),
                    Some(element.render),
                ));
            }
            for (element_child, render_child) in element.children.iter().zip(&render.children) {
                let child = self.elements.get(element_child.0).ok_or_else(|| {
                    Self::invariant_error(
                        InvariantCategory::Topology,
                        "element child is not live",
                        Some(*element_child),
                        None,
                    )
                })?;
                if child.parent != Some(id) || child.render != *render_child {
                    return Err(Self::invariant_error(
                        InvariantCategory::Topology,
                        "element/render child ordering or parent link disagrees",
                        Some(*element_child),
                        Some(*render_child),
                    ));
                }
            }

            if matches!(element.widget.kind(), WidgetKind::SliverViewport { .. }) {
                if element.sliver_child_ids.len() != element.children.len()
                    || element.sliver_child_semantic_indices.len() != element.children.len()
                {
                    return Err(Self::invariant_error(
                        InvariantCategory::DynamicChildren,
                        "sliver child bookkeeping is not parallel to retained children",
                        Some(id),
                        Some(element.render),
                    ));
                }
            } else if !element.sliver_child_ids.is_empty()
                || !element.sliver_child_semantic_indices.is_empty()
                || !element.sliver_pinned_ids.is_empty()
            {
                return Err(Self::invariant_error(
                    InvariantCategory::DynamicChildren,
                    "non-sliver element retains sliver child bookkeeping",
                    Some(id),
                    Some(element.render),
                ));
            }
            if !element.advanced_child_keys.is_empty()
                && element.advanced_child_keys.len() != element.children.len()
            {
                return Err(Self::invariant_error(
                    InvariantCategory::DynamicChildren,
                    "advanced-scrolling keys are not parallel to retained children",
                    Some(id),
                    Some(element.render),
                ));
            }
        }

        for (raw, render) in self.renders.iter() {
            let id = RenderObjectId(raw);
            if !render_owners.contains_key(&id) {
                return Err(Self::invariant_error(
                    InvariantCategory::Ownership,
                    "live render object has no live element owner",
                    None,
                    Some(id),
                ));
            }
            if !render.feature_matches_kind() {
                let (expected, actual) = render.feature_class_names();
                return Err(Self::invariant_error(
                    InvariantCategory::FeatureState,
                    format!("render kind requires feature class {expected}, found {actual}"),
                    render_owners.get(&id).copied(),
                    Some(id),
                ));
            }
            for layer in render.object.layers.owned_ids().into_iter().flatten() {
                if !self.compositor.contains(layer) {
                    return Err(Self::invariant_error(
                        InvariantCategory::Compositor,
                        format!("render object owns dead compositor layer {layer:?}"),
                        render_owners.get(&id).copied(),
                        Some(id),
                    ));
                }
            }
        }

        if let Some(root) = self.root {
            let render_root = self
                .elements
                .get(root.0)
                .and_then(|element| self.renders.get(element.render.0))
                .map(|render| render.object.layers.root);
            if self.compositor.root() != render_root {
                return Err(Self::invariant_error(
                    InvariantCategory::Compositor,
                    "compositor root does not match retained render root",
                    Some(root),
                    self.elements.get(root.0).map(|element| element.render),
                ));
            }
        } else if self.compositor.root().is_some() {
            return Err(Self::invariant_error(
                InvariantCategory::Compositor,
                "compositor root remains after retained root removal",
                None,
                None,
            ));
        }

        for (&element, &semantic) in &self.semantic_ids {
            if !self.elements.contains(element.0) {
                return Err(Self::invariant_error(
                    InvariantCategory::Semantics,
                    "semantic mapping references a dead element",
                    Some(element),
                    None,
                ));
            }
            if self.semantics.node(semantic).is_none() {
                return Err(Self::invariant_error(
                    InvariantCategory::Semantics,
                    format!("semantic mapping references dead semantic node {semantic:?}"),
                    Some(element),
                    None,
                ));
            }
        }
        for (_, node) in self.semantics.iter() {
            if node
                .children
                .iter()
                .any(|child| self.semantics.node(*child).is_none())
            {
                return Err(Self::invariant_error(
                    InvariantCategory::Semantics,
                    "semantic node references a dead semantic child",
                    None,
                    None,
                ));
            }
        }
        if self
            .semantics
            .root()
            .is_some_and(|root| self.semantics.node(root).is_none())
        {
            return Err(Self::invariant_error(
                InvariantCategory::Semantics,
                "semantic root is not live",
                None,
                None,
            ));
        }

        let live = |id: &ElementId| self.elements.contains(id.0);
        if let Some((&_, &element)) = self.pointer_captures.iter().find(|(_, id)| !live(id)) {
            return Err(Self::invariant_error(
                InvariantCategory::Interaction,
                "pointer capture references a dead element",
                Some(element),
                None,
            ));
        }
        for active in self.active_gestures.values() {
            if !live(&active.element) || active.members.iter().any(|member| !live(&member.element))
            {
                return Err(Self::invariant_error(
                    InvariantCategory::Interaction,
                    "active gesture references a dead element",
                    Some(active.element),
                    None,
                ));
            }
        }
        for active in self.raw_gesture_streams.values() {
            if !live(&active.element) || active.members.iter().any(|member| !live(&member.element))
            {
                return Err(Self::invariant_error(
                    InvariantCategory::Interaction,
                    "raw gesture stream references a dead element",
                    Some(active.element),
                    None,
                ));
            }
        }
        for route in self
            .raw_pointer_routes
            .values()
            .chain(self.mouse_hover.values())
        {
            if let Some(element) = route.iter().find(|id| !live(id)) {
                return Err(Self::invariant_error(
                    InvariantCategory::Interaction,
                    "pointer route references a dead element",
                    Some(*element),
                    None,
                ));
            }
        }
        if let Some((element, _)) = self
            .active_drags
            .values()
            .filter_map(|drag| drag.target.as_ref())
            .find(|(element, _)| !live(element))
        {
            return Err(Self::invariant_error(
                InvariantCategory::Interaction,
                "active drag target references a dead element",
                Some(*element),
                None,
            ));
        }
        if let Some(element) = self.scale_gestures.keys().find(|id| !live(id)) {
            return Err(Self::invariant_error(
                InvariantCategory::Interaction,
                "scale gesture registry references a dead element",
                Some(*element),
                None,
            ));
        }
        if let Some(drag) = self.scrollbar_drag
            && !self.renders.contains(drag.render.0)
        {
            return Err(Self::invariant_error(
                InvariantCategory::Interaction,
                "scrollbar drag references a dead render object",
                None,
                Some(drag.render),
            ));
        }
        for selection in self.static_selections.values() {
            for element in [
                selection.area,
                selection.anchor.element,
                selection.extent.element,
            ] {
                if !live(&element) {
                    return Err(Self::invariant_error(
                        InvariantCategory::Interaction,
                        "selection state references a dead element",
                        Some(element),
                        None,
                    ));
                }
            }
        }
        for (element, _) in self.inherited_consumers.values() {
            if !live(element) {
                return Err(Self::invariant_error(
                    InvariantCategory::Interaction,
                    "inherited dependency registry references a dead element",
                    Some(*element),
                    None,
                ));
            }
        }

        Ok(())
    }
}
