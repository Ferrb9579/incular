//! LAYOUT-phase cost across tree shapes and cache states.
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use incular_config::{Alignment, Constraints};
use incular_core::Size;
use incular_widgets::internal::{ElementId, WidgetTree};
use incular_widgets::{Padding, Text, Widget};

fn frame(size: f32) -> Constraints {
    Constraints::tight(Size::new(size, size))
}

fn deep_tree(depth: usize) -> Widget {
    let mut child = incular_widgets::Widget::from(Text::new("leaf"));
    for _ in 0..depth {
        child = incular_widgets::Widget::from(Padding::all(2., child));
    }
    child
}

fn wide_tree(width: usize) -> Widget {
    let children = (0..width)
        .map(|index| {
            Widget::from(Padding::all(
                1.,
                Widget::from(Text::new(format!("w{index}"))),
            ))
        })
        .collect::<Vec<_>>();
    Widget::column(children)
}

fn flex_tree(width: usize) -> Widget {
    let children = (0..width)
        .map(|index| incular_widgets::Widget::from(Text::new(format!("f{index}"))))
        .collect::<Vec<_>>();
    Widget::row(children)
}

fn stack_tree(depth: usize) -> Widget {
    let children = (0..depth)
        .map(|index| {
            Widget::align(
                Alignment::new(0.25 * (index % 4) as f32, 0.75),
                Widget::from(Text::new(format!("s{index}"))),
            )
        })
        .collect::<Vec<_>>();
    Widget::stack(Alignment::CENTER, children)
}

fn prepared(root: Widget) -> (WidgetTree, ElementId) {
    let mut tree = WidgetTree::default();
    let id = tree.mount(root).expect("mount");
    tree.layout(frame(600.)).expect("layout");
    (tree, id)
}

fn bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("layout");

    group.bench_function("deep_100_fully_cached", |b| {
        let (mut tree, root) = prepared(deep_tree(100));
        b.iter(|| {
            tree.layout(frame(std::hint::black_box(600.)))
                .expect("layout");
        });
        let _ = root;
    });

    group.bench_function("wide_1000_fully_cached", |b| {
        let (mut tree, _) = prepared(wide_tree(1_000));
        b.iter(|| {
            tree.layout(frame(std::hint::black_box(600.)))
                .expect("layout");
        });
    });

    group.bench_function("flex_500_fully_cached", |b| {
        let (mut tree, _) = prepared(flex_tree(500));
        b.iter(|| {
            tree.layout(frame(std::hint::black_box(600.)))
                .expect("layout");
        });
    });

    group.bench_function("stack_200_fully_cached", |b| {
        let (mut tree, _) = prepared(stack_tree(200));
        b.iter(|| {
            tree.layout(frame(std::hint::black_box(600.)))
                .expect("layout");
        });
    });

    // Root constraints changed: every node re-resolves (worst case).
    group.bench_function("wide_1000_root_constraints_changed", |b| {
        let (mut tree, _) = prepared(wide_tree(1_000));
        let mut width = 601.;
        b.iter(|| {
            width += 1.;
            tree.layout(frame(std::hint::black_box(width)))
                .expect("layout");
        });
    });

    for size in [10usize, 100, 1_000] {
        group.bench_with_input(
            BenchmarkId::new("cold_mount_and_layout", size),
            &size,
            |b, &size| {
                b.iter(|| {
                    let mut tree = WidgetTree::default();
                    tree.mount(wide_tree(size)).expect("mount");
                    tree.layout(frame(600.)).expect("layout");
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
