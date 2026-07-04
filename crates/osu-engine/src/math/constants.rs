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
/// Each pair of consecutive control points generates this many
/// interpolated points. Matches danser `ApproximateCatmullRom` detail.
pub const CATMULL_DETAIL: usize = 50;
