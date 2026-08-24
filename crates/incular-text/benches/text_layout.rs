//! Parley-backed text layout cost: cold shaping versus warm cache hits.
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use incular_text::{TextAlign, TextEngine, TextStyle};

fn paragraph() -> String {
    "The quick brown fox jumps over the lazy dog. ".repeat(40)
}

fn large_document() -> String {
    (0..200)
        .map(|index| format!("Paragraph {index}: lorem ipsum dolor sit amet. "))
        .collect()
}

fn bidi_sample() -> String {
    "Hello שלום مرحبا world 123 עברית text".repeat(8)
}

fn bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("text_layout");
    let style = TextStyle::default();

    for (name, sample) in [
        ("short_label", "OK".to_string()),
        ("paragraph", paragraph()),
        ("large_document", large_document()),
        ("mixed_scripts_bidi", bidi_sample()),
    ] {
        group.bench_with_input(
            BenchmarkId::new("cold_layout", name),
            &sample,
            |b, sample| {
                b.iter_batched(
                    TextEngine::new,
                    |engine| {
                        let mut engine = engine;
                        engine.layout(sample, &style, Some(400.), TextAlign::Start)
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );

        group.bench_with_input(
            BenchmarkId::new("warm_cache_hit", name),
            &sample,
            |b, sample| {
                let mut engine = TextEngine::new();
                engine.layout(sample, &style, Some(400.), TextAlign::Start);
                b.iter(|| {
                    std::hint::black_box(engine.layout(
                        std::hint::black_box(sample),
                        &style,
                        Some(400.),
                        TextAlign::Start,
                    ))
                });
            },
        );
    }
    group.finish();
}

fn document(paragraphs: usize) -> String {
    (0..paragraphs)
        .map(|index| {
            format!(
                "Paragraph {index}: the retained per-paragraph cache keeps warm documents cheap."
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn edited_document(paragraphs: usize) -> String {
    let mut lines: Vec<String> = (0..paragraphs)
        .map(|index| {
            format!(
                "Paragraph {index}: the retained per-paragraph cache keeps warm documents cheap."
            )
        })
        .collect();
    let middle = paragraphs / 2;
    lines[middle] = format!("EDITED paragraph {middle}: only this line is new.");
    lines.join("\n")
}

fn sized_sample(chars: usize, seed: &str) -> String {
    if chars <= seed.len() {
        return seed[..chars].to_string();
    }
    let mut out = String::with_capacity(chars);
    while out.len() < chars {
        out.push_str(seed);
        out.push(' ');
    }
    out.truncate(chars);
    out
}

fn extended_bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("text_document");
    let style = TextStyle::default();

    // Size ladder on ASCII prose.
    for chars in [10usize, 100, 1_000, 10_000, 100_000] {
        let sample = sized_sample(chars, "the quick brown fox jumps over the lazy dog");
        group.bench_with_input(
            criterion::BenchmarkId::new("cold_ascii", chars),
            &sample,
            |b, sample| {
                b.iter_batched(
                    TextEngine::new,
                    |engine| {
                        let mut engine = engine;
                        engine.layout(sample, &style, Some(400.), TextAlign::Start)
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }

    // Script coverage at ~4k chars.
    for (name, sample) in [
        ("latin_prose", paragraph()),
        ("cjk_mixed", "Hello 世界 mixed 中英文 text 🎉".repeat(120)),
        (
            "arabic_rtl",
            "النص العربي يكتمل من اليمين إلى اليسار ".repeat(90),
        ),
        ("indic", "हिन्दी देवनागरी लिपि पाठ ".repeat(140)),
        ("emoji_heavy", "🎉🚀🌈👍💡 ".repeat(400)),
    ] {
        group.bench_with_input(
            criterion::BenchmarkId::new("cold_scripts", name),
            &sample,
            |b, sample| {
                b.iter_batched(
                    TextEngine::new,
                    |engine| {
                        let mut engine = engine;
                        engine.layout(sample, &style, Some(400.), TextAlign::Start)
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }

    // Document scenarios: cold / warm / single-paragraph edit.
    group.bench_function("document_300_cold", |b| {
        b.iter_batched(
            TextEngine::new,
            |engine| {
                let mut engine = engine;
                engine.layout(&document(300), &style, Some(480.), TextAlign::Start)
            },
            criterion::BatchSize::SmallInput,
        );
    });
    group.bench_function("document_300_warm", |b| {
        let mut engine = TextEngine::new();
        let doc = document(300);
        engine.layout(&doc, &style, Some(480.), TextAlign::Start);
        b.iter(|| {
            std::hint::black_box(engine.layout(
                std::hint::black_box(&doc),
                &style,
                Some(480.),
                TextAlign::Start,
            ))
        });
    });
    group.bench_function("document_300_single_edit", |b| {
        let mut engine = TextEngine::new();
        let doc = document(300);
        engine.layout(&doc, &style, Some(480.), TextAlign::Start);
        let edited = edited_document(300);
        b.iter(|| {
            std::hint::black_box(engine.layout(
                std::hint::black_box(&edited),
                &style,
                Some(480.),
                TextAlign::Start,
            ))
        });
    });

    // Width change: reports current behavior (width participates in the
    // layout key, so this reshapes; see PERFORMANCE.md notes).
    group.bench_function("document_300_width_change", |b| {
        let mut engine = TextEngine::new();
        let doc = document(300);
        let mut width = 480f32;
        engine.layout(&doc, &style, Some(width), TextAlign::Start);
        b.iter(|| {
            width = if width > 500. { 480. } else { 520. };
            std::hint::black_box(engine.layout(
                std::hint::black_box(&doc),
                &style,
                Some(std::hint::black_box(width)),
                TextAlign::Start,
            ))
        });
    });

    // Color-only mutation must be a pure cache hit.
    group.bench_function("color_only_change", |b| {
        let plain = style.clone();
        let mut colored = style.clone();
        colored.color = incular_core::Color::rgba(255, 0, 0, 255);
        let mut engine = TextEngine::new();
        engine.layout(&paragraph(), &plain, None, TextAlign::Start);
        let mut flip = false;
        b.iter(|| {
            flip = !flip;
            std::hint::black_box(engine.layout(
                std::hint::black_box(paragraph().as_str()),
                if flip { &colored } else { &plain },
                None,
                TextAlign::Start,
            ))
        });
    });
    group.finish();
}

criterion_group!(benches, bench, extended_bench);
criterion_main!(benches);
