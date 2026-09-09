//! Explicit frame presentation outcomes: typed results instead of
//! inferred success from counters or default statistics.
//!
//! These tests inject backend acquisition results through the real
//! decision code (`SurfaceAcquisitionStatus::of` / `disposition`) and
//! every skip reason through the real retry mapping. The two
//! frame-carrying backend results (`Success`, `Suboptimal`) need a GPU
//! texture and are covered by actual GPU presentation, recorded as
//! unverified here; their status-level dispositions are still asserted.

use incular_wgpu::{
    AcquisitionDisposition, FrameOutcome, FrameSkipReason, RenderStats, SurfaceAcquisitionStatus,
};

/// Every backend status classifies to its documented disposition through
/// the same function production acquisition calls.
#[test]
fn acquisition_dispositions_cover_every_backend_status() {
    use SurfaceAcquisitionStatus as Status;
    let cases = [
        (
            Status::Ready,
            AcquisitionDisposition::Present {
                reconfigure_after_present: false,
            },
        ),
        (
            Status::ReadySuboptimal,
            AcquisitionDisposition::Present {
                reconfigure_after_present: true,
            },
        ),
        (
            Status::Timeout,
            AcquisitionDisposition::Deferred(FrameSkipReason::AcquisitionTimeout),
        ),
        (
            Status::Occluded,
            AcquisitionDisposition::Deferred(FrameSkipReason::SurfaceOccluded),
        ),
        (
            Status::Outdated,
            AcquisitionDisposition::Deferred(FrameSkipReason::SurfaceReconfigured),
        ),
        (
            Status::Lost,
            AcquisitionDisposition::Deferred(FrameSkipReason::SurfaceRecreated),
        ),
        (Status::Validation, AcquisitionDisposition::Failed),
    ];
    for (status, expected) in cases {
        assert_eq!(status.disposition(), expected, "{status:?}");
    }
}

/// The texture-less backend results inject directly: five of seven
/// `CurrentSurfaceTexture` variants are constructible without a GPU.
/// `Success`/`Suboptimal` carry textures and are covered by real
/// presentation (unverified here).
#[test]
fn texture_less_backend_results_classify_through_of() {
    use wgpu::CurrentSurfaceTexture as Backend;
    let cases = [
        (Backend::Timeout, SurfaceAcquisitionStatus::Timeout),
        (Backend::Occluded, SurfaceAcquisitionStatus::Occluded),
        (Backend::Outdated, SurfaceAcquisitionStatus::Outdated),
        (Backend::Lost, SurfaceAcquisitionStatus::Lost),
        (Backend::Validation, SurfaceAcquisitionStatus::Validation),
    ];
    for (backend, expected) in cases {
        let status = SurfaceAcquisitionStatus::of(&backend);
        assert_eq!(status, expected);
        // End to end through the real decision function.
        assert_eq!(status.disposition(), expected.disposition());
    }
}

/// Retry scheduling per skip reason: recovered surfaces and timeouts
/// retry host-paced; unconfigured and occluded surfaces wait for
/// resize/unocclude instead of spinning full-frame work.
#[test]
fn skip_reasons_schedule_retries_without_spinning() {
    use FrameSkipReason as Reason;
    let cases = [
        (Reason::UnconfiguredSurface, false),
        (Reason::AcquisitionTimeout, true),
        (Reason::SurfaceOccluded, false),
        (Reason::SurfaceReconfigured, true),
        (Reason::SurfaceRecreated, true),
    ];
    for (reason, retry) in cases {
        assert_eq!(reason.should_request_retry(), retry, "{reason:?}");
        // Skips never fail waiters: pending work stays eligible.
        assert!(reason.retains_pending_work());
    }
}

/// The outcome type carries control flow: presented frames expose
/// statistics and never re-request; skips expose their reason, no
/// statistics, and reason-gated retries.
#[test]
fn frame_outcome_distinguishes_presentation_from_skips() {
    // Default statistics never read as presented.
    assert!(!RenderStats::default().presented);

    let presented = FrameOutcome::Presented(RenderStats {
        presented: true,
        draw_calls: 7,
        ..RenderStats::default()
    });
    assert!(presented.presented());
    assert_eq!(presented.stats().map(|stats| stats.draw_calls), Some(7));
    assert_eq!(presented.skip_reason(), None);
    assert!(!presented.requests_retry());

    for (reason, retry) in [
        (FrameSkipReason::UnconfiguredSurface, false),
        (FrameSkipReason::AcquisitionTimeout, true),
        (FrameSkipReason::SurfaceOccluded, false),
        (FrameSkipReason::SurfaceReconfigured, true),
        (FrameSkipReason::SurfaceRecreated, true),
    ] {
        let skipped = FrameOutcome::Skipped(reason);
        assert!(!skipped.presented());
        assert_eq!(skipped.stats(), None);
        assert_eq!(skipped.skip_reason(), Some(reason));
        assert_eq!(skipped.requests_retry(), retry, "{reason:?}");
    }
}
