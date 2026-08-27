//! Attribution probe: raw Widget clone/equality cost at 10k siblings.
use criterion::{Criterion, criterion_group, criterion_main};
use incular_widgets::internal::Key;
use incular_widgets::{Text, Widget};

fn row(generation: u64) -> Vec<Widget> {
    (0..10_000)
        .map(|index| {
            Widget::from(Text::new(format!("row {index} {generation}")))
                .with_key(Key::Value(index as u64))
        })
        .collect()
}

fn bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("widget_eq_probe");
    let old = row(0);
    let same = row(0);
    let diff = row(1);
    group.bench_function("eq_10k_identical", |b| {
        b.iter(|| {
            let mut hits = 0usize;
            for (a, b) in old.iter().zip(same.iter()) {
                if a == b {
                    hits += 1;
                }
            }
            std::hint::black_box(hits)
        });
    });
    group.bench_function("eq_10k_differing", |b| {
        b.iter(|| {
            let mut misses = 0usize;
            for (a, b) in old.iter().zip(diff.iter()) {
                if a != b {
                    misses += 1;
                }
            }
            std::hint::black_box(misses)
        });
    });
    group.bench_function("clone_10k", |b| {
        b.iter(|| {
            let cloned: Vec<Widget> = old.to_vec();
            std::hint::black_box(cloned)
        });
    });
    group.bench_function("build_desired_tree_10k", |b| {
        b.iter(|| std::hint::black_box(row(std::hint::black_box(0))));
    });
    group.bench_function("check_keys_10k", |b| {
        b.iter(|| {
            let mut keys = std::collections::HashSet::new();
            for widget in old.iter() {
                if let Some(key) = widget.key()
                    && !keys.insert(key)
                {
                    panic!("dup");
                }
            }
            std::hint::black_box(keys.len())
        });
    });
    group.finish();
}
criterion_group!(benches, bench);
criterion_main!(benches);
// appended probe
