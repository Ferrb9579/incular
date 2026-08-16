//! Renderer-neutral, retained accessibility semantics.
//!
//! This crate deliberately has no window-system dependency. Widget trees own
//! the mapping from their persistent identities to these nodes; platform
//! adapters consume owned snapshots and return actions by `SemanticNodeId`.

use incular_core::{Arena, ArenaId, Rect};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SemanticNodeId(pub ArenaId);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    Button,
    Text,
    TextField,
    TextArea,
    List,
    ListItem,
    ScrollView,
    GenericContainer,
    Checkbox,
    Radio,
    Slider,
    Menu,
    Dialog,
    Image,
    Heading,
    Link,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SemanticActionKind {
    Focus,
    Activate,
    SetText,
    SetSelection,
    Increment,
    Decrement,
    ScrollForward,
    ScrollBackward,
}
/// A request received from a native adapter. Text offsets use the editing
/// model's UTF-8 byte indexing convention.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SemanticAction {
    Focus,
    Activate,
    SetText(String),
    SetSelection { base: usize, extent: usize },
    Increment,
    Decrement,
    ScrollForward,
    ScrollBackward,
}
impl SemanticAction {
    #[must_use]
    pub const fn kind(&self) -> SemanticActionKind {
        match self {
            Self::Focus => SemanticActionKind::Focus,
            Self::Activate => SemanticActionKind::Activate,
            Self::SetText(_) => SemanticActionKind::SetText,
            Self::SetSelection { .. } => SemanticActionKind::SetSelection,
            Self::Increment => SemanticActionKind::Increment,
            Self::Decrement => SemanticActionKind::Decrement,
            Self::ScrollForward => SemanticActionKind::ScrollForward,
            Self::ScrollBackward => SemanticActionKind::ScrollBackward,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextSelection {
    pub base: usize,
    pub extent: usize,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SemanticState {
    pub enabled: bool,
    pub focused: bool,
    pub focusable: bool,
    pub selected: bool,
    pub checked: Option<bool>,
    pub expanded: Option<bool>,
    pub read_only: bool,
    pub editable: bool,
    pub multiline: bool,
    pub selection: Option<TextSelection>,
    pub item_index: Option<usize>,
    pub set_size: Option<usize>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct SemanticNode {
    pub id: SemanticNodeId,
    pub role: Role,
    pub label: Option<String>,
    pub value: Option<String>,
    pub description: Option<String>,
    pub bounds: Rect,
    pub state: SemanticState,
    pub actions: Vec<SemanticActionKind>,
    pub children: Vec<SemanticNodeId>,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SemanticsDiagnostics {
    pub nodes_created: u64,
    pub nodes_removed: u64,
    pub nodes_updated: u64,
    pub property_updates: u64,
    pub geometry_updates: u64,
    pub native_updates: u64,
    pub actions_received: u64,
}
/// Retained semantic arena. IDs are generational, so stale platform actions
/// cannot address a node that later reuses the same storage slot.
#[derive(Default)]
pub struct SemanticsTree {
    nodes: Arena<SemanticNode>,
    root: Option<SemanticNodeId>,
    diagnostics: SemanticsDiagnostics,
}
impl SemanticsTree {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    #[must_use]
    pub const fn root(&self) -> Option<SemanticNodeId> {
        self.root
    }
    pub fn set_root(&mut self, root: Option<SemanticNodeId>) {
        self.root = root;
    }
    #[must_use]
    pub fn node(&self, id: SemanticNodeId) -> Option<&SemanticNode> {
        self.nodes.get(id.0)
    }
    #[must_use]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
    #[must_use]
    pub const fn diagnostics(&self) -> SemanticsDiagnostics {
        self.diagnostics
    }
    pub fn insert(&mut self, mut node: SemanticNode) -> SemanticNodeId {
        let id = SemanticNodeId(self.nodes.insert(node.clone()));
        node.id = id;
        *self.nodes.get_mut(id.0).expect("new semantic node") = node;
        self.diagnostics.nodes_created += 1;
        id
    }
    pub fn update(&mut self, id: SemanticNodeId, node: SemanticNode) -> bool {
        let Some(old) = self.nodes.get_mut(id.0) else {
            return false;
        };
        let properties_changed = old.role != node.role
            || old.label != node.label
            || old.value != node.value
            || old.description != node.description
            || old.state != node.state
            || old.actions != node.actions
            || old.children != node.children;
        let geometry_changed = old.bounds != node.bounds;
        if properties_changed || geometry_changed {
            *old = node;
            self.diagnostics.nodes_updated += 1;
        }
        if properties_changed {
            self.diagnostics.property_updates += 1;
        }
        if geometry_changed {
            self.diagnostics.geometry_updates += 1;
        }
        true
    }
    pub fn remove(&mut self, id: SemanticNodeId) -> Option<SemanticNode> {
        let node = self.nodes.remove(id.0)?;
        self.diagnostics.nodes_removed += 1;
        if self.root == Some(id) {
            self.root = None;
        }
        Some(node)
    }
    pub fn note_action(&mut self) {
        self.diagnostics.actions_received += 1;
    }
    pub fn note_native_update(&mut self) {
        self.diagnostics.native_updates += 1;
    }
    pub fn iter(&self) -> impl Iterator<Item = (SemanticNodeId, &SemanticNode)> {
        self.nodes
            .iter()
            .map(|(id, node)| (SemanticNodeId(id), node))
    }
    #[must_use]
    pub fn debug_dump(&self) -> String {
        fn dump(tree: &SemanticsTree, id: SemanticNodeId, depth: usize, out: &mut String) {
            let Some(n) = tree.node(id) else { return };
            out.push_str(&format!("{}{:?}#{:?} label={:?} value={:?} bounds=({:.1},{:.1},{:.1},{:.1}) focused={} actions={:?}\n", "  ".repeat(depth), n.role, n.id, n.label, n.value, n.bounds.origin.x, n.bounds.origin.y, n.bounds.size.width, n.bounds.size.height, n.state.focused, n.actions));
            for child in &n.children {
                dump(tree, *child, depth + 1, out);
            }
        }
        let mut out = String::new();
        if let Some(root) = self.root {
            dump(self, root, 0, &mut out);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use incular_core::{Offset, Size};
    #[test]
    fn stale_ids_do_not_address_reused_nodes() {
        let mut t = SemanticsTree::new();
        let node = |id| SemanticNode {
            id,
            role: Role::Text,
            label: None,
            value: None,
            description: None,
            bounds: Rect::from_origin_size(Offset::ZERO, Size::ZERO),
            state: SemanticState::default(),
            actions: vec![],
            children: vec![],
        };
        let a = t.insert(node(SemanticNodeId(ArenaId::from_parts(0, 0))));
        t.remove(a);
        let b = t.insert(node(a));
        assert_ne!(a, b);
        assert!(t.node(a).is_none());
        assert!(t.node(b).is_some());
    }
}
