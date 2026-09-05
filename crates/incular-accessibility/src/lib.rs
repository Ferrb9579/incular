//! Native accessibility projection for Incular.
//!
//! [`incular_semantics::SemanticsTree`] remains the retained, platform-neutral
//! source of truth. This crate projects it to AccessKit at the desktop-native
//! boundary; it does not introduce a second semantic model. AccessKit node IDs
//! are private to one live native window and never enter widget/facade APIs.

use accesskit::{
    Action, ActionData, ActionRequest, Invalid, Node, NodeId, Rect as AccessKitRect,
    Role as AccessKitRole, TextPosition, TextSelection as AccessKitSelection, Toggled, Tree,
    TreeId, TreeUpdate,
};
use incular_core::Rect;
use std::collections::HashMap;

pub use incular_semantics::*;

const HOST_NODE_ID: NodeId = NodeId(0);

/// An owned semantic tree snapshot suitable for a platform bridge.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SemanticsSnapshot {
    pub root: Option<SemanticNodeId>,
    pub nodes: Vec<SemanticNode>,
}

impl From<&SemanticsTree> for SemanticsSnapshot {
    fn from(tree: &SemanticsTree) -> Self {
        Self {
            root: tree.root(),
            nodes: tree.iter().map(|(_, node)| node.clone()).collect(),
        }
    }
}

/// An action emitted by a native accessibility bridge for a semantic node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SemanticActionRequest {
    pub node: SemanticNodeId,
    pub action: SemanticAction,
}

/// Small platform-neutral testing seam retained from the previous adapter API.
pub trait SemanticsAdapter {
    fn publish(&mut self, snapshot: SemanticsSnapshot);
    fn poll_action(&mut self) -> Option<SemanticActionRequest>;
}

/// Change categories emitted by the mobile accessibility projection. Native
/// Android and iOS bridges translate these into their platform notifications.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MobileAccessibilityEventKind {
    ContentChanged,
    BoundsChanged,
    FocusChanged,
    ValueChanged,
    StateChanged,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MobileAccessibilityEvent {
    pub native_id: u64,
    pub kind: MobileAccessibilityEventKind,
}

/// A node snapshot using stable native-facing IDs while retaining the
/// renderer-independent semantic node for platform conversion.
#[derive(Clone, Debug, PartialEq)]
pub struct MobileAccessibilityNode {
    pub native_id: u64,
    pub semantic: SemanticNode,
    pub children: Vec<u64>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MobileAccessibilityUpdate {
    pub revision: u64,
    pub root: Option<u64>,
    pub nodes: Vec<MobileAccessibilityNode>,
    pub removed: Vec<u64>,
    pub events: Vec<MobileAccessibilityEvent>,
}

/// Incremental, generation-safe semantic projection for mobile screen-reader
/// bridges. It deliberately has no JNI, Objective-C, or window references;
/// each mobile crate can consume the same update contract on its UI thread.
pub struct MobileAccessibilityProjection {
    semantic_to_native: HashMap<SemanticNodeId, u64>,
    native_to_semantic: HashMap<u64, SemanticNodeId>,
    previous: HashMap<SemanticNodeId, SemanticNode>,
    previous_root: Option<SemanticNodeId>,
    synced_revision: Option<u64>,
    next_native_id: u64,
    active: bool,
    force_full_update: bool,
}

impl Default for MobileAccessibilityProjection {
    fn default() -> Self {
        Self::new()
    }
}

impl MobileAccessibilityProjection {
    #[must_use]
    pub fn new() -> Self {
        Self {
            semantic_to_native: HashMap::new(),
            native_to_semantic: HashMap::new(),
            previous: HashMap::new(),
            previous_root: None,
            synced_revision: None,
            next_native_id: 1,
            active: true,
            force_full_update: true,
        }
    }

    pub fn activate(&mut self) {
        self.active = true;
        self.force_full_update = true;
    }

    pub fn deactivate(&mut self) {
        self.active = false;
        self.force_full_update = true;
    }

    #[must_use]
    pub fn native_node_id(&self, node: SemanticNodeId) -> Option<u64> {
        self.semantic_to_native.get(&node).copied()
    }

    #[must_use]
    pub fn semantic_node_id(&self, native_id: u64) -> Option<SemanticNodeId> {
        self.native_to_semantic.get(&native_id).copied()
    }

