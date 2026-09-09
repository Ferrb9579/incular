//! Bounded parsed-font retention for glyph rasterization.
//!
//! These tests run the production rasterization path
//! ([`GlyphAtlas::lookup_or_rasterize`]) with minted font identities built
//! from real shaped font bytes. Counters (`font_parser_cache_hits`,
//! `font_parser_cache_misses`, `font_parser_evictions`) and the retained
//! entry count (`font_count`) measure actual parsing and retention —
//! nothing here simulates an LRU. Only GPU upload/bind/draw stays out of
//! scope, consistent with the repository's native-test policy.

use std::collections::HashSet;
use std::sync::Arc;

use incular_assets::{FontHandle, FontId};
use incular_core::Offset;
use incular_rendering::{GlyphPosition, GlyphRun};
use incular_text::{TextAlign, TextEngine, TextStyle};
use incular_wgpu::{GlyphAtlas, SharedGpuResourceRegistry};

/// Real shaped bytes plus two genuine glyph ids from the test engine's
/// default font. All minted handles below share these bytes, so every
/// parse succeeds exactly like production's — only the identities vary.
fn shaped_source(text: &mut TextEngine) -> (Arc<[u8]>, u16, u16) {
    let layout = text.layout("AB", &TextStyle::default(), None, TextAlign::Start);
    let run = &layout.lines[0].runs[0];
    (run.font.bytes().clone(), run.glyphs[0].id, run.glyphs[1].id)
}

fn run_with(font: FontHandle, glyph: u16) -> GlyphRun {
    GlyphRun {
        font,
        font_size: 32.,
        origin: Offset::new(0., 0.),
        glyphs: Arc::from([GlyphPosition {
            id: glyph,
            offset: Offset::new(0., 0.),
            advance: 0.,
            cluster: 0,
        }]),
    }
}

fn resolve(
    atlas: &mut GlyphAtlas,
    run: &GlyphRun,
    glyph: u16,
) -> Option<incular_wgpu::RasterizedGlyph> {
    atlas.lookup_or_rasterize(run, glyph, 1.0, &HashSet::new())
}

/// Repeated requests reuse one parsed font: one miss, then hits, with a
/// single retained entry.
#[test]
fn repeated_glyph_requests_reuse_a_parsed_font() {
    let mut text = TextEngine::new();
    let (bytes, id_a, id_b) = shaped_source(&mut text);
    assert_ne!(id_a, id_b);
    let run = run_with(FontHandle::new(FontId(101), bytes), id_a);

    let mut atlas = GlyphAtlas::new();
    let first = resolve(&mut atlas, &run, id_a).expect("supported glyph");
    assert!(first.bitmap.is_some());
    assert_eq!(atlas.counters().font_parser_cache_misses, 1);
    assert_eq!(atlas.font_count(), 1);

    // A second glyph from the same font parses nothing new.
    let run_b = GlyphRun {
        glyphs: Arc::from([GlyphPosition {
            id: id_b,
            ..run.glyphs[0]
        }]),
        ..run.clone()
    };
    let second = resolve(&mut atlas, &run_b, id_b).expect("supported glyph");
    assert!(second.bitmap.is_some());
    assert_eq!(atlas.counters().font_parser_cache_misses, 1);
    assert_eq!(atlas.font_count(), 1);

    // The cached glyph serves as a hit with no further parsing.
    let misses = atlas.counters().font_parser_cache_misses;
    let hits = atlas.counters().glyph_cache_hits;
    let cached = resolve(&mut atlas, &run, id_a).expect("cached glyph");
    assert!(cached.bitmap.is_none());
    assert_eq!(atlas.counters().font_parser_cache_misses, misses);
    assert_eq!(atlas.counters().glyph_cache_hits, hits + 1);
}

