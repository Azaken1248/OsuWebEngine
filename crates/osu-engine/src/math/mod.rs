//! L1: Core Math — Geometric primitives and curve evaluation.
//!
//! This module provides the mathematical foundation used by all downstream
//! pipeline stages. It has no dependencies on other engine modules.
//!
//! ## Components
//!
//! - [`vec2`]: 2D vector type with distance, lerp, and arithmetic operations
//! - [`curves`]: Bézier, Catmull-Rom, and perfect circular arc evaluation
//! - [`trunc`]: Integer truncation helpers matching C#'s `(int)` cast behavior

pub mod curves;
pub mod trunc;
pub mod vec2;
