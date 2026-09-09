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
    GlyphAtlas, GlyphCacheKey, GlyphRasterRequest, RasterizedGlyph, ReclaimStaleTextures,
    RendererGlyphPages, SharedGpuResourceRegistry, SharedImageMaintenance,
    prune_vacant_glyph_page_slots, retire_glyph_page_resources,
};

type SharedAtlas = Rc<RefCell<GlyphAtlas>>;

#[test]
fn zero_budget_starts_without_residency() {
    let mut atlas = GlyphAtlas::with_max_pages(0);
    assert_eq!(atlas.live_page_count(), 0);
    assert_eq!(atlas.memory().normal_bytes, 0);
    assert_eq!(atlas.counters().glyph_atlas_pages, 0);
    let mut text = TextEngine::new();
    let layout = styled_layout(&mut text, "A", 24.);
    let run = &layout.lines[0].runs[0];
    assert!(
        atlas
            .lookup_or_rasterize(run, run.glyphs[0].id, 1., &HashSet::new())
            .is_none()
    );
    assert_eq!(atlas.live_page_count(), 0);
}

#[test]
fn submission_release_retires_shared_resources_without_another_lookup() {
    let mut atlas = GlyphAtlas::with_max_pages(8);
    let mut registry = SharedGpuResourceRegistry::default();
    let mut text = TextEngine::new();
    let placed = fill_pages(&mut atlas, &mut registry, &mut text, &[800., 820., 840.]);
    let protected = placed.iter().map(|(_, raster)| raster.entry.page).collect();
    let mut slots: Vec<Option<Arc<u32>>> = (0..atlas.page_count())
        .map(|index| Some(Arc::new(index as u32)))
        .collect();
    let page = placed[0].1.entry.page;
    let active = Arc::clone(slots[usize::from(page)].as_ref().unwrap());
    let weak = Arc::downgrade(&active);
    let mut local = RendererGlyphPages::new();
    for (_, raster) in &placed {
        let page = raster.entry.page;
        local.ensure(page, raster.entry.generation, || {
            Arc::clone(slots[usize::from(page)].as_ref().unwrap())
        });
    }
    let mut revision = 0;
    atlas.set_max_pages(0, &protected);
    retire_glyph_page_resources(&mut atlas, &mut registry, &mut slots, &mut revision);
    assert_eq!(atlas.live_page_count(), 3);
    assert_eq!(registry.glyph_count(), 3);

    // This is the explicit submission boundary, not another glyph resolve.
    atlas.release_frame_protection();
    assert_eq!(
        retire_glyph_page_resources(&mut atlas, &mut registry, &mut slots, &mut revision),
        3
    );
    assert_eq!(atlas.live_page_count(), 0);
    assert_eq!(registry.glyph_count(), 0);
    assert!(slots.iter().all(Option::is_none));
    assert_eq!(local.reclaim(&|page| atlas.page_generation(page)), 3);
    assert!(weak.upgrade().is_some());
    drop(active);
    assert!(weak.upgrade().is_none());
    assert_eq!(
        retire_glyph_page_resources(&mut atlas, &mut registry, &mut slots, &mut revision),
        0
    );

    atlas.set_max_pages(2, &HashSet::new());
    let (_, fresh) = place(
        &mut atlas,
        &mut registry,
        &mut text,
        "A",
        800.,
        &HashSet::new(),
    );
    assert_eq!(atlas.live_page_count(), 1);
    assert!(
        atlas.page_generation(page).is_none()
            || atlas.page_generation(page) != Some(placed[0].1.entry.generation)
    );
    assert_eq!(
        atlas.page_generation(fresh.entry.page),
        Some(fresh.entry.generation)
    );
}

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
/// Only the page textures (`u32`, or `Arc<u32>` where shared ownership
/// is under test) are stand-ins.
struct FakeGlyphClient<T> {
    pages: RendererGlyphPages<T>,
    atlas: SharedAtlas,
    reclaim_calls: usize,
}

