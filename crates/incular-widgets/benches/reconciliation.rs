//! Retained reconciliation cost across tree sizes and mutation shapes.
//!
//! These benchmarks quantify the BUILD-phase diff: unchanged keyed trees must
//! stay cheap through the prefix/suffix fast paths, while structural changes
//! exercise the keyed middle-diff.
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use incular_config::Constraints;
use incular_core::Size;
use incular_widgets::internal::{ElementId, Key, WidgetTree};
use incular_widgets::{Text, Widget};

fn frame_constraints() -> Constraints {
    Constraints::tight(Size::new(600., 6000.))
}

fn keyed_row(items: usize, generation: u64) -> Widget {
    let mut children = Vec::with_capacity(items);
    for index in 0..items {
        let label = if index == items / 2 && generation > 0 {
            format!("changed {generation}")
        } else {
            format!("row {index}")
        };
        children.push(Widget::from(Text::new(label)).with_key(Key::Value(index as u64)));
    }
    incular_widgets::Column::new(children).into()
}

/// Mounts `root`, lays it out once, and returns the tree with its root id.
fn prepared(root: Widget) -> (WidgetTree, ElementId) {
    let mut tree = WidgetTree::default();
    let id = tree.mount(root).expect("mount");
    tree.layout(frame_constraints()).expect("layout");
    (tree, id)
}

fn bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("reconciliation");
    for size in [10usize, 100, 1_000, 10_000] {
        // Unchanged keyed tree: prefix fast path only.
        group.bench_with_input(BenchmarkId::new("unchanged", size), &size, |b, &size| {
            let (mut tree, root) = prepared(keyed_row(size, 0));
            b.iter(|| {
                let same = keyed_row(std::hint::black_box(size), 0);
                tree.update(root, same).expect("update");
                tree.layout(frame_constraints()).expect("layout");
            });
        });

        // One middle leaf changes; everything else matches by key.
        group.bench_with_input(
            BenchmarkId::new("single_leaf_changed", size),
            &size,
            |b, &size| {
                let (mut tree, root) = prepared(keyed_row(size, 0));
                b.iter(|| {
                    let next = keyed_row(std::hint::black_box(size), 1);
                    tree.update(root, next).expect("update");
                    tree.layout(frame_constraints()).expect("layout");
                });
            },
        );

        // Every child's content changes: full middle diff, no remounts.
        group.bench_with_input(
            BenchmarkId::new("all_text_changed", size),
            &size,
            |b, &size| {
                let (mut tree, root) = prepared(unkeyed_row(size, 0));
                b.iter(|| {
                    let next = unkeyed_row(std::hint::black_box(size), 1);
                    tree.update(root, next).expect("update");
                    tree.layout(frame_constraints()).expect("layout");
                });
            },
        );

        // Insert one child at the front of a keyed list.
        group.bench_with_input(BenchmarkId::new("insert_front", size), &size, |b, &size| {
            let (mut tree, root) = prepared(keyed_row(size, 0));
            b.iter(|| {
                let mut children: Vec<Widget> =
                    vec![Widget::from(Text::new("inserted")).with_key(Key::Value(u64::MAX))];
                for index in 0..std::hint::black_box(size) {
                    children.push(
                        Widget::from(Text::new(format!("row {index}")))
                            .with_key(Key::Value(index as u64)),
                    );
                }
                let next = incular_widgets::Column::new(children).into();
                tree.update(root, next).expect("update");
                tree.layout(frame_constraints()).expect("layout");
            });
        });
    }
    group.finish();
}

fn unkeyed_row(items: usize, generation: u64) -> Widget {
    let mut children = Vec::with_capacity(items);
    for index in 0..items {
        children.push(Widget::from(Text::new(format!(
            "gen {generation} row {index}"
        ))));
    }
    incular_widgets::Column::new(children).into()
}

/// Alternating keyed/unkeyed siblings.
fn mixed_row(items: usize, generation: u64) -> Widget {
    let mut children = Vec::with_capacity(items);
    for index in 0..items {
        let widget = Widget::from(Text::new(if index % 7 == 0 && generation > 0 {
            format!("mixed changed {generation}")
        } else {
            format!("mixed row {index}")
        }));
        children.push(if index % 2 == 0 {
            widget.with_key(Key::Value(index as u64))
        } else {
            widget
        });
    }
    incular_widgets::Column::new(children).into()
}

