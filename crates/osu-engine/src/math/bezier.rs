//! Bézier curve flattening via adaptive subdivision.
//!
//! Behavior derived from osu!lazer `PathApproximator` and cross-checked
//! against danser-go `bezierapproximator.go` for implementation clarity.
//!
//! ## Algorithm
//!
//! Curves are adaptively subdivided using an **iterative** stack (not
//! recursion) to avoid stack overflow on pathological high-degree curves.
//! The flatness check uses 2nd-order finite differences of the control
//! polygon — when curvature is below `BEZIER_TOLERANCE_SQ`, the segment
//! is approximated as a polyline using De Casteljau midpoints.
//!
//! ## References
//!
//! - `osu/PathApproximator.cs` — specification
//! - `danser-go/bezierapproximator.go` — reading aid

use super::constants::BEZIER_TOLERANCE_SQ;
use super::vec2::Vec2;

/// Checks if the 2nd-order derivative (approximated via finite differences)
/// of the control polygon is within tolerance, meaning the curve is
/// locally "flat enough" to approximate as a polyline.
///
/// Ref: danser `IsFlatEnough()`
fn is_flat_enough(points: &[Vec2]) -> bool {
    for i in 1..points.len() - 1 {
        // p[i-1] - 2*p[i] + p[i+1] approximates the 2nd derivative
        let diff = points[i - 1] - points[i] * 2.0 + points[i + 1];
        if diff.length_sq() > BEZIER_TOLERANCE_SQ {
            return false;
        }
    }
    true
}

/// Subdivides a Bézier curve at t=0.5 into left and right halves
/// using De Casteljau's algorithm (in-place midpoint averaging).
///
/// `left` and `right` must have the same length as `points`.
///
/// Ref: danser `Subdivide()`
fn subdivide(points: &[Vec2], left: &mut [Vec2], right: &mut [Vec2]) {
    let count = points.len();
    let mut midpoints = points.to_vec();

    for i in 0..count {
        left[i] = midpoints[0];
        right[count - i - 1] = midpoints[count - i - 1];

        for j in 0..count - i - 1 {
            midpoints[j] = (midpoints[j] + midpoints[j + 1]) * 0.5;
        }
    }
}

/// Produces a piecewise-linear approximation from a single subdivision
/// pass, using the De Casteljau midpoints.
///
/// Ref: danser `Approximate()`
fn approximate_segment(points: &[Vec2], left: &mut [Vec2], output: &mut Vec<Vec2>) {
    let count = points.len();

    let mut right = vec![Vec2::default(); count];
    subdivide(points, left, &mut right);

    // Merge left and right halves
    left[count..count + count - 1].copy_from_slice(&right[1..count]);

    output.push(points[0]);

    for i in 1..count - 1 {
        let index = 2 * i;
        let p = (left[index - 1] + left[index] * 2.0 + left[index + 1]) * 0.25;
        output.push(p);
    }
}

/// Adaptively flattens a single Bézier segment to a polyline.
///
/// Uses an **iterative** stack (not recursion) to avoid stack overflow
/// on pathological high-degree curves.
///
/// Ref: danser `CreateBezier()` — iterative DFS with `toFlatten` stack.
fn flatten_single_bezier(points: &[Vec2]) -> Vec<Vec2> {
    let count = points.len();
    if count == 0 {
        return Vec::new();
    }

    // Estimate output capacity
    let mut output = Vec::with_capacity(count * 2);
    let mut to_flatten: Vec<Vec<Vec2>> = Vec::new();
    let mut free_buffers: Vec<Vec<Vec2>> = Vec::new();

    to_flatten.push(points.to_vec());

    // Subdivision buffer (2*count - 1 to hold both halves)
    let mut left_child = vec![Vec2::default(); count * 2 - 1];

    while let Some(parent) = to_flatten.pop() {
        if is_flat_enough(&parent) {
            approximate_segment(&parent, &mut left_child, &mut output);
            free_buffers.push(parent);
            continue;
        }

        // Subdivide: reuse parent buffer for left, allocate/reuse for right
        let mut right_child = free_buffers
            .pop()
            .unwrap_or_else(|| vec![Vec2::default(); count]);

        subdivide(&parent, &mut left_child, &mut right_child);

        // Copy left half from left_child into parent (reuse buffer)
        let mut left_copy = parent;
        left_copy[..count].copy_from_slice(&left_child[..count]);

        // Push right first (DFS: process left first when popping)
        to_flatten.push(right_child);
        to_flatten.push(left_copy);
    }

    // Always include the last control point
    if let Some(&last) = points.last() {
        output.push(last);
    }

    output
}

