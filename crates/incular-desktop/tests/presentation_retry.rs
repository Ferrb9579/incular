//! Host-paced presentation retries: bounded deadlines instead of an
//! unrestricted redraw cycle.
//!
//! These tests drive the production scheduling operation
//! ([`PresentationRetry::poll`]) with injected outcomes, visibility, and
//! a fake clock, asserting the resulting redraw dispatches and wake
//! deadlines together — never deadline selection apart from eligibility.
//! A miniature host harness below polls policies exactly like the event
//! loop does (dispatch plus earliest wake). Pending-waiter survival
//! across skips is covered by the runtime simulation contract; here a
//! skip arming a future attempt is the observable eligibility.

use std::time::{Duration, Instant};

use incular_desktop::PresentationRetry;
use incular_wgpu::{FrameOutcome, FrameSkipReason, RenderStats};

fn skipped(reason: FrameSkipReason) -> FrameOutcome {
    FrameOutcome::Skipped(reason)
}

fn presented() -> FrameOutcome {
    FrameOutcome::Presented(RenderStats::default())
}

/// Mimics the event loop's maintenance pass: polls every live window and
/// returns how many redraws were dispatched plus the wait deadline the
/// loop would sleep on. Closed windows are simply absent.
fn maintenance(
    windows: &mut [(PresentationRetry, bool)],
    now: Instant,
) -> (usize, Option<Instant>) {
    let mut redraws = 0;
    let mut wake_at: Option<Instant> = None;
    for (policy, runnable) in windows.iter_mut() {
        let (dispatch, wake) = policy.poll(now, *runnable);
        if dispatch {
            redraws += 1;
        }
        wake_at = match (wake_at, wake) {
            (Some(current), Some(next)) => Some(current.min(next)),
            (current, next) => current.or(next),
        };
    }
    (redraws, wake_at)
}

/// The reported reproduction: arm a retry, hide before its deadline, pass
/// it — no redraw is dispatched and no expired deadline wakes the loop,
/// however often maintenance runs. Backoff progress is preserved
/// silently.
#[test]
fn hidden_window_contributes_no_redraw_and_no_expired_wake() {
    let t0 = Instant::now();
    let mut retry = PresentationRetry::new();
    retry.note_outcome(&skipped(FrameSkipReason::AcquisitionTimeout), t0);
    retry.note_outcome(&skipped(FrameSkipReason::AcquisitionTimeout), t0);
    assert_eq!(retry.retry_at(), Some(t0 + Duration::from_millis(16)));

    // Hidden before the deadline: suppressed, deadline included while live.
    assert_eq!(
        retry.poll(t0 + Duration::from_millis(5), false),
        (false, None)
    );
    // Past the deadline while still hidden: still nothing — neither a
    // redraw nor the expired wake — on repeated polls.
    assert_eq!(
        retry.poll(t0 + Duration::from_millis(1000), false),
        (false, None)
    );
    assert_eq!(
        retry.poll(t0 + Duration::from_millis(2000), false),
        (false, None)
    );
    // The owed retry survives hiding: the deadline is unchanged and the
    // attempt is due the moment the window is runnable again.
    assert_eq!(retry.retry_at(), Some(t0 + Duration::from_millis(16)));
    assert_eq!(
        retry.poll(t0 + Duration::from_millis(2000), true),
        (true, None)
    );
}

/// A dispatched retry whose redraw is delayed dispatches exactly once:
/// repeated maintenance neither re-dispatches nor returns a deadline.
#[test]
fn dispatched_retry_dispatches_once() {
    let t0 = Instant::now();
    let mut retry = PresentationRetry::new();
    retry.note_outcome(&skipped(FrameSkipReason::SurfaceRecreated), t0);

    // Immediate first retry dispatches on the first maintenance pass.
    assert_eq!(retry.poll(t0, true), (true, None));
    // Redraw still delayed: no second dispatch, no wake deadline.
    assert_eq!(retry.poll(t0, true), (false, None));
    assert_eq!(
        retry.poll(t0 + Duration::from_secs(60), true),
        (false, None)
    );

    // The attempt runs and presents: back to idle, demand decides.
    retry.note_outcome(&presented(), t0 + Duration::from_secs(60));
    assert_eq!(
        retry.poll(t0 + Duration::from_secs(60), true),
        (false, None)
    );
    assert!(retry.attempt_due(t0 + Duration::from_secs(60)));
}

/// A dispatched retry suppressed by hiding falls back to owed-immediately,
/// so restoring runnability dispatches one prompt attempt with backoff
/// progress (and pending work) preserved.
#[test]
fn suppressed_dispatch_resumes_promptly_on_restore() {
    let t0 = Instant::now();
    let mut retry = PresentationRetry::new();
    retry.note_outcome(&skipped(FrameSkipReason::AcquisitionTimeout), t0);
    assert_eq!(retry.poll(t0, true), (true, None));

    // Hidden before the attempt runs: no dispatch, no wake.
    assert_eq!(
        retry.poll(t0 + Duration::from_millis(500), false),
        (false, None)
    );
    // Restored: one prompt dispatch, still no wake deadline.
    assert_eq!(
        retry.poll(t0 + Duration::from_millis(500), true),
        (true, None)
    );
    assert_eq!(
        retry.poll(t0 + Duration::from_millis(500), true),
        (false, None)
    );
}

