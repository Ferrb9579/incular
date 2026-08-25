//! SEMANTICS-phase collection cost and unchanged-tree skip behavior.
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use incular_config::Constraints;
use incular_core::Size;
use incular_widgets::{Text, Widget, WidgetTree, internal::ActionSurface};

fn semantic_tree(width: usize) -> Widget {
    let mut children = Vec::with_capacity(width);
    for index in 0..width {
        if index % 3 == 0 {
            children.push(ActionSurface::new(format!("action {index}")).into());
        } else {
            children.push(Text::new(format!("label {index}")).into());
        }
    }
    Widget::column(children)
}

fn bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("semantics");
    for size in [10usize, 100, 1_000] {
        group.bench_with_input(BenchmarkId::new("collect", size), &size, |b, &size| {
            let mut tree = WidgetTree::default();
            tree.mount(semantic_tree(size)).expect("mount");
            tree.layout(Constraints::tight(Size::new(600., 6000.)));
            b.iter(|| tree.update_semantics());
        });
    }
    group.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
