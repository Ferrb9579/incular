//! Variable-extent index cost at 10k/100k/1M rows: offset↔index lookups,
//! extent updates, and deep jumps must stay logarithmic, never linear.
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use incular_scroll::MeasuredExtentIndex;

fn bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("variable_list");
    for rows in [10_000usize, 100_000, 1_000_000] {
        group.bench_with_input(
            BenchmarkId::new("offset_to_index", rows),
            &rows,
            |b, &rows| {
                let index = MeasuredExtentIndex::new(rows, 24.);
                let target = (rows as f32 - 1.) * 24.;
                b.iter(|| {
                    std::hint::black_box(index.index_at_offset(std::hint::black_box(target)))
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("index_to_offset", rows),
            &rows,
            |b, &rows| {
                let index = MeasuredExtentIndex::new(rows, 24.);
                let last = rows - 1;
                b.iter(|| std::hint::black_box(index.offset_for_index(last)));
            },
        );

        group.bench_with_input(
            BenchmarkId::new("deep_jump_viewport", rows),
            &rows,
            |b, &rows| {
                let index = MeasuredExtentIndex::new(rows, 24.);
                let jump_offset = 0.9 * (rows as f32) * 24.;
                b.iter(|| std::hint::black_box(index.materialized_range(jump_offset, 600., 300.)));
            },
        );
    }

    // Extent updates on a fully materialized window (post-layout feedback).
    group.bench_function("extent_updates_visible_window", |b| {
        let index = MeasuredExtentIndex::new(1_000_000, 24.);
        let range = index.materialized_range(0.9 * 1_000_000. * 24., 600., 300.);
        let measured: Vec<usize> = range.collect();
        b.iter(|| {
            for row in &measured {
                index.set_measured_extent(*row, 20.);
            }
            std::hint::black_box(&measured);
        });
    });
    group.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