    /// Produces a full update on activation and only changed nodes afterwards.
    pub fn sync(&mut self, tree: &SemanticsTree) -> Option<MobileAccessibilityUpdate> {
        if !self.active {
            return None;
        }
        let revision = tree.revision();
        if !self.force_full_update && self.synced_revision == Some(revision) {
            return None;
        }
        let current: HashMap<_, _> = tree.iter().map(|(id, node)| (id, node.clone())).collect();
        for id in current.keys().copied() {
            self.ensure_native_id(id);
        }
        let full = self.force_full_update;
        let mut changed = if full {
            current.keys().copied().collect::<Vec<_>>()
        } else {
            current
                .iter()
                .filter_map(|(id, node)| (self.previous.get(id) != Some(node)).then_some(*id))
                .collect::<Vec<_>>()
        };
        changed.sort_by_key(|id| self.semantic_to_native[id]);
        let removed = self
            .previous
            .keys()
            .filter(|id| !current.contains_key(id))
            .copied()
            .collect::<Vec<_>>();
        let root = tree
            .root()
            .and_then(|id| self.semantic_to_native.get(&id).copied());
        if !full && changed.is_empty() && removed.is_empty() && self.previous_root == tree.root() {
            self.synced_revision = Some(revision);
            return None;
        }

        let mut nodes = Vec::with_capacity(changed.len());
        let mut events = Vec::new();
        for id in changed {
            let Some(semantic) = current.get(&id) else {
                continue;
            };
            let native_id = self.semantic_to_native[&id];
            let children = semantic
                .children
                .iter()
                .filter_map(|child| self.semantic_to_native.get(child).copied())
                .collect();
            nodes.push(MobileAccessibilityNode {
                native_id,
                semantic: mobile_semantic_node(semantic),
                children,
            });
            if full || !self.previous.contains_key(&id) {
                events.push(MobileAccessibilityEvent {
                    native_id,
                    kind: MobileAccessibilityEventKind::ContentChanged,
                });
            } else if let Some(old) = self.previous.get(&id) {
                if old.state.focused != semantic.state.focused {
                    events.push(MobileAccessibilityEvent {
                        native_id,
                        kind: MobileAccessibilityEventKind::FocusChanged,
                    });
                }
                if old.value != semantic.value {
                    events.push(MobileAccessibilityEvent {
                        native_id,
                        kind: MobileAccessibilityEventKind::ValueChanged,
                    });
                }
                if old.state != semantic.state {
                    events.push(MobileAccessibilityEvent {
                        native_id,
                        kind: MobileAccessibilityEventKind::StateChanged,
                    });
                }
                if old.bounds != semantic.bounds {
                    events.push(MobileAccessibilityEvent {
                        native_id,
                        kind: MobileAccessibilityEventKind::BoundsChanged,
                    });
                }
                if old.role != semantic.role
                    || old.label != semantic.label
                    || old.description != semantic.description
                    || old.actions != semantic.actions
                    || old.children != semantic.children
                {
                    events.push(MobileAccessibilityEvent {
                        native_id,
                        kind: MobileAccessibilityEventKind::ContentChanged,
                    });
                }
            }
        }
        let mut removed_native = removed
            .iter()
            .filter_map(|id| self.semantic_to_native.get(id).copied())
            .collect::<Vec<_>>();
        removed_native.sort_unstable();
        for id in removed {
            if let Some(native_id) = self.semantic_to_native.remove(&id) {
                self.native_to_semantic.remove(&native_id);
            }
        }
        self.previous = current;
        self.previous_root = tree.root();
        self.synced_revision = Some(revision);
        self.force_full_update = false;
        Some(MobileAccessibilityUpdate {
            revision,
            root,
            nodes,
            removed: removed_native,
            events,
        })
    }

    /// Validates that a native action is still supported by the live semantic
    /// node before handing it to the runtime.
    pub fn translate_action(
        &self,
        native_id: u64,
        action: SemanticAction,
    ) -> Option<SemanticActionRequest> {
        if !self.active {
            return None;
        }
        let node = self.native_to_semantic.get(&native_id).copied()?;
        let semantic = self.previous.get(&node)?;
        semantic
            .actions
            .contains(&action.kind())
            .then_some(SemanticActionRequest { node, action })
    }

