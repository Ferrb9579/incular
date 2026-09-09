//! Host dispatch for settled frame outcomes: every outcome maps to its
//! accounting and retry scheduling through the real `present_window`
//! mapping (`dispatch_frame_outcome`). Presented frames record a
//! presentation, settle waiters, and never re-request; skips record no
//! presentation, keep waiters pending, and retry only for reasons where
//! another attempt could present.

use incular_desktop::{FrameHostDispatch, dispatch_frame_outcome};
use incular_wgpu::{FrameOutcome, FrameSkipReason, RenderStats};

#[test]
fn presented_frames_record_settle_and_never_retry() {
    let dispatch = dispatch_frame_outcome(&FrameOutcome::Presented(RenderStats::default()));
    assert_eq!(
        dispatch,
        FrameHostDispatch {
            record_presented: true,
            request_retry: false,
            settle_waiters: true,
        }
    );
}

#[test]
fn skipped_frames_keep_waiters_and_retry_only_when_useful() {
    let cases = [
        (FrameSkipReason::UnconfiguredSurface, false),
        (FrameSkipReason::AcquisitionTimeout, true),
        (FrameSkipReason::SurfaceOccluded, false),
        (FrameSkipReason::SurfaceReconfigured, true),
        (FrameSkipReason::SurfaceRecreated, true),
    ];
    for (reason, retry) in cases {
        let dispatch = dispatch_frame_outcome(&FrameOutcome::Skipped(reason));
        assert_eq!(
            dispatch,
            FrameHostDispatch {
                record_presented: false,
                request_retry: retry,
                settle_waiters: false,
            },
            "{reason:?}"
        );
    }
}