/// Closing a window while it waits or has a queued dispatch removes it
/// from dispatch and wake computation without further action.
#[test]
fn closed_windows_cancel_waiting_and_queued_retries() {
    let t0 = Instant::now();
    // Slot per open window: Some live policy, None closed.
    let mut waiting = Some(PresentationRetry::new());
    let mut queued = Some(PresentationRetry::new());

    waiting
        .as_mut()
        .expect("window open")
        .note_outcome(&skipped(FrameSkipReason::AcquisitionTimeout), t0);
    waiting
        .as_mut()
        .expect("window open")
        .note_outcome(&skipped(FrameSkipReason::AcquisitionTimeout), t0);
    queued
        .as_mut()
        .expect("window open")
        .note_outcome(&skipped(FrameSkipReason::SurfaceRecreated), t0);
    // One dispatch moves the immediate retry to queued-behind-delivery.
    assert_eq!(
        queued.as_mut().expect("window open").poll(t0, true),
        (true, None)
    );

    // Maintenance over the live set sees the waiting deadline only.
    let live = |waiting: &Option<PresentationRetry>, queued: &Option<PresentationRetry>| {
        let mut windows = Vec::new();
        windows.extend(waiting.iter().map(|policy| (*policy, true)));
        windows.extend(queued.iter().map(|policy| (*policy, true)));
        windows
    };
    assert_eq!(
        maintenance(&mut live(&waiting, &queued), t0),
        (0, Some(t0 + Duration::from_millis(16)))
    );

    // Close the waiting window: its deadline leaves with it.
    waiting = None;
    assert_eq!(maintenance(&mut live(&waiting, &queued), t0), (0, None));
    // Close the queued one too: nothing dispatched, nothing to wait on.
    queued = None;
    assert_eq!(maintenance(&mut live(&waiting, &queued), t0), (0, None));
}

/// Visible-window backoff still progresses through the dispatcher and
/// resets on success: exact dispatch times asserted end to end.
#[test]
fn visible_backoff_progresses_and_resets_on_success() {
    let t0 = Instant::now();
    let mut windows = vec![(PresentationRetry::new(), true)];

    // First failure retries immediately.
    windows[0]
        .0
        .note_outcome(&skipped(FrameSkipReason::AcquisitionTimeout), t0);
    assert_eq!(maintenance(&mut windows, t0), (1, None));

    // Second failure backs off to t0+16ms: no dispatch before, dispatch at.
    windows[0]
        .0
        .note_outcome(&skipped(FrameSkipReason::AcquisitionTimeout), t0);
    assert_eq!(
        maintenance(&mut windows, t0),
        (0, Some(t0 + Duration::from_millis(16)))
    );
    assert_eq!(
        maintenance(&mut windows, t0 + Duration::from_millis(16)),
        (1, None)
    );

    // The dispatched attempt presents: idle, no dispatch, no wake.
    windows[0]
        .0
        .note_outcome(&presented(), t0 + Duration::from_millis(16));
    assert_eq!(
        maintenance(&mut windows, t0 + Duration::from_millis(16)),
        (0, None)
    );

    // The next failure starts over at immediate.
    windows[0].0.note_outcome(
        &skipped(FrameSkipReason::AcquisitionTimeout),
        t0 + Duration::from_millis(16),
    );
    assert_eq!(
        maintenance(&mut windows, t0 + Duration::from_millis(16)),
        (1, None)
    );
}

/// Dormant windows never dispatch and never contribute wakes at any clock
/// reading; recovery makes them eligible and the next maintenance pass
/// picks demand up normally.
#[test]
fn dormant_windows_wait_for_recovery_then_resume() {
    let t0 = Instant::now();
    for reason in [
        FrameSkipReason::UnconfiguredSurface,
        FrameSkipReason::SurfaceOccluded,
    ] {
        let mut windows = vec![(PresentationRetry::new(), true)];
        windows[0].0.note_outcome(&skipped(reason), t0);
        assert_eq!(
            maintenance(&mut windows, t0 + Duration::from_secs(3600)),
            (0, None),
            "{reason:?}"
        );

        // Resize/unocclude recovers: eligible again, still nothing owed.
        windows[0].0.note_recovered();
        assert!(windows[0].0.attempt_due(t0));
        assert_eq!(maintenance(&mut windows, t0), (0, None));
    }
}

/// Normal and transient windows share one scheduling contract: the same
/// outcome stream yields the same dispatch/wake decisions, diverging only
/// where the host reports runnability differently.
#[test]
fn normal_and_transient_windows_share_the_scheduling_contract() {
    let t0 = Instant::now();
    let mut normal = PresentationRetry::new();
    let mut transient = PresentationRetry::new();

    // Identical streams, both runnable: identical tuples throughout.
    let stream = [
        skipped(FrameSkipReason::SurfaceRecreated),
        skipped(FrameSkipReason::AcquisitionTimeout),
        presented(),
        skipped(FrameSkipReason::SurfaceOccluded),
    ];
    for outcome in &stream {
        normal.note_outcome(outcome, t0);
        transient.note_outcome(outcome, t0);
        assert_eq!(normal.poll(t0, true), transient.poll(t0, true));
    }

    // Same owed retry, different runnability: the hidden normal window is
    // suppressed while the transient dispatches — one rule, two inputs.
    let mut hidden_normal = PresentationRetry::new();
    let mut shown_transient = PresentationRetry::new();
    for policy in [&mut hidden_normal, &mut shown_transient] {
        policy.note_outcome(&skipped(FrameSkipReason::AcquisitionTimeout), t0);
        policy.note_outcome(&skipped(FrameSkipReason::AcquisitionTimeout), t0);
    }
    assert_eq!(
        hidden_normal.poll(t0 + Duration::from_millis(16), false),
        (false, None)
    );
    assert_eq!(
        shown_transient.poll(t0 + Duration::from_millis(16), true),
        (true, None)
    );
}