    fn ensure_native_id(&mut self, node: SemanticNodeId) {
        if self.semantic_to_native.contains_key(&node) {
            return;
        }
        let native_id = self.next_native_id;
        self.next_native_id = self.next_native_id.wrapping_add(1).max(1);
        self.semantic_to_native.insert(node, native_id);
        self.native_to_semantic.insert(native_id, node);
    }
}

/// Keeps clear-text password values inside the retained runtime while making
/// the mobile projection safe to hand to a native accessibility tree.
fn mobile_semantic_node(node: &SemanticNode) -> SemanticNode {
    let mut node = node.clone();
    if node.state.obscured {
        node.value = None;
    }
    node
}

/// Per-native-window health counters. They intentionally contain no private
/// labels, descriptions, or text-field values.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AccessibilityDiagnostics {
    pub adapters_created: u64,
    pub adapters_destroyed: u64,
    pub adapter_activations: u64,
    pub adapter_deactivations: u64,
    pub full_tree_updates: u64,
    pub incremental_tree_updates: u64,
    pub nodes_published: u64,
    pub nodes_updated: u64,
    pub nodes_removed: u64,
    pub actions_received: u64,
    pub actions_dispatched: u64,
    pub stale_actions_rejected: u64,
    pub unsupported_actions: u64,
    pub focus_updates: u64,
    pub bounds_updates: u64,
    pub semantic_updates_skipped_unchanged: u64,
    pub accesskit_conversion_failures: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessKitUpdateKind {
    Full,
    Incremental,
}

/// Generated native update consumed immediately by a Winit adapter.
#[derive(Debug)]
pub struct NativeAccessibilityUpdate {
    kind: AccessKitUpdateKind,
    update: TreeUpdate,
}

impl NativeAccessibilityUpdate {
    #[must_use]
    pub const fn kind(&self) -> AccessKitUpdateKind {
        self.kind
    }

    /// Native-runner boundary only; this is deliberately not re-exported by
    /// Incular's facade or prelude.
    #[doc(hidden)]
    #[must_use]
    pub fn into_accesskit(self) -> TreeUpdate {
        self.update
    }
}

#[derive(Clone, Debug, PartialEq)]
struct ProjectedNode {
    semantic: SemanticNode,
    scale_factor: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct HostProjection {
    root: Option<SemanticNodeId>,
    bounds: Option<Rect>,
    scale_factor: f64,
}

/// Stateful, generation-safe projection from one Incular tree to one native
/// AccessKit tree. One instance belongs to one native window.
pub struct AccessKitProjection {
    semantic_to_native: HashMap<SemanticNodeId, NodeId>,
    native_to_semantic: HashMap<NodeId, SemanticNodeId>,
    previous: HashMap<SemanticNodeId, ProjectedNode>,
    host: Option<HostProjection>,
    synced_revision: Option<u64>,
    next_native_id: u64,
    force_full_update: bool,
    diagnostics: AccessibilityDiagnostics,
}

impl Default for AccessKitProjection {
    fn default() -> Self {
        Self::new()
    }
}

impl AccessKitProjection {
    #[must_use]
    pub fn new() -> Self {
        Self {
            semantic_to_native: HashMap::new(),
            native_to_semantic: HashMap::new(),
            previous: HashMap::new(),
            host: None,
            synced_revision: None,
            next_native_id: 1,
            force_full_update: true,
            diagnostics: AccessibilityDiagnostics::default(),
        }
    }

    #[must_use]
    pub const fn diagnostics(&self) -> AccessibilityDiagnostics {
        self.diagnostics
    }

    /// Native bridge/test plumbing only, not an application identity.
    #[doc(hidden)]
    #[must_use]
    pub fn native_node_id(&self, node: SemanticNodeId) -> Option<u64> {
        self.semantic_to_native.get(&node).map(|id| id.0)
    }

    pub fn note_adapter_created(&mut self) {
        self.diagnostics.adapters_created = self.diagnostics.adapters_created.wrapping_add(1);
    }

    pub fn note_adapter_destroyed(&mut self) {
        self.diagnostics.adapters_destroyed = self.diagnostics.adapters_destroyed.wrapping_add(1);
    }

    /// Request a full retained-tree snapshot at native activation.
    pub fn activate(&mut self) {
        self.diagnostics.adapter_activations = self.diagnostics.adapter_activations.wrapping_add(1);
        self.force_full_update = true;
    }

    /// Native deactivation doesn't discard Incular semantics. The next native
    /// activation publishes a complete current tree using the same live-window
    /// node map when possible.
    pub fn deactivate(&mut self) {
        self.diagnostics.adapter_deactivations =
            self.diagnostics.adapter_deactivations.wrapping_add(1);
        self.force_full_update = true;
    }