impl<T> ReclaimStaleTextures for FakeGlyphClient<T> {
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

/// Resolve mirroring the shared context's registry discipline: drain
/// retired placement identities, then (re-)register the resolved key.
/// Lets tests observe registry metadata alongside atlas residency.
fn place(
    atlas: &mut GlyphAtlas,
    registry: &mut SharedGpuResourceRegistry,
    text: &mut TextEngine,
    value: &str,
    size: f32,
    protected: &HashSet<u16>,
) -> (GlyphCacheKey, RasterizedGlyph) {
    let layout = styled_layout(text, value, size);
    let run: &GlyphRun = &layout.lines[0].runs[0];
    let key = GlyphCacheKey {
        font: run.font.id(),
        glyph: run.glyphs[0].id,
        physical_size: GlyphRasterRequest::new(run.font_size, 1.0).physical_size,
    };
    let raster = atlas
        .lookup_or_rasterize(run, run.glyphs[0].id, 1.0, protected)
        .expect("supported raster size");
    for retired in atlas.take_retired_keys() {
        registry.remove_glyph(retired);
    }
    registry.glyph_identity(key);
    (key, raster)
}

/// One fresh page per oversize key while under budget: oversize glyphs
/// bypass shelf sharing, so each resolve grows live residency by exactly
/// one page. Returns keys with their placements in resolve order.
fn fill_pages(
    atlas: &mut GlyphAtlas,
    registry: &mut SharedGpuResourceRegistry,
    text: &mut TextEngine,
    sizes: &[f32],
) -> Vec<(GlyphCacheKey, RasterizedGlyph)> {
    let empty = HashSet::new();
    let mut placed = Vec::new();
    for &size in sizes {
        let (key, raster) = place(atlas, registry, text, "A", size, &empty);
        let page = raster.entry.page;
        assert!(
            placed
                .iter()
                .all(|(_, prior): &(_, RasterizedGlyph)| prior.entry.page != page),
            "oversize key {size} must take a fresh page"
        );
        placed.push((key, raster));
    }
    placed
}

/// Lowering the budget retires excess unprotected pages eagerly — live
/// count drops at the call, slots stay index-stable, placements and
/// registry identities drain, shared texture slots prune through the
/// production path, and host maintenance reclaims idle bindings.
#[test]
fn lowering_budget_to_two_retires_excess_pages_eagerly() {
    let shared: SharedAtlas = Rc::new(RefCell::new(GlyphAtlas::with_max_pages(8)));
    let mut registry = SharedGpuResourceRegistry::default();
    let mut text = TextEngine::new();
    let empty = HashSet::new();

    // Four oversize keys land on slots 1..=4; slot 0 is the atlas's
    // initial (empty) page, least-recently-used by construction.
    let placed = fill_pages(
        &mut shared.borrow_mut(),
        &mut registry,
        &mut text,
        &[800., 820., 840., 860.],
    );
    let pages: Vec<u16> = placed.iter().map(|(_, raster)| raster.entry.page).collect();
    assert_eq!(pages, vec![1, 2, 3, 4]);
    assert_eq!(shared.borrow().live_page_count(), 5);
    assert_eq!(shared.borrow().page_count(), 5);
    assert_eq!(registry.glyph_count(), 4);

    // Shared texture slots and renderer bindings over production tables;
    // `Arc` stand-ins make shared ownership observable by reference count.
    // Slot 0 never uploaded (empty page), like production.
    let mut shared_slots: Vec<Option<Arc<u32>>> = vec![None];
    shared_slots.extend(pages.iter().map(|page| Some(Arc::new(u32::from(*page)))));
    let mut client = FakeGlyphClient {
        pages: RendererGlyphPages::new(),
        atlas: Rc::clone(&shared),
        reclaim_calls: 0,
    };
    for (_, raster) in &placed {
        let owned = shared_slots[usize::from(raster.entry.page)]
            .as_ref()
            .expect("shared slot live")
            .clone();
        client
            .pages
            .ensure(raster.entry.page, raster.entry.generation, || owned);
    }
    let live_binding = client.pages.get(pages[2]).cloned().expect("bound");

    // Least-recently-used first: the empty initial slot plus the two
    // earliest resolves retire; each retired page advances the revision.
    let revision_before = shared.borrow().eviction_revision();
    shared.borrow_mut().set_max_pages(2, &empty);
    {
        let atlas = shared.borrow();
        assert_eq!(atlas.live_page_count(), 2);
        assert_eq!(atlas.page_count(), 5, "slots must not compact");
        assert_eq!(atlas.eviction_revision(), revision_before + 3);
        assert_eq!(atlas.page_generation(0), None);
        assert_eq!(atlas.page_generation(pages[0]), None);
        assert_eq!(atlas.page_generation(pages[1]), None);
        assert_eq!(
            atlas.page_generation(pages[2]),
            Some(placed[2].1.entry.generation)
        );
        assert_eq!(
            atlas.page_generation(pages[3]),
            Some(placed[3].1.entry.generation)
        );
        assert!(atlas.memory().normal_pages <= 2);
    }
    let retired = shared.borrow_mut().take_retired_keys();
    assert_eq!(retired.len(), 2);
    assert!(retired.contains(&placed[0].0));
    assert!(retired.contains(&placed[1].0));
    for key in retired {
        assert!(registry.remove_glyph(key));
    }
    assert_eq!(registry.glyph_count(), 2);
    assert!(!debug_present(&shared.borrow(), &mut text, "A", 800.));
    assert!(!debug_present(&shared.borrow(), &mut text, "A", 820.));
    assert!(debug_present(&shared.borrow(), &mut text, "A", 840.));

    // Production shared-texture retirement with stand-in resources:
    // exactly the two vacant slots prune.
    let pruned = prune_vacant_glyph_page_slots(&mut shared_slots, &|page| {
        shared.borrow().page_generation(page)
    });
    assert_eq!(pruned, 2);
    assert_eq!(shared_slots[usize::from(pages[0])], None);
    assert_eq!(shared_slots[usize::from(pages[1])], None);
    assert!(shared_slots[usize::from(pages[2])].is_some());

    // Host maintenance reclaims exactly the two stale bindings; live
    // bindings keep their identity.
    let mut host = SharedImageMaintenance::new();
    let released = host.maintain(
        shared.borrow().eviction_revision(),
        [&mut client as &mut dyn ReclaimStaleTextures],
    );
    assert_eq!(released, 2);
    assert_eq!(client.pages.get(pages[0]), None);
    assert_eq!(client.pages.get(pages[1]), None);
    assert!(Arc::ptr_eq(
        &client.pages.get(pages[2]).expect("live binding").clone(),
        &live_binding
    ));

    // Retired identities resolve to nothing: `None`, never new contents.
    assert_eq!(shared.borrow().page_generation(pages[0]), None);

    // Re-expansion reuses the lowest vacant slot — the initial slot 0 —
    // under a fresh generation exactly one epoch past its retired (empty)
    // epoch, which coincides with the placed keys' initial epoch here.
    shared.borrow_mut().set_max_pages(8, &empty);
    let (new_key, new_raster) = place(
        &mut shared.borrow_mut(),
        &mut registry,
        &mut text,
        "A",
        880.,
        &empty,
    );
    assert_eq!(new_raster.entry.page, 0);
    assert_eq!(
        new_raster.entry.generation,
        placed[0].1.entry.generation + 1
    );
    assert_eq!(
        shared.borrow().page_generation(0),
        Some(new_raster.entry.generation)
    );
    assert_eq!(shared.borrow().live_page_count(), 3);
    // And the retired key re-resolves as a miss with a fresh bitmap,
    // never by trusting its old coordinates.
    let misses = shared.borrow().counters().glyph_cache_misses;
    let relayout = styled_layout(&mut text, "A", 800.);
    let rerun: &GlyphRun = &relayout.lines[0].runs[0];
    let refreshed = shared
        .borrow_mut()
        .lookup_or_rasterize(rerun, rerun.glyphs[0].id, 1.0, &empty)
        .expect("supported raster size");
    assert!(refreshed.bitmap.is_some());
    assert_ne!(
        (refreshed.entry.page, refreshed.entry.generation),
        (placed[0].1.entry.page, placed[0].1.entry.generation)
    );
    assert_eq!(shared.borrow().counters().glyph_cache_misses, misses + 1);
    let _ = new_key;
}

/// A zero budget retires every unprotected page and admits no new
/// placements through the remaining (vacant) slots.
#[test]
fn zero_budget_retires_everything_and_admits_nothing() {
    let mut atlas = GlyphAtlas::with_max_pages(4);
    let mut registry = SharedGpuResourceRegistry::default();
    let mut text = TextEngine::new();
    let empty = HashSet::new();

    // Two oversize keys land on slots 1 and 2 beside the initial page.
    let placed = fill_pages(&mut atlas, &mut registry, &mut text, &[800., 820.]);
    assert_eq!(atlas.live_page_count(), 3);
    let first_gen = placed[0].1.entry.generation;

    atlas.set_max_pages(0, &empty);
    assert_eq!(atlas.live_page_count(), 0);
    assert_eq!(atlas.page_count(), 3, "slots must not compact");
    assert!(atlas.page_generation(placed[0].1.entry.page).is_none());
    assert!(atlas.page_generation(placed[1].1.entry.page).is_none());
    assert!(!debug_present(&atlas, &mut text, "A", 800.));
    let retired = atlas.take_retired_keys();
    assert_eq!(retired.len(), 2);
    for key in retired {
        assert!(registry.remove_glyph(key));
    }
    assert_eq!(registry.glyph_count(), 0);

    // New placements are refused without growing slots or evicting: one
    // counted skip, an immediate `None`.
    let skips = atlas.counters().glyph_pressure_skips;
    let evictions = atlas.counters().glyph_page_evictions;
    let layout = styled_layout(&mut text, "A", 840.);
    let run: &GlyphRun = &layout.lines[0].runs[0];
    assert!(
        atlas
            .lookup_or_rasterize(run, run.glyphs[0].id, 1.0, &empty)
            .is_none()
    );
    assert_eq!(atlas.counters().glyph_pressure_skips, skips + 1);
    assert_eq!(atlas.counters().glyph_page_evictions, evictions);
    assert_eq!(atlas.live_page_count(), 0);
    assert_eq!(atlas.page_count(), 3);

    // Re-expansion admits again on the lowest vacant slot (the initial
    // slot 0) with a fresh generation, never a retired epoch.
    atlas.set_max_pages(2, &empty);
    let (_, raster) = place(&mut atlas, &mut registry, &mut text, "A", 840., &empty);
    assert_eq!(raster.entry.page, 0);
    assert_eq!(raster.entry.generation, first_gen + 1);
    assert_eq!(atlas.live_page_count(), 1);
}

/// Protected pages temporarily exceed a lowered budget; once protection
/// ends, even a cache hit — no fresh allocation — releases the pending
/// excess.
#[test]
fn protected_pages_defer_tightening_until_protection_ends() {
    let shared: SharedAtlas = Rc::new(RefCell::new(GlyphAtlas::with_max_pages(8)));
    let mut registry = SharedGpuResourceRegistry::default();
    let mut text = TextEngine::new();
    let empty = HashSet::new();

    // Three oversize keys land on slots 1..=3 beside the initial page.
    let placed = fill_pages(
        &mut shared.borrow_mut(),
        &mut registry,
        &mut text,
        &[800., 820., 840.],
    );
    let pages: Vec<u16> = placed.iter().map(|(_, raster)| raster.entry.page).collect();
    assert_eq!(pages, vec![1, 2, 3]);
    assert_eq!(shared.borrow().live_page_count(), 4);

    let mut client: FakeGlyphClient<u32> = FakeGlyphClient {
        pages: RendererGlyphPages::new(),
        atlas: Rc::clone(&shared),
        reclaim_calls: 0,
    };
    for (_, raster) in &placed {
        let page = raster.entry.page;
        client
            .pages
            .ensure(page, raster.entry.generation, || page.into());
    }

    // Refresh the first page so it is the most-recently-used survivor.
    let touched = resolve(&mut shared.borrow_mut(), &mut text, "A", 800., &empty);
    assert_eq!(touched.entry, placed[0].1.entry);

    // Budget one with two pages protected: the empty initial slot and
    // the unprotected page retire; the protected excess stays pending
    // instead of touching protection.
    let protected: HashSet<u16> = [pages[0], pages[1]].into_iter().collect();
    let revision_before = shared.borrow().eviction_revision();
    shared.borrow_mut().set_max_pages(1, &protected);
    assert_eq!(shared.borrow().live_page_count(), 2);
    assert_eq!(shared.borrow().eviction_revision(), revision_before + 2);
    assert!(shared.borrow().page_generation(pages[2]).is_none());
    assert!(debug_present(&shared.borrow(), &mut text, "A", 800.));
    assert!(debug_present(&shared.borrow(), &mut text, "A", 820.));
    let retired = shared.borrow_mut().take_retired_keys();
    assert_eq!(retired, vec![placed[2].0]);
    for key in retired {
        registry.remove_glyph(key);
    }
    assert_eq!(registry.glyph_count(), 2);

    // Host maintenance releases exactly the retired binding; protected
    // bindings survive with matching generations.
    let mut host = SharedImageMaintenance::new();
    let released = host.maintain(
        shared.borrow().eviction_revision(),
        [&mut client as &mut dyn ReclaimStaleTextures],
    );
    assert_eq!(released, 1);
    assert_eq!(client.pages.get(pages[2]), None);
    assert!(client.pages.get(pages[0]).is_some());
    assert!(client.pages.get(pages[1]).is_some());

    // Protection ends: a cache hit enforces the pending excess with no
    // fresh allocation — the bitmap stays `None` while residency drops.
    let hits = shared.borrow().counters().glyph_cache_hits;
    let hit = resolve(&mut shared.borrow_mut(), &mut text, "A", 800., &empty);
    assert_eq!(hit.entry, placed[0].1.entry);
    assert!(
        hit.bitmap.is_none(),
        "pending tightening needs no allocation"
    );
    assert_eq!(shared.borrow().counters().glyph_cache_hits, hits + 1);
    assert_eq!(shared.borrow().live_page_count(), 1);
    assert_eq!(
        shared.borrow().page_generation(pages[0]),
        Some(placed[0].1.entry.generation)
    );
    assert!(shared.borrow().page_generation(pages[1]).is_none());
    assert!(!debug_present(&shared.borrow(), &mut text, "A", 820.));
    let pending = shared.borrow_mut().take_retired_keys();
    assert_eq!(pending, vec![placed[1].0]);
    for key in pending {
        registry.remove_glyph(key);
    }
    assert_eq!(registry.glyph_count(), 1);

    // The second maintenance pass releases the newly retired binding.
    let released_again = host.maintain(
        shared.borrow().eviction_revision(),
        [&mut client as &mut dyn ReclaimStaleTextures],
    );
    assert_eq!(released_again, 1);
    assert_eq!(client.pages.get(pages[1]), None);
    assert!(client.pages.get(pages[0]).is_some());

    // Re-expansion reuses the lowest vacant slot (the initial slot 0)
    // under a fresh generation, while the retired middle page stays
    // vacant so its old identity cannot name anything.
    shared.borrow_mut().set_max_pages(8, &empty);
    let (_, raster) = place(
        &mut shared.borrow_mut(),
        &mut registry,
        &mut text,
        "A",
        880.,
        &empty,
    );
    assert_eq!(raster.entry.page, 0);
    assert_eq!(raster.entry.generation, placed[0].1.entry.generation + 1);
    assert_eq!(
        shared.borrow().page_generation(0),
        Some(raster.entry.generation)
    );
    assert_eq!(shared.borrow().page_generation(pages[1]), None);
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
