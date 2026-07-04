use criterion::{criterion_group, criterion_main, Criterion};
use osu_engine::math::bezier;
use osu_engine::math::catmull;
use osu_engine::math::circular_arc;
use osu_engine::math::curves::CurveType;
use osu_engine::math::slider_path::SliderPath;
use osu_engine::math::vec2::Vec2;

/// Benchmark: flatten a cubic Bézier (4 control points).
/// Baseline expectation: < 200 µs.
fn bench_bezier_flatten(c: &mut Criterion) {
    let points = vec![
        Vec2::new(0.0, 0.0),
        Vec2::new(30.0, 100.0),
        Vec2::new(70.0, 100.0),
        Vec2::new(100.0, 0.0),
    ];

    c.bench_function("bezier_flatten_cubic", |b| {
        b.iter(|| std::hint::black_box(bezier::split_and_flatten(&points)));
    });
}

/// Benchmark: flatten a 6-point Catmull-Rom spline.
/// Baseline expectation: < 200 µs.
fn bench_catmull_flatten(c: &mut Criterion) {
    let points = vec![
        Vec2::new(0.0, 0.0),
        Vec2::new(100.0, 200.0),
        Vec2::new(200.0, 0.0),
        Vec2::new(300.0, 200.0),
        Vec2::new(400.0, 0.0),
        Vec2::new(500.0, 200.0),
    ];

    c.bench_function("catmull_flatten_6pt", |b| {
        b.iter(|| std::hint::black_box(catmull::flatten(&points)));
    });
}

/// Benchmark: flatten a 3-point perfect arc.
/// Baseline expectation: < 200 µs.
fn bench_arc_flatten(c: &mut Criterion) {
    let points = vec![
        Vec2::new(0.0, 0.0),
        Vec2::new(50.0, 50.0),
        Vec2::new(100.0, 0.0),
    ];

    c.bench_function("arc_flatten_3pt", |b| {
        b.iter(|| std::hint::black_box(circular_arc::flatten(&points)));
    });
}

/// Benchmark: full SliderPath construction (Bézier, 200px).
/// Baseline expectation: < 500 µs.
fn bench_slider_path_construct(c: &mut Criterion) {
    let points = vec![
        Vec2::new(0.0, 0.0),
        Vec2::new(50.0, 100.0),
        Vec2::new(100.0, 0.0),
    ];

    c.bench_function("slider_path_construct_bezier", |b| {
        b.iter(|| std::hint::black_box(SliderPath::new(CurveType::Bezier, points.clone(), 200.0)));
    });
}

/// Benchmark: 1000 position_at() calls on a pre-built SliderPath.
/// Baseline expectation: < 1 µs per call.
fn bench_slider_path_position_at(c: &mut Criterion) {
    let path = SliderPath::new(
        CurveType::Bezier,
        vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(50.0, 100.0),
            Vec2::new(100.0, 0.0),
        ],
        200.0,
    );

    c.bench_function("slider_path_position_at_1000", |b| {
        b.iter(|| {
            for i in 0..1000 {
                let t = i as f64 / 999.0;
                std::hint::black_box(path.position_at(t));
            }
        });
    });
}

criterion_group!(
    benches,
    bench_bezier_flatten,
    bench_catmull_flatten,
    bench_arc_flatten,
    bench_slider_path_construct,
    bench_slider_path_position_at,
);
criterion_main!(benches);
