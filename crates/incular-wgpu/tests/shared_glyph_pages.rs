//! Bounded shared glyph-atlas page ownership: generation-validated residency.
//!
//! These tests run production components headlessly — a shared [`GlyphAtlas`]
//! behind two client handles, the renderer-local [`RendererGlyphPages`]
//! table, and host [`SharedImageMaintenance`] dispatch with a fake client
//! that reclaims through the same production table. Only the GPU page
//! textures themselves are stand-ins (`u32`); residency, eviction,
//! generation validation, and reclamation all run real code.

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;

use incular_rendering::GlyphRun;
use incular_text::{TextAlign, TextEngine, TextLayout, TextStyle};
use incular_wgpu::{
    GlyphAtlas, RasterizedGlyph, ReclaimStaleTextures, RendererGlyphPages, SharedImageMaintenance,
};

type SharedAtlas = Rc<RefCell<GlyphAtlas>>;

fn styled_layout(text: &mut TextEngine, value: &str, size: f32) -> Arc<TextLayout> {
    let style = TextStyle {
        size,
        ..TextStyle::default()
    };
    text.layout(value, &style, None, TextAlign::Start)
}

fn resolve(
    atlas: &mut GlyphAtlas,
    text: &mut TextEngine,
    value: &str,
    size: f32,
    protected: &HashSet<u16>,
) -> RasterizedGlyph {
    let layout = styled_layout(text, value, size);
    let run: &GlyphRun = &layout.lines[0].runs[0];
    let glyph = run.glyphs[0].id;
    atlas
        .lookup_or_rasterize(run, glyph, 1.0, protected)
        .expect("supported raster size")
}

fn debug_present(atlas: &GlyphAtlas, text: &mut TextEngine, value: &str, size: f32) -> bool {
    let layout = styled_layout(text, value, size);
    let run: &GlyphRun = &layout.lines[0].runs[0];
    atlas.debug_glyph(run, run.glyphs[0].id, 1.0).is_some()
}

/// Total bytes the shelf packer has handed out across live pages.
fn allocated(atlas: &GlyphAtlas) -> u64 {
    atlas.memory().normal_allocated_area + atlas.memory().oversize_allocated_area
}

/// Ascending-size 'A' keys totalling ~5M px of coverage against a 2M px
/// two-page budget: bounded churn that must turn both pages over.
fn churn_sizes() -> Vec<f32> {
    (64..=960).step_by(32).map(|s| s as f32).collect()
}

fn churn(atlas: &mut GlyphAtlas, text: &mut TextEngine, protected: &HashSet<u16>) {
    for size in churn_sizes() {
        resolve(atlas, text, "A", size, protected);
    }
}

/// Fill with ascending-size keys until a second page appears. Bounded:
/// the size ramp totals several pagefuls, so the spill is guaranteed.
/// Returns the spilled entry living on the new page plus its font size.
fn fill_until_spill(
    atlas: &mut GlyphAtlas,
    text: &mut TextEngine,
    first_page: u16,
) -> (RasterizedGlyph, f32) {
    for size in (310..=900).step_by(13).map(|s| s as f32) {
        let raster = resolve(atlas, text, "A", size, &HashSet::new());
        if raster.entry.page != first_page {
            assert_eq!(atlas.page_count(), 2);
            return (raster, size);
        }
    }
    panic!("ascending sizes must spill onto a second page");
}

