//! iOS platform integration for Incular.
//!
//! UIKit lifecycle, touch and gesture input, safe areas, system UI,
//! accessibility, and native resource access will be introduced here.

use incular_accessibility::{
    MobileAccessibilityProjection, MobileAccessibilityUpdate, SemanticAction,
    SemanticActionRequest, SemanticsTree,
};

/// iOS-facing accessibility adapter. UIKit/SwiftUI bridges consume the same
/// stable-node update contract as Android, while actions are validated against
/// the retained semantic snapshot before reaching the runtime.
#[derive(Default)]
pub struct IosAccessibilityAdapter {
    projection: MobileAccessibilityProjection,
}

impl IosAccessibilityAdapter {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn activate(&mut self) {
        self.projection.activate();
    }

    pub fn deactivate(&mut self) {
        self.projection.deactivate();
    }

    pub fn sync(&mut self, tree: &SemanticsTree) -> Option<MobileAccessibilityUpdate> {
        self.projection.sync(tree)
    }

    #[must_use]
    pub fn action(&self, native_id: u64, action: SemanticAction) -> Option<SemanticActionRequest> {
        self.projection.translate_action(native_id, action)
    }

    #[must_use]
    pub fn native_node_id(&self, node: incular_accessibility::SemanticNodeId) -> Option<u64> {
        self.projection.native_node_id(node)
    }
}
