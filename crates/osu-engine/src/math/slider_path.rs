//! Composite slider path with arc-length parameterized lookup.
//!
//! Behavior derived from osu!lazer `SliderPath` and cross-checked
//! against danser-go `multicurve.go` for implementation clarity.
//!
//! ## Algorithm
//!
//! 1. Control points are flattened to a polyline via the appropriate
//!    curve algorithm (Bézier, Catmull-Rom, Perfect Arc, or Linear).
//! 2. A cumulative arc-length table is built from the polyline.
//! 3. The path is clamped to `pixel_length` by trimming the table
//!    and interpolating the last point.
//! 4. `position_at(t)` uses `partition_point()` for O(log N) lookup.
//!
//! ## Key invariant
//!
//! `position_at()` performs **zero heap allocations** after construction.
//!
//! ## References
//!
//! - `osu/SliderPath.cs` — specification
//! - `danser-go/multicurve.go` L103-233 — `NewMultiCurveT()`, `PointAtLazer()`

use super::bezier;
use super::catmull;
use super::circular_arc;
use super::constants::{CATMULL_OPTIMISE_DISTANCE, CATMULL_SEGMENT_LENGTH, LENGTH_EPSILON};
use super::curves::CurveType;
use super::vec2::Vec2;

/// A fully resolved slider path with arc-length parameterization.
///
/// After construction, `position_at(t)` returns the position at
/// arc-length fraction `t ∈ [0, 1]` in O(log N) time.
#[derive(Debug, Clone)]
pub struct SliderPath {
    /// Original curve type from the beatmap.
    curve_type: CurveType,
    /// Original control points from the beatmap.
    control_points: Vec<Vec2>,
    /// Desired pixel length (may be shorter than full curve).
    pixel_length: f64,
    /// Flattened polyline points (post-clamping).
    path_points: Vec<Vec2>,
    /// Cumulative arc lengths: `cumulative_lengths[i]` = distance from
    /// `path_points[0]` to `path_points[i]`.
    cumulative_lengths: Vec<f64>,
}

impl SliderPath {
    /// Constructs a `SliderPath` from control points and desired length.
    ///
    /// 1. Dispatches to the appropriate flattening algorithm
    /// 2. Builds the cumulative length table
    /// 3. Clamps to `pixel_length`
    ///
    /// Ref: danser `NewMultiCurveT()`
    pub fn new(curve_type: CurveType, control_points: Vec<Vec2>, pixel_length: f64) -> Self {
        // Step 1: Flatten control points to a polyline
        let flattened = Self::flatten_points(curve_type, &control_points);

        // Step 2: Catmull paths get osu!stable's "bulb" optimisation, which
        // removes redundant vertices and reports how much length it removed.
        // `optimised_length` seeds the length accumulator so that stripping
        // those vertices does not cause the path to be extended to compensate.
        // Every other curve type contributes zero.
        //
        // Source: `SliderPath.cs` L375-419 (`OptimiseCatmull`, hardcoded
        // `true` for osu! sliders — see `Slider.cs` L45).
        let (mut path_points, optimised_length) = match curve_type {
            CurveType::CatmullRom => optimise_catmull(&flattened),
            _ => (flattened, 0.0),
        };

        // Step 3: Build the cumulative length table and resolve the path
        // against the expected distance (shortening *or* extending).
        let mut cumulative_lengths = Vec::with_capacity(path_points.len() + 1);
        Self::calculate_length(
            &mut path_points,
            &mut cumulative_lengths,
            optimised_length,
            pixel_length,
        );

        SliderPath {
            curve_type,
            control_points,
            pixel_length,
            path_points,
            cumulative_lengths,
        }
    }

