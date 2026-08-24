//! Restoration snapshot serialization and store round-trip cost at realistic
//! payload sizes, using the framework's actual JSON value contract.
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use incular_runtime::{InMemoryRestorationStore, RestorationStore};
use serde_json::{Value, json};

fn payload(items: usize) -> Value {
    json!({
        "title": format!("document with {items} selections"),
        "scroll": 42.5,
        "selections": (0..items).map(|index| json!(index)).collect::<Vec<_>>(),
    })
}

fn bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("restoration");
    for items in [100usize, 10_000] {
        // Capture: framework state → canonical bytes.
        group.bench_with_input(
            BenchmarkId::new("capture_json", items),
            &items,
            |b, &items| {
                b.iter(|| {
                    let bytes = serde_json::to_vec(&payload(std::hint::black_box(items)))
                        .expect("serialize");
                    std::hint::black_box(bytes)
                });
            },
        );

        let bytes = serde_json::to_vec(&payload(items)).expect("serialize");
        // Restore: bytes → live values.
        group.bench_with_input(BenchmarkId::new("restore_json", items), &items, |b, _| {
            b.iter(|| {
                let restored: Value =
                    serde_json::from_slice(std::hint::black_box(&bytes)).expect("deserialize");
                std::hint::black_box(restored)
            });
        });

        // Store round-trip through the framework's byte contract.
        group.bench_with_input(
            BenchmarkId::new("store_round_trip", items),
            &items,
            |b, _| {
                let store = InMemoryRestorationStore::new();
                b.iter(|| {
                    store.save(std::hint::black_box(&bytes)).expect("save");
                    let loaded = store.load().expect("load").expect("payload");
                    std::hint::black_box(loaded);
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
