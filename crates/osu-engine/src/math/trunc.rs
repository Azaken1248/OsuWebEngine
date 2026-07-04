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

    // --- UT-TRUNC-004: Near-zero negative truncation ---
    #[test]
    fn ut_trunc_004_near_zero_negative() {
        assert_eq!(trunc_i32(-0.5), Some(0));
    }

    // --- UT-TRUNC-005: Near-zero negative floor ---
    #[test]
    fn ut_trunc_005_near_zero_floor() {
        assert_eq!(floor_i32(-0.5), Some(-1));
    }

    // --- UT-TRUNC-006: Stacking subtraction ---
    #[test]
    fn ut_trunc_006_stacking_subtraction() {
        let a = trunc_i32(1000.3).unwrap();
        let b = trunc_i32(1002.7).unwrap();
        assert_eq!(a - b, -2);
    }

    // --- UT-TRUNC-007: Hit window with floor ---
    #[test]
    fn ut_trunc_007_hit_window_floor() {
        let result = (79.5_f64).floor() - 0.5;
        assert!((result - 78.5).abs() < f64::EPSILON);
    }

    // --- UT-TRUNC-008: NaN truncation ---
    #[test]
    fn ut_trunc_008_nan() {
        assert_eq!(trunc_i32(f64::NAN), None);
    }

    // --- UT-TRUNC-009: Overflow truncation ---
    #[test]
    fn ut_trunc_009_overflow() {
        assert_eq!(trunc_i32(1e20), None);
    }

    // --- UT-TRUNC-010: Exact zero ---
    #[test]
    fn ut_trunc_010_exact_zero() {
        assert_eq!(trunc_i32(0.0), Some(0));
    }
}
