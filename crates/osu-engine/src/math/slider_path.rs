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
use super::constants::LENGTH_EPSILON;
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

        // Step 2: Build cumulative length table
        let mut path_points = flattened;
        let mut cumulative_lengths = Self::build_cumulative_lengths(&path_points);

        // Step 3: Clamp to pixel_length
        Self::clamp_to_length(&mut path_points, &mut cumulative_lengths, pixel_length);

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

    /// Builds the cumulative arc-length table from a polyline.
    fn build_cumulative_lengths(points: &[Vec2]) -> Vec<f64> {
        if points.is_empty() {
            return vec![0.0];
        }

        let mut cumulative = Vec::with_capacity(points.len());
        cumulative.push(0.0);

        for i in 1..points.len() {
            let prev_len = cumulative[i - 1];
            let seg_len = points[i].distance(points[i - 1]);
            cumulative.push(prev_len + seg_len);
        }

        cumulative
    }

    /// Clamps the path to `pixel_length` by trimming and interpolating.
    ///
    /// Ref: danser `NewMultiCurveT()` L103-183
    fn clamp_to_length(
        path_points: &mut Vec<Vec2>,
        cumulative_lengths: &mut Vec<f64>,
        pixel_length: f64,
    ) {
        if cumulative_lengths.is_empty() {
            return;
        }

        // Zero or negative pixel_length: collapse to start point
        if pixel_length <= 0.0 {
            let start = path_points.first().copied().unwrap_or_default();
            path_points.clear();
            path_points.push(start);
            cumulative_lengths.clear();
            cumulative_lengths.push(0.0);
            return;
        }

        let full_length = *cumulative_lengths.last().unwrap_or(&0.0);

        if full_length <= pixel_length {
            // Curve is shorter than desired length — no clamping needed.
            // However, if the last two points coincide and desired length
            // exceeds the path length, add a zero-length entry (matches danser).
            if path_points.len() >= 2
                && path_points[path_points.len() - 1] == path_points[path_points.len() - 2]
                && pixel_length > full_length
            {
                cumulative_lengths.push(full_length);
            }
            return;
        }

        // Trim: remove entries that exceed pixel_length
        // First remove the last entry, then trim further
        cumulative_lengths.pop();
        path_points.pop();

        while !cumulative_lengths.is_empty()
            && *cumulative_lengths.last().unwrap_or(&0.0) >= pixel_length
        {
            cumulative_lengths.pop();
            path_points.pop();
        }

        // Interpolate the final point
        if path_points.len() <= 1 {
            cumulative_lengths.push(0.0);
            return;
        }

        let last_idx = path_points.len() - 1;
        let dir = (path_points[last_idx] - path_points[last_idx - 1]).normalized();
        let remaining = pixel_length - cumulative_lengths[last_idx];

        path_points.push(path_points[last_idx] + dir * remaining);
        cumulative_lengths.push(pixel_length);
    }
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
}
