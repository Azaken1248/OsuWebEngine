//! Curve evaluation for osu! slider paths.
//!
//! osu! uses three curve types for sliders:
//! - **Composite Bézier**: Chained Bézier segments of arbitrary degree
//! - **Catmull-Rom**: Cubic splines through control points
//! - **Perfect Circular Arc**: Three-point circular arc
//!
//! All curve types are arc-length parameterized via a lookup table for
//! uniform-speed slider ball movement.
//!
//! ## Reference Implementation
//!
//! - Primary: `osu/Rulesets/Objects/SliderPath.cs` (524 lines)
//! - Secondary: `danser-go/framework/math/curves/` (all files)
//!
//! ## Status: Stub — implementation in L1

use serde::{Deserialize, Serialize};

use super::vec2::Vec2;

/// The type of curve used by a slider segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CurveType {
    /// Composite Bézier curve (default for most sliders).
    Bezier,
    /// Catmull-Rom spline (legacy, rarely used in modern maps).
    CatmullRom,
    /// Perfect circular arc defined by exactly 3 control points.
    PerfectArc,
    /// Linear interpolation between two points.
    Linear,
}

/// A resolved slider path with arc-length parameterization.
///
/// After construction, positions can be queried at any arc-length
/// fraction `t ∈ [0.0, 1.0]` in O(log N) time via the lookup table.
#[derive(Debug, Clone)]
pub struct SliderPath {
    /// The control points defining this path.
    pub control_points: Vec<Vec2>,

    /// The curve type for each segment.
    pub curve_type: CurveType,

    /// Total arc length in osu! pixels.
    pub total_length: f64,

    /// Flattened polyline points (evenly spaced by arc length).
    pub path_points: Vec<Vec2>,
}

impl SliderPath {
    /// Returns the position at arc-length fraction `t ∈ [0.0, 1.0]`.
    ///
    /// Uses binary search over the arc-length table for O(log N) lookup.
    ///
    /// # Stub
    /// Returns (0, 0) until L1 implementation.
    pub fn position_at(&self, _t: f64) -> Vec2 {
        // TODO(L1): Implement arc-length parameterized lookup
        Vec2::default()
    }

    /// Returns the total arc length in osu! pixels.
    pub fn length(&self) -> f64 {
        self.total_length
    }

    /// Pre-computes `n` evenly-spaced points along the curve for rendering.
    ///
    /// # Stub
    /// Returns empty vec until L1 implementation.
    pub fn render_points(&self, _n: usize) -> Vec<Vec2> {
        // TODO(L1): Implement render point generation
        Vec::new()
    }
}
