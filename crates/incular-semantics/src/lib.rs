//! Renderer- and platform-neutral retained accessibility semantics.
//!
//! This crate owns the semantic data model and its generational retained tree.
//! Widget trees map their persistent identities to these nodes, while runtimes
//! and native adapters communicate exclusively through `SemanticNodeId` and
//! `SemanticAction`.

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
    /// A two-state toggle distinct from a momentary button.
    Switch,
    Slider,
    Menu,
    Dialog,
    Image,
    Heading,
    Link,
    Group,
    Tab,
    TabList,
    TabPanel,
    MenuItem,
    ProgressBar,
    Meter,
    SearchField,
}

pub type SemanticRole = Role;

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

/// A request received from a native accessibility adapter. Text offsets use
/// the editing model's UTF-8 byte indexing convention.
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

/// Value of a checkable control. `None` in [`SemanticState::checked`] means the
/// node is not checkable; an indeterminate control is still checkable.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CheckedState {
    #[default]
    Unchecked,
    Checked,
    Indeterminate,
}

impl CheckedState {
    #[must_use]
    pub const fn is_checked(self) -> bool {
        matches!(self, Self::Checked)
    }
}

impl From<bool> for CheckedState {
    fn from(checked: bool) -> Self {
        if checked {
            Self::Checked
        } else {
            Self::Unchecked
        }
    }
}

impl std::fmt::Display for CheckedState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Unchecked => "false",
            Self::Checked => "true",
            Self::Indeterminate => "mixed",
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SemanticState {
    pub enabled: bool,
    pub focused: bool,
    pub focusable: bool,
    pub selected: bool,
    pub checked: Option<CheckedState>,
    pub expanded: Option<bool>,
    pub read_only: bool,
    pub editable: bool,
    /// Whether a text field's value is visually obscured (for example a
    /// password field).  Keeping this in the portable tree lets native
    /// accessibility adapters make the same privacy decision as the widget
    /// layer without exposing the clear-text paint representation.
    pub obscured: bool,
    pub multiline: bool,
    pub selection: Option<TextSelection>,
    pub item_index: Option<usize>,
    pub set_size: Option<usize>,
    /// Indicates that the node's value is still being produced.
    pub busy: bool,
    /// Validation state exposed independently from enabled/read-only state.
    pub invalid: bool,
    /// Whether the user must provide a value.
    pub required: bool,
    /// Numeric range/value metadata used by sliders, meters, and progress
    /// indicators. Values stay floating point because Flutter controls do.
    pub numeric_value: Option<f64>,
    pub numeric_min: Option<f64>,
    pub numeric_max: Option<f64>,
    pub numeric_step: Option<f64>,
    /// Heading level in the portable tree (1 is the most prominent level).
    pub heading_level: Option<u8>,
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
    /// Monotonically increases only when the retained semantic graph changes.
    /// Native adapters use this to avoid rebuilding or even walking an
    /// unchanged tree on paint/compositor-only frames.
    pub revisions: u64,
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
        if self.root != root {
            self.root = root;
            self.diagnostics.revisions = self.diagnostics.revisions.wrapping_add(1);
        }
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

    /// Revision of the retained semantic graph.
    ///
    /// This is deliberately independent from paint and compositor state.  A
    /// platform bridge can use it as a cheap dirty signal before performing
    /// any projection work.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.diagnostics.revisions
    }

    pub fn insert(&mut self, mut node: SemanticNode) -> SemanticNodeId {
        let id = SemanticNodeId(self.nodes.insert(node.clone()));
        node.id = id;
        *self.nodes.get_mut(id.0).expect("new semantic node") = node;
        self.diagnostics.nodes_created += 1;
        self.diagnostics.revisions = self.diagnostics.revisions.wrapping_add(1);
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
            self.diagnostics.revisions = self.diagnostics.revisions.wrapping_add(1);
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
        self.diagnostics.revisions = self.diagnostics.revisions.wrapping_add(1);
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
        self.debug_dump_limited(usize::MAX, usize::MAX, usize::MAX)
    }

    /// Produces a deterministic diagnostic dump with hard node, depth, and
    /// text limits. Accessibility trees can be application-sized, so debug
    /// tooling must not recurse or allocate in proportion to an unbounded
    /// subtree. A truncation marker is included in the returned text when a
    /// limit is reached.
    #[must_use]
    pub fn debug_dump_bounded(&self, max_nodes: usize) -> String {
        self.debug_dump_limited(max_nodes, 256, 256)
    }

    /// Lower-level bounded dump used by retained diagnostics and visual
    /// debuggers that need a tighter label/depth budget.
    #[must_use]
    pub fn debug_dump_limited(
        &self,
        max_nodes: usize,
        max_depth: usize,
        max_text_chars: usize,
    ) -> String {
        fn truncate(value: Option<&String>, max_chars: usize) -> Option<String> {
            let value = value?;
            if value.chars().count() <= max_chars {
                return Some(value.clone());
            }
            let mut result = value.chars().take(max_chars).collect::<String>();
            result.push('…');
            Some(result)
        }

        let mut output = String::new();
        let Some(root) = self.root else {
            return output;
        };
        let mut stack = vec![(root, 0usize)];
        let mut visited = 0usize;
        while let Some((id, depth)) = stack.pop() {
            if visited >= max_nodes {
                output.push_str("… semantic dump truncated (node limit)\n");
                break;
            }
            if depth > max_depth {
                output.push_str(&format!(
                    "{}… semantic dump truncated (depth limit)\n",
                    "  ".repeat(max_depth.min(256))
                ));
                continue;
            }
            let Some(node) = self.node(id) else { continue };
            visited = visited.saturating_add(1);
            output.push_str(&format!(
                "{}{:?}#{:?} label={:?} value={:?} bounds=({:.1},{:.1},{:.1},{:.1}) focused={} actions={:?}\n",
                "  ".repeat(depth), node.role, node.id,
                truncate(node.label.as_ref(), max_text_chars),
                truncate(node.value.as_ref(), max_text_chars),
                node.bounds.origin.x, node.bounds.origin.y, node.bounds.size.width,
                node.bounds.size.height, node.state.focused, node.actions,
            ));
            for child in node.children.iter().rev() {
                stack.push((*child, depth.saturating_add(1)));
            }
        }
        output
    }
}