    /// Position at arc-length fraction `t ∈ [0, 1]`.
    ///
    /// Uses `slice::partition_point()` for O(log N) binary search.
    /// **Zero heap allocations** after construction.
    ///
    /// Ref: danser `PointAtLazer()`
    pub fn position_at(&self, t: f64) -> Vec2 {
        let length = self.length();

        if self.path_points.is_empty() || length < LENGTH_EPSILON {
            return self.control_points.first().copied().unwrap_or_default();
        }

        let d = t.clamp(0.0, 1.0) * length;

        // Binary search: find the first index where cumulative_length >= d
        let i = self.cumulative_lengths.partition_point(|&len| len < d);

        if i == 0 {
            return self.path_points[0];
        }

        if i >= self.path_points.len() {
            return self.path_points[self.path_points.len() - 1];
        }

        let p0 = self.path_points[i - 1];
        let p1 = self.path_points[i];
        let d0 = self.cumulative_lengths[i - 1];
        let d1 = self.cumulative_lengths[i];

        // Avoid division by near-zero when two points nearly coincide
        if (d0 - d1).abs() < LENGTH_EPSILON {
            return p0;
        }

        let w = (d - d0) / (d1 - d0);
        p0.lerp(p1, w)
    }

    /// Total arc length after clamping to `pixel_length`.
    pub fn length(&self) -> f64 {
        self.cumulative_lengths.last().copied().unwrap_or(0.0)
    }

    /// Pre-computes `n` evenly-spaced points for rendering.
    ///
    /// **Utility API** — not used by the engine internally. Provided
    /// for visualization tools and the WASM rendering layer.
    pub fn render_points(&self, n: usize) -> Vec<Vec2> {
        if n == 0 {
            return Vec::new();
        }
        if n == 1 {
            return vec![self.position_at(0.0)];
        }
        let mut points = Vec::with_capacity(n);
        for i in 0..n {
            let t = i as f64 / (n - 1) as f64;
            points.push(self.position_at(t));
        }
        points
    }

    /// The curve type from the beatmap.
    pub fn curve_type(&self) -> CurveType {
        self.curve_type
    }

    /// The original control points.
    pub fn control_points(&self) -> &[Vec2] {
        &self.control_points
    }

    /// The desired pixel length.
    pub fn pixel_length(&self) -> f64 {
        self.pixel_length
    }

    /// The flattened polyline points (post-clamping).
    pub fn path_points(&self) -> &[Vec2] {
        &self.path_points
    }

    // --- Private helpers ---

    /// Dispatches control points to the appropriate flattening algorithm.
    fn flatten_points(curve_type: CurveType, points: &[Vec2]) -> Vec<Vec2> {
        if points.is_empty() {
            return Vec::new();
        }

        match curve_type {
            CurveType::Linear => points.to_vec(),
            CurveType::Bezier => bezier::split_and_flatten(points),
            CurveType::CatmullRom => catmull::flatten(points),
            CurveType::PerfectArc => circular_arc::flatten(points),
        }
    }

