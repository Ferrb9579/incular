//! Keyboard focus discovery and traversal.

use super::*;

impl WidgetTree {
    #[must_use]
    pub fn focusable_elements(&self) -> Vec<ElementId> {
        with_recursive_tree_stack(|| self.focusable_elements_recursive())
    }

    fn focusable_elements_recursive(&self) -> Vec<ElementId> {
        let mut members = Vec::new();
        let mut sequence = 0;
        if let Some(root) = self.root {
            if self
                .elements
                .get(root.0)
                .is_some_and(|element| element.widget.semantics.focus_traversal_policy.is_some())
            {
                self.collect_focus_group(
                    root,
                    FocusTraversalPolicyKind::WidgetOrder,
                    false,
                    &mut members,
                    &mut sequence,
                );
            } else {
                self.append_focus_members(
                    root,
                    FocusTraversalPolicyKind::WidgetOrder,
                    false,
                    &mut members,
                    &mut sequence,
                );
            }
        }
        let mut result = Vec::new();
        flatten_focus_group(
            FocusTraversalGroupMembers {
                policy: FocusTraversalPolicyKind::WidgetOrder,
                members,
            },
            &mut result,
        );
        result
    }

    /// Dispatches a platform keyboard event through the nearest retained
    /// keyboard listeners surrounding the focused element. The tree owns the
    /// ordering; the runtime remains responsible for text-editing fallthrough
    /// when no listener consumes the event.
    #[must_use]
    pub fn dispatch_keyboard(&self, focused: Option<ElementId>, event: KeyboardEvent) -> bool {
        let mut current = focused;
        while let Some(id) = current {
            if let Some(element) = self.elements.get(id.0)
                && let WidgetKind::Gesture { callbacks, .. } = &element.widget.kind
                && callbacks.has_keyboard_listener()
                && callbacks.handle_keyboard(event.clone())
            {
                return true;
            }
            current = self.parent(id);
        }
        false
    }

    /// Routes a native accessibility increment/decrement through the same
    /// keyboard listener used by a slider. This keeps a control's value logic
    /// in its existing callback rather than creating a second action path.
    #[must_use]
    pub fn dispatch_semantic_increment(&self, element: ElementId, increment: bool) -> bool {
        let code = if increment {
            incular_core::Code::ArrowRight
        } else {
            incular_core::Code::ArrowLeft
        };
        self.dispatch_keyboard(
            Some(element),
            KeyboardEvent::key_down(KeyboardKey::Named(NamedKey::Unidentified), code),
        )
    }

    /// Finds the retained element associated with an externally managed focus
    /// node. This lets a `FocusScopeNode` request focus without coupling the
    /// gestures crate to runtime element IDs.
    #[must_use]
    pub fn focused_keyboard_element(&self) -> Option<ElementId> {
        self.focusable_elements().into_iter().find(|id| {
            self.elements
                .get(id.0)
                .and_then(|element| match &element.widget.kind {
                    WidgetKind::Gesture { callbacks, .. } => callbacks.focus_node.as_ref(),
                    _ => None,
                })
                .is_some_and(|node| node.has_focus())
        })
    }

    /// Returns the first listener that requested autofocus and can currently
    /// receive focus.
    #[must_use]
    pub fn autofocus_element(&self) -> Option<ElementId> {
        let focusable = self.focusable_elements();
        if let Some(listener) = focusable.iter().find(|id| {
            self.elements
                .get(id.0)
                .and_then(|element| match &element.widget.kind {
                    WidgetKind::Gesture { callbacks, .. } => callbacks
                        .focus_node
                        .as_ref()
                        .filter(|node| callbacks.autofocus && node.can_request_focus()),
                    _ => None,
                })
                .is_some()
        }) {
            return Some(*listener);
        }
        self.elements
            .iter()
            .filter(|(_, element)| element.widget.semantics.focus_scope_autofocus)
            .map(|(raw, _)| ElementId(raw))
            .find_map(|scope| {
                focusable
                    .iter()
                    .copied()
                    .find(|candidate| self.is_descendant_or_self(*candidate, scope))
            })
    }

    /// Mirrors runtime focus changes onto a listener's external focus node.
    pub fn set_keyboard_focus(&self, id: ElementId, focused: bool) {
        let Some(element) = self.elements.get(id.0) else {
            return;
        };
        let WidgetKind::Gesture { callbacks, .. } = &element.widget.kind else {
            return;
        };
        let Some(node) = callbacks.focus_node.as_ref() else {
            return;
        };
        if focused {
            node.request_focus();
        } else {
            node.unfocus();
        }
    }

