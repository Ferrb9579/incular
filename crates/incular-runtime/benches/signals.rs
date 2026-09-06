//! Reactive invalidation cost: subscriber-count scaling on a large tree.
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use incular_config::Constraints;
use incular_core::Size;
use incular_runtime::{Runtime, Signal};

fn frame() -> Constraints {
    Constraints::tight(Size::new(400., 400.))
}

fn tree_with_subscribers(subscribers: usize) -> (Runtime, Signal<u64>) {
    let signal = Signal::new(0_u64);
    let mut children: Vec<incular_widgets::Widget> = Vec::with_capacity(subscribers + 1_000);
    for index in 0..subscribers {
        children.push(incular_widgets::Text::new(format!("hot {index}")).into());
    }
    for index in 0..1_000 {
        children.push(incular_widgets::Text::new(format!("cold {index}")).into());
    }
    let root = incular_widgets::Widget::from(incular_widgets::Column::new(children));
    let mut app = Runtime::new(root).expect("runtime");
    let root = app.tree().root().expect("root");
    let elements = app.tree().children(root).expect("children")[..subscribers].to_vec();
    for (index, element) in elements.into_iter().enumerate() {
        let hot = signal.clone();
        app.register_builder(element, move || {
            incular_widgets::Text::new(format!("hot {index} {}", hot.get())).into()
        })
        .expect("builder");
    }
    app.run_frame(frame()).expect("warm frame");
    assert_eq!(signal.dependent_count(), subscribers);
    (app, signal)
}

fn bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("signals");
    for subscribers in [1usize, 10, 100] {
        group.bench_with_input(
            BenchmarkId::new("update_1k_cold_tree", subscribers),
            &subscribers,
            |b, &subscribers| {
                let (mut app, signal) = tree_with_subscribers(subscribers);
                let mut generation = 0_u64;
                b.iter(|| {
                    generation += 1;
                    signal.set(generation);
                    let _ = app.run_frame(frame()).expect("frame");
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