fn extended_bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("reconciliation_matrix");

    // Update-only versus cached-layout-only attribution.
    for size in [1_000usize, 10_000] {
        group.bench_with_input(
            BenchmarkId::new("unchanged_update_only", size),
            &size,
            |b, &size| {
                let (mut tree, root) = prepared(keyed_row(size, 0));
                b.iter(|| {
                    let same = keyed_row(std::hint::black_box(size), 0);
                    tree.update(root, same).expect("update");
                });
            },
        );
        group.bench_with_input(
            BenchmarkId::new("cached_layout_only", size),
            &size,
            |b, &size| {
                let (mut tree, _root) = prepared(keyed_row(size, 0));
                b.iter(|| {
                    tree.layout(frame_constraints()).expect("layout");
                });
            },
        );
    }

    // Change position sensitivity (first/middle/last) at 10k.
    for (name, index_of_change) in [
        ("change_first", 0usize),
        ("change_middle", 5_000),
        ("change_last", 9_999),
    ] {
        group.bench_function(name, |b| {
            let (mut tree, root) = prepared(keyed_row(10_000, 0));
            b.iter(|| {
                let children: Vec<Widget> = (0..10_000)
                    .map(|index| {
                        Widget::from(Text::new(if index == index_of_change {
                            format!("hit {}", std::hint::black_box(index))
                        } else {
                            format!("row {index}")
                        }))
                        .with_key(Key::Value(index as u64))
                    })
                    .collect();
                let next = incular_widgets::Column::new(children).into();
                tree.update(root, next).expect("update");
                tree.layout(frame_constraints()).expect("layout");
            });
        });
    }

    // Insert/remove middle at 10k.
    group.bench_function("insert_middle", |b| {
        let (mut tree, root) = prepared(keyed_row(10_000, 0));
        b.iter(|| {
            let mut children: Vec<Widget> = (0..10_000)
                .map(|index| {
                    Widget::from(Text::new(format!("row {index}")))
                        .with_key(Key::Value(index as u64))
                })
                .collect();
            children.insert(
                5_000,
                Widget::from(Text::new("inserted")).with_key(Key::Value(u64::MAX)),
            );
            let next = incular_widgets::Column::new(children).into();
            tree.update(root, next).expect("update");
            tree.layout(frame_constraints()).expect("layout");
        });
    });
    group.bench_function("remove_middle", |b| {
        let (mut tree, root) = prepared(keyed_row(10_001, 0));
        b.iter(|| {
            let children: Vec<Widget> = (0..10_001)
                .filter(|index| *index != 5_000)
                .map(|index| {
                    Widget::from(Text::new(format!("row {index}")))
                        .with_key(Key::Value(index as u64))
                })
                .collect();
            let next = incular_widgets::Column::new(children).into();
            tree.update(root, next).expect("update");
            tree.layout(frame_constraints()).expect("layout");
        });
    });

    // Reverse + rotate keyed order at 10k (pure reorder, O(N) target).
    group.bench_function("reverse_keyed", |b| {
        let (mut tree, root) = prepared(keyed_row(10_000, 0));
        b.iter(|| {
            let children: Vec<Widget> = (0..10_000)
                .rev()
                .map(|index| {
                    Widget::from(Text::new(format!("row {index}")))
                        .with_key(Key::Value(index as u64))
                })
                .collect();
            let next = incular_widgets::Column::new(children).into();
            tree.update(root, next).expect("update");
            tree.layout(frame_constraints()).expect("layout");
        });
    });
    group.bench_function("rotate_keyed_by_one", |b| {
        let (mut tree, root) = prepared(keyed_row(10_000, 0));
        b.iter(|| {
            let children: Vec<Widget> = (0..10_000)
                .map(|index| (index + 9_999) % 10_000)
                .map(|index| {
                    Widget::from(Text::new(format!("row {index}")))
                        .with_key(Key::Value(index as u64))
                })
                .collect();
            let next = incular_widgets::Column::new(children).into();
            tree.update(root, next).expect("update");
            tree.layout(frame_constraints()).expect("layout");
        });
    });

    // Mixed keyed/unkeyed unchanged at 10k.
    group.bench_function("unchanged_mixed_10k", |b| {
        let (mut tree, root) = prepared(mixed_row(10_000, 0));
        b.iter(|| {
            let same = mixed_row(std::hint::black_box(10_000), 0);
            tree.update(root, same).expect("update");
            tree.layout(frame_constraints()).expect("layout");
        });
    });

    // 100k unchanged: OPT-IN ONLY (`INCULAR_HEAVY_BENCH=1`). The retained
    // 100k-node tree costs >6 GB resident memory (≈65 KB per widget across
    // element/render/layer/display-list state), which must never be mounted
    // incidentally by a default benchmark sweep. See MEMORY.md for the
    // workspace's low-memory build policy.
    if std::env::var("INCULAR_HEAVY_BENCH").is_ok() {
        group.bench_function("unchanged_100k", |b| {
            let (mut tree, root) = prepared(keyed_row(100_000, 0));
            b.iter(|| {
                let same = keyed_row(std::hint::black_box(100_000), 0);
                tree.update(root, same).expect("update");
                tree.layout(frame_constraints()).expect("layout");
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench, extended_bench);
criterion_main!(benches);
