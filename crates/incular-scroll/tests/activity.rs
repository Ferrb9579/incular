//! Owned activity tokens: brackets identified by generation, not by a
//! boolean saying whether `Start` was emitted.
//!
//! Only the still-current token may close the bracket. Takeovers bump
//! the generation without a duplicate `Start`; stale tokens finish
//! silently without touching a newer activity.

use std::{cell::RefCell, rc::Rc};

use incular_scroll::{
    ActivityOrigin, ScrollController, ScrollNotification, ScrollNotificationType,
};

fn event_log(
    controller: &ScrollController,
) -> (
    Rc<RefCell<Vec<ScrollNotificationType>>>,
    incular_scroll::ScrollNotificationSubscription,
) {
    let log = Rc::new(RefCell::new(Vec::new()));
    let logged = log.clone();
    let subscription = controller.add_listener(move |notification: ScrollNotification| {
        logged.borrow_mut().push(notification.kind);
        false
    });
    (log, subscription)
}

#[test]
fn takeover_emits_no_duplicate_start() {
    // An open bracket taken over by a new token continues silently: one
    // `Start` total, and the taking token closes with one `End`.
    let controller = ScrollController::new();
    controller
        .update_extents(500., 100.)
        .expect("free controller publishes");
    let (log, _guard) = event_log(&controller);
    assert!(controller.begin_activity());
    let activity = controller.start_owned_activity(ActivityOrigin::Scrollbar);
    assert_eq!(activity.origin(), ActivityOrigin::Scrollbar);
    assert!(activity.is_current());
    assert!(controller.jump_to(10.));
    assert!(activity.finish());
    assert_eq!(
        log.borrow().as_slice(),
        &[
            ScrollNotificationType::Start,
            ScrollNotificationType::Update,
            ScrollNotificationType::End,
        ]
    );
}

#[test]
fn stale_finish_after_takeover_stays_silent() {
    // The old token survives physically but owns nothing: after an
    // external close and a fresh bracket, its finish emits nothing and
    // the newer activity stays open.
    use incular_scroll::ScrollNotificationType::{End, Start};
    let controller = ScrollController::new();
    let (log, _guard) = event_log(&controller);
    let old = controller.start_owned_activity(ActivityOrigin::Scrollbar);
    assert!(controller.end_activity());
    assert!(controller.begin_activity());
    assert!(!old.is_current());
    assert!(!old.finish(), "stale finish stays silent");
    assert_eq!(log.borrow().as_slice(), &[Start, End, Start]);
    assert!(!controller.begin_activity(), "newer bracket still open");
    assert!(controller.end_activity());
    assert_eq!(log.borrow().as_slice(), &[Start, End, Start, End]);
}

#[test]
fn dropped_live_token_aborts_silently() {
    // Unfinished tokens tear down silently on drop when still current;
    // stale drops change nothing at all.
    use incular_scroll::ScrollNotificationType::End;
    let controller = ScrollController::new();
    let ends = Rc::new(std::cell::Cell::new(0usize));
    let counted = ends.clone();
    let _guard = controller.add_listener(move |notification: ScrollNotification| {
        if notification.kind == End {
            counted.set(counted.get() + 1);
        }
        false
    });
    let live = controller.start_owned_activity(ActivityOrigin::Scrollbar);
    drop(live);
    assert_eq!(ends.get(), 0);
    assert!(controller.begin_activity());
    assert!(controller.end_activity());
    assert_eq!(ends.get(), 1);
    let stale = controller.start_owned_activity(ActivityOrigin::Scrollbar);
    assert!(controller.end_activity());
    drop(stale);
    assert_eq!(ends.get(), 2, "stale drop adds no abort");
}

#[test]
fn finished_token_needs_no_further_cleanup() {
    // Consuming finish closes exactly once: the token is gone afterwards
    // (single `End`), and a fresh bracket starts cleanly on the same
    // controller.
    let controller = ScrollController::new();
    let (log, _guard) = event_log(&controller);
    let activity = controller.start_owned_activity(ActivityOrigin::Scrollbar);
    let generation = activity.generation();
    assert!(activity.is_current());
    assert_eq!(controller.current_activity_id(), Some(activity.id()));
    assert!(activity.finish());
    assert_eq!(controller.current_activity_id(), None);
    assert_eq!(
        log.borrow().as_slice(),
        &[ScrollNotificationType::Start, ScrollNotificationType::End,]
    );
    assert!(controller.begin_activity());
    assert_ne!(
        controller
            .start_owned_activity(ActivityOrigin::Scrollbar)
            .generation(),
        generation,
        "generations advance across brackets"
    );
}
