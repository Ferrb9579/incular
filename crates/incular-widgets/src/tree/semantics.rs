//! Semantic-tree construction and semantic action classification.

use super::*;

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct SemanticCollectionContext {
    item_index: Option<usize>,
    set_size: Option<usize>,
}

impl WidgetTree {
    pub fn update_semantics(&mut self) {
        let _phase_guard = self.guard_phase_root(FramePhase::Semantics);
        #[cfg(feature = "devtools")]
        let trace = self
            .root
            .and_then(|root| self.devtools_trace_begin_element(root, TracePhase::Semantics));
        let mut built = Vec::new();
        if let Some(root) = self.root {
            self.collect_semantics(root, None, None, &mut built);
        }
        let live: HashSet<_> = built.iter().map(|node| node.element).collect();
        let stale: Vec<_> = self
            .semantic_ids
            .iter()
            .filter_map(|(element, node)| (!live.contains(element)).then_some((*element, *node)))
            .collect();
        for (element, node) in stale {
            self.semantic_ids.remove(&element);
            let _ = self.semantics.remove(node);
        }
        for build in &built {
            #[cfg(feature = "devtools")]
            let semantic_revision_before = self.semantics.revision();
            let id = *self.semantic_ids.entry(build.element).or_insert_with(|| {
                self.semantics.insert(SemanticNode {
                    id: SemanticNodeId(ArenaId::from_parts(0, 0)),
                    role: build.role,
                    label: build.label.clone(),
                    value: build.value.clone(),
                    description: build.description.clone(),
                    bounds: build.bounds,
                    state: build.state.clone(),
                    actions: build.actions.clone(),
                    children: Vec::new(),
                })
            });
            let children = built
                .iter()
                .filter(|child| child.parent == Some(build.element))
                .filter_map(|child| self.semantic_ids.get(&child.element).copied())
                .collect();
            let _ = self.semantics.update(
                id,
                SemanticNode {
                    id,
                    role: build.role,
                    label: build.label.clone(),
                    value: build.value.clone(),
                    description: build.description.clone(),
                    bounds: build.bounds,
                    state: build.state.clone(),
                    actions: build.actions.clone(),
                    children,
                },
            );
            #[cfg(feature = "devtools")]
            if self.semantics.revision() != semantic_revision_before
                && let Some(element) = self.elements.get_mut(build.element.0)
            {
                element.dev.semantic_updates = element.dev.semantic_updates.saturating_add(1);
            }
        }
        self.semantics.set_root(
            built
                .iter()
                .find(|node| node.parent.is_none())
                .and_then(|node| self.semantic_ids.get(&node.element).copied()),
        );
        #[cfg(feature = "devtools")]
        self.devtools_trace_end(trace);
    }
    pub(super) fn collect_semantics(
        &self,
        element: ElementId,
        semantic_parent: Option<ElementId>,
        collection: Option<SemanticCollectionContext>,
        out: &mut Vec<SemanticBuild>,
    ) {
        let mut work = vec![(element, semantic_parent, collection)];
        while let Some((element, semantic_parent, collection)) = work.pop() {
            let children = self.collect_semantics_inner(element, semantic_parent, collection, out);
            work.extend(children.into_iter().rev());
        }
    }

