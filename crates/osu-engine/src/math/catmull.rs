//! Catmull-Rom spline evaluation.
//!
//! Behavior derived from osu!lazer `PathApproximator` and cross-checked
//! against danser-go `catmull.go` for implementation clarity.
//!
//! ## Algorithm
//!
//! Each pair of consecutive control points [P₁, P₂] defines one cubic
//! segment using 4 surrounding points [P₀, P₁, P₂, P₃]. The parametric
//! formula is the standard Catmull-Rom matrix form.
//!
//! Endpoint handling:
//! - Missing P₀ (before first): duplicate P₁
//! - Missing P₃ (after last): extrapolate as P₃ = P₂ + (P₂ - P₁)
//!
//! ## References
//!
//! - `osu/PathApproximator.cs` — specification
//! - `danser-go/catmull.go` — reading aid
//! - `danser-go/multicurve.go` L361-386 — endpoint handling

use super::constants::CATMULL_DETAIL;
use super::vec2::Vec2;

/// Evaluates a single Catmull-Rom segment at parameter `t ∈ [0, 1]`.
///
/// Formula: `q(t) = 0.5 × ((2P₁) + (-P₀+P₂)t + (2P₀-5P₁+4P₂-P₃)t² + (-P₀+3P₁-3P₂+P₃)t³)`
///
/// Ref: danser `findPoint()`
fn catmull_point(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, t: f64) -> Vec2 {
    let t2 = t * t;
    let t3 = t * t2;

    Vec2::new(
        0.5 * (2.0 * p1.x
            + (-p0.x + p2.x) * t
            + (2.0 * p0.x - 5.0 * p1.x + 4.0 * p2.x - p3.x) * t2
            + (-p0.x + 3.0 * p1.x - 3.0 * p2.x + p3.x) * t3),
        0.5 * (2.0 * p1.y
            + (-p0.y + p2.y) * t
            + (2.0 * p0.y - 5.0 * p1.y + 4.0 * p2.y - p3.y) * t2
            + (-p0.y + 3.0 * p1.y - 3.0 * p2.y + p3.y) * t3),
    )
}

/// Flattens a Catmull-Rom spline into a polyline.
///
/// Emits **two points per step** — `t = c/detail` and `t = (c+1)/detail` —
/// producing `CATMULL_DETAIL * 2` points per segment with duplicated
/// interior vertices. This looks redundant, and it is: the duplicates
/// create zero-length segments. But it is exactly what lazer does, and
/// the Catmull optimisation pass (`slider_path::optimise_catmull`) keys
/// its knot detection off this doubled index layout. Collapsing it here
/// would silently break that pass.
///
/// Endpoint handling:
/// - Missing start: `v1 = v2` (duplicate first control point)
/// - Missing end:   `v4 = v3 + v3 - v2` (extrapolate)
///
/// Source: `PathApproximator.cs` L150–169 (`CatmullToPiecewiseLinear`).
pub fn flatten(points: &[Vec2]) -> Vec<Vec2> {
    if points.len() < 2 {
        return points.to_vec();
    }

    let segment_count = points.len() - 1;
    let mut output = Vec::with_capacity(segment_count * CATMULL_DETAIL * 2);

    for i in 0..segment_count {
        // Resolve surrounding control points with endpoint handling.
        // `v3` is always in range here because the loop stops at len-1.
        let v1 = if i > 0 { points[i - 1] } else { points[i] };
        let v2 = points[i];
        let v3 = points[i + 1];
        let v4 = if i + 2 < points.len() {
            points[i + 2]
        } else {
            v3 + v3 - v2
        };

        for c in 0..CATMULL_DETAIL {
            let t0 = c as f64 / CATMULL_DETAIL as f64;
            let t1 = (c + 1) as f64 / CATMULL_DETAIL as f64;
            output.push(catmull_point(v1, v2, v3, v4, t0));
            output.push(catmull_point(v1, v2, v3, v4, t1));
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catmull_point_at_zero_returns_p1() {
        let p0 = Vec2::new(0.0, 0.0);
        let p1 = Vec2::new(100.0, 100.0);
        let p2 = Vec2::new(200.0, 0.0);
        let p3 = Vec2::new(300.0, 100.0);

        let result = catmull_point(p0, p1, p2, p3, 0.0);
        assert!(result.approx_eq(p1, 1e-10));
    }

    #[test]
    fn catmull_point_at_one_returns_p2() {
        let p0 = Vec2::new(0.0, 0.0);
        let p1 = Vec2::new(100.0, 100.0);
        let p2 = Vec2::new(200.0, 0.0);
        let p3 = Vec2::new(300.0, 100.0);

        let result = catmull_point(p0, p1, p2, p3, 1.0);
        assert!(result.approx_eq(p2, 1e-10));
    }

    #[test]
    fn flatten_two_points() {
        let points = [Vec2::new(0.0, 0.0), Vec2::new(100.0, 100.0)];
        let result = flatten(&points);

        // lazer emits two points per step: `(n-1) * catmull_detail * 2`.
        // Source: `PathApproximator.cs` L152.
        assert_eq!(result.len(), CATMULL_DETAIL * 2);
        assert!(result[0].approx_eq(points[0], 1e-10));
        assert!(result.last().unwrap().approx_eq(points[1], 1e-10));
    }

    /// The doubled emission must produce duplicated interior vertices —
    /// `optimise_catmull` depends on this layout to find knot boundaries.
    #[test]
    fn flatten_emits_duplicated_interior_vertices() {
        let points = [
            Vec2::new(0.0, 0.0),
            Vec2::new(100.0, 200.0),
            Vec2::new(200.0, 0.0),
        ];
        let result = flatten(&points);

        assert_eq!(result.len(), 2 * CATMULL_DETAIL * 2);

        // Within a segment, result[2c+1] (t=(c+1)/d) and result[2c+2]
        // (t=(c+1)/d of the next step) are the same point.
        assert!(
            result[1].approx_eq(result[2], 1e-10),
            "expected duplicated vertex pair, got {:?} and {:?}",
            result[1],
            result[2]
        );
    }

    #[test]
    fn flatten_four_points_no_nan() {
        let points = [
            Vec2::new(0.0, 0.0),
            Vec2::new(100.0, 200.0),
            Vec2::new(200.0, 0.0),
            Vec2::new(300.0, 200.0),
        ];
        let result = flatten(&points);

        for p in &result {
            assert!(!p.x.is_nan() && !p.y.is_nan());
        }
    }

    #[test]
    fn flatten_single_point() {
        let points = [Vec2::new(50.0, 50.0)];
        let result = flatten(&points);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], points[0]);
    }

    #[test]
    fn flatten_empty() {
        let result = flatten(&[]);
        assert!(result.is_empty());
    }
}
