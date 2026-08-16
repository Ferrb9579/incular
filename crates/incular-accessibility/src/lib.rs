//! Native-accessibility adapter contracts.
//!
//! The canonical semantic model lives in `incular-semantics`. This crate owns
//! the boundary used by platform adapters: immutable snapshots sent to an OS
//! bridge and owned action requests returned from it. Re-exports preserve the
//! previous `incular_accessibility` import path during the extraction.

pub use incular_semantics::*;

/// An owned semantic tree snapshot suitable for a platform bridge.
///
/// It deliberately contains no `Arena` or widget references, so adapters can
/// retain it across native callbacks without borrowing the application tree.
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

/// Boundary implemented by platform-specific accessibility bridges.
///
/// Runtime code publishes a fresh owned snapshot after semantic
/// synchronization, then drains requests and dispatches them by node ID.
pub trait SemanticsAdapter {
    fn publish(&mut self, snapshot: SemanticsSnapshot);
    fn poll_action(&mut self) -> Option<SemanticActionRequest>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use incular_core::{ArenaId, Offset, Rect, Size};

    #[test]
    fn snapshots_are_owned_and_preserve_the_root() {
        let mut tree = SemanticsTree::new();
        let id = tree.insert(SemanticNode {
            id: SemanticNodeId(ArenaId::from_parts(0, 0)),
            role: Role::Button,
            label: Some("Save".into()),
            value: None,
            description: None,
            bounds: Rect::from_origin_size(Offset::ZERO, Size::new(20.0, 10.0)),
            state: SemanticState::default(),
            actions: vec![SemanticActionKind::Activate],
            children: vec![],
        });
        tree.set_root(Some(id));
        let snapshot = SemanticsSnapshot::from(&tree);
        tree.remove(id);

        assert_eq!(snapshot.root, Some(id));
        assert_eq!(snapshot.nodes[0].label.as_deref(), Some("Save"));
    }
}