    /// Records a request that was valid when projected but was rejected by the
    /// retained runtime before UI dispatch (for example, a node removed in the
    /// small interval before the Winit user event was handled).
    pub fn note_runtime_stale_action(&mut self) {
        self.diagnostics.stale_actions_rejected =
            self.diagnostics.stale_actions_rejected.wrapping_add(1);
    }

    /// Projects retained semantic changes. Incular bounds are logical; the
    /// supplied native window scale is applied exactly once for AccessKit.
    pub fn sync(
        &mut self,
        tree: &SemanticsTree,
        scale_factor: f64,
    ) -> Option<NativeAccessibilityUpdate> {
        let scale_factor = if scale_factor.is_finite() && scale_factor > 0.0 {
            scale_factor
        } else {
            1.0
        };
        let revision = tree.revision();
        if !self.force_full_update && self.synced_revision == Some(revision) {
            self.diagnostics.semantic_updates_skipped_unchanged = self
                .diagnostics
                .semantic_updates_skipped_unchanged
                .wrapping_add(1);
            return None;
        }

        let current: HashMap<_, _> = tree
            .iter()
            .map(|(id, semantic)| {
                (
                    id,
                    ProjectedNode {
                        semantic: semantic.clone(),
                        scale_factor,
                    },
                )
            })
            .collect();
        for id in current.keys().copied() {
            self.ensure_native_id(id);
        }
        let next_host = HostProjection {
            root: tree.root(),
            bounds: tree
                .root()
                .and_then(|root| tree.node(root))
                .map(|node| node.bounds),
            scale_factor,
        };
        let full = self.force_full_update || self.synced_revision.is_none();
        let removed: Vec<_> = self
            .previous
            .keys()
            .copied()
            .filter(|id| !current.contains_key(id))
            .collect();
        let mut changed: Vec<_> = if full {
            current.keys().copied().collect()
        } else {
            current
                .iter()
                .filter_map(|(id, node)| (self.previous.get(id) != Some(node)).then_some(*id))
                .collect()
        };
        let host_changed = full || self.host != Some(next_host);
        if !full && changed.is_empty() && removed.is_empty() && !host_changed {
            self.synced_revision = Some(revision);
            self.diagnostics.semantic_updates_skipped_unchanged = self
                .diagnostics
                .semantic_updates_skipped_unchanged
                .wrapping_add(1);
            return None;
        }

        let old_focus = self.previous_focus();
        let mut nodes = Vec::with_capacity(changed.len() + usize::from(host_changed));
        if host_changed {
            nodes.push((HOST_NODE_ID, self.host_node(next_host)));
        }
        changed.sort_by_key(|id| self.semantic_to_native[id].0);
        for id in changed {
            let projected = current.get(&id).expect("changed node is current");
            nodes.push((
                self.semantic_to_native[&id],
                self.accesskit_node(&projected.semantic, projected.scale_factor),
            ));
        }
        let focus = current
            .iter()
            .find_map(|(id, node)| node.semantic.state.focused.then_some(*id))
            .and_then(|id| self.semantic_to_native.get(&id).copied())
            .unwrap_or(HOST_NODE_ID);

        self.diagnostics.focus_updates = self
            .diagnostics
            .focus_updates
            .wrapping_add(u64::from(focus != old_focus));
        self.diagnostics.bounds_updates = self.diagnostics.bounds_updates.wrapping_add(
            current
                .iter()
                .filter(|(id, node)| {
                    self.previous.get(id).is_some_and(|old| {
                        old.semantic.bounds != node.semantic.bounds
                            || old.scale_factor != node.scale_factor
                    })
                })
                .count() as u64,
        );
        if full {
            self.diagnostics.full_tree_updates = self.diagnostics.full_tree_updates.wrapping_add(1);
            self.diagnostics.nodes_published = self
                .diagnostics
                .nodes_published
                .wrapping_add(current.len() as u64 + 1);
        } else {
            self.diagnostics.incremental_tree_updates =
                self.diagnostics.incremental_tree_updates.wrapping_add(1);
            self.diagnostics.nodes_updated = self
                .diagnostics
                .nodes_updated
                .wrapping_add(nodes.len().saturating_sub(usize::from(host_changed)) as u64);
            self.diagnostics.nodes_removed = self
                .diagnostics
                .nodes_removed
                .wrapping_add(removed.len() as u64);
        }
        for id in removed {
            if let Some(native) = self.semantic_to_native.remove(&id) {
                self.native_to_semantic.remove(&native);
            }
        }
        self.previous = current;
        self.host = Some(next_host);
        self.synced_revision = Some(revision);
        self.force_full_update = false;

        Some(NativeAccessibilityUpdate {
            kind: if full {
                AccessKitUpdateKind::Full
            } else {
                AccessKitUpdateKind::Incremental
            },
            update: TreeUpdate {
                nodes,
                tree: full.then(|| {
                    let mut tree = Tree::new(HOST_NODE_ID);
                    tree.toolkit_name = Some("Incular".into());
                    tree.toolkit_version = Some(env!("CARGO_PKG_VERSION").into());
                    tree
                }),
                tree_id: TreeId::ROOT,
                focus,
            },
        })
    }