    pub(super) fn collect_focus_group(
        &self,
        id: ElementId,
        inherited_policy: FocusTraversalPolicyKind,
        excluded_focus: bool,
        out: &mut Vec<FocusTraversalMember>,
        sequence: &mut usize,
    ) {
        let Some(element) = self.elements.get(id.0) else {
            return;
        };
        if element.widget.semantics.exclude_focus_traversal {
            return;
        }
        let policy = element
            .widget
            .semantics
            .focus_traversal_policy
            .unwrap_or(inherited_policy);
        let mut members = Vec::new();
        self.append_focus_members(id, policy, excluded_focus, &mut members, sequence);
        out.push(FocusTraversalMember::Group(FocusTraversalGroupMembers {
            policy,
            members,
        }));
    }

    pub(super) fn append_focus_members(
        &self,
        id: ElementId,
        policy: FocusTraversalPolicyKind,
        excluded_focus: bool,
        out: &mut Vec<FocusTraversalMember>,
        sequence: &mut usize,
    ) {
        enum FocusWork {
            Node(ElementId, FocusTraversalPolicyKind, bool),
            Group(ElementId, FocusTraversalPolicyKind, bool),
        }

        let mut work = vec![FocusWork::Node(id, policy, excluded_focus)];
        while let Some(next) = work.pop() {
            let FocusWork::Node(id, policy, excluded_focus) = next else {
                let FocusWork::Group(id, inherited_policy, excluded_focus) = next else {
                    unreachable!();
                };
                self.collect_focus_group(id, inherited_policy, excluded_focus, out, sequence);
                continue;
            };
            let Some(element) = self.elements.get(id.0) else {
                continue;
            };
            if element.widget.semantics.exclude_focus_traversal {
                continue;
            }
            let excluded_focus = excluded_focus || element.widget.semantics.exclude_focus;
            let focusable = match &element.widget.kind {
                WidgetKind::Button {
                    enabled,
                    focusable_when_disabled,
                    ..
                } => *enabled || *focusable_when_disabled,
                WidgetKind::TextField { enabled, .. } => *enabled,
                WidgetKind::SelectableText { .. } => true,
                WidgetKind::Gesture { callbacks, .. } => callbacks
                    .focus_node
                    .as_ref()
                    .is_some_and(|node| node.can_request_focus()),
                _ => false,
            };
            if focusable && !excluded_focus {
                let bounds = self.element_bounds(id).unwrap_or_default();
                let order = element.widget.semantics.focus_traversal_order;
                if let WidgetKind::Gesture { callbacks, .. } = &element.widget.kind
                    && let Some(node) = callbacks.focus_node.as_ref()
                {
                    node.set_rect(bounds);
                    node.set_traversal_order(order);
                }
                out.push(FocusTraversalMember::Candidate(FocusCandidate {
                    id,
                    bounds,
                    order,
                    sequence: *sequence,
                }));
                *sequence = sequence.saturating_add(1);
            }
            let (descendants_are_focusable, descendants_are_traversable) =
                match &element.widget.kind {
                    WidgetKind::Gesture { callbacks, .. } => {
                        callbacks.focus_node.as_ref().map_or((true, true), |node| {
                            (
                                node.descendants_are_focusable(),
                                node.descendants_are_traversable(),
                            )
                        })
                    }
                    _ => (true, true),
                };
            if !descendants_are_traversable {
                continue;
            }
            let excluded_focus = excluded_focus || !descendants_are_focusable;
            let focus_children: Vec<_> = match element.widget.kind {
                WidgetKind::IndexedStack { index, .. } => {
                    element.children.get(index).copied().into_iter().collect()
                }
                _ => element.children.clone(),
            };

            for child in focus_children.into_iter().rev() {
                let child_policy = self
                    .elements
                    .get(child.0)
                    .and_then(|child| child.widget.semantics.focus_traversal_policy);
                if child_policy.is_some() {
                    work.push(FocusWork::Group(child, policy, excluded_focus));
                } else {
                    work.push(FocusWork::Node(child, policy, excluded_focus));
                }
            }
        }
    }
    pub(super) fn is_descendant_or_self(&self, candidate: ElementId, ancestor: ElementId) -> bool {
        let mut current = Some(candidate);
        while let Some(id) = current {
            if id == ancestor {
                return true;
            }
            current = self.parent(id);
        }
        false
    }
}