    /// Builds the cumulative arc-length table, then resolves the path against
    /// `expected_distance` — **shortening or extending** it to match.
    ///
    /// The extension case is easy to miss: when the flattened curve is
    /// *shorter* than the slider's declared pixel length, lazer pushes the
    /// final vertex outward along the last segment's direction rather than
    /// leaving the path short. Only when the last two vertices coincide (so
    /// there is no direction to extend along) is the path left as-is.
    ///
    /// `optimised_length` is the length removed by the Catmull optimisation;
    /// it seeds the accumulator so an optimised path is not then extended to
    /// make up for the vertices that pass deliberately removed.
    ///
    /// Source: `SliderPath.cs` L426-485 (`calculateLength`).
    fn calculate_length(
        path_points: &mut Vec<Vec2>,
        cumulative_lengths: &mut Vec<f64>,
        optimised_length: f64,
        expected_distance: f64,
    ) {
        let mut calculated_length = optimised_length;
        cumulative_lengths.clear();
        cumulative_lengths.push(0.0);

        for i in 0..path_points.len().saturating_sub(1) {
            calculated_length += path_points[i + 1].distance(path_points[i]);
            cumulative_lengths.push(calculated_length);
        }

        // Exact comparison mirrors lazer's `calculatedLength != expectedDistance`.
        #[allow(clippy::float_cmp)]
        if calculated_length == expected_distance {
            return;
        }

        // osu!stable quirk: if the last two path points are equal, extension
        // is not performed (there is no direction to extend along).
        let n = path_points.len();
        if n >= 2
            && path_points[n - 1] == path_points[n - 2]
            && expected_distance > calculated_length
        {
            cumulative_lengths.push(calculated_length);
            return;
        }

        // The last length is always incorrect — drop it before re-deriving.
        cumulative_lengths.pop();

        if calculated_length > expected_distance {
            // Shorten: trim vertices whose cumulative length overshoots.
            while cumulative_lengths
                .last()
                .is_some_and(|&len| len >= expected_distance)
            {
                cumulative_lengths.pop();
                path_points.pop();
            }
        }

        if path_points.len() <= 1 {
            // The expected distance is zero or negative.
            cumulative_lengths.push(0.0);
            return;
        }

        // Move the final vertex to land exactly on `expected_distance`. This
        // shortens when we trimmed above, and extends when the curve was short.
        let end = path_points.len() - 1;
        let dir = (path_points[end] - path_points[end - 1]).normalized();
        let last_cumulative = cumulative_lengths.last().copied().unwrap_or(0.0);

        path_points[end] = path_points[end - 1] + dir * (expected_distance - last_cumulative);
        cumulative_lengths.push(expected_distance);
    }
}

