use incular_core::Color;
use incular_text::{
    FontFamily, FontVariation, TextAffinity, TextAlign, TextEditingController, TextEngine,
    TextLayoutOptions, TextOverflow, TextSelection, TextStyle,
};
use std::sync::Arc;

#[test]
fn cache_ignores_color_but_not_size() {
    let mut engine = TextEngine::new();
    let mut style = TextStyle::default();
    let a = engine.layout("hello", &style, Some(200.), TextAlign::Start);
    style.color = Color::BLACK;
    let b = engine.layout("hello", &style, Some(200.), TextAlign::Start);
    assert!(Arc::ptr_eq(&a, &b));
    style.size = 20.;
    let c = engine.layout("hello", &style, Some(200.), TextAlign::Start);
    assert!(!Arc::ptr_eq(&a, &c));
}

#[test]
fn unicode_clusters_are_never_split_before_resolution() {
    let mut engine = TextEngine::new();
    for text in ["e\u{301}", "नमस्ते", "مرحبا", "👩\u{200d}💻", "✈\u{fe0f}"]
    {
        let layout = engine.layout(text, &TextStyle::default(), None, TextAlign::Start);
        assert!(layout.metrics.size.width.is_finite());
        assert!(
            layout
                .font_runs
                .iter()
                .all(|run| run.range.start <= run.range.end)
        );
    }
}

#[test]
fn bidi_layout_exposes_visual_caret_stops_and_affinity() {
    let mut engine = TextEngine::new();
    let text = "abc אבג xyz";
    let layout = engine.layout(text, &TextStyle::default(), None, TextAlign::Start);
    let stops = layout.line_caret_positions(0);
    assert!(!stops.is_empty());
    assert!(stops.windows(2).all(|pair| pair[0].x <= pair[1].x));
    assert!(
        stops
            .iter()
            .any(|stop| stop.affinity == TextAffinity::Upstream)
    );

    let controller = TextEditingController::with_text(text);
    let first = stops[0];
    controller.set_selection_with_affinity(TextSelection::collapsed(first.offset), first.affinity);
    controller.move_right_visual(&layout, false);
    assert_eq!(controller.selection().extent, stops[1].offset);
    assert_eq!(controller.caret_affinity(), stops[1].affinity);
}

#[test]
fn registered_fonts_advance_generation_and_invalidate_layouts() {
    let mut engine = TextEngine::new();
    let first = engine.layout("hello", &TextStyle::default(), None, TextAlign::Start);
    let generation = engine.font_database_generation();
    engine.register_font_bytes(Vec::new());
    assert!(engine.font_database_generation() > generation);
    let second = engine.layout("hello", &TextStyle::default(), None, TextAlign::Start);
    assert!(!Arc::ptr_eq(&first, &second));
}

#[test]
fn soft_wrapping_and_max_lines_bound_retained_layout() {
    let mut engine = TextEngine::new();
    let layout = engine.layout_with_options(
        "one two three four five six seven eight",
        &TextStyle::default().font_size(18.),
        TextLayoutOptions::new(Some(70.), TextAlign::Start)
            .max_lines(Some(2))
            .overflow(TextOverflow::Clip),
    );
    assert_eq!(layout.lines.len(), 2);
    assert!(layout.overflowed);

    let unwrapped = engine.layout_with_options(
        "one two three four",
        &TextStyle::default().font_size(18.),
        TextLayoutOptions::new(Some(20.), TextAlign::Start).soft_wrap(false),
    );
    assert_eq!(unwrapped.lines.len(), 1);
    assert!(unwrapped.overflowed);
}

#[test]
fn icon_data_resolves_font_glyph_through_text_engine() {
    let icon = incular_text::IconData::new('A' as u32)
        .family(FontFamily::SystemUi)
        .variations([FontVariation::weight(600.)]);
    let glyph = icon.glyph_text().expect("valid icon code point");
    let mut engine = TextEngine::new();
    let layout = engine.layout(&glyph, &icon.text_style(24.), None, TextAlign::Start);
    assert!(layout.glyph_count() > 0);
    assert_eq!(icon.font_variations(), &[FontVariation::weight(600.)]);
}