    pub(super) fn collect_semantics_inner(
        &self,
        element: ElementId,
        semantic_parent: Option<ElementId>,
        collection: Option<SemanticCollectionContext>,
        out: &mut Vec<SemanticBuild>,
    ) -> Vec<(
        ElementId,
        Option<ElementId>,
        Option<SemanticCollectionContext>,
    )> {
        let _node_guard = self.guard_element(FramePhase::Semantics, element);
        let Some(entry) = self.elements.get(element.0) else {
            return Vec::new();
        };
        if entry.widget.semantics.hidden
            || matches!(
                entry.widget.kind,
                WidgetKind::Visibility { visible: false, .. }
            )
        {
            return Vec::new();
        }
        let render = match self.renders.get(entry.render.0) {
            Some(render) => render,
            None => return Vec::new(),
        };
        // IndexedSemantics is a transparent render object. Its explicit index
        // wins over an automatically supplied sliver index and is carried
        // through transparent wrappers until the first semantic node emits.
        let mut collection = collection;
        if let WidgetKind::IndexedSemantics { index, .. } = entry.widget.kind {
            collection = Some(SemanticCollectionContext {
                item_index: Some(index),
                set_size: collection.and_then(|context| context.set_size),
            });
        } else if collection.is_none()
            && let Some(parent) = entry.parent.and_then(|parent| self.elements.get(parent.0))
            && let WidgetKind::SliverViewport { config } = &parent.widget.kind
            && let Some(slot) = parent.children.iter().position(|child| *child == element)
        {
            collection = Some(SemanticCollectionContext {
                item_index: parent
                    .sliver_child_semantic_indices
                    .get(slot)
                    .copied()
                    .flatten()
                    .or_else(|| {
                        parent
                            .sliver_child_ids
                            .get(slot)
                            .and_then(|id| id.item_index())
                    }),
                set_size: config.delegate.child_count(),
            });
        }
        let (mut role, mut default_label, mut value, mut state, mut actions) =
            match &entry.widget.kind {
                WidgetKind::Button {
                    has_callback,
                    enabled,
                    focusable_when_disabled,
                    ..
                } => (
                    Some(SemanticRole::Button),
                    widget_text(&entry.widget),
                    None,
                    SemanticState {
                        enabled: *enabled,
                        focused: render
                            .button_state()
                            .expect("button render must own button state")
                            .focused,
                        focusable: *enabled || *focusable_when_disabled,
                        ..SemanticState::default()
                    },
                    if *has_callback && *enabled {
                        vec![SemanticActionKind::Focus, SemanticActionKind::Activate]
                    } else {
                        vec![SemanticActionKind::Focus]
                    },
                ),
                WidgetKind::Text { text, .. } => (
                    Some(SemanticRole::Text),
                    Some(text.clone()),
                    None,
                    SemanticState::default(),
                    vec![],
                ),
                WidgetKind::SelectableText { text, .. } => (
                    Some(SemanticRole::Text),
                    Some(text.clone()),
                    None,
                    SemanticState {
                        focusable: true,
                        focused: render
                            .selectable_text_state()
                            .expect("selectable-text render must own selectable state")
                            .focused,
                        ..SemanticState::default()
                    },
                    vec![SemanticActionKind::Focus],
                ),
                WidgetKind::Image { .. } if entry.widget.semantics.label.is_some() => (
                    Some(SemanticRole::Image),
                    None,
                    None,
                    SemanticState::default(),
                    vec![],
                ),
                WidgetKind::TextField {
                    controller,
                    multiline,
                    enabled,
                    read_only,
                    obscure_text,
                    ..
                } => {
                    let value = controller.value();
                    let mut actions =
                        vec![SemanticActionKind::Focus, SemanticActionKind::SetSelection];
                    if *enabled && !*read_only {
                        actions.push(SemanticActionKind::SetText);
                    }
                    (
                        Some(if *multiline {
                            SemanticRole::TextArea
                        } else {
                            SemanticRole::TextField
                        }),
                        None,
                        Some(value.text),
                        SemanticState {
                            enabled: *enabled,
                            focused: render
                                .text_field_state()
                                .expect("text-field render must own text-field state")
                                .focused,
                            focusable: *enabled,
                            editable: *enabled && !*read_only,
                            multiline: *multiline,
                            obscured: *obscure_text,
                            read_only: *read_only,
                            selection: Some(SemanticTextSelection {
                                base: value.selection.base,
                                extent: value.selection.extent,
                            }),
                            ..SemanticState::default()
                        },
                        actions,
                    )
                }
                WidgetKind::Scroll { controller, .. } => (
                    Some(SemanticRole::ScrollView),
                    None,
                    Some(format!(
                        "{:.0}/{:.0}",
                        controller.offset(),
                        controller.max_offset()
                    )),
                    SemanticState::default(),
                    vec![
                        SemanticActionKind::ScrollForward,
                        SemanticActionKind::ScrollBackward,
                    ],
                ),
                WidgetKind::SliverViewport { config } => (
                    Some(SemanticRole::ScrollView),
                    None,
                    Some(format!(
                        "{:.0}/{:.0}",
                        config.controller.offset(),
                        config.controller.max_offset()
                    )),
                    SemanticState::default(),
                    vec![
                        SemanticActionKind::ScrollForward,
                        SemanticActionKind::ScrollBackward,
                    ],
                ),
                _ => (None, None, None, SemanticState::default(), vec![]),
            };
        let explicit_description = entry
            .widget
            .semantics
            .explicit
            .as_ref()
            .and_then(|semantics| semantics.description.clone());
        if let Some(explicit) = &entry.widget.semantics.explicit {
            role = Some(explicit.role);
            default_label = explicit.label.clone().or(default_label);
            value = explicit.value.clone().or(value);
            state = explicit.state.clone();
            actions = explicit.actions.clone();
        }
        // A native adapter must never advertise an action that the retained
        // runtime cannot route to an existing controller/handler. Explicit
        // semantic metadata configures roles/state; the Semantics callbacks
        // below are the opt-in pathway for arbitrary painted controls.
        for action in [
            SemanticActionKind::Activate,
            SemanticActionKind::Increment,
            SemanticActionKind::Decrement,
            SemanticActionKind::ScrollForward,
            SemanticActionKind::ScrollBackward,
        ] {
            if entry.widget.semantics.callbacks.supports(action) && !actions.contains(&action) {
                actions.push(action);
            }
        }
        actions.retain(|action| {
            semantic_action_is_executable(
                &entry.widget.kind,
                *action,
                &entry.widget.semantics.callbacks,
            )
        });
        let this_parent = if let Some(role) = role {
            let mut state = state;
            if let Some(collection) = collection {
                if collection.item_index.is_some() {
                    state.item_index = collection.item_index;
                }
                if collection.set_size.is_some() {
                    state.set_size = collection.set_size;
                }
            }
            out.push(SemanticBuild {
                element,
                parent: semantic_parent,
                role,
                label: entry
                    .widget
                    .semantics
                    .explicit
                    .as_ref()
                    .and_then(|semantics| semantics.label.clone())
                    .or_else(|| entry.widget.semantics.label.clone())
                    .or(default_label),
                value,
                description: entry
                    .widget
                    .semantics
                    .description
                    .clone()
                    .or(explicit_description),
                bounds: self.semantic_bounds(entry.render, render.size),
                state,
                actions,
            });
            Some(element)
        } else {
            semantic_parent
        };
        // A Button deliberately merges its visual label/icon subtree into the
        // one control node. Other containers preserve logical child order.
        if !matches!(entry.widget.kind, WidgetKind::Button { .. })
            && !entry.widget.semantics.merge_descendants
        {
            let first_visible_child = entry
                .children
                .iter()
                .rposition(|child| {
                    self.elements
                        .get(child.0)
                        .is_some_and(|child| child.widget.semantics.block_previous_siblings)
                })
                .unwrap_or(0);
            let semantic_children: Vec<_> = match entry.widget.kind {
                WidgetKind::IndexedStack { index, .. } => {
                    entry.children.get(index).copied().into_iter().collect()
                }
                _ => entry.children[first_visible_child..].to_vec(),
            };
            let child_collection = role.is_none().then_some(collection).flatten();
            return semantic_children
                .into_iter()
                .map(|child| (child, this_parent, child_collection))
                .collect();
        }
        Vec::new()
    }
}