/// Distinct font identities — and collection faces — never alias: each
/// parses (or fails) on its own, and a foreign face cannot hit another
/// font's cached entry.
#[test]
fn distinct_fonts_and_collection_faces_never_alias() {
    let mut text = TextEngine::new();
    let (bytes, id_a, _) = shaped_source(&mut text);
    let mut atlas = GlyphAtlas::new();

    let first = run_with(FontHandle::new(FontId(102), bytes.clone()), id_a);
    assert!(resolve(&mut atlas, &first, id_a).is_some());
    assert_eq!(atlas.counters().font_parser_cache_misses, 1);

    // Same bytes, different identity: a second parse, not a hit.
    let second = run_with(FontHandle::new(FontId(103), bytes.clone()), id_a);
    assert!(resolve(&mut atlas, &second, id_a).is_some());
    assert_eq!(atlas.counters().font_parser_cache_misses, 2);
    assert_eq!(atlas.font_count(), 2);

    // Same bytes, out-of-range collection face: the parse fails with
    // existing error behavior — and, crucially, does not serve the other
    // fonts' cached entry for the same glyph id.
    let misses = atlas.counters().font_parser_cache_misses;
    let foreign = run_with(FontHandle::with_face_index(FontId(104), bytes, 1), id_a);
    assert!(resolve(&mut atlas, &foreign, id_a).is_none());
    assert_eq!(atlas.counters().font_parser_cache_misses, misses);
    assert_eq!(atlas.font_count(), 2);

    // Both real fonts still serve their own cached glyphs.
    assert!(resolve(&mut atlas, &first, id_a).is_some());
    assert!(resolve(&mut atlas, &second, id_a).is_some());
}

/// Cross-client use affects eviction order: a hit through a second handle
/// refreshes recency, so churn evicts the untouched font instead.
#[test]
fn cross_client_use_affects_eviction_order() {
    let mut text = TextEngine::new();
    let (bytes, id_a, id_b) = shaped_source(&mut text);
    let run_x = run_with(FontHandle::new(FontId(201), bytes.clone()), id_a);
    let run_y = run_with(FontHandle::new(FontId(202), bytes.clone()), id_a);
    let run_z = run_with(FontHandle::new(FontId(203), bytes), id_a);
    // A second glyph per font: retention is only observable through
    // uncached glyphs, since cached placements serve without parsing.
    let run_y_new = GlyphRun {
        font: run_y.font.clone(),
        glyphs: Arc::from([GlyphPosition {
            id: id_b,
            ..run_y.glyphs[0]
        }]),
        ..run_y.clone()
    };

    let mut atlas = GlyphAtlas::new();
    atlas.set_max_fonts(2);
    // Client A parses X; client B parses Y through its own handle.
    assert!(resolve(&mut atlas, &run_x, id_a).is_some());
    assert!(resolve(&mut atlas, &run_y, id_a).is_some());
    assert_eq!(atlas.font_count(), 2);
    // Client A touches X again — a pure atlas hit that must still count
    // as use for retention ordering.
    let hits = atlas.counters().glyph_cache_hits;
    assert!(resolve(&mut atlas, &run_x, id_a).is_some());
    assert_eq!(atlas.counters().glyph_cache_hits, hits + 1);

    // Churn evicts Y (least recently used), not the re-touched X.
    assert!(resolve(&mut atlas, &run_z, id_a).is_some());
    assert_eq!(atlas.counters().font_parser_evictions, 1);
    assert_eq!(atlas.font_count(), 2);

    // A cached glyph serves without parsing either way; retention only
    // shows through uncached glyphs. Exactly one eviction happened, so a
    // re-parse below identifies Y as the victim and X as retained.
    let misses = atlas.counters().font_parser_cache_misses;
    assert!(resolve(&mut atlas, &run_x, id_a).is_some());
    assert_eq!(atlas.counters().font_parser_cache_misses, misses);

    // A new glyph from evicted Y must re-parse, which evicts the
    // now-oldest X in turn.
    let fresh = resolve(&mut atlas, &run_y_new, id_b).expect("re-parse succeeds");
    assert!(fresh.bitmap.is_some());
    assert_eq!(atlas.counters().font_parser_cache_misses, misses + 1);
    assert_eq!(atlas.counters().font_parser_evictions, 2);
    assert_eq!(atlas.font_count(), 2);
}

