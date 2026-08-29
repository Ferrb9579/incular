//! Android platform integration for Incular.
//!
//! Android activity lifecycle, input, surfaces, system UI, back handling, and
//! platform resource access will be introduced here.

use incular_accessibility::{
    MobileAccessibilityProjection, MobileAccessibilityUpdate, SemanticAction,
    SemanticActionRequest, SemanticsTree,
};

/// Android-facing accessibility adapter. The host's JNI/View layer consumes
/// the stable-node update and emits actions back through [`Self::action`].
/// Keeping this adapter data-only makes it usable by both a View host and a
/// Compose-style bridge without leaking either API into the core runtime.
#[derive(Default)]
pub struct AndroidAccessibilityAdapter {
    projection: MobileAccessibilityProjection,
}

impl AndroidAccessibilityAdapter {
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
