use criterion::{criterion_group, criterion_main, Criterion};

/// Benchmark placeholder — per-stage benchmarks will be added as
/// each pipeline stage is implemented.
fn bench_placeholder(c: &mut Criterion) {
    c.bench_function("version_lookup", |b| {
        b.iter(|| {
            let v = osu_engine::version::version();
            std::hint::black_box(v);
        });
    });
}

criterion_group!(benches, bench_placeholder);
criterion_main!(benches);
