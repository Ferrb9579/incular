//! Metric-attachment ownership: the enforced claim behind viewport layout.
//!
//! The controller holds one attachment slot. [`ScrollController::try_attach`]
//! fills it or reports the live owner; only the returned handle releases,
//! and a stale handle changes nothing.

use incular_scroll::{MetricOwner, ScrollController};

fn owner(tree: u64) -> MetricOwner {
    MetricOwner::of_tree(tree)
}

#[test]
fn free_controller_attaches_and_releases() {
    let controller = ScrollController::new();
    assert_eq!(controller.metric_owner(), None);
    let attachment = controller
        .try_attach(owner(1))
        .expect("free controller attaches");
    assert_eq!(controller.metric_owner(), Some(1));
    assert_eq!(controller.attachment_id(), Some(attachment.id()));
    assert!(attachment.release());
    assert_eq!(controller.metric_owner(), None);
    assert_eq!(controller.attachment_id(), None);
    // Releasing again is a harmless no-op, not a second release.
    assert!(!attachment.release());
}

#[test]
fn occupied_controller_rejects_a_different_owner() {
    let controller = ScrollController::new();
    let first = controller
        .try_attach(owner(1))
        .expect("free controller attaches");
    let conflict = controller
        .try_attach(owner(2))
        .expect_err("occupied controller rejects");
    assert_eq!(conflict.owner_tree(), 1);
    assert_eq!(conflict.attachment_id(), first.id());
    // The rejection disturbed nothing: the first attachment is still the
    // live owner and still the only releaser.
    assert_eq!(controller.attachment_id(), Some(first.id()));
    assert!(first.release());
    assert_eq!(controller.metric_owner(), None);
}

#[test]
fn read_only_clones_claim_nothing() {
    let controller = ScrollController::new();
    let alias = controller.clone();
    // Mere cloning attaches nothing, either way round.
    assert_eq!(controller.metric_owner(), None);
    assert_eq!(alias.metric_owner(), None);
    let attachment = controller
        .try_attach(owner(7))
        .expect("free controller attaches");
    // The clone observes the owner but owns nothing itself: it cannot
    // attach over the live owner, and no clone API clears it.
    assert_eq!(alias.metric_owner(), Some(7));
    assert!(alias.try_attach(owner(7)).is_err());
    assert!(attachment.release());
    assert_eq!(alias.metric_owner(), None);
    // A clone of a free controller attaches exactly like the original —
    // same controller, one slot — and reports the same conflict shape.
    let via_alias = alias
        .try_attach(owner(7))
        .expect("clone attaches the free slot");
    assert!(controller.try_attach(owner(9)).is_err());
    assert!(via_alias.release());
}

#[test]
fn stale_attachment_cannot_release_a_newer_attachment() {
    let controller = ScrollController::new();
    let first = controller
        .try_attach(owner(1))
        .expect("free controller attaches");
    assert!(controller.begin_activity());
    assert!(first.release());
    let second = controller
        .try_attach(owner(2))
        .expect("released controller attaches again");
    assert_ne!(first.id(), second.id());
    // The stale handle releases nothing: owner, activity, and metrics
    // all survive it.
    assert!(!first.release());
    assert_eq!(controller.metric_owner(), Some(2));
    assert_eq!(controller.attachment_id(), Some(second.id()));
    assert!(!controller.begin_activity());
    // Only the live handle releases.
    assert!(second.release());
    assert_eq!(controller.metric_owner(), None);
}

#[test]
fn attachment_identities_stay_unique_across_controllers() {
    let first_controller = ScrollController::new();
    let second_controller = ScrollController::new();
    let first = first_controller.try_attach(owner(1)).expect("attach first");
    let second = second_controller
        .try_attach(owner(1))
        .expect("other controller attaches too");
    assert_ne!(first.id(), second.id());
    assert!(first.release());
    let renewed = first_controller
        .try_attach(owner(2))
        .expect("reattach after release");
    assert_ne!(first.id(), renewed.id());
    assert_ne!(renewed.id(), second.id());
}