/// Splits control points at repeated-point boundaries and flattens
/// each Bézier segment, producing a single combined polyline.
///
/// A repeated control point (e.g., `[A, B, C, C, D, E]`) splits into
/// two segments: `[A, B, C]` and `[C, D, E]`.
///
/// Two-point segments are treated as straight lines (no subdivision).
///
/// Ref: danser `processBezier()`
pub fn split_and_flatten(points: &[Vec2]) -> Vec<Vec2> {
    if points.is_empty() {
        return Vec::new();
    }

    let mut output = Vec::with_capacity(points.len() * 2);
    let mut last_index = 0;

    for i in 0..points.len() {
        let is_segment_boundary = i < points.len() - 2 && points[i] == points[i + 1];
        let is_last = i == points.len() - 1;

        if is_segment_boundary || is_last {
            let sub_points = &points[last_index..=i];

            let flattened = if sub_points.len() == 2 {
                // Two points = straight line, no subdivision needed
                vec![sub_points[0], sub_points[1]]
            } else {
                flatten_single_bezier(sub_points)
            };

            // Append, skipping duplicate first point if it matches output tail
            if output.is_empty() || output.last() != flattened.first() {
                output.extend_from_slice(&flattened);
            } else {
                output.extend_from_slice(&flattened[1..]);
            }

            // Skip the duplicate control point
            if is_segment_boundary {
                last_index = i + 1;
            }
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_point_is_flat() {
        let points = [Vec2::new(0.0, 0.0), Vec2::new(100.0, 0.0)];
        assert!(is_flat_enough(&points));
    }

    #[test]
    fn straight_line_is_flat() {
        let points = [
            Vec2::new(0.0, 0.0),
            Vec2::new(50.0, 0.0),
            Vec2::new(100.0, 0.0),
        ];
        assert!(is_flat_enough(&points));
    }

    #[test]
    fn curved_is_not_flat() {
        let points = [
            Vec2::new(0.0, 0.0),
            Vec2::new(50.0, 100.0),
            Vec2::new(100.0, 0.0),
        ];
        assert!(!is_flat_enough(&points));
    }

    #[test]
    fn flatten_linear_produces_endpoints() {
        let points = [Vec2::new(0.0, 0.0), Vec2::new(100.0, 0.0)];
        let result = split_and_flatten(&points);
        assert!(result.len() >= 2);
        assert_eq!(result[0], points[0]);
        assert_eq!(*result.last().unwrap(), points[1]);
    }

    #[test]
    fn flatten_quadratic_bezier() {
        let points = [
            Vec2::new(0.0, 0.0),
            Vec2::new(50.0, 100.0),
            Vec2::new(100.0, 0.0),
        ];
        let result = split_and_flatten(&points);
        assert!(result.len() >= 3);
        assert_eq!(result[0], points[0]);
        assert_eq!(*result.last().unwrap(), points[2]);

        // No NaN/Inf
        for p in &result {
            assert!(!p.x.is_nan() && !p.y.is_nan());
            assert!(!p.x.is_infinite() && !p.y.is_infinite());
        }
    }

    #[test]
    fn flatten_composite_bezier_with_split() {
        // [A, B, C, C, D, E] → two segments: [A,B,C] and [C,D,E]
        let points = [
            Vec2::new(0.0, 0.0),
            Vec2::new(50.0, 100.0),
            Vec2::new(100.0, 50.0),
            Vec2::new(100.0, 50.0), // repeated = boundary
            Vec2::new(150.0, 0.0),
            Vec2::new(200.0, 50.0),
        ];
        let result = split_and_flatten(&points);

        // Should start at first point and end at last
        assert_eq!(result[0], points[0]);
        assert_eq!(*result.last().unwrap(), points[5]);

        // Boundary point should appear (continuity)
        assert!(result
            .iter()
            .any(|p| p.approx_eq(Vec2::new(100.0, 50.0), 0.01)));
    }

    #[test]
    fn flatten_high_degree_no_nan() {
        // UT-CRV-012: degree 8 Bézier
        let points: Vec<Vec2> = (0..9)
            .map(|i| Vec2::new(i as f64 * 20.0, ((i as f64) * 0.7).sin() * 50.0))
            .collect();
        let result = split_and_flatten(&points);

        for p in &result {
            assert!(!p.x.is_nan() && !p.y.is_nan(), "NaN in high-degree bezier");
            assert!(
                !p.x.is_infinite() && !p.y.is_infinite(),
                "Inf in high-degree bezier"
            );
        }
    }

    #[test]
    fn flatten_empty_returns_empty() {
        assert!(split_and_flatten(&[]).is_empty());
    }
}