/// Churn and limit tightening enforce the entry limit exactly, keeping
/// the most-recently-used fonts.
#[test]
fn churn_and_tightening_enforce_the_entry_limit() {
    let mut text = TextEngine::new();
    let (bytes, id_a, id_b) = shaped_source(&mut text);
    let mut atlas = GlyphAtlas::new();
    atlas.set_max_fonts(3);

    let runs: Vec<GlyphRun> = (301..=306)
        .map(|id| run_with(FontHandle::new(FontId(id), bytes.clone()), id_a))
        .collect();
    for run in &runs {
        assert!(resolve(&mut atlas, run, id_a).is_some());
    }
    assert_eq!(atlas.counters().font_parser_cache_misses, 6);
    assert_eq!(atlas.counters().font_parser_evictions, 3);
    assert_eq!(atlas.font_count(), 3);

    // Retention shows only through uncached glyphs: a new glyph from the
    // newest survivor parses nothing, while one from the oldest re-parses
    // (and evicts in turn).
    let probe = |font: &FontHandle| GlyphRun {
        font: font.clone(),
        glyphs: Arc::from([GlyphPosition {
            id: id_b,
            ..runs[0].glyphs[0]
        }]),
        ..runs[0].clone()
    };
    let font_hits = atlas.counters().font_parser_cache_hits;
    let newest = resolve(&mut atlas, &probe(&runs[5].font), id_b).expect("supported glyph");
    assert!(newest.bitmap.is_some());
    assert_eq!(atlas.counters().font_parser_cache_hits, font_hits + 1);
    assert_eq!(atlas.counters().font_parser_evictions, 3);
    let misses = atlas.counters().font_parser_cache_misses;
    let oldest = resolve(&mut atlas, &probe(&runs[0].font), id_b).expect("re-parse succeeds");
    assert!(oldest.bitmap.is_some());
    assert_eq!(atlas.counters().font_parser_cache_misses, misses + 1);
    assert_eq!(atlas.counters().font_parser_evictions, 4);
    assert_eq!(atlas.font_count(), 3);

    // Tightening to one evicts eagerly down to the newest survivor — the
    // just-re-parsed oldest font — whose cached glyph then serves as a hit.
    atlas.set_max_fonts(1);
    assert_eq!(atlas.font_count(), 1);
    assert_eq!(atlas.counters().font_parser_evictions, 6);
    let misses = atlas.counters().font_parser_cache_misses;
    assert!(resolve(&mut atlas, &runs[0], id_a).is_some());
    assert_eq!(atlas.counters().font_parser_cache_misses, misses);
}

