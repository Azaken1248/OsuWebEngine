//! Perfect circular arc computation and flattening.
//!
//! Behavior derived from osu!lazer `PathApproximator` and cross-checked
//! against danser-go `cirarc.go` + `approximation.go` for implementation
//! clarity.
//!
//! ## Algorithm
//!
//! Given 3 non-collinear points A, B, C:
//! 1. Compute the circumcenter (center of the circle through all 3)
//! 2. Derive radius, start angle, and total arc angle
//! 3. Determine arc direction (CW/CCW) from cross product
//! 4. Flatten to polyline with adaptive segment count
//!
//! ## Fallbacks
//!
//! - Collinear points → linear interpolation
//! - > 3 control points → Bézier fallback
//! - < 3 control points → linear
//!
//! ## References
//!
//! - `osu/PathApproximator.cs` — specification
//! - `danser-go/cirarc.go` — circumcenter, angles
//! - `danser-go/approximation.go` L30-49 — `ApproximateCircularArcLazer()`
//! - `danser-go/multicurve.go` L301-313 — `processPerfect()`

use super::bezier;
use super::constants::{ARC_TOLERANCE, COLLINEAR_EPSILON, MAX_ARC_POINTS};
use super::vec2::Vec2;

/// A circle defined by 3 points, with arc traversal parameters.
struct CircularArc {
    /// Center of the circle through the 3 defining points.
    centre: Vec2,
    /// Radius of the circle.
    radius: f64,
    /// Angle from center to the first point.
    start_angle: f64,
    /// Total angle swept by the arc.
    total_angle: f64,
    /// Direction: +1.0 (CCW) or -1.0 (CW).
    direction: f64,
}

impl CircularArc {
    /// Computes circumcenter, radius, and arc direction from 3 points.
    ///
    /// Returns `None` if the points are collinear.
    ///
    /// Ref: danser `NewCirArc()`
    fn new(a: Vec2, b: Vec2, c: Vec2) -> Option<Self> {
        if is_straight_line(a, b, c) {
            return None;
        }

        // Circumcenter formula (TDD §4.3)
        let d = 2.0 * (a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y));

        // d ≈ 0 means nearly collinear (shouldn't reach here after check above)
        if d.abs() < f64::EPSILON {
            return None;
        }

        let a_sq = a.x * a.x + a.y * a.y;
        let b_sq = b.x * b.x + b.y * b.y;
        let c_sq = c.x * c.x + c.y * c.y;

        let centre = Vec2::new(
            (a_sq * (b.y - c.y) + b_sq * (c.y - a.y) + c_sq * (a.y - b.y)) / d,
            (a_sq * (c.x - b.x) + b_sq * (a.x - c.x) + c_sq * (b.x - a.x)) / d,
        );

        let radius = a.distance(centre);
        let start_angle = (a.y - centre.y).atan2(a.x - centre.x);
        let mut end_angle = (c.y - centre.y).atan2(c.x - centre.x);

        while end_angle < start_angle {
            end_angle += 2.0 * std::f64::consts::PI;
        }

        let mut total_angle = end_angle - start_angle;
        let mut direction = 1.0;

        // Check arc direction: is B on the shorter arc from A to C?
        // Use cross product of (C-A) rotated 90° dotted with (B-A)
        let a_to_c = c - a;
        let a_to_c_perp = Vec2::new(a_to_c.y, -a_to_c.x);

        if a_to_c_perp.dot(b - a) < 0.0 {
            direction = -1.0;
            total_angle = 2.0 * std::f64::consts::PI - total_angle;
        }

        Some(CircularArc {
            centre,
            radius,
            start_angle,
            total_angle,
            direction,
        })
    }

    /// Position at parameter `t ∈ [0, 1]` along the arc.
    ///
    /// Ref: danser `PointAtL()`
    fn point_at(&self, t: f64) -> Vec2 {
        let theta = self.start_angle + self.direction * t * self.total_angle;
        Vec2::new(
            theta.cos() * self.radius + self.centre.x,
            theta.sin() * self.radius + self.centre.y,
        )
    }
}

