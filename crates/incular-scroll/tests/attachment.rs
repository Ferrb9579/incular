//! Metric-attachment ownership: the enforced claim behind viewport layout.
//!
//! The controller holds one attachment slot. [`ScrollController::try_attach`]
//! fills it or reports the live owner; only the returned handle releases,
//! and a stale handle changes nothing.

use std::{cell::Cell, rc::Rc};

use incular_scroll::{
    MetricOwner, MetricWriteError, ScrollController, ScrollNotificationType, ScrollPhysics,
};

fn notification_log(
    controller: &ScrollController,
) -> (
    Rc<std::cell::RefCell<Vec<ScrollNotificationType>>>,
    incular_scroll::ScrollNotificationSubscription,
) {
    let log = Rc::new(std::cell::RefCell::new(Vec::new()));
    let logged = log.clone();
    let subscription = controller.add_listener(move |notification| {
        logged.borrow_mut().push(notification.kind);
        false
    });
    (log, subscription)
}

fn owner(tree: u64) -> MetricOwner {
    MetricOwner::of_tree(tree)
}

fn end_counter(
    controller: &ScrollController,
) -> (
    Rc<Cell<usize>>,
    incular_scroll::ScrollNotificationSubscription,
) {
    let ends = Rc::new(Cell::new(0));
    let counted = ends.clone();
    let subscription = controller.add_listener(move |notification| {
        if notification.kind == ScrollNotificationType::End {
            counted.set(counted.get() + 1);
        }
        false
    });
    (ends, subscription)
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
fn stale_handle_cannot_abort_a_new_owners_activity() {
    // A stale tree record must not cancel a newer attachment: teardown
    // through an old handle releases nothing and aborts nothing.
    let controller = ScrollController::new();
    let (ends, _guard) = end_counter(&controller);
    let old = controller
        .try_attach(owner(1))
        .expect("free controller attaches");
    assert!(controller.begin_activity());
    assert!(old.release());
    let fresh = controller
        .try_attach(owner(2))
        .expect("released controller attaches again");
    old.teardown();
    assert_eq!(ends.get(), 0);
    assert_eq!(controller.metric_owner(), Some(2));
    assert_eq!(controller.attachment_id(), Some(fresh.id()));
    assert!(!controller.begin_activity());
    // The live teardown itself is silent by policy, then starts fresh.
    fresh.teardown();
    assert_eq!(ends.get(), 0);
    assert_eq!(controller.metric_owner(), None);
    assert!(controller.begin_activity());
    assert!(controller.end_activity());
    assert_eq!(ends.get(), 1);
}

#[test]
fn detach_notifies_once_and_repeated_cleanup_is_harmless() {
    // Normal detach ends the open activity with exactly one End; every
    // later cleanup through the same handle is a silent no-op.
    let controller = ScrollController::new();
    let (ends, _guard) = end_counter(&controller);
    let attachment = controller
        .try_attach(owner(1))
        .expect("free controller attaches");
    assert!(controller.begin_activity());
    attachment.detach();
    assert_eq!(ends.get(), 1);
    assert_eq!(controller.metric_owner(), None);
    attachment.detach();
    attachment.teardown();
    assert!(!attachment.release());
    assert_eq!(ends.get(), 1);
    assert!(controller.begin_activity());
    assert!(controller.end_activity());
    assert_eq!(ends.get(), 2);
}

#[test]
fn torn_down_controller_state_stays_usable() {
    // Externally retained state survives teardown: geometry persists and
    // every programmatic operation keeps working for the next owner.
    let controller = ScrollController::new();
    controller.update_extents(300., 100.);
    assert!(controller.jump_to(40.));
    let attachment = controller
        .try_attach(owner(1))
        .expect("free controller attaches");
    assert!(controller.begin_activity());
    attachment.teardown();
    assert_eq!(controller.offset(), 40.);
    assert_eq!(controller.max_offset(), 200.);
    assert!(controller.jump_to(60.));
    assert!(controller.deferred_jump_to(10.));
    controller.update_extents(500., 100.);
    assert_eq!(controller.max_offset(), 400.);
    let renewed = controller.try_attach(owner(2)).expect("reattaches cleanly");
    assert!(renewed.release());
}

#[test]
fn attached_publication_writes_and_rejects_when_stale() {
    // The lease authorizes publication: the live handle writes through
    // the shared algorithm, while a stale handle fails preserving
    // content extent, viewport extent, offset, revision, ownership, and
    // notifications — every observable stays put.
    let controller = ScrollController::new();
    let (log, _guard) = notification_log(&controller);
    let attachment = controller
        .try_attach(owner(1))
        .expect("free controller attaches");
    attachment
        .update_extents(300., 100., ScrollPhysics::default())
        .expect("live attachment publishes");
    assert_eq!(controller.content_extent(), 300.);
    assert_eq!(controller.viewport_extent(), 100.);
    assert_eq!(controller.max_offset(), 200.);
    assert_eq!(log.borrow().as_slice(), &[ScrollNotificationType::Metrics]);
    assert!(attachment.release());
    let revision = controller.revision();
    let error = attachment
        .update_extents(900., 50., ScrollPhysics::default())
        .expect_err("stale handle cannot publish");
    assert_eq!(error, MetricWriteError::StaleAttachment);
    assert_eq!(controller.content_extent(), 300.);
    assert_eq!(controller.viewport_extent(), 100.);
    assert_eq!(controller.max_offset(), 200.);
    assert_eq!(controller.offset(), 0.);
    assert_eq!(controller.revision(), revision);
    assert_eq!(controller.metric_owner(), None);
    assert_eq!(log.borrow().len(), 1, "rejection emits nothing");
}

#[test]
fn unattached_publication_succeeds_only_while_free() {
    // Headless publishers use the checked path: free controllers accept,
    // owned controllers reject with full preservation — including the
    // live attachment, which the rejection never disturbs.
    let controller = ScrollController::new();
    let (log, _guard) = notification_log(&controller);
    controller
        .try_update_unattached_extents(300., 100., ScrollPhysics::default())
        .expect("free controller accepts headless publication");
    assert_eq!(controller.max_offset(), 200.);
    log.borrow_mut().clear();
    let attachment = controller
        .try_attach(owner(3))
        .expect("free controller attaches");
    assert!(controller.jump_to(40.));
    let revision = controller.revision();
    log.borrow_mut().clear();
    let error = controller
        .try_update_unattached_extents(900., 50., ScrollPhysics::default())
        .expect_err("owned controller refuses headless publication");
    assert_eq!(error.owner_tree(), Some(3));
    assert_eq!(controller.content_extent(), 300.);
    assert_eq!(controller.viewport_extent(), 100.);
    assert_eq!(controller.max_offset(), 200.);
    assert_eq!(controller.offset(), 40.);
    assert_eq!(controller.revision(), revision);
    assert_eq!(controller.metric_owner(), Some(3));
    assert_eq!(controller.attachment_id(), Some(attachment.id()));
    assert!(log.borrow().is_empty(), "rejection emits nothing");
    assert!(attachment.release());
    controller
        .try_update_unattached_extents(900., 50., ScrollPhysics::default())
        .expect("released controller accepts again");
    assert_eq!(controller.max_offset(), 850.);
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