/// Evicting a font never invalidates atlas placements: cached glyphs keep
/// serving, a new glyph re-parses, and glyph identities in the production
/// registry are untouched by font turnover.
#[test]
fn atlas_hits_survive_font_eviction_and_new_glyphs_reparse() {
    let mut text = TextEngine::new();
    let (bytes, id_a, id_b) = shaped_source(&mut text);
    let run = run_with(FontHandle::new(FontId(401), bytes.clone()), id_a);
    let run_b = GlyphRun {
        glyphs: Arc::from([GlyphPosition {
            id: id_b,
            ..run.glyphs[0]
        }]),
        ..run.clone()
    };

    let mut atlas = GlyphAtlas::new();
    atlas.set_max_fonts(1);
    let mut registry = SharedGpuResourceRegistry::default();
    assert!(resolve(&mut atlas, &run, id_a).is_some());
    assert!(resolve(&mut atlas, &run_b, id_b).is_some());

    // Mirror the shared context's registry discipline for both glyph
    // keys, using the same minted identities the atlas entries name.
    let physical_size = incular_wgpu::GlyphRasterRequest::new(32., 1.0).physical_size;
    for glyph in [id_a, id_b] {
        registry.glyph_identity(incular_wgpu::GlyphCacheKey {
            font: FontId(401),
            glyph,
            physical_size,
        });
    }
    assert_eq!(registry.glyph_count(), 2);

    // Evict the only font via churn with an unrelated font.
    let other = run_with(FontHandle::new(FontId(402), bytes.clone()), id_a);
    assert!(resolve(&mut atlas, &other, id_a).is_some());
    assert_eq!(atlas.counters().font_parser_evictions, 1);
    assert_eq!(atlas.font_count(), 1);

    // Cached glyphs still serve as hits with no parsing.
    let misses = atlas.counters().font_parser_cache_misses;
    let cached_a = resolve(&mut atlas, &run, id_a).expect("placement survives");
    let cached_b = resolve(&mut atlas, &run_b, id_b).expect("placement survives");
    assert!(cached_a.bitmap.is_none());
    assert!(cached_b.bitmap.is_none());
    assert_eq!(atlas.counters().font_parser_cache_misses, misses);

    // A new glyph from the evicted font re-parses and retains again.
    let layout_c = text.layout("AC", &TextStyle::default(), None, TextAlign::Start);
    let id_c = layout_c.lines[0].runs[0].glyphs[1].id;
    assert_ne!(id_c, id_a);
    assert_ne!(id_c, id_b);
    let run_c = GlyphRun {
        glyphs: Arc::from([GlyphPosition {
            id: id_c,
            ..run.glyphs[0]
        }]),
        ..run.clone()
    };
    let misses = atlas.counters().font_parser_cache_misses;
    let reparsed = resolve(&mut atlas, &run_c, id_c).expect("re-parse succeeds");
    assert!(reparsed.bitmap.is_some());
    assert_eq!(atlas.counters().font_parser_cache_misses, misses + 1);
    assert_eq!(atlas.font_count(), 1);

    // Source handles keep their bytes and minted identity; the registry
    // never heard about font turnover.
    assert!(!run.font.bytes().is_empty());
    assert_eq!(run.font.id(), FontId(401));
    assert_eq!(registry.glyph_count(), 2);
}

/// Zero retention still renders: every miss parses transiently without
/// retaining, while atlas hits keep serving from placements.
#[test]
fn zero_retention_still_renders_supported_glyphs() {
    let mut text = TextEngine::new();
    let (bytes, id_a, id_b) = shaped_source(&mut text);
    let run = run_with(FontHandle::new(FontId(501), bytes), id_a);
    let run_b = GlyphRun {
        glyphs: Arc::from([GlyphPosition {
            id: id_b,
            ..run.glyphs[0]
        }]),
        ..run.clone()
    };

    let mut atlas = GlyphAtlas::new();
    atlas.set_max_fonts(0);
    assert_eq!(atlas.font_count(), 0);

    let first = resolve(&mut atlas, &run, id_a).expect("transient parse renders");
    assert!(first.bitmap.is_some());
    assert_eq!(atlas.counters().font_parser_cache_misses, 1);
    assert_eq!(atlas.font_count(), 0);
    assert_eq!(atlas.counters().font_parser_evictions, 0);

    // The placement caches normally: a hit parses nothing and retains nothing.
    let cached = resolve(&mut atlas, &run, id_a).expect("cached glyph serves");
    assert!(cached.bitmap.is_none());
    assert_eq!(atlas.counters().font_parser_cache_misses, 1);
    assert_eq!(atlas.font_count(), 0);

    // The next miss parses transiently again.
    let second = resolve(&mut atlas, &run_b, id_b).expect("transient parse renders");
    assert!(second.bitmap.is_some());
    assert_eq!(atlas.counters().font_parser_cache_misses, 2);
    assert_eq!(atlas.font_count(), 0);
}