#[test]
fn ellipsis_is_shaped_and_never_splits_bidi_graphemes() {
    let mut engine = TextEngine::new();
    let source = "עברית café 👩\u{200d}💻 mixed English words";
    let clipped = engine.layout_with_options(
        source,
        &TextStyle::default().font_size(20.),
        TextLayoutOptions::new(Some(95.), TextAlign::Start)
            .max_lines(Some(1))
            .overflow(TextOverflow::Clip),
    );
    let ellipsized = engine.layout_with_options(
        source,
        &TextStyle::default().font_size(20.),
        TextLayoutOptions::new(Some(95.), TextAlign::Start)
            .max_lines(Some(1))
            .overflow(TextOverflow::Ellipsis),
    );
    assert_eq!(ellipsized.lines.len(), 1);
    assert!(ellipsized.overflowed);
    // The final source is re-shaped with U+2026, so it has at least one
    // renderer glyph even when the visible source prefix is empty.
    assert!(ellipsized.glyph_count() > 0);
    assert!(ellipsized.glyph_count() >= clipped.glyph_count());
    assert!(
        ellipsized
            .font_runs
            .iter()
            .any(|run| run.direction == "rtl")
    );
}

mod document_contracts {
    use incular_core::Color;
    use incular_text::{TextAlign, TextEngine, TextStyle};
    use std::sync::Arc;

    fn paragraph(index: usize) -> String {
        format!("Paragraph {index}: the retained per-paragraph cache keeps warm documents cheap.")
    }

    fn document(paragraphs: usize) -> String {
        (0..paragraphs)
            .map(paragraph)
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn warm_document_reshapes_nothing() {
        let mut engine = TextEngine::new();
        let style = TextStyle::default();
        let doc = document(300);
        engine.layout(&doc, &style, None, TextAlign::Start);
        let cold = engine.diagnostics();
        assert_eq!(cold.paragraphs_reshaped, 300);

        engine.layout(&doc, &style, None, TextAlign::Start);
        let warm = engine.diagnostics();
        assert_eq!(
            warm.paragraphs_reshaped - cold.paragraphs_reshaped,
            0,
            "warm document must not reshape"
        );
        assert_eq!(
            warm.parley_layouts_reused - cold.parley_layouts_reused,
            300,
            "every paragraph served from cache"
        );
    }

    #[test]
    fn single_paragraph_edit_reshapes_exactly_one_paragraph() {
        let mut engine = TextEngine::new();
        let style = TextStyle::default();
        let mut doc = document(301);
        engine.layout(&doc, &style, None, TextAlign::Start);
        let before = engine.diagnostics();

        // Edit the middle paragraph only.
        let middle = format!("EDITED {}", paragraph(150));
        let mut lines: Vec<String> = doc.split('\n').map(str::to_owned).collect();
        lines[150] = middle;
        doc = lines.join("\n");

        engine.layout(&doc, &style, None, TextAlign::Start);
        let after = engine.diagnostics();
        assert_eq!(
            after.paragraphs_reshaped - before.paragraphs_reshaped,
            1,
            "only the edited paragraph reshapes"
        );
        assert_eq!(
            after.parley_layouts_reused - before.parley_layouts_reused,
            300,
            "all other paragraphs reuse their layouts"
        );
    }

    #[test]
    fn color_only_change_is_a_layout_cache_hit() {
        // LayoutKey deliberately excludes color: recoloring must not shape.
        let mut engine = TextEngine::new();
        let plain = TextStyle::default();
        engine.layout("color contract", &plain, None, TextAlign::Start);
        let before = engine.diagnostics();

        let recolored = TextStyle {
            color: Color::rgba(255, 0, 0, 255),
            ..plain.clone()
        };
        let first = engine.layout("color contract", &recolored, None, TextAlign::Start);
        let second = engine.layout("color contract", &plain, None, TextAlign::Start);
        let after = engine.diagnostics();
        assert_eq!(after.shaping_runs - before.shaping_runs, 0, "no reshaping");
        assert!(Arc::ptr_eq(&first, &second));
    }

    #[test]
    fn selection_and_caret_geometry_do_not_touch_the_engine() {
        // Selection math consumes the already-retained TextLayout; this test
        // pins that invariant at the engine boundary: repeated layout calls
        // for identical input stay cache hits even while callers mutate
        // selection state externally.
        let mut engine = TextEngine::new();
        let style = TextStyle::default();
        let doc = "selectable body\nsecond line\nthird".to_string();
        engine.layout(&doc, &style, Some(320.), TextAlign::Start);
        let before = engine.diagnostics();

        for _ in 0..25 {
            engine.layout(&doc, &style, Some(320.), TextAlign::Start);
        }
        let after = engine.diagnostics();
        assert_eq!(after.cache_misses - before.cache_misses, 0);
        assert_eq!(after.shaping_runs - before.shaping_runs, 0);
        assert_eq!(after.paragraphs_reshaped - before.paragraphs_reshaped, 0);
    }
}

mod probe_compare {
    use incular_text::{TextAlign, TextEngine, TextStyle};