pub(super) fn semantic_action_is_executable(
    kind: &WidgetKind,
    action: SemanticActionKind,
    callbacks: &SemanticCallbacks,
) -> bool {
    if callbacks.supports(action) {
        return true;
    }
    match action {
        SemanticActionKind::Focus => true,
        SemanticActionKind::Activate => {
            matches!(
                kind,
                WidgetKind::Button {
                    has_callback: true,
                    enabled: true,
                    ..
                }
            )
        }
        SemanticActionKind::SetText | SemanticActionKind::SetSelection => {
            matches!(kind, WidgetKind::TextField { .. })
        }
        SemanticActionKind::ScrollForward | SemanticActionKind::ScrollBackward => {
            matches!(
                kind,
                WidgetKind::Scroll { .. } | WidgetKind::SliverViewport { .. }
            )
        }
        SemanticActionKind::Increment | SemanticActionKind::Decrement => {
            matches!(
                kind,
                WidgetKind::Gesture { callbacks, .. } if callbacks.has_keyboard_listener()
            )
        }
    }
}

pub(super) fn widget_text(widget: &Widget) -> Option<String> {
    let mut work = vec![widget];
    let mut fragments = Vec::new();
    while let Some(current) = work.pop() {
        match &current.kind {
            WidgetKind::Text { text, .. } | WidgetKind::SelectableText { text, .. } => {
                fragments.push(text.clone());
            }
            WidgetKind::Button { child, .. } | WidgetKind::Banner { child, .. } => {
                work.extend(child.iter().map(|child| child.as_ref()));
            }
            WidgetKind::Decorated { child, .. }
            | WidgetKind::Padding { child, .. }
            | WidgetKind::Constrained { child, .. }
            | WidgetKind::Limited { child, .. }
            | WidgetKind::Overflow { child, .. }
            | WidgetKind::Unconstrained { child, .. }
            | WidgetKind::Fractional { child, .. }
            | WidgetKind::Baseline { child, .. }
            | WidgetKind::RepaintBoundary { child }
            | WidgetKind::Gesture { child, .. }
            | WidgetKind::Draggable { child, .. }
            | WidgetKind::DragTarget { child, .. }
            | WidgetKind::IgnorePointer { child, .. }
            | WidgetKind::AbsorbPointer { child, .. }
            | WidgetKind::Align { child, .. }
            | WidgetKind::Flexible { child, .. }
            | WidgetKind::Positioned { child, .. }
            | WidgetKind::SafeArea { child, .. }
            | WidgetKind::ClipRect { child, .. }
            | WidgetKind::ClipRRect { child, .. }
            | WidgetKind::ClipOval { child, .. }
            | WidgetKind::ClipPath { child, .. }
            | WidgetKind::Visibility { child, .. }
            | WidgetKind::AspectRatio { child, .. }
            | WidgetKind::Scroll { child, .. }
            | WidgetKind::PersistentHeader { child, .. }
            | WidgetKind::NotificationListener { child, .. }
            | WidgetKind::Translate { child, .. }
            | WidgetKind::Transform { child, .. }
            | WidgetKind::Scale { child, .. }
            | WidgetKind::Rotation { child, .. }
            | WidgetKind::FittedBox { child, .. }
            | WidgetKind::Opacity { child, .. }
            | WidgetKind::Blur { child, .. }
            | WidgetKind::DropShadow { child, .. }
            | WidgetKind::ColorFiltered { child, .. }
            | WidgetKind::Blend { child, .. }
            | WidgetKind::SelectionArea { child, .. }
            | WidgetKind::SelectionContainer { child, .. }
            | WidgetKind::SelectionListener { child, .. }
            | WidgetKind::IndexedSemantics { child, .. }
            | WidgetKind::SemanticsDebugger { child, .. } => work.push(child),
            WidgetKind::RawInput {
                child: Some(child), ..
            } => work.push(child),
            WidgetKind::Flex { children, .. }
            | WidgetKind::Stack { children, .. }
            | WidgetKind::IndexedStack { children, .. } => {
                work.extend(children.iter().rev().map(Rc::as_ref));
            }
            _ => {}
        }
    }
    (!fragments.is_empty()).then(|| fragments.join(" "))
}