    /// Converts a native request into an owned, validated Incular request. The
    /// caller must dispatch it through the UI-thread runtime.
    pub fn translate_action(&mut self, request: &ActionRequest) -> Option<SemanticActionRequest> {
        self.diagnostics.actions_received = self.diagnostics.actions_received.wrapping_add(1);
        if request.target_tree != TreeId::ROOT {
            self.diagnostics.unsupported_actions =
                self.diagnostics.unsupported_actions.wrapping_add(1);
            return None;
        }
        let Some(node) = self.native_to_semantic.get(&request.target_node).copied() else {
            self.diagnostics.stale_actions_rejected =
                self.diagnostics.stale_actions_rejected.wrapping_add(1);
            return None;
        };
        let Some(projected) = self.previous.get(&node) else {
            self.diagnostics.stale_actions_rejected =
                self.diagnostics.stale_actions_rejected.wrapping_add(1);
            return None;
        };
        let action = match request.action {
            Action::Click if supports(&projected.semantic, SemanticActionKind::Activate) => {
                Some(SemanticAction::Activate)
            }
            Action::Focus if supports(&projected.semantic, SemanticActionKind::Focus) => {
                Some(SemanticAction::Focus)
            }
            Action::ScrollDown | Action::ScrollRight
                if supports(&projected.semantic, SemanticActionKind::ScrollForward) =>
            {
                Some(SemanticAction::ScrollForward)
            }
            Action::ScrollUp | Action::ScrollLeft
                if supports(&projected.semantic, SemanticActionKind::ScrollBackward) =>
            {
                Some(SemanticAction::ScrollBackward)
            }
            Action::SetValue | Action::ReplaceSelectedText
                if supports(&projected.semantic, SemanticActionKind::SetText) =>
            {
                match request.data.as_ref() {
                    Some(ActionData::Value(value)) => {
                        Some(SemanticAction::SetText(value.to_string()))
                    }
                    _ => None,
                }
            }
            Action::SetTextSelection
                if supports(&projected.semantic, SemanticActionKind::SetSelection) =>
            {
                match request.data.as_ref() {
                    Some(ActionData::SetTextSelection(selection))
                        if selection.anchor.node == request.target_node
                            && selection.focus.node == request.target_node =>
                    {
                        let text = projected.semantic.value.as_deref().unwrap_or_default();
                        Some(SemanticAction::SetSelection {
                            base: utf8_offset(text, selection.anchor.character_index),
                            extent: utf8_offset(text, selection.focus.character_index),
                        })
                    }
                    _ => None,
                }
            }
            Action::Increment if supports(&projected.semantic, SemanticActionKind::Increment) => {
                Some(SemanticAction::Increment)
            }
            Action::Decrement if supports(&projected.semantic, SemanticActionKind::Decrement) => {
                Some(SemanticAction::Decrement)
            }
            _ => None,
        };
        let Some(action) = action else {
            self.diagnostics.unsupported_actions =
                self.diagnostics.unsupported_actions.wrapping_add(1);
            return None;
        };
        self.diagnostics.actions_dispatched = self.diagnostics.actions_dispatched.wrapping_add(1);
        Some(SemanticActionRequest { node, action })
    }

    fn ensure_native_id(&mut self, node: SemanticNodeId) {
        if self.semantic_to_native.contains_key(&node) {
            return;
        }
        let native = NodeId(self.next_native_id);
        self.next_native_id = self.next_native_id.wrapping_add(1).max(1);
        self.semantic_to_native.insert(node, native);
        self.native_to_semantic.insert(native, node);
    }

