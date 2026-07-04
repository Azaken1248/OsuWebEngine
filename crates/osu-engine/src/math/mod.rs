//! L1: Core Math — Geometric primitives and curve evaluation.
//!
//! This module provides the mathematical foundation used by all downstream
//! pipeline stages. It has no dependencies on other engine modules.
//!
//! ## Components
//!
//! - [`vec2`]: 2D vector type with distance, lerp, and arithmetic operations
//! - [`constants`]: Centralized epsilon and tolerance values
//! - [`curves`]: Curve type enumeration
//! - [`bezier`]: Bézier curve flattening via adaptive subdivision
//! - [`catmull`]: Catmull-Rom spline evaluation
//! - [`circular_arc`]: Perfect circular arc computation
//! - [`slider_path`]: Composite slider path with arc-length parameterization
//! - [`trunc`]: Integer truncation helpers matching C#'s `(int)` cast behavior

pub mod bezier;
pub mod catmull;
pub mod circular_arc;
pub mod constants;
pub mod curves;
pub mod slider_path;
pub mod trunc;
pub mod vec2;