/// Checks if 3 points are collinear via cross product magnitude.
///
/// Returns `true` if `|(B-A) × (C-A)| < COLLINEAR_EPSILON`.
pub fn is_straight_line(a: Vec2, b: Vec2, c: Vec2) -> bool {
    let ab = b - a;
    let ac = c - a;
    // 2D cross product magnitude
    let cross = (ab.x * ac.y - ab.y * ac.x).abs();
    cross < COLLINEAR_EPSILON
}

/// Number of points required to approximate this arc within `ARC_TOLERANCE`.
///
/// `2 * radius <= tolerance` is a pathological case (an arc shorter than
/// the tolerance itself), handled by falling back to a 2-point line.
///
/// The `as usize` cast saturates (NaN → 0, +∞ → usize::MAX), so a
/// degenerate radius yields a value that trips the `MAX_ARC_POINTS`
/// guard in `flatten()` rather than overflowing.
///
/// Source: `PathApproximator.cs` L186.
fn amount_points(arc: &CircularArc) -> usize {
    if 2.0 * arc.radius <= ARC_TOLERANCE {
        return 2;
    }

    let angle_step = 2.0 * (1.0 - ARC_TOLERANCE / arc.radius).acos();
    2_usize.max((arc.total_angle / angle_step).ceil() as usize)
}

