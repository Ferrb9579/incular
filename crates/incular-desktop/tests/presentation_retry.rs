//! Host-paced presentation retries: bounded deadlines instead of an
//! unrestricted redraw cycle.
//!
//! These tests drive the production scheduling component
//! ([`PresentationRetry`] plus [`earliest_retry_after`]) with injected
//! outcomes and a fake clock, asserting when redraws are actually
//! dispatched (deadlines and due/attempt gates) rather than bare retry
//! booleans. Pending-waiter survival across skips is covered by the
//! runtime simulation contract; here a skip arming a future attempt is
//! the observable eligibility.

use std::time::{Duration, Instant};

use incular_desktop::{PresentationRetry, earliest_retry_after};
use incular_wgpu::{FrameOutcome, FrameSkipReason, RenderStats};

fn skipped(reason: FrameSkipReason) -> FrameOutcome {
    FrameOutcome::Skipped(reason)
}

fn presented() -> FrameOutcome {
    FrameOutcome::Presented(RenderStats::default())
}

/// Repeated timeouts back off with bounded frequency: immediate first,
/// then 16/32/64/128ms, capped at 250ms. Exact dispatch times asserted.
#[test]
fn repeated_timeouts_back_off_with_bounded_frequency() {
    let t0 = Instant::now();
    let mut retry = PresentationRetry::new();

    retry.note_outcome(&skipped(FrameSkipReason::AcquisitionTimeout), t0);
    assert_eq!(retry.retry_at(), Some(t0));
    assert!(retry.attempt_due(t0));
    assert!(retry.retry_due(t0));

    retry.note_outcome(&skipped(FrameSkipReason::AcquisitionTimeout), t0);
    assert_eq!(retry.retry_at(), Some(t0 + Duration::from_millis(16)));
    assert!(!retry.attempt_due(t0));
    assert!(!retry.retry_due(t0));
    assert!(retry.attempt_due(t0 + Duration::from_millis(16)));

    let t1 = t0 + Duration::from_millis(16);
    retry.note_outcome(&skipped(FrameSkipReason::AcquisitionTimeout), t1);
    assert_eq!(retry.retry_at(), Some(t1 + Duration::from_millis(32)));

    let t2 = t1 + Duration::from_millis(32);
    retry.note_outcome(&skipped(FrameSkipReason::AcquisitionTimeout), t2);
    assert_eq!(retry.retry_at(), Some(t2 + Duration::from_millis(64)));

    let t3 = t2 + Duration::from_millis(64);
    retry.note_outcome(&skipped(FrameSkipReason::AcquisitionTimeout), t3);
    assert_eq!(retry.retry_at(), Some(t3 + Duration::from_millis(128)));

    // The cap holds indefinitely: no overflow, no growth past 250ms.
    let mut now = t3 + Duration::from_millis(128);
    for _ in 0..300 {
        retry.note_outcome(&skipped(FrameSkipReason::AcquisitionTimeout), now);
        assert_eq!(retry.retry_at(), Some(now + Duration::from_millis(250)));
        assert!(!retry.attempt_due(now));
        now += Duration::from_millis(250);
    }
    assert!(retry.attempt_due(now));
    assert!(retry.retry_due(now));
}

/// Recovered surfaces retry immediately on the first skip, then join the
/// same backoff.
#[test]
fn recovery_results_retry_immediately_first() {
    let t0 = Instant::now();
    for reason in [
        FrameSkipReason::SurfaceReconfigured,
        FrameSkipReason::SurfaceRecreated,
    ] {
        let mut retry = PresentationRetry::new();
        retry.note_outcome(&skipped(reason), t0);
        assert_eq!(retry.retry_at(), Some(t0), "{reason:?}");
        retry.note_outcome(&skipped(reason), t0);
        assert_eq!(
            retry.retry_at(),
            Some(t0 + Duration::from_millis(16)),
            "{reason:?}"
        );
    }
}

/// Success resets backoff: the next skip after a presentation starts at
/// immediate again, and no deadline stays armed.
#[test]
fn success_resets_backoff() {
    let t0 = Instant::now();
    let mut retry = PresentationRetry::new();
    retry.note_outcome(&skipped(FrameSkipReason::AcquisitionTimeout), t0);
    retry.note_outcome(&skipped(FrameSkipReason::AcquisitionTimeout), t0);
    assert_eq!(retry.retry_at(), Some(t0 + Duration::from_millis(16)));

    retry.note_outcome(&presented(), t0 + Duration::from_millis(16));
    assert_eq!(retry.retry_at(), None);
    assert!(retry.attempt_due(t0 + Duration::from_millis(16)));
    assert!(!retry.retry_due(t0 + Duration::from_millis(16)));

    retry.note_outcome(
        &skipped(FrameSkipReason::AcquisitionTimeout),
        t0 + Duration::from_millis(16),
    );
    assert_eq!(
        retry.retry_at(),
        Some(t0 + Duration::from_millis(16)),
        "backoff restarts at immediate after success"
    );
}

