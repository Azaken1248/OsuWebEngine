//! Centralized constants for curve mathematics.
//!
//! All epsilon and tolerance values used across the math module live here.
//! No magic numbers in algorithm files — easy to audit and tune.

/// Bézier flatness tolerance squared (0.5² = 0.25).
///
/// A Bézier segment is considered "flat enough" when the maximum
/// 2nd-order finite difference of its control polygon is below this
/// threshold. Matches danser `BEZIER_QUANTIZATIONSQ`.
///
/// Behavior derived from osu!lazer `PathApproximator`, cross-checked
/// against danser-go.
pub const BEZIER_TOLERANCE_SQ: f64 = 0.25;

/// Epsilon for collinear point detection via cross product magnitude.
///
/// Three points A, B, C are considered collinear when
/// `|(B-A) × (C-A)| < COLLINEAR_EPSILON`.
pub const COLLINEAR_EPSILON: f64 = 1e-3;

/// Epsilon for near-zero arc-length segment checks.
///
/// When two consecutive cumulative-length entries differ by less than
/// this, we treat them as the same point (avoid division by near-zero).
/// Matches danser's 0.00000001 threshold in `PointAtLazer()`.
pub const LENGTH_EPSILON: f64 = 1e-8;

/// Minimum part width for slider path clamping.
///
/// When trimming a slider path to `pixel_length`, line segments
/// shorter than this are removed entirely. Matches danser
/// `minPartWidth = 0.0001`.
pub const MIN_PART_WIDTH: f64 = 0.0001;

/// Circular arc point density tolerance.
///
/// Controls how many line segments approximate a circular arc.
/// Segment count = `ceil(totalAngle / (2 * acos(1 - ARC_TOLERANCE / radius)))`.
/// Matches danser's `0.1` in `ApproximateCircularArcLazer()`.
pub const ARC_TOLERANCE: f64 = 0.1;

/// Number of samples per Catmull-Rom segment.
///
/// Each pair of consecutive control points generates `CATMULL_DETAIL`
/// *steps*, and each step emits **two** points (see `catmull::flatten`),
/// for `CATMULL_DETAIL * 2` points per segment.
///
/// Source: `PathApproximator.cs` L23 (`catmull_detail = 50`).
pub const CATMULL_DETAIL: usize = 50;

/// Points per Catmull-Rom segment in the flattened output.
///
/// Because `CatmullToPiecewiseLinear` emits two points per step, one
/// segment spans `CATMULL_DETAIL * 2` indices. The Catmull optimisation
/// pass relies on this to detect knot boundaries.
///
/// Source: `SliderPath.cs` L406 (`catmull_segment_length = catmull_detail * 2`).
pub const CATMULL_SEGMENT_LENGTH: usize = CATMULL_DETAIL * 2;

/// Distance threshold (osu!px) for the Catmull path optimisation.
///
/// osu!stable only keeps piecewise segments that are at least 6px apart.
/// lazer reproduces a basic form of this to avoid "bulbs" forming around
/// sequential Catmull knots with identical positions.
///
/// Source: `SliderPath.cs` L409.
pub const CATMULL_OPTIMISE_DISTANCE: f64 = 6.0;

/// Maximum number of points a circular arc may be approximated with.
///
/// An arc requiring this many points or more falls back to a numerically
/// stable Bézier approximation. 1000 subpoints requires an arc length of
/// at least ~120,000 osu!px to occur, so this is a pathological case —
/// but it is also the guard that prevents an adversarial `.osu` from
/// forcing an unbounded allocation.
///
/// Source: `SliderPath.cs` L359.
pub const MAX_ARC_POINTS: usize = 1000;