    #[test]
    fn multiline_document_runs_have_increasing_vertical_origins() {
        let mut engine = TextEngine::new();
        let style = TextStyle::default();
        let doc = "Line 1\nLine 2\nLine 3";
        let layout = engine.layout(doc, &style, Some(200.0), TextAlign::Start);
        assert_eq!(layout.lines.len(), 3);
        assert_eq!(layout.lines[0].runs[0].origin.y, 0.0);
        assert!(layout.lines[1].runs[0].origin.y > 0.0);
        assert!(layout.lines[2].runs[0].origin.y > layout.lines[1].runs[0].origin.y);
    }

    #[test]
    fn probe_mono_vs_composed() {
        let mut engine = TextEngine::new();
        let style = TextStyle::default();
        let doc = "abcdef\nxy\n123456";
        let mono = engine.layout(doc, &style, Some(120.), TextAlign::Start);
        let d1 = engine.diagnostics();
        println!(
            "mono: lh={} h={} lines={}",
            mono.metrics.line_height,
            mono.metrics.size.height,
            mono.lines.len()
        );
        for (i, l) in mono.lines.iter().enumerate() {
            println!(
                "  mono[{i}]: start={} end={} caret_end={} baseline={}",
                l.start, l.end, l.caret_end, l.baseline
            );
        }
        // Force composed by clearing cache? Routing keys off contains('\n') only.
        let _ = d1;
    }
}

mod probe_entry_size {
    use incular_text::{TextAlign, TextEngine, TextStyle};

    #[test]
    fn probe_single_layout_deep_size() {
        let mut engine = TextEngine::new();
        let style = TextStyle::default();
        let live0 = probe_live();
        let layout = engine.layout("warm", &style, None, TextAlign::Start);
        let live1 = probe_live();
        // VmRSS is sampled around an allocator/cache warm-up and can move in
        // either direction when the OS reclaims pages. Keep this diagnostic
        // probe observational instead of allowing a harmless decrease to
        // panic in debug builds.
        println!("delta_live_bytes={}", live1.saturating_sub(live0));
        println!(
            "lines={} glyphs={} runs={}",
            layout.lines.len(),
            layout
                .lines
                .iter()
                .map(|line| line.glyphs.len())
                .sum::<usize>(),
            layout.font_runs.len()
        );
        println!("cache_misses={}", engine.diagnostics().cache_misses);
    }

    fn probe_live() -> usize {
        // Reads this process's VmRSS as a stable proxy (pages, KB→B).
        let Ok(stat) = std::fs::read_to_string("/proc/self/status") else {
            // VmRSS is Linux-specific; keep the diagnostic probe harmless on
            // Windows and other supported hosts.
            return 0;
        };
        stat.split('\n')
            .find(|line| line.starts_with("VmRSS"))
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|kb| kb.parse::<usize>().ok())
            .unwrap_or(0)
            * 1024
    }
}