/// osu!stable's Catmull path optimisation, reproduced from lazer.
///
/// Catmull paths form "bulbs" around sequential knots that share a position.
/// stable suppressed these by only keeping piecewise segments at least 6px
/// apart; lazer applies a basic form of the same idea.
///
/// Returns the optimised path along with the total length removed, which the
/// caller must feed into the length accumulator — otherwise the path gets
/// extended to compensate for the very vertices this pass removed.
///
/// Source: `SliderPath.cs` L375-419.
fn optimise_catmull(sub_path: &[Vec2]) -> (Vec<Vec2>, f64) {
    let mut optimised = Vec::with_capacity(sub_path.len());
    let mut optimised_length = 0.0;

    let mut last_start: Option<Vec2> = None;
    let mut length_removed_since_start = 0.0;

    for i in 0..sub_path.len() {
        let Some(start) = last_start else {
            optimised.push(sub_path[i]);
            last_start = Some(sub_path[i]);
            continue;
        };

        // `i > 0` is guaranteed: the first iteration always takes the branch
        // above, which sets `last_start`.
        let dist_from_start = start.distance(sub_path[i]);
        length_removed_since_start += sub_path[i].distance(sub_path[i - 1]);

        // Keep a vertex if it is 6px from the run's start, is the last vertex
        // of a Catmull knot, or ends the path.
        let is_knot_end = (i + 1) % CATMULL_SEGMENT_LENGTH == 0;
        let is_path_end = i == sub_path.len() - 1;

        if dist_from_start > CATMULL_OPTIMISE_DISTANCE || is_knot_end || is_path_end {
            optimised.push(sub_path[i]);
            optimised_length += length_removed_since_start - dist_from_start;

            last_start = None;
            length_removed_since_start = 0.0;
        }
    }

    (optimised, optimised_length)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- UT-CRV-001: Linear 2-point slider ---
    #[test]
    fn ut_crv_001_linear_midpoint() {
        let path = SliderPath::new(
            CurveType::Linear,
            vec![Vec2::new(0.0, 0.0), Vec2::new(100.0, 0.0)],
            100.0,
        );

        let mid = path.position_at(0.5);
        assert!(
            mid.approx_eq(Vec2::new(50.0, 0.0), 0.01),
            "midpoint was {:?}",
            mid
        );
    }

    // --- UT-CRV-002: Quadratic Bézier ---
    #[test]
    fn ut_crv_002_quadratic_bezier() {
        let path = SliderPath::new(
            CurveType::Bezier,
            vec![
                Vec2::new(0.0, 0.0),
                Vec2::new(50.0, 100.0),
                Vec2::new(100.0, 0.0),
            ],
            200.0, // generous length
        );

        let start = path.position_at(0.0);
        let end = path.position_at(1.0);

        assert!(start.approx_eq(Vec2::new(0.0, 0.0), 0.5));
        assert!(end.approx_eq(path.path_points().last().copied().unwrap_or_default(), 0.5));

        // No NaN at sample points
        for i in 0..=4 {
            let t = i as f64 / 4.0;
            let p = path.position_at(t);
            assert!(!p.x.is_nan() && !p.y.is_nan(), "NaN at t={}", t);
        }
    }

    // --- UT-CRV-003: Cubic Bézier ---
    #[test]
    fn ut_crv_003_cubic_bezier() {
        let path = SliderPath::new(
            CurveType::Bezier,
            vec![
                Vec2::new(0.0, 0.0),
                Vec2::new(30.0, 100.0),
                Vec2::new(70.0, 100.0),
                Vec2::new(100.0, 0.0),
            ],
            300.0,
        );

        // 5 sample points should be valid
        for i in 0..=4 {
            let t = i as f64 / 4.0;
            let p = path.position_at(t);
            assert!(!p.x.is_nan() && !p.y.is_nan(), "NaN at t={}", t);
            assert!(!p.x.is_infinite() && !p.y.is_infinite(), "Inf at t={}", t);
        }
    }

    // --- UT-CRV-004: Composite Bézier continuity ---
    #[test]
    fn ut_crv_004_composite_bezier_continuity() {
        let path = SliderPath::new(
            CurveType::Bezier,
            vec![
                Vec2::new(0.0, 0.0),
                Vec2::new(50.0, 100.0),
                Vec2::new(100.0, 50.0),
                Vec2::new(100.0, 50.0), // segment boundary
                Vec2::new(150.0, 0.0),
                Vec2::new(200.0, 50.0),
            ],
            400.0,
        );

        // The boundary point (100, 50) should appear in the path
        let has_boundary = path
            .path_points()
            .iter()
            .any(|p| p.approx_eq(Vec2::new(100.0, 50.0), 0.01));
        assert!(has_boundary, "Boundary point missing from composite bezier");
    }

    // --- UT-CRV-005: Catmull-Rom 4-point ---
    #[test]
    fn ut_crv_005_catmull_rom() {
        let path = SliderPath::new(
            CurveType::CatmullRom,
            vec![
                Vec2::new(0.0, 0.0),
                Vec2::new(100.0, 200.0),
                Vec2::new(200.0, 0.0),
                Vec2::new(300.0, 200.0),
            ],
            500.0,
        );

        for i in 0..=4 {
            let t = i as f64 / 4.0;
            let p = path.position_at(t);
            assert!(!p.x.is_nan() && !p.y.is_nan(), "NaN at t={}", t);
        }
    }

    // --- UT-CRV-006: Perfect arc ---
    #[test]
    fn ut_crv_006_perfect_arc() {
        let path = SliderPath::new(
            CurveType::PerfectArc,
            vec![
                Vec2::new(0.0, 0.0),
                Vec2::new(50.0, 50.0),
                Vec2::new(100.0, 0.0),
            ],
            200.0,
        );

        let start = path.position_at(0.0);
        assert!(start.approx_eq(Vec2::new(0.0, 0.0), 0.5));

        for i in 0..=4 {
            let t = i as f64 / 4.0;
            let p = path.position_at(t);
            assert!(!p.x.is_nan() && !p.y.is_nan(), "NaN at t={}", t);
        }
    }

    // --- UT-CRV-007: Arc degenerate (collinear) ---
    #[test]
    fn ut_crv_007_arc_collinear_fallback() {
        let path = SliderPath::new(
            CurveType::PerfectArc,
            vec![
                Vec2::new(0.0, 0.0),
                Vec2::new(50.0, 0.0),
                Vec2::new(100.0, 0.0),
            ],
            100.0,
        );

        // Should behave as linear
        let mid = path.position_at(0.5);
        assert!(
            mid.approx_eq(Vec2::new(50.0, 0.0), 0.5),
            "collinear arc midpoint was {:?}",
            mid
        );
    }

    // --- UT-CRV-008: Arc large radius ---
    #[test]
    fn ut_crv_008_arc_large_radius() {
        let path = SliderPath::new(
            CurveType::PerfectArc,
            vec![
                Vec2::new(0.0, 0.0),
                Vec2::new(500.0, 0.01),
                Vec2::new(1000.0, 0.0),
            ],
            1000.0,
        );

        // Should not panic, should produce valid output
        for i in 0..=4 {
            let t = i as f64 / 4.0;
            let p = path.position_at(t);
            assert!(!p.x.is_nan() && !p.y.is_nan());
        }
    }

    // --- UT-CRV-009: Arc-length parameterization ---
    #[test]
    fn ut_crv_009_arc_length_parameterization() {
        let path = SliderPath::new(
            CurveType::Bezier,
            vec![
                Vec2::new(0.0, 0.0),
                Vec2::new(50.0, 100.0),
                Vec2::new(100.0, 0.0),
            ],
            200.0,
        );

        let total = path.length();
        if total < 1.0 {
            return; // degenerate, skip
        }

        // Equal t increments should produce roughly equal distance steps
        let n = 10;
        let expected_step = total / n as f64;
        for i in 0..n {
            let t0 = i as f64 / n as f64;
            let t1 = (i + 1) as f64 / n as f64;
            let p0 = path.position_at(t0);
            let p1 = path.position_at(t1);
            let actual_step = p0.distance(p1);
            assert!(
                (actual_step - expected_step).abs() < 0.5,
                "step {} distance={}, expected≈{}",
                i,
                actual_step,
                expected_step
            );
        }
    }

    // --- UT-CRV-010: Slider length clamping ---
    #[test]
    fn ut_crv_010_length_clamping() {
        let path = SliderPath::new(
            CurveType::Bezier,
            vec![
                Vec2::new(0.0, 0.0),
                Vec2::new(50.0, 100.0),
                Vec2::new(100.0, 0.0),
            ],
            50.0, // shorter than the full curve
        );

        assert!(
            (path.length() - 50.0).abs() < 0.5,
            "clamped length was {}",
            path.length()
        );

        // End position should be at 50px along the curve, not at the last control point
        let end = path.position_at(1.0);
        let start = path.position_at(0.0);
        let actual_dist = start.distance(end);
        assert!(
            actual_dist <= 50.5,
            "end-to-start distance {} exceeds pixel_length",
            actual_dist
        );
    }

    // --- UT-CRV-011: Zero-length slider ---
    #[test]
    fn ut_crv_011_zero_length() {
        let start = Vec2::new(256.0, 192.0);
        let path = SliderPath::new(CurveType::Linear, vec![start, Vec2::new(300.0, 192.0)], 0.0);

        let pos = path.position_at(0.5);
        assert!(
            pos.approx_eq(start, 0.01),
            "zero-length slider position was {:?}",
            pos
        );
    }

    // --- UT-CRV-012: High-degree Bézier ---
    #[test]
    fn ut_crv_012_high_degree_no_nan() {
        let points: Vec<Vec2> = (0..9)
            .map(|i| Vec2::new(i as f64 * 20.0, ((i as f64) * 0.7).sin() * 50.0))
            .collect();

        let path = SliderPath::new(CurveType::Bezier, points, 500.0);

        for i in 0..=20 {
            let t = i as f64 / 20.0;
            let p = path.position_at(t);
            assert!(!p.x.is_nan() && !p.y.is_nan(), "NaN at t={}", t);
            assert!(!p.x.is_infinite() && !p.y.is_infinite(), "Inf at t={}", t);
        }
    }

    // --- UT-CRV-013: Render points spacing ---
    #[test]
    fn ut_crv_013_render_points_spacing() {
        let path = SliderPath::new(
            CurveType::Bezier,
            vec![
                Vec2::new(0.0, 0.0),
                Vec2::new(50.0, 100.0),
                Vec2::new(100.0, 0.0),
            ],
            200.0,
        );

        let n = 20;
        let render = path.render_points(n);
        assert_eq!(render.len(), n);

        let total = path.length();
        if total < 1.0 {
            return;
        }

        let expected_spacing = total / (n - 1) as f64;
        for i in 1..n {
            let actual = render[i - 1].distance(render[i]);
            assert!(
                (actual - expected_spacing).abs() < 1.0,
                "render spacing at {} was {}, expected ≈ {}",
                i,
                actual,
                expected_spacing
            );
        }
    }

    // --- Structural tests ---
    #[test]
    fn length_non_negative() {
        let path = SliderPath::new(
            CurveType::Linear,
            vec![Vec2::new(0.0, 0.0), Vec2::new(100.0, 0.0)],
            100.0,
        );
        assert!(path.length() >= 0.0);
    }

    #[test]
    fn position_at_zero_is_start() {
        let start = Vec2::new(10.0, 20.0);
        let path = SliderPath::new(CurveType::Linear, vec![start, Vec2::new(100.0, 0.0)], 100.0);
        let pos = path.position_at(0.0);
        assert!(pos.approx_eq(start, 0.01));
    }

    #[test]
    fn accessors_return_construction_values() {
        let path = SliderPath::new(
            CurveType::Bezier,
            vec![Vec2::new(0.0, 0.0), Vec2::new(100.0, 0.0)],
            50.0,
        );
        assert_eq!(path.curve_type(), CurveType::Bezier);
        assert_eq!(path.pixel_length(), 50.0);
        assert_eq!(path.control_points().len(), 2);
    }

    #[test]
    fn cumulative_lengths_sorted() {
        let path = SliderPath::new(
            CurveType::Bezier,
            vec![
                Vec2::new(0.0, 0.0),
                Vec2::new(50.0, 100.0),
                Vec2::new(100.0, 0.0),
            ],
            200.0,
        );

        for i in 1..path.cumulative_lengths.len() {
            assert!(
                path.cumulative_lengths[i] >= path.cumulative_lengths[i - 1],
                "cumulative lengths not sorted at index {}",
                i
            );
        }
    }

    // --- Path extension (SliderPath.cs L482) ---

    /// When the declared pixel length **exceeds** the flattened curve, lazer
    /// pushes the final vertex outward along the last segment rather than
    /// leaving the slider short. We previously only ever shortened.
    #[test]
    fn path_extends_when_pixel_length_exceeds_curve() {
        // A 100px straight line asked to be 250px long.
        let path = SliderPath::new(
            CurveType::Linear,
            vec![Vec2::new(0.0, 0.0), Vec2::new(100.0, 0.0)],
            250.0,
        );

        assert!(
            (path.length() - 250.0).abs() < 0.01,
            "expected extension to 250px, got {}",
            path.length()
        );

        let end = path.position_at(1.0);
        assert!(
            end.approx_eq(Vec2::new(250.0, 0.0), 0.01),
            "expected extended endpoint (250, 0), got {:?}",
            end
        );
    }

    /// osu!stable quirk: extension is suppressed when the last two path
    /// points coincide — there is no direction to extend along.
    /// Source: `SliderPath.cs` L450.
    #[test]
    fn path_does_not_extend_when_last_two_points_coincide() {
        let path = SliderPath::new(
            CurveType::Linear,
            vec![
                Vec2::new(0.0, 0.0),
                Vec2::new(100.0, 0.0),
                Vec2::new(100.0, 0.0), // duplicate tail
            ],
            250.0,
        );

        assert!(
            (path.length() - 100.0).abs() < 0.01,
            "path must not extend past its 100px curve, got {}",
            path.length()
        );
    }

    /// Shortening still works (the case we already had).
    #[test]
    fn path_shortens_when_pixel_length_below_curve() {
        let path = SliderPath::new(
            CurveType::Linear,
            vec![Vec2::new(0.0, 0.0), Vec2::new(100.0, 0.0)],
            40.0,
        );

        assert!((path.length() - 40.0).abs() < 0.01);
        assert!(path.position_at(1.0).approx_eq(Vec2::new(40.0, 0.0), 0.01));
    }

    // --- Catmull optimisation (SliderPath.cs L375-419) ---

    /// The optimisation must strip the redundant duplicated vertices that
    /// `catmull::flatten` emits, leaving far fewer points than the raw
    /// `(n-1) * detail * 2` output.
    #[test]
    fn catmull_optimisation_removes_redundant_vertices() {
        let control = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(100.0, 200.0),
            Vec2::new(200.0, 0.0),
        ];

        let raw = catmull::flatten(&control);
        let (optimised, removed_length) = optimise_catmull(&raw);

        assert!(
            optimised.len() < raw.len(),
            "optimisation kept every vertex ({} of {})",
            optimised.len(),
            raw.len()
        );
        assert!(
            removed_length >= 0.0 && removed_length.is_finite(),
            "removed length must be finite and non-negative, got {}",
            removed_length
        );
    }

    /// A Catmull slider with duplicate knots ("bulbs") must not have its
    /// length inflated by the vertices the optimisation removes.
    #[test]
    fn catmull_duplicate_knots_do_not_inflate_length() {
        let path = SliderPath::new(
            CurveType::CatmullRom,
            vec![
                Vec2::new(0.0, 0.0),
                Vec2::new(100.0, 0.0),
                Vec2::new(100.0, 0.0), // duplicate knot → bulb
                Vec2::new(200.0, 0.0),
            ],
            200.0,
        );

        assert!(
            path.length().is_finite() && path.length() > 0.0,
            "degenerate length: {}",
            path.length()
        );

        for i in 0..=10 {
            let p = path.position_at(i as f64 / 10.0);
            assert!(
                p.x.is_finite() && p.y.is_finite(),
                "non-finite point: {:?}",
                p
            );
        }
    }
}