/// Flattens a perfect arc to a polyline with adaptive point count.
///
/// Every fallback below routes to the Bézier approximation, **not** to a
/// straight line. lazer's `calculateSubPath` `break`s out of the
/// `PerfectCurve` case on each of these conditions, and the fall-through
/// at the end of the switch is `BSplineToPiecewiseLinear` — which, for a
/// legacy slider with no explicit degree, reduces to a plain Bézier.
///
/// Fallback cases:
/// - control point count != 3          (`SliderPath.cs` L345)
/// - arc is invalid / collinear         (`SliderPath.cs` L351, `PathApproximator.cs` L178)
/// - arc needs >= `MAX_ARC_POINTS`      (`SliderPath.cs` L359)
///
/// Point count for valid arcs:
/// `max(2, ceil(thetaRange / (2 * acos(1 - ARC_TOLERANCE / radius))))`
///
/// Source: `PathApproximator.cs` L175–199 (`CircularArcToPiecewiseLinear`)
///         + `SliderPath.cs` L343–369 (guards).
pub fn flatten(points: &[Vec2]) -> Vec<Vec2> {
    // lazer only treats an exactly-3-point control set as a perfect curve.
    if points.len() != 3 {
        return bezier::split_and_flatten(points);
    }

    let arc = match CircularArc::new(points[0], points[1], points[2]) {
        Some(arc) => arc,
        // Invalid (collinear) arc → Bézier, not linear.
        None => return bezier::split_and_flatten(points),
    };

    let amount = amount_points(&arc);

    // Pathological arcs fall back to a numerically stable Bézier. This is
    // also what bounds the allocation below on adversarial input.
    if amount >= MAX_ARC_POINTS {
        return bezier::split_and_flatten(points);
    }

    let mut output = Vec::with_capacity(amount);
    for i in 0..amount {
        let fract = i as f64 / (amount - 1) as f64;
        output.push(arc.point_at(fract));
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An invalid (collinear) arc must fall back to **Bézier**, not linear.
    /// Source: `SliderPath.cs` L351 → switch fall-through.
    #[test]
    fn collinear_arc_falls_back_to_bezier() {
        let points = [
            Vec2::new(0.0, 0.0),
            Vec2::new(50.0, 0.0),
            Vec2::new(100.0, 0.0),
        ];

        assert_eq!(
            flatten(&points),
            bezier::split_and_flatten(&points),
            "collinear arc must route to the Bézier fallback"
        );
    }

    /// A non-3-point control set is not a perfect curve — Bézier fallback.
    /// Source: `SliderPath.cs` L345.
    #[test]
    fn non_three_point_arc_falls_back_to_bezier() {
        let two = [Vec2::new(0.0, 0.0), Vec2::new(100.0, 0.0)];
        assert_eq!(flatten(&two), bezier::split_and_flatten(&two));

        let four = [
            Vec2::new(0.0, 0.0),
            Vec2::new(50.0, 50.0),
            Vec2::new(100.0, 0.0),
            Vec2::new(150.0, 50.0),
        ];
        assert_eq!(flatten(&four), bezier::split_and_flatten(&four));
    }

    /// An arc requiring >= MAX_ARC_POINTS must fall back to Bézier rather
    /// than allocating an unbounded point buffer.
    ///
    /// This is the guard that stops a crafted `.osu` from forcing a huge
    /// allocation. Source: `SliderPath.cs` L359.
    #[test]
    fn pathological_arc_falls_back_to_bezier() {
        // A circle of radius 200_000 traversed over ~3 radians needs roughly
        // theta * sqrt(r / arc_tolerance) / 2 ~= 1500 points — over the limit.
        let r = 200_000.0_f64;
        let at = |theta: f64| Vec2::new(r * theta.cos(), r * theta.sin());
        let points = [at(0.0), at(1.5), at(3.0)];

        // Sanity: this really would exceed the cap via the arc path.
        let arc = CircularArc::new(points[0], points[1], points[2])
            .expect("three points on a circle form a valid arc");
        assert!(
            amount_points(&arc) >= MAX_ARC_POINTS,
            "test fixture no longer exceeds the cap ({} points)",
            amount_points(&arc)
        );

        assert_eq!(
            flatten(&points),
            bezier::split_and_flatten(&points),
            "arc over the point cap must route to the Bézier fallback"
        );
    }

    #[test]
    fn collinear_points_detected() {
        let a = Vec2::new(0.0, 0.0);
        let b = Vec2::new(50.0, 0.0);
        let c = Vec2::new(100.0, 0.0);
        assert!(is_straight_line(a, b, c));
    }

    #[test]
    fn non_collinear_points_not_detected() {
        let a = Vec2::new(0.0, 0.0);
        let b = Vec2::new(50.0, 50.0);
        let c = Vec2::new(100.0, 0.0);
        assert!(!is_straight_line(a, b, c));
    }

    #[test]
    fn collinear_falls_back_to_linear() {
        let points = [
            Vec2::new(0.0, 0.0),
            Vec2::new(50.0, 0.0),
            Vec2::new(100.0, 0.0),
        ];
        let result = flatten(&points);
        // Should return all 3 points as-is (linear fallback)
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], points[0]);
        assert_eq!(result[2], points[2]);
    }

    #[test]
    fn arc_starts_and_ends_correctly() {
        let a = Vec2::new(0.0, 0.0);
        let b = Vec2::new(50.0, 50.0);
        let c = Vec2::new(100.0, 0.0);
        let result = flatten(&[a, b, c]);

        assert!(result.len() >= 2);
        assert!(result[0].approx_eq(a, 0.5));
        assert!(result.last().unwrap().approx_eq(c, 0.5));
    }

    #[test]
    fn arc_no_nan() {
        let a = Vec2::new(0.0, 0.0);
        let b = Vec2::new(50.0, 100.0);
        let c = Vec2::new(100.0, 0.0);
        let result = flatten(&[a, b, c]);

        for p in &result {
            assert!(!p.x.is_nan() && !p.y.is_nan());
            assert!(!p.x.is_infinite() && !p.y.is_infinite());
        }
    }

    #[test]
    fn more_than_three_falls_back_to_bezier() {
        let points = [
            Vec2::new(0.0, 0.0),
            Vec2::new(50.0, 100.0),
            Vec2::new(100.0, 0.0),
            Vec2::new(150.0, 100.0),
        ];
        let result = flatten(&points);
        // Should produce a polyline (Bézier fallback), not crash
        assert!(result.len() >= 2);
    }

    #[test]
    fn fewer_than_three_returns_linear() {
        let points = [Vec2::new(0.0, 0.0), Vec2::new(100.0, 100.0)];
        let result = flatten(&points);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn large_radius_no_panic() {
        // UT-CRV-008: near-collinear but not quite → large radius
        let a = Vec2::new(0.0, 0.0);
        let b = Vec2::new(500.0, 0.01);
        let c = Vec2::new(1000.0, 0.0);
        let result = flatten(&[a, b, c]);

        // Should produce some output without panicking
        assert!(!result.is_empty());
        for p in &result {
            assert!(!p.x.is_nan() && !p.y.is_nan());
        }
    }
}