    fn host_node(&self, host: HostProjection) -> Node {
        let mut node = Node::new(AccessKitRole::Window);
        node.set_label("Incular");
        node.set_children(
            host.root
                .and_then(|id| self.semantic_to_native.get(&id).copied())
                .into_iter()
                .collect::<Vec<_>>(),
        );
        if let Some(bounds) = host.bounds {
            node.set_bounds(to_accesskit_rect(bounds, host.scale_factor));
        }
        node
    }

    fn accesskit_node(&self, semantic: &SemanticNode, scale_factor: f64) -> Node {
        let native_id = self.semantic_to_native[&semantic.id];
        let mut node = Node::new(map_role(semantic.role, semantic.state.obscured));
        node.set_bounds(to_accesskit_rect(semantic.bounds, scale_factor));
        if semantic.role == Role::Text && !semantic.state.obscured {
            if let Some(text) = semantic.value.as_ref().or(semantic.label.as_ref()) {
                node.set_value(text.as_str());
            }
        } else if !semantic.state.obscured {
            if let Some(label) = &semantic.label {
                node.set_label(label.as_str());
            }
            if let Some(value) = &semantic.value {
                node.set_value(value.as_str());
            }
        } else if let Some(label) = &semantic.label {
            // Password values never cross the native accessibility boundary,
            // but their accessible label still does.
            node.set_label(label.as_str());
        }
        if let Some(description) = &semantic.description {
            node.set_description(description.as_str());
        }
        if is_interactive_role(semantic.role) && !semantic.state.enabled {
            node.set_disabled();
        }
        if semantic.state.selected {
            node.set_selected(true);
        }
        if let Some(checked) = semantic.state.checked {
            node.set_toggled(match checked {
                incular_semantics::CheckedState::Checked => Toggled::True,
                incular_semantics::CheckedState::Unchecked => Toggled::False,
                incular_semantics::CheckedState::Indeterminate => Toggled::Mixed,
            });
        }
        if let Some(expanded) = semantic.state.expanded {
            node.set_expanded(expanded);
        }
        if semantic.state.busy {
            node.set_busy();
        }
        if semantic.state.invalid {
            node.set_invalid(Invalid::True);
        }
        if semantic.state.required {
            node.set_required();
        }
        if let Some(level) = semantic.state.heading_level {
            node.set_level(usize::from(level));
        }
        if let Some(value) = semantic
            .state
            .numeric_value
            .filter(|value| value.is_finite())
        {
            node.set_numeric_value(value);
        }
        if let Some(value) = semantic.state.numeric_min.filter(|value| value.is_finite()) {
            node.set_min_numeric_value(value);
        }
        if let Some(value) = semantic.state.numeric_max.filter(|value| value.is_finite()) {
            node.set_max_numeric_value(value);
        }
        if let Some(value) = semantic
            .state
            .numeric_step
            .filter(|value| value.is_finite())
        {
            node.set_numeric_value_step(value);
        }
        if semantic.state.read_only {
            node.set_read_only();
        }
        if let Some(size) = semantic.state.set_size {
            node.set_size_of_set(size);
        }
        if let Some(index) = semantic.state.item_index {
            node.set_position_in_set(index.saturating_add(1));
        }
        if matches!(semantic.role, Role::ScrollView | Role::List) {
            let (offset, maximum) = scroll_value(semantic.value.as_deref());
            node.set_scroll_y(offset);
            node.set_scroll_y_min(0.0);
            node.set_scroll_y_max(maximum);
        }
        if semantic.state.editable
            && !semantic.state.obscured
            && let Some(value) = semantic.value.as_deref()
        {
            node.set_character_lengths(
                value
                    .chars()
                    .map(|character| character.len_utf8() as u8)
                    .collect::<Vec<_>>(),
            );
            if let Some(selection) = semantic.state.selection {
                node.set_text_selection(AccessKitSelection {
                    anchor: TextPosition {
                        node: native_id,
                        character_index: character_offset(value, selection.base),
                    },
                    focus: TextPosition {
                        node: native_id,
                        character_index: character_offset(value, selection.extent),
                    },
                });
            }
        }
        for action in &semantic.actions {
            if let Some(action) = map_action(*action) {
                node.add_action(action);
            }
        }
        node.set_children(
            semantic
                .children
                .iter()
                .filter_map(|child| self.semantic_to_native.get(child).copied())
                .collect::<Vec<_>>(),
        );
        node
    }

