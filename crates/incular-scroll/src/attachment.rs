use std::sync::atomic::{AtomicU64, Ordering};

use crate::controller::ScrollController;

/// Process-wide sequence minting attachment identities. One live
/// attachment exists per controller, but identities are never reused, so
/// a handle kept past its release can never alias a later attachment —
/// including one created by another tree.
static ATTACHMENT_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// Diagnostic owner identity for an attachment request: names the widget
/// tree asking to drive the controller's metrics. A tree id alone cannot
/// distinguish two viewports in one tree, so this value carries no
/// authority — authority lives only in the [`MetricAttachment`] returned
/// on success.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MetricOwner {
    tree: u64,
}

impl MetricOwner {
    /// Names the requesting widget tree. Framework-internal.
    #[doc(hidden)]
    #[must_use]
    pub fn of_tree(tree: u64) -> Self {
        Self { tree }
    }

    /// The requesting tree, for diagnostics and conflict reporting.
    #[must_use]
    pub fn tree(self) -> u64 {
        self.tree
    }
}

/// Rejection from [`ScrollController::try_attach`]: the controller
/// already has a live owner. Both fields are diagnostics for reporting
/// the conflict; neither grants any power to release.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AttachmentConflict {
    owner_tree: u64,
    attachment: u64,
}

impl AttachmentConflict {
    pub(crate) fn for_owner(live: StoredAttachment) -> Self {
        Self {
            owner_tree: live.tree,
            attachment: live.id,
        }
    }

    /// The tree holding the live attachment, for diagnostics.
    #[must_use]
    pub fn owner_tree(&self) -> u64 {
        self.owner_tree
    }

    /// The live attachment's identity, for diagnostics. A tree holding
    /// leases resolves its owning viewport by matching this value.
    #[must_use]
    pub fn attachment_id(&self) -> u64 {
        self.attachment
    }
}

/// Non-cloneable proof that one viewport drives a controller's metrics.
///
/// The handle is created only by a successful
/// [`ScrollController::try_attach`] on a free controller. Cloning the
/// controller never creates one: read-only clones and coordination
/// handles observe without owning. Only the live attachment releases;
/// a stale handle reports `false` and changes nothing, so an older
/// generation can never release a newer attachment.
pub struct MetricAttachment {
    controller: ScrollController,
    id: u64,
    tree: u64,
}

impl std::fmt::Debug for MetricAttachment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MetricAttachment")
            .field("id", &self.id)
            .field("tree", &self.tree)
            .finish()
    }
}

impl MetricAttachment {
    pub(crate) fn new(controller: ScrollController, id: u64, tree: u64) -> Self {
        Self {
            controller,
            id,
            tree,
        }
    }

    /// This attachment's unique identity, for diagnostics and
    /// lease bookkeeping. Never reused, even across trees.
    #[must_use]
    pub fn id(&self) -> u64 {
        self.id
    }

    /// The tree that created this attachment, for diagnostics.
    #[must_use]
    pub fn tree(&self) -> u64 {
        self.tree
    }

    /// The attached controller. The returned clone observes and drives
    /// values like any handle, but claims nothing by itself.
    #[must_use]
    pub fn controller(&self) -> &ScrollController {
        &self.controller
    }

    /// Releases ownership when this attachment is still the live owner.
    /// Returns whether the release happened; a stale handle returns
    /// `false` without touching the current owner, its activity, or its
    /// metrics. State commits before this returns, so callers can notify
    /// afterwards without racing a half-released owner.
    pub fn release(&self) -> bool {
        let mut state = self.controller.state.borrow_mut();
        match state.metric_attachment {
            Some(live) if live.id == self.id => {
                state.metric_attachment = None;
                true
            }
            _ => false,
        }
    }

    /// Normal detach: releases ownership, then ends any open activity
    /// with its documented `End` notification. Ownership commits before
    /// the callback runs, so a listener reattaching during `End` finds
    /// the controller free; nothing after the callback can cancel an
    /// activity it starts. A stale handle ends nothing — it can never
    /// close another owner's activity. Framework-internal.
    #[doc(hidden)]
    pub fn detach(&self) {
        if self.release() {
            self.controller.end_activity();
        }
    }

    /// Teardown/unwind: releases ownership, then silently clears any
    /// open activity without notifying. Listeners belong to torn-down
    /// context by then — notifying from `Drop` could panic during
    /// unwinding — so the next `begin_activity` starts fresh instead of
    /// bricking on a stuck flag. A stale handle aborts nothing, so it
    /// can never cancel a new owner's activity. Repeated calls are
    /// harmless. Framework-internal.
    #[doc(hidden)]
    pub fn teardown(&self) {
        if self.release() {
            self.controller.abort_activity();
        }
    }
}

/// The controller's current owner record: attachment identity plus the
/// owning tree as a diagnostic. The identity decides release; the tree
/// id only names the owner in conflict reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct StoredAttachment {
    pub(crate) id: u64,
    pub(crate) tree: u64,
}

impl StoredAttachment {
    pub(crate) fn mint(tree: u64) -> Self {
        Self {
            id: ATTACHMENT_SEQUENCE.fetch_add(1, Ordering::Relaxed),
            tree,
        }
    }
}
