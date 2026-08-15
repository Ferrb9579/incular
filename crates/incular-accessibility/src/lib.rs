//! Renderer-neutral accessibility semantics.
//!
//! Widget/render state will build this tree; platform crates translate it to
//! native accessibility APIs. It deliberately has no OS bridge of its own.

use incular_core::Rect;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SemanticNodeId(pub u64);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Button,
    Text,
    Image,
    Container,
    Checkbox,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Focus,
    Activate,
    Increment,
    Decrement,
    SetValue,
}
#[derive(Clone, Debug, PartialEq)]
pub struct SemanticNode {
    pub id: SemanticNodeId,
    pub role: Role,
    pub label: String,
    pub value: Option<String>,
    pub hint: Option<String>,
    pub bounds: Rect,
    pub enabled: bool,
    pub focused: bool,
    pub actions: Vec<Action>,
    pub children: Vec<SemanticNodeId>,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SemanticsTree {
    pub root: Option<SemanticNodeId>,
    pub nodes: Vec<SemanticNode>,
}
impl SemanticsTree {
    #[must_use]
    pub fn node(&self, id: SemanticNodeId) -> Option<&SemanticNode> {
        self.nodes.iter().find(|node| node.id == id)
    }
}
