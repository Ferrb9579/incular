//! Native accessibility projection for Incular.
//!
//! [`incular_semantics::SemanticsTree`] remains the retained, platform-neutral
//! source of truth. This crate projects it to AccessKit at the desktop-native
//! boundary; it does not introduce a second semantic model. AccessKit node IDs
//! are private to one live native window and never enter widget/facade APIs.

use accesskit::{
    Action, ActionData, ActionRequest, Node, NodeId, Rect as AccessKitRect, Role as AccessKitRole,
    TextPosition, TextSelection as AccessKitSelection, Toggled, Tree, TreeId, TreeUpdate,
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
        let mut node = Node::new(map_role(semantic.role));
        node.set_bounds(to_accesskit_rect(semantic.bounds, scale_factor));
        if semantic.role == Role::Text {
            if let Some(text) = semantic.value.as_ref().or(semantic.label.as_ref()) {
                node.set_value(text.as_str());
            }
        } else {
            if let Some(label) = &semantic.label {
                node.set_label(label.as_str());
            }
            if let Some(value) = &semantic.value {
                node.set_value(value.as_str());
            }
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
            node.set_toggled(if checked {
                Toggled::True
            } else {
                Toggled::False
            });
        }
        if let Some(expanded) = semantic.state.expanded {
            node.set_expanded(expanded);
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
        if semantic.state.editable {
            if let Some(value) = semantic.value.as_deref() {
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

fn map_role(role: Role) -> AccessKitRole {
    match role {
        Role::Button => AccessKitRole::Button,
        Role::Text => AccessKitRole::Label,
        Role::TextField => AccessKitRole::TextInput,
        Role::TextArea => AccessKitRole::MultilineTextInput,
        Role::List => AccessKitRole::List,
        Role::ListItem => AccessKitRole::ListItem,
        Role::ScrollView => AccessKitRole::ScrollView,
        Role::GenericContainer => AccessKitRole::GenericContainer,
        Role::Checkbox => AccessKitRole::CheckBox,
        Role::Radio => AccessKitRole::RadioButton,
        Role::Slider => AccessKitRole::Slider,
        Role::Menu => AccessKitRole::Menu,
        Role::Dialog => AccessKitRole::Dialog,
        Role::Image => AccessKitRole::Image,
        Role::Heading => AccessKitRole::Heading,
        Role::Link => AccessKitRole::Link,
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
            | Role::Slider
            | Role::Link
    )
}

fn map_action(action: SemanticActionKind) -> Option<Action> {
    match action {
        SemanticActionKind::Focus => Some(Action::Focus),
        SemanticActionKind::Activate => Some(Action::Click),
        SemanticActionKind::SetText => Some(Action::SetValue),
        SemanticActionKind::SetSelection => Some(Action::SetTextSelection),
        SemanticActionKind::Increment | SemanticActionKind::Decrement => None,
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

#[cfg(test)]
mod tests {
    use super::*;
    use incular_core::{ArenaId, Offset, Size};

    fn node(role: Role) -> SemanticNode {
        SemanticNode {
            id: SemanticNodeId(ArenaId::from_parts(0, 0)),
            role,
            label: None,
            value: None,
            description: None,
            bounds: Rect::from_origin_size(Offset::new(2.0, 3.0), Size::new(20.0, 10.0)),
            state: SemanticState::default(),
            actions: vec![],
            children: vec![],
        }
    }

    fn full(projection: &mut AccessKitProjection, tree: &SemanticsTree) -> TreeUpdate {
        projection.activate();
        projection
            .sync(tree, 2.0)
            .expect("full update")
            .into_accesskit()
    }

    #[test]
    fn button_role_name_action_and_bounds_are_projected() {
        let mut tree = SemanticsTree::new();
        let mut button = node(Role::Button);
        button.label = Some("Save".into());
        button.state.enabled = true;
        button.actions = vec![SemanticActionKind::Focus, SemanticActionKind::Activate];
        let id = tree.insert(button);
        tree.set_root(Some(id));
        let mut projection = AccessKitProjection::new();
        let update = full(&mut projection, &tree);
        let native = NodeId(projection.native_node_id(id).expect("mapped"));
        let node = &update.nodes.iter().find(|(id, _)| *id == native).unwrap().1;
        assert_eq!(node.role(), AccessKitRole::Button);
        assert_eq!(node.label(), Some("Save"));
        assert!(node.supports_action(Action::Click));
        assert_eq!(node.bounds().unwrap().x1, 44.0);
    }

    #[test]
    fn state_text_collection_and_scroll_properties_are_projected() {
        let mut tree = SemanticsTree::new();
        let mut input = node(Role::TextArea);
        input.value = Some("héllo".into());
        input.state.enabled = true;
        input.state.editable = true;
        input.state.read_only = true;
        input.state.checked = Some(true);
        input.state.selection = Some(TextSelection { base: 1, extent: 3 });
        input.actions = vec![
            SemanticActionKind::SetText,
            SemanticActionKind::SetSelection,
        ];
        let input_id = tree.insert(input);
        let mut list = node(Role::ListItem);
        list.state.selected = true;
        list.state.item_index = Some(4);
        list.state.set_size = Some(12);
        let list_id = tree.insert(list);
        let mut scroll = node(Role::ScrollView);
        scroll.value = Some("8/50".into());
        scroll.actions = vec![
            SemanticActionKind::ScrollForward,
            SemanticActionKind::ScrollBackward,
        ];
        let scroll_id = tree.insert(scroll);
        let mut root = node(Role::GenericContainer);
        root.children = vec![input_id, list_id, scroll_id];
        let root_id = tree.insert(root);
        tree.set_root(Some(root_id));
        let mut projection = AccessKitProjection::new();
        let update = full(&mut projection, &tree);
        let get = |id| {
            update
                .nodes
                .iter()
                .find(|(native, _)| native.0 == projection.native_node_id(id).unwrap())
                .map(|(_, node)| node)
                .unwrap()
        };
        let input = get(input_id);
        assert_eq!(input.role(), AccessKitRole::MultilineTextInput);
        assert_eq!(input.value(), Some("héllo"));
        assert!(input.is_read_only());
        assert_eq!(input.toggled(), Some(Toggled::True));
        assert!(input.supports_action(Action::SetValue));
        assert_eq!(input.text_selection().unwrap().focus.character_index, 2);
        let list = get(list_id);
        assert_eq!(list.is_selected(), Some(true));
        assert_eq!(list.position_in_set(), Some(5));
        assert_eq!(list.size_of_set(), Some(12));
        let scroll = get(scroll_id);
        assert_eq!(scroll.scroll_y(), Some(8.0));
        assert_eq!(scroll.scroll_y_max(), Some(50.0));
        assert!(scroll.supports_action(Action::ScrollDown));
    }

    #[test]
    fn static_content_controls_and_modal_roles_have_documented_native_mappings() {
        let mut tree = SemanticsTree::new();
        let mut button = node(Role::Button);
        button.label = Some("Unavailable".into());
        button.actions = vec![SemanticActionKind::Activate];
        let button = tree.insert(button);
        let mut text = node(Role::Text);
        text.label = Some("Read this static text".into());
        let text = tree.insert(text);
        let mut heading = node(Role::Heading);
        heading.label = Some("Preferences".into());
        let heading = tree.insert(heading);
        let mut image = node(Role::Image);
        image.label = Some("Company logo".into());
        let image = tree.insert(image);
        let mut dialog = node(Role::Dialog);
        dialog.label = Some("Confirm deletion".into());
        let dialog = tree.insert(dialog);
        let mut root = node(Role::GenericContainer);
        root.children = vec![button, text, heading, image, dialog];
        let root = tree.insert(root);
        tree.set_root(Some(root));

        let mut projection = AccessKitProjection::new();
        let update = full(&mut projection, &tree);
        let get = |semantic| {
            update
                .nodes
                .iter()
                .find(|(native, _)| native.0 == projection.native_node_id(semantic).unwrap())
                .map(|(_, node)| node)
                .unwrap()
        };
        assert!(get(button).is_disabled());
        assert_eq!(get(text).role(), AccessKitRole::Label);
        assert_eq!(get(text).value(), Some("Read this static text"));
        assert_eq!(get(heading).role(), AccessKitRole::Heading);
        assert_eq!(get(image).role(), AccessKitRole::Image);
        assert_eq!(get(image).label(), Some("Company logo"));
        assert_eq!(get(dialog).role(), AccessKitRole::Dialog);
    }

    #[test]
    fn unchanged_tree_emits_no_update_but_label_focus_and_removal_are_incremental() {
        let mut tree = SemanticsTree::new();
        let mut button = node(Role::Button);
        button.label = Some("Save".into());
        button.state.enabled = true;
        button.actions = vec![SemanticActionKind::Focus, SemanticActionKind::Activate];
        let id = tree.insert(button);
        tree.set_root(Some(id));
        let mut projection = AccessKitProjection::new();
        let _ = full(&mut projection, &tree);
        assert!(projection.sync(&tree, 2.0).is_none());
        let mut changed = tree.node(id).unwrap().clone();
        changed.label = Some("Saved".into());
        tree.update(id, changed);
        let update = projection.sync(&tree, 2.0).unwrap();
        assert_eq!(update.kind(), AccessKitUpdateKind::Incremental);
        assert_eq!(update.into_accesskit().nodes.len(), 1);
        let mut focused = tree.node(id).unwrap().clone();
        focused.state.focused = true;
        tree.update(id, focused);
        assert!(projection.sync(&tree, 2.0).is_some());
        tree.remove(id);
        assert!(projection.sync(&tree, 2.0).is_some());
        assert!(projection.diagnostics().nodes_removed >= 1);
    }

    #[test]
    fn stale_native_action_cannot_reach_a_reused_semantic_slot() {
        let mut tree = SemanticsTree::new();
        let mut first = node(Role::Button);
        first.state.enabled = true;
        first.actions = vec![SemanticActionKind::Activate];
        let first = tree.insert(first);
        tree.set_root(Some(first));
        let mut projection = AccessKitProjection::new();
        let _ = full(&mut projection, &tree);
        let stale_native = NodeId(projection.native_node_id(first).unwrap());
        tree.remove(first);
        let replacement = tree.insert(node(Role::Button));
        tree.set_root(Some(replacement));
        let _ = projection.sync(&tree, 1.0);
        let request = ActionRequest {
            action: Action::Click,
            target_tree: TreeId::ROOT,
            target_node: stale_native,
            data: None,
        };
        assert!(projection.translate_action(&request).is_none());
        assert_eq!(projection.diagnostics().stale_actions_rejected, 1);
        assert_ne!(projection.native_node_id(replacement), Some(stale_native.0));
    }

    #[test]
    fn native_actions_translate_to_existing_semantic_actions() {
        let mut tree = SemanticsTree::new();
        let mut field = node(Role::TextField);
        field.value = Some("héllo".into());
        field.state.enabled = true;
        field.state.editable = true;
        field.actions = vec![
            SemanticActionKind::Focus,
            SemanticActionKind::SetText,
            SemanticActionKind::SetSelection,
        ];
        let id = tree.insert(field);
        tree.set_root(Some(id));
        let mut projection = AccessKitProjection::new();
        let _ = full(&mut projection, &tree);
        let native = NodeId(projection.native_node_id(id).unwrap());
        let set_text = ActionRequest {
            action: Action::SetValue,
            target_tree: TreeId::ROOT,
            target_node: native,
            data: Some(ActionData::Value("edited".into())),
        };
        assert_eq!(
            projection.translate_action(&set_text),
            Some(SemanticActionRequest {
                node: id,
                action: SemanticAction::SetText("edited".into())
            })
        );
        let selection = ActionRequest {
            action: Action::SetTextSelection,
            target_tree: TreeId::ROOT,
            target_node: native,
            data: Some(ActionData::SetTextSelection(AccessKitSelection {
                anchor: TextPosition {
                    node: native,
                    character_index: 1,
                },
                focus: TextPosition {
                    node: native,
                    character_index: 3,
                },
            })),
        };
        assert_eq!(
            projection.translate_action(&selection),
            Some(SemanticActionRequest {
                node: id,
                action: SemanticAction::SetSelection { base: 1, extent: 4 }
            })
        );

        let mut scroll_tree = SemanticsTree::new();
        let mut scroll = node(Role::ScrollView);
        scroll.actions = vec![
            SemanticActionKind::ScrollForward,
            SemanticActionKind::ScrollBackward,
        ];
        let scroll_id = scroll_tree.insert(scroll);
        scroll_tree.set_root(Some(scroll_id));
        let mut scroll_projection = AccessKitProjection::new();
        let _ = full(&mut scroll_projection, &scroll_tree);
        let request = ActionRequest {
            action: Action::ScrollDown,
            target_tree: TreeId::ROOT,
            target_node: NodeId(scroll_projection.native_node_id(scroll_id).unwrap()),
            data: None,
        };
        assert_eq!(
            scroll_projection.translate_action(&request),
            Some(SemanticActionRequest {
                node: scroll_id,
                action: SemanticAction::ScrollForward
            })
        );
    }
}