    fn previous_focus(&self) -> NodeId {
        self.previous
            .iter()
            .find_map(|(id, node)| node.semantic.state.focused.then_some(*id))
            .and_then(|id| self.semantic_to_native.get(&id).copied())
            .unwrap_or(HOST_NODE_ID)
    }
}

fn supports(node: &SemanticNode, action: SemanticActionKind) -> bool {
    node.actions.contains(&action)
}

fn map_role(role: Role, obscured: bool) -> AccessKitRole {
    match role {
        Role::Button => AccessKitRole::Button,
        Role::Text => AccessKitRole::Label,
        Role::TextField => {
            if obscured {
                AccessKitRole::PasswordInput
            } else {
                AccessKitRole::TextInput
            }
        }
        Role::TextArea => AccessKitRole::MultilineTextInput,
        Role::List => AccessKitRole::List,
        Role::ListItem => AccessKitRole::ListItem,
        Role::ScrollView => AccessKitRole::ScrollView,
        Role::GenericContainer => AccessKitRole::GenericContainer,
        Role::Checkbox => AccessKitRole::CheckBox,
        Role::Radio => AccessKitRole::RadioButton,
        Role::Switch => AccessKitRole::Switch,
        Role::Slider => AccessKitRole::Slider,
        Role::Menu => AccessKitRole::Menu,
        Role::Dialog => AccessKitRole::Dialog,
        Role::Image => AccessKitRole::Image,
        Role::Heading => AccessKitRole::Heading,
        Role::Link => AccessKitRole::Link,
        Role::Group => AccessKitRole::Group,
        Role::Tab => AccessKitRole::Tab,
        Role::TabList => AccessKitRole::TabList,
        Role::TabPanel => AccessKitRole::TabPanel,
        Role::MenuItem => AccessKitRole::MenuItem,
        Role::ProgressBar => AccessKitRole::ProgressIndicator,
        Role::Meter => AccessKitRole::Meter,
        Role::SearchField => AccessKitRole::SearchInput,
    }
}

fn is_interactive_role(role: Role) -> bool {
    matches!(
        role,
        Role::Button
            | Role::TextField
            | Role::TextArea
            | Role::Checkbox
            | Role::Radio
            | Role::Switch
            | Role::Slider
            | Role::Link
            | Role::Tab
            | Role::MenuItem
            | Role::SearchField
    )
}

fn map_action(action: SemanticActionKind) -> Option<Action> {
    match action {
        SemanticActionKind::Focus => Some(Action::Focus),
        SemanticActionKind::Activate => Some(Action::Click),
        SemanticActionKind::SetText => Some(Action::SetValue),
        SemanticActionKind::SetSelection => Some(Action::SetTextSelection),
        SemanticActionKind::Increment => Some(Action::Increment),
        SemanticActionKind::Decrement => Some(Action::Decrement),
        SemanticActionKind::ScrollForward => Some(Action::ScrollDown),
        SemanticActionKind::ScrollBackward => Some(Action::ScrollUp),
    }
}

fn to_accesskit_rect(rect: Rect, scale_factor: f64) -> AccessKitRect {
    AccessKitRect {
        x0: f64::from(rect.origin.x) * scale_factor,
        y0: f64::from(rect.origin.y) * scale_factor,
        x1: f64::from(rect.origin.x + rect.size.width) * scale_factor,
        y1: f64::from(rect.origin.y + rect.size.height) * scale_factor,
    }
}

fn scroll_value(value: Option<&str>) -> (f64, f64) {
    let Some((offset, maximum)) = value.and_then(|value| value.split_once('/')) else {
        return (0.0, 0.0);
    };
    let parse = |value: &str| {
        value
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite())
            .unwrap_or(0.0)
            .max(0.0)
    };
    (parse(offset), parse(maximum))
}

fn character_offset(text: &str, byte_offset: usize) -> usize {
    let mut boundary = byte_offset.min(text.len());
    while boundary > 0 && !text.is_char_boundary(boundary) {
        boundary -= 1;
    }
    text[..boundary].chars().count()
}

fn utf8_offset(text: &str, character_offset: usize) -> usize {
    text.char_indices()
        .nth(character_offset)
        .map_or(text.len(), |(offset, _)| offset)
}