/// Unconfigured and occluded windows park dormant without deadlines at
/// any clock reading; recovery events make them immediately eligible.
#[test]
fn dormant_windows_wait_for_recovery_events() {
    let t0 = Instant::now();
    for reason in [
        FrameSkipReason::UnconfiguredSurface,
        FrameSkipReason::SurfaceOccluded,
    ] {
        let mut retry = PresentationRetry::new();
        retry.note_outcome(&skipped(reason), t0);
        assert_eq!(retry.retry_at(), None, "{reason:?}");
        assert!(!retry.attempt_due(t0), "{reason:?}");
        assert!(
            !retry.attempt_due(t0 + Duration::from_secs(3600)),
            "{reason:?}"
        );
        assert!(!retry.retry_due(t0 + Duration::from_secs(3600)));

        // Resize/unocclude recovers promptly.
        retry.note_recovered();
        assert_eq!(retry.retry_at(), None);
        assert!(retry.attempt_due(t0));
    }
}

/// Recovery events also clear an armed backoff to immediate eligibility.
#[test]
fn recovery_clears_armed_backoff() {
    let t0 = Instant::now();
    let mut retry = PresentationRetry::new();
    retry.note_outcome(&skipped(FrameSkipReason::AcquisitionTimeout), t0);
    retry.note_outcome(&skipped(FrameSkipReason::AcquisitionTimeout), t0);
    assert!(!retry.attempt_due(t0));

    retry.note_recovered();
    assert_eq!(retry.retry_at(), None);
    assert!(retry.attempt_due(t0));
}

/// New demand respects the gate: attempts are due only when idle or past
/// the deadline, so demand during backoff waits instead of bypassing.
#[test]
fn new_demand_respects_the_retry_gate() {
    let t0 = Instant::now();
    let mut retry = PresentationRetry::new();
    // Idle: demand may attempt at once.
    assert!(retry.attempt_due(t0));

    retry.note_outcome(&skipped(FrameSkipReason::AcquisitionTimeout), t0);
    retry.note_outcome(&skipped(FrameSkipReason::AcquisitionTimeout), t0);
    // Backoff armed: fresh demand at t0 must wait for the deadline.
    assert!(!retry.attempt_due(t0));
    assert!(!retry.retry_due(t0));
    assert!(retry.attempt_due(t0 + Duration::from_millis(16)));
    assert!(retry.retry_due(t0 + Duration::from_millis(16)));
}

/// The loop waits on the earliest armed deadline across live windows;
/// closing a window (dropping its policy) cancels its pending retry from
/// the computation without further action.
#[test]
fn earliest_deadline_covers_live_windows_only() {
    let t0 = Instant::now();
    // Open windows hold their policies in slots; closing one takes its
    // policy with it.
    let mut first = Some(PresentationRetry::new());
    let mut second = Some(PresentationRetry::new());
    let live = |first: &Option<PresentationRetry>, second: &Option<PresentationRetry>| {
        first
            .iter()
            .chain(second.iter())
            .map(PresentationRetry::retry_at)
            .collect::<Vec<_>>()
    };
    assert_eq!(earliest_retry_after(live(&first, &second)), None);

    for policy in [&mut first, &mut second] {
        policy
            .as_mut()
            .expect("window open")
            .note_outcome(&skipped(FrameSkipReason::AcquisitionTimeout), t0);
    }
    first
        .as_mut()
        .expect("window open")
        .note_outcome(&skipped(FrameSkipReason::AcquisitionTimeout), t0);
    assert_eq!(earliest_retry_after(live(&first, &second)), Some(t0));

    // Closing the later window leaves the immediate deadline; closing
    // both leaves nothing to wait on — pending retries cancel without
    // further action.
    first = None;
    assert_eq!(earliest_retry_after(live(&first, &second)), Some(t0));
    second = None;
    assert_eq!(earliest_retry_after(live(&first, &second)), None);
}

/// Normal and transient windows share one policy type, so the same
/// outcome stream paces them identically.
#[test]
fn transient_windows_follow_the_same_rules() {
    let t0 = Instant::now();
    let stream = [
        skipped(FrameSkipReason::SurfaceRecreated),
        skipped(FrameSkipReason::AcquisitionTimeout),
        skipped(FrameSkipReason::AcquisitionTimeout),
        presented(),
        skipped(FrameSkipReason::SurfaceOccluded),
    ];
    let mut normal = PresentationRetry::new();
    let mut transient = PresentationRetry::new();
    let mut now = t0;
    for outcome in &stream {
        normal.note_outcome(outcome, now);
        transient.note_outcome(outcome, now);
        assert_eq!(normal.retry_at(), transient.retry_at());
        assert_eq!(normal.attempt_due(now), transient.attempt_due(now));
        now = normal.retry_at().unwrap_or(now);
    }
    assert_eq!(normal.retry_at(), None);
    assert!(!normal.attempt_due(now));
}
