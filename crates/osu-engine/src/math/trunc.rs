//! Integer truncation helpers matching C#'s `(int)` cast behavior.
//!
//! C# truncates toward zero when casting `double` → `int`. Rust's
//! `as i32` does the same, but this module makes the intent explicit
//! and provides overflow-safe variants.
//!
//! ## Why This Matters
//!
//! osu!lazer uses `(int)` casts in stacking threshold computation
//! (TDD §11.2.4). Behavioral equivalence requires matching this
//! exact truncation behavior.
//!
//! ## Reference
//!
//! - `OsuBeatmapProcessor.cs` L280–281 (stack threshold)

/// Truncates `f64` toward zero, matching C#'s `(int)` cast.
///
/// Returns `None` if the value is outside `i32` range, NaN, or infinity.
///
/// ```
/// use osu_engine::math::trunc::trunc_i32;
///
/// assert_eq!(trunc_i32(3.7), Some(3));
/// assert_eq!(trunc_i32(-3.7), Some(-3));
/// assert_eq!(trunc_i32(0.999), Some(0));
/// ```
#[inline]
pub fn trunc_i32(value: f64) -> Option<i32> {
    if value.is_nan() || value.is_infinite() {
        return None;
    }
    let truncated = value as i64;
    if truncated > i32::MAX as i64 || truncated < i32::MIN as i64 {
        return None;
    }
    Some(truncated as i32)
}

/// Floors `f64` to the nearest integer toward negative infinity,
/// matching C#'s `Math.Floor` → `(int)` pattern.
///
/// Returns `None` if the value is outside `i32` range, NaN, or infinity.
#[inline]
pub fn floor_i32(value: f64) -> Option<i32> {
    if value.is_nan() || value.is_infinite() {
        return None;
    }
    let floored = value.floor() as i64;
    if floored > i32::MAX as i64 || floored < i32::MIN as i64 {
        return None;
    }
    Some(floored as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trunc_positive() {
        assert_eq!(trunc_i32(3.7), Some(3));
        assert_eq!(trunc_i32(3.0), Some(3));
        assert_eq!(trunc_i32(0.999), Some(0));
    }

    #[test]
    fn trunc_negative() {
        assert_eq!(trunc_i32(-3.7), Some(-3));
        assert_eq!(trunc_i32(-0.1), Some(0));
    }

    #[test]
    fn trunc_nan_inf() {
        assert_eq!(trunc_i32(f64::NAN), None);
        assert_eq!(trunc_i32(f64::INFINITY), None);
        assert_eq!(trunc_i32(f64::NEG_INFINITY), None);
    }

    #[test]
    fn floor_positive() {
        assert_eq!(floor_i32(3.7), Some(3));
        assert_eq!(floor_i32(3.0), Some(3));
    }

    #[test]
    fn floor_negative() {
        assert_eq!(floor_i32(-3.7), Some(-4));
        assert_eq!(floor_i32(-3.0), Some(-3));
    }

    #[test]
    fn floor_nan_inf() {
        assert_eq!(floor_i32(f64::NAN), None);
        assert_eq!(floor_i32(f64::INFINITY), None);
    }
}
