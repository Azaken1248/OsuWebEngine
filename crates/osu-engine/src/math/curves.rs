//! Curve type enumeration for osu! slider paths.
//!
//! This module defines only the `CurveType` enum. Actual curve
//! implementations live in their respective modules:
//! - [`bezier`](super::bezier) — Bézier flattening
//! - [`catmull`](super::catmull) — Catmull-Rom spline
//! - [`circular_arc`](super::circular_arc) — Perfect circular arc
//! - [`slider_path`](super::slider_path) — Composite path assembly

use serde::{Deserialize, Serialize};

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