/// Two clients share one atlas: the second handle reuses placements
/// without re-rasterizing, and touching a glyph refreshes its page
/// recency so churn evicts the *other* page.
#[test]
fn two_clients_reuse_glyphs_and_refresh_page_recency() {
    let shared: SharedAtlas = Rc::new(RefCell::new(GlyphAtlas::with_max_pages(2)));
    let mut text = TextEngine::new();
    let empty = HashSet::new();

    // Client A places its glyph; client B resolves the same key through a
    // second handle and must hit the cached placement.
    let first = resolve(&mut shared.borrow_mut(), &mut text, "A", 200., &empty);
    let rasterized = shared.borrow().counters().glyphs_rasterized;
    let second = resolve(&mut shared.borrow_mut(), &mut text, "A", 200., &empty);
    assert_eq!(first.entry, second.entry);
    assert_eq!(shared.borrow().counters().glyphs_rasterized, rasterized);
    assert!(second.bitmap.is_none(), "reuse must not re-rasterize");

    // Fill page 0 until one key spills onto page 1. Page 0 is frozen
    // from here on: every later key is bigger than the spilled one, so
    // none of them can use page 0's rejected shelves either.
    for size in (201..=220).map(|s| s as f32) {
        resolve(&mut shared.borrow_mut(), &mut text, "A", size, &empty);
    }
    let (other, spill_size) =
        fill_until_spill(&mut shared.borrow_mut(), &mut text, first.entry.page);
    assert_ne!(other.entry.page, first.entry.page);
    // Freeze page 0's allocated bytes: total minus the spill's own
    // allocation (read back from its debug rect).
    let spill_layout = styled_layout(&mut text, "A", spill_size);
    let spill_run: &GlyphRun = &spill_layout.lines[0].runs[0];
    let spill_rect = shared
        .borrow()
        .debug_glyph(spill_run, spill_run.glyphs[0].id, 1.0)
        .expect("spilled key is live")
        .allocation_rect;
    let page0_frozen =
        allocated(&shared.borrow()) - u64::from(spill_rect[2]) * u64::from(spill_rect[3]);

    // Top up page 1 past 700k allocated bytes with page 0 protected, so
    // filler turnovers can only recycle page 1. Afterwards no ~490k
    // contiguous placement fits either page's remainder.
    let guard: HashSet<u16> = [first.entry.page].into_iter().collect();
    let mut top_size = spill_size;
    for _ in 0..60 {
        if allocated(&shared.borrow()) - page0_frozen >= 700_000 {
            break;
        }
        top_size += 13.;
        resolve(&mut shared.borrow_mut(), &mut text, "A", top_size, &guard);
    }
    assert!(
        allocated(&shared.borrow()) - page0_frozen >= 700_000,
        "page 1 must be topped up for a deterministic overflow"
    );

    // Client B touches A's glyph again, refreshing page 0 recency past
    // page 1. The near-page-size overflow then cannot fit either
    // remainder and must evict page 1, not A's glyph.
    let touched = resolve(&mut shared.borrow_mut(), &mut text, "A", 200., &empty);
    assert_eq!(touched.entry, first.entry);
    let evictions_before = shared.borrow().counters().glyph_page_evictions;
    let overflow = resolve(&mut shared.borrow_mut(), &mut text, "A", 1000., &empty);
    let atlas = shared.borrow();
    assert_eq!(atlas.counters().glyph_page_evictions, evictions_before + 1);
    assert_eq!(
        atlas.page_generation(first.entry.page),
        Some(first.entry.generation),
        "refreshed page must survive"
    );
    assert_ne!(
        atlas.page_generation(other.entry.page),
        Some(other.entry.generation),
        "unrefreshed page must be the victim"
    );
    assert!(!debug_present(&atlas, &mut text, "A", spill_size));
    drop(atlas);
    let _ = overflow;
}

/// Churn past the budget retires pages and their placement metadata:
/// evictions are counted, debug entries disappear, and re-resolving a
/// retired key re-rasterizes with a fresh generation.
#[test]
fn churn_retires_pages_and_their_metadata() {
    let mut atlas = GlyphAtlas::with_max_pages(2);
    let mut text = TextEngine::new();
    let empty = HashSet::new();

    let first = resolve(&mut atlas, &mut text, "A", 200., &empty);
    assert!(debug_present(&atlas, &mut text, "A", 200.));

    // Bounded churn totalling ~5M px of coverage against the 2M px
    // budget: both pages must turn over.
    churn(&mut atlas, &mut text, &empty);
    assert!(atlas.counters().glyph_page_evictions >= 2);
    assert!(atlas.eviction_revision() >= 2);
    assert!(atlas.page_count() <= 2);

    // The original placement metadata is retired: no debug entry, and the
    // stored generation no longer names live content.
    assert!(!debug_present(&atlas, &mut text, "A", 200.));
    assert_ne!(
        atlas.page_generation(first.entry.page),
        Some(first.entry.generation)
    );

    // Re-resolving re-rasterizes (fresh bitmap) under a new generation
    // instead of sampling whatever replaced the old coordinates.
    // Eviction retires entries outright, so this is a miss — the
    // generation-mismatch branch in lookup stays purely defensive.
    let misses = atlas.counters().glyph_cache_misses;
    let refreshed = resolve(&mut atlas, &mut text, "A", 200., &empty);
    assert!(refreshed.bitmap.is_some());
    assert_ne!(
        (refreshed.entry.page, refreshed.entry.generation),
        (first.entry.page, first.entry.generation)
    );
    assert_eq!(
        atlas.page_generation(refreshed.entry.page),
        Some(refreshed.entry.generation)
    );
    assert_eq!(atlas.counters().glyph_cache_misses, misses + 1);
}

