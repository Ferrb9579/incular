//! Selection-span projection: synthetic merge coverage plus shaped
//! tiling evidence.
//!
//! The synthetic cases below are algorithm coverage for the merge, not
//! shaping evidence: hand-built spans exercise gaps the shaper has not
//! been observed to produce. The mixed-direction case asserts what
//! shaping actually emits.

use incular_text::{
    TextAlign, TextClusterSpan, TextEngine, TextLayoutOptions, TextLine, TextStyle,
};
use std::sync::Arc;

fn synthetic_line(spans: &[(usize, usize, f32, f32)]) -> TextLine {
    TextLine {
        runs: Arc::new([]),
        glyphs: Arc::new([]),
        clusters: spans
            .iter()
            .map(|(start, end, left, right)| TextClusterSpan {
                start: *start,
                end: *end,
                left: *left,
                right: *right,
            })
            .collect(),
        width: 40.,
        offset: 0.,
        baseline: 0.,
        start: 0,
        caret_end: 10,
        end: 10,
    }
}

#[test]
fn synthetic_disjoint_spans_stay_separate() {
    // Algorithm coverage only: full tilings join edge-to-edge, but
    // selected subsets of mixed-direction lines genuinely gap, as the
    // prefix regression below proves against shaped output.
    let line = synthetic_line(&[(0, 2, 0., 10.), (2, 4, 10., 20.), (8, 10, 30., 40.)]);
    assert_eq!(line.selection_spans(0, 10), vec![(0., 20.), (30., 40.)]);
    assert_eq!(line.selection_spans(0, 4), vec![(0., 20.)]);
    // A range splitting clusters snaps to whole spans on both sides.
    assert_eq!(line.selection_spans(3, 9), vec![(10., 20.), (30., 40.)]);
    // A range inside the gap selects nothing visible.
    assert!(line.selection_spans(4, 8).is_empty());
    // Collapsed ranges select nothing.
    assert!(line.selection_spans(2, 2).is_empty());
}

#[test]
fn synthetic_empty_line_yields_caret_point() {
    let line = TextLine {
        runs: Arc::new([]),
        glyphs: Arc::new([]),
        clusters: Arc::new([]),
        width: 0.,
        offset: 5.,
        baseline: 0.,
        start: 0,
        caret_end: 0,
        end: 0,
    };
    assert_eq!(line.selection_spans(0, 0), vec![(5., 5.)]);
    assert!(line.selection_spans(1, 1).is_empty());
}

#[test]
fn shaped_mixed_line_tiles_contiguously() {
    // Shaping evidence: one LTR/RTL paragraph tiles its ink without
    // gaps, so single ranges merge to one segment here. Disjoint
    // visuals are absent from shaped output, not lost in extraction.
    let mut engine = TextEngine::new();
    let layout = engine.layout_with_options(
        "hi שלום bye",
        &TextStyle::default(),
        TextLayoutOptions::new(Some(300.), TextAlign::Start).soft_wrap(false),
    );
    let line = &layout.lines[0];
    assert!(!line.clusters.is_empty());
    let mut previous = line.offset;
    for span in line.clusters.iter() {
        assert!(
            (span.left - previous).abs() < 0.01,
            "gap or overlap at span {span:?}"
        );
        assert!(span.left <= span.right);
        previous = span.right;
    }
    assert!((previous - (line.offset + line.width)).abs() < 0.01);
    // A cross-run range projects to the exact covered union, one segment.
    let projected = line.selection_spans(2, 12);
    assert_eq!(projected.len(), 1);
    assert!((projected[0].0 - 12.9296875).abs() < 1.5);
    assert!((projected[0].1 - 58.453125).abs() < 1.5);

    // A logical prefix ending inside the Hebrew run (bytes 0..5: the
    // LTR head plus ש) covers two separated visual intervals with
    // unselected ink between them.
    let separated = line.selection_spans(0, 5);
    assert_eq!(separated.len(), 2);
    assert!(separated[0].1 < separated[1].0);
    assert!((separated[0].0 - 0.0).abs() < 1.5);
    assert!((separated[1].1 - 54.070313).abs() < 1.5);
}
