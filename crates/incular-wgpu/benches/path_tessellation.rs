//! CPU path tessellation cost (Kurbo geometry through Lyon) and the shape of
//! cold versus warm cache behavior.
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use incular_rendering::{FillRule, Path};

fn simple_path() -> Path {
    let mut builder = Path::builder();
    builder.move_to(incular_core::Offset::new(0., 0.));
    builder.line_to(incular_core::Offset::new(40., 0.));
    builder.line_to(incular_core::Offset::new(40., 40.));
    builder.close();
    builder.build()
}

fn medium_path() -> Path {
    let mut builder = Path::builder();
    builder.move_to(incular_core::Offset::new(0., 20.));
    for step in 0..24 {
        let x = (step as f32) * 8.;
        builder.quadratic_to(
            incular_core::Offset::new(x + 4., if step % 2 == 0 { -18. } else { 58. }),
            incular_core::Offset::new(x + 8., 20.),
        );
    }
    builder.close();
    builder.build()
}

fn complex_path() -> Path {
    let mut builder = Path::builder();
    builder.move_to(incular_core::Offset::ZERO);
    for ring in 0..12 {
        let radius = ((ring + 1) as f32) * 6.;
        for point in 0..48 {
            let angle = (point as f32) * std::f32::consts::TAU / 48.;
            let x = radius * angle.cos();
            let y = radius * angle.sin();
            let next_angle = angle + std::f32::consts::TAU / 48.;
            builder.cubic_to(
                incular_core::Offset::new(x, y),
                incular_core::Offset::new(radius * next_angle.cos(), radius * next_angle.sin()),
                incular_core::Offset::new(
                    radius * (angle + std::f32::consts::TAU / 96.).cos(),
                    radius * (angle + std::f32::consts::TAU / 96.).sin(),
                ),
            );
        }
        builder.close();
    }
    builder.build()
}

fn bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("path_tessellation");
    for (name, path) in [
        ("simple", simple_path()),
        ("medium", medium_path()),
        ("complex", complex_path()),
    ] {
        group.bench_with_input(BenchmarkId::new("cold_fill", name), &path, |b, path| {
            b.iter(|| {
                std::hint::black_box(incular_wgpu::tessellate_path(
                    std::hint::black_box(path),
                    FillRule::NonZero,
                    None,
                ))
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