/// A stale (page, generation) pair is detectable before drawing: the
/// retained generation no longer matches the shared page, so bind-time
/// validation must re-resolve instead of sampling.
#[test]
fn stale_page_references_are_detected_before_drawing() {
    let mut atlas = GlyphAtlas::with_max_pages(2);
    let mut text = TextEngine::new();
    let empty = HashSet::new();

    let stale = resolve(&mut atlas, &mut text, "A", 200., &empty);
    churn(&mut atlas, &mut text, &empty);
    assert!(atlas.counters().glyph_page_evictions >= 1);

    // The exact check renderer bind paths perform: generation mismatch
    // means "do not sample, re-resolve".
    let live = atlas.page_generation(stale.entry.page) == Some(stale.entry.generation);
    assert!(!live, "evicted page must fail generation validation");
}

/// A reused page index never displays a different glyph through an old
/// identity: the renderer-local table replaces superseded bindings on
/// next ensure instead of returning the stale resource.
#[test]
fn page_reuse_cannot_display_a_different_glyph_through_an_old_identity() {
    let mut pages = RendererGlyphPages::new();
    pages.ensure(0, 4, || 111_u32);
    assert_eq!(pages.get(0), Some(&111_u32));

    // The shared page was evicted and now carries generation 5 for
    // different content. Ensuring the fresh generation replaces the
    // binding; the old resource is unreachable afterwards.
    pages.ensure(0, 5, || 222_u32);
    assert_eq!(pages.get(0), Some(&222_u32));
    assert_eq!(pages.generation(0), Some(5));

    // Untouched slots keep their bindings and generations.
    pages.ensure(1, 5, || 333_u32);
    assert_eq!(pages.get(1), Some(&333_u32));
}

/// Fake renderer client: production page table plus a shared-atlas
/// handle, reclaimed through the same host dispatch as real renderers.
/// Only the page textures (`u32`) are stand-ins.
struct FakeGlyphClient {
    pages: RendererGlyphPages<u32>,
    atlas: SharedAtlas,
    reclaim_calls: usize,
}

impl ReclaimStaleTextures for FakeGlyphClient {
    fn reclaim_stale_textures(&mut self) -> usize {
        self.reclaim_calls += 1;
        let atlas = self.atlas.borrow();
        self.pages.reclaim(&|page| atlas.page_generation(page))
    }
}

/// Idle bindings are reclaimed through host maintenance once the shared
/// page is evicted; passes with no evictions visit no client.
#[test]
fn idle_bindings_are_reclaimed_through_host_maintenance() {
    let shared: SharedAtlas = Rc::new(RefCell::new(GlyphAtlas::with_max_pages(1)));
    let mut text = TextEngine::new();
    let empty = HashSet::new();

    let placed = resolve(&mut shared.borrow_mut(), &mut text, "A", 200., &empty);
    let mut client = FakeGlyphClient {
        pages: RendererGlyphPages::new(),
        atlas: Rc::clone(&shared),
        reclaim_calls: 0,
    };
    client
        .pages
        .ensure(placed.entry.page, placed.entry.generation, || 7_u32);
    assert_eq!(client.pages.get(placed.entry.page), Some(&7_u32));

    let mut host = SharedImageMaintenance::new();
    // No eviction yet: the pass costs one revision comparison and visits
    // no client.
    let idle = host.maintain(
        shared.borrow().eviction_revision(),
        [&mut client as &mut dyn ReclaimStaleTextures],
    );
    assert_eq!(idle, 0);
    assert_eq!(client.reclaim_calls, 0);

    // Evict the page out from under the idle binding, then run host
    // maintenance: the stale binding is released exactly once. The ramp
    // is bounded and must overflow the single page.
    for size in (240..=960).step_by(40).map(|s| s as f32) {
        resolve(&mut shared.borrow_mut(), &mut text, "A", size, &empty);
        if shared.borrow().counters().glyph_page_evictions >= 1 {
            break;
        }
    }
    assert!(shared.borrow().counters().glyph_page_evictions >= 1);
    let released = host.maintain(
        shared.borrow().eviction_revision(),
        [&mut client as &mut dyn ReclaimStaleTextures],
    );
    assert_eq!(released, 1);
    assert_eq!(client.reclaim_calls, 1);
    assert_eq!(client.pages.get(placed.entry.page), None);

    // Steady state again: no evictions, no visits.
    let steady = host.maintain(
        shared.borrow().eviction_revision(),
        [&mut client as &mut dyn ReclaimStaleTextures],
    );
    assert_eq!(steady, 0);
    assert_eq!(client.reclaim_calls, 1);
}