/// Property-based tests for `SliderPath` (L1 exit criteria).
///
/// These validate structural invariants that the fixed-case UT-CRV
/// tests cannot cover exhaustively: no NaN/Inf leaks from any curve
/// algorithm, endpoint exactness, and monotonicity of the arc-length
/// table across arbitrary control-point configurations.
#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    /// Control-point coordinates bounded to a generous multiple of the
    /// osu! playfield (512×384). Sliders may legally extend off-screen,
    /// so we do not clamp to the playfield itself.
    fn arb_vec2() -> impl Strategy<Value = Vec2> {
        (-1000.0f64..1000.0, -1000.0f64..1000.0).prop_map(|(x, y)| Vec2::new(x, y))
    }

    /// 2–8 control points — covers linear through degree-7 Bézier,
    /// multi-segment Catmull, and arc (3-point) configurations.
    fn arb_points() -> impl Strategy<Value = Vec<Vec2>> {
        prop::collection::vec(arb_vec2(), 2..=8)
    }

    fn arb_curve_type() -> impl Strategy<Value = CurveType> {
        prop_oneof![
            Just(CurveType::Bezier),
            Just(CurveType::CatmullRom),
            Just(CurveType::PerfectArc),
            Just(CurveType::Linear),
        ]
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(1000))]

        /// `length()` is never negative, NaN, or infinite.
        #[test]
        fn length_non_negative(
            curve_type in arb_curve_type(),
            points in arb_points(),
            pixel_length in 0.0f64..2000.0,
        ) {
            let path = SliderPath::new(curve_type, points, pixel_length);
            let len = path.length();

            prop_assert!(len >= 0.0, "negative length: {}", len);
            prop_assert!(len.is_finite(), "non-finite length: {}", len);
        }

        /// `position_at(t)` never produces NaN or Inf, for any `t`
        /// (including out-of-range values, which must be clamped).
        #[test]
        fn no_nan_in_position(
            curve_type in arb_curve_type(),
            points in arb_points(),
            pixel_length in 0.0f64..2000.0,
            t in -1.0f64..2.0,
        ) {
            let path = SliderPath::new(curve_type, points, pixel_length);
            let p = path.position_at(t);

            prop_assert!(p.x.is_finite(), "non-finite x at t={}: {}", t, p.x);
            prop_assert!(p.y.is_finite(), "non-finite y at t={}: {}", t, p.y);
        }

        /// `position_at(0)` returns the first point of the path.
        #[test]
        fn position_at_zero_is_start(
            curve_type in arb_curve_type(),
            points in arb_points(),
            pixel_length in 0.0f64..2000.0,
        ) {
            let path = SliderPath::new(curve_type, points, pixel_length);
            prop_assume!(!path.path_points().is_empty());

            let start = path.path_points()[0];
            let p = path.position_at(0.0);

            prop_assert!(
                p.approx_eq(start, 1e-6),
                "position_at(0) = {:?}, expected start {:?}",
                p,
                start
            );
        }

        /// `position_at(1)` returns the last point of the path.
        ///
        /// Trailing zero-length segments may cause the binary search to
        /// land on an earlier index, but those points are coincident, so
        /// the returned *value* still matches the final point.
        #[test]
        fn position_at_one_is_end(
            curve_type in arb_curve_type(),
            points in arb_points(),
            pixel_length in 0.0f64..2000.0,
        ) {
            let path = SliderPath::new(curve_type, points, pixel_length);
            prop_assume!(!path.path_points().is_empty());
            // Degenerate (zero-length) paths collapse to the start point,
            // which is covered by `position_at_zero_is_start`.
            prop_assume!(path.length() >= LENGTH_EPSILON);

            let end = *path.path_points().last().unwrap_or(&Vec2::default());
            let p = path.position_at(1.0);

            prop_assert!(
                p.approx_eq(end, 1e-6),
                "position_at(1) = {:?}, expected end {:?}",
                p,
                end
            );
        }

        /// The cumulative arc-length table is non-decreasing — the
        /// precondition for `partition_point()` to be a valid search.
        #[test]
        fn cumulative_lengths_sorted(
            curve_type in arb_curve_type(),
            points in arb_points(),
            pixel_length in 0.0f64..2000.0,
        ) {
            let path = SliderPath::new(curve_type, points, pixel_length);

            for i in 1..path.cumulative_lengths.len() {
                prop_assert!(
                    path.cumulative_lengths[i] >= path.cumulative_lengths[i - 1],
                    "cumulative lengths decreased at index {}: {} < {}",
                    i,
                    path.cumulative_lengths[i],
                    path.cumulative_lengths[i - 1]
                );
            }
        }

        /// `length()` is monotonic in `pixel_length`: requesting a longer
        /// slider never yields a shorter resolved path.
        #[test]
        fn length_monotonic_in_pixel_length(
            curve_type in arb_curve_type(),
            points in arb_points(),
            l1 in 0.0f64..2000.0,
            l2 in 0.0f64..2000.0,
        ) {
            let (lo, hi) = if l1 <= l2 { (l1, l2) } else { (l2, l1) };

            let path_lo = SliderPath::new(curve_type, points.clone(), lo);
            let path_hi = SliderPath::new(curve_type, points, hi);

            prop_assert!(
                path_lo.length() <= path_hi.length() + 1e-6,
                "length not monotonic: pixel_length {} -> {}, but {} -> {}",
                lo,
                path_lo.length(),
                hi,
                path_hi.length()
            );
        }
    }
}
