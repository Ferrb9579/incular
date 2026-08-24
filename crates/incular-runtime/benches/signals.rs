//! Reactive invalidation cost: subscriber-count scaling on a large tree.
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use incular_config::Constraints;
use incular_core::Size;
use incular_runtime::{Application, Signal};
use std::time::Instant;

fn frame() -> Constraints {
    Constraints::tight(Size::new(400., 400.))
}

fn tree_with_subscribers(subscribers: usize) -> (Application, Signal<u64>) {
    let signal = Signal::new(0_u64);
    let app_signal = signal.clone();
    let mut children = Vec::with_capacity(subscribers + 1_000);
    for index in 0..subscribers {
        let hot = app_signal.clone();
        children.push(incular_widgets::Text::new(format!("hot {index} {}", hot.get())).into());
    }
    for index in 0..1_000 {
        children.push(incular_widgets::Text::new(format!("cold {index}")).into());
    }
    let root = incular_widgets::Widget::column(children);
    let mut app = Application::new(move |_| root.clone()).expect("app");
    // Warm frame establishes the retained tree and subscriptions.
    let _ = app.run_window_frame_at(
        *app.active_window_ids().first().expect("window"),
        frame(),
        Instant::now(),
    );
    (app, signal)
}

fn bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("signals");
    for subscribers in [1usize, 10, 100] {
        group.bench_with_input(
            BenchmarkId::new("update_100k_tree", subscribers),
            &subscribers,
            |b, &subscribers| {
                let (mut app, signal) = tree_with_subscribers(subscribers);
                let window = *app.active_window_ids().first().expect("window");
                let mut generation = 0_u64;
                b.iter(|| {
                    generation += 1;
                    signal.set(generation);
                    let _ = app
                        .run_window_frame_at(window, frame(), Instant::now())
                        .expect("frame");
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