/// Frame-pinned pages survive churn (active references stay valid), and
/// a fully pinned budget degrades to one counted pressure skip — never
/// a render/evict/re-upload loop.
#[test]
fn active_references_remain_valid_and_pinned_pressure_skips_cleanly() {
    let mut atlas = GlyphAtlas::with_max_pages(1);
    let mut text = TextEngine::new();
    let empty = HashSet::new();
    let pinned: HashSet<u16> = [0].into_iter().collect();

    let active = resolve(&mut atlas, &mut text, "A", 300., &empty);
    assert_eq!(active.entry.page, 0);
    // A few fillers so the page is genuinely shared; all comfortably
    // fit, so no eviction can fire during setup.
    for size in [313., 326., 339.] {
        let filler = resolve(&mut atlas, &mut text, "A", size, &empty);
        assert_eq!(filler.entry.page, 0);
    }
    assert_eq!(atlas.counters().glyph_page_evictions, 0);

    // Churn against the pinned page: the active glyph keeps hitting its
    // live placement, and nothing is evicted from under it.
    for _ in 0..8 {
        let hit = resolve(&mut atlas, &mut text, "A", 300., &pinned);
        assert_eq!(hit.entry, active.entry);
        assert!(hit.bitmap.is_none());
    }
    assert_eq!(atlas.counters().glyph_page_evictions, 0);
    assert_eq!(atlas.page_generation(0), Some(active.entry.generation));

    // A near-page-size glyph cannot fit any remainder of the occupied
    // page; with every page pinned that is one counted skip and an
    // immediate `None` — no loop and no eviction.
    let skips = atlas.counters().glyph_pressure_skips;
    let layout = styled_layout(&mut text, "A", 1000.);
    let run: &GlyphRun = &layout.lines[0].runs[0];
    assert!(
        atlas
            .lookup_or_rasterize(run, run.glyphs[0].id, 1.0, &pinned)
            .is_none()
    );
    assert_eq!(atlas.counters().glyph_pressure_skips, skips + 1);
    assert_eq!(atlas.counters().glyph_page_evictions, 0);
}

/// An over-budget working set behaves deterministically: two identical
/// passes over fresh atlases produce identical residency outcomes, and
/// with no protection every request resolves (hit or re-rasterize) —
/// nothing is dropped and nothing loops.
#[test]
fn over_budget_working_sets_behave_deterministically() {
    fn pass() -> (Vec<(u16, u64)>, incular_wgpu::GpuCounters) {
        let mut atlas = GlyphAtlas::with_max_pages(2);
        let mut text = TextEngine::new();
        let empty = HashSet::new();
        let mut placements = Vec::new();
        for size in churn_sizes() {
            let raster = resolve(&mut atlas, &mut text, "A", size, &empty);
            placements.push((raster.entry.page, raster.entry.generation));
        }
        (placements, atlas.counters())
    }

    let (first_places, first_counters) = pass();
    let (second_places, second_counters) = pass();
    assert_eq!(first_places, second_places);
    assert_eq!(
        first_counters.glyph_page_evictions,
        second_counters.glyph_page_evictions
    );
    assert!(first_counters.glyph_page_evictions >= 2);
    assert_eq!(first_counters.glyph_pressure_skips, 0);
    assert_eq!(
        first_counters.glyph_cache_hits,
        second_counters.glyph_cache_hits
    );
}
