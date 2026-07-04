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
use super::constants::{ARC_TOLERANCE, COLLINEAR_EPSILON};
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

/// Flattens a perfect arc to a polyline with adaptive segment count.
///
/// Handles all fallback cases:
/// - `> 3` control points → Bézier fallback
/// - `< 3` control points → linear
/// - Collinear → linear
///
/// Segment count for valid arcs:
/// `max(2, ceil(totalAngle / (2 * acos(1 - ARC_TOLERANCE / radius))))`
///
/// Ref: danser `ApproximateCircularArcLazer()` + `processPerfect()`
pub fn flatten(points: &[Vec2]) -> Vec<Vec2> {
    if points.len() > 3 {
        // > 3 points: fall back to Bézier
        return bezier::split_and_flatten(points);
    }

    if points.len() < 3 {
        // < 3 points: treat as linear
        return points.to_vec();
    }

    let a = points[0];
    let b = points[1];
    let c = points[2];

    // Try to construct the arc; collinear → linear fallback
    let arc = match CircularArc::new(a, b, c) {
        Some(arc) => arc,
        None => return vec![a, b, c], // collinear: linear fallback
    };

    // Adaptive segment count (matches danser ApproximateCircularArcLazer)
    let segments = if 2.0 * arc.radius > ARC_TOLERANCE {
        let angle_step = 2.0 * (1.0 - ARC_TOLERANCE / arc.radius).acos();
        2_usize.max((arc.total_angle / angle_step).ceil() as usize)
    } else {
        2
    };

    let mut output = Vec::with_capacity(segments);
    for i in 0..segments {
        let fract = i as f64 / (segments - 1) as f64;
        output.push(arc.point_at(fract));
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

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
