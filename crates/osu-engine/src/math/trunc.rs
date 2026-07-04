//! Integer truncation helpers matching C#'s `(int)` cast behavior.
//!
//! C# truncates toward zero when casting `double` → `int`. Rust's
//! `as i32` does the same with saturating semantics (guaranteed
//! since Rust 1.45):
//!
//! - `NaN → 0`
//! - `+∞ → i32::MAX`
//! - `-∞ → i32::MIN`
//! - Overflow → nearest bound
//!
//! This module makes the intent explicit via named functions.
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
/// Uses Rust's saturating `as i32` semantics:
/// - `NaN → 0`
/// - `+∞ → i32::MAX`
/// - `-∞ → i32::MIN`
/// - Values beyond i32 range → nearest bound
///
/// ```
/// use osu_engine::math::trunc::trunc_i32;
///
/// assert_eq!(trunc_i32(3.7), 3);
/// assert_eq!(trunc_i32(-3.7), -3);
/// assert_eq!(trunc_i32(0.999), 0);
/// ```
#[inline]
pub fn trunc_i32(value: f64) -> i32 {
    value as i32
}

/// Floors `f64` to the nearest integer toward negative infinity,
/// then saturating-casts to `i32`. Matches C#'s
/// `(int)Math.Floor(v)` pattern.
///
/// - `NaN → 0`
/// - `+∞ → i32::MAX`
/// - `-∞ → i32::MIN`
#[inline]
pub fn floor_i32(value: f64) -> i32 {
    value.floor() as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trunc_positive() {
        assert_eq!(trunc_i32(3.7), 3);
        assert_eq!(trunc_i32(3.0), 3);
        assert_eq!(trunc_i32(0.999), 0);
    }

    #[test]
    fn trunc_negative() {
        assert_eq!(trunc_i32(-3.7), -3);
        assert_eq!(trunc_i32(-0.1), 0);
    }

    #[test]
    fn trunc_nan_inf() {
        // Rust saturating cast: NaN → 0, ±∞ → i32 bounds
        assert_eq!(trunc_i32(f64::NAN), 0);
        assert_eq!(trunc_i32(f64::INFINITY), i32::MAX);
        assert_eq!(trunc_i32(f64::NEG_INFINITY), i32::MIN);
    }

    #[test]
    fn floor_positive() {
        assert_eq!(floor_i32(3.7), 3);
        assert_eq!(floor_i32(3.0), 3);
    }

    #[test]
    fn floor_negative() {
        assert_eq!(floor_i32(-3.7), -4);
        assert_eq!(floor_i32(-3.0), -3);
    }

    #[test]
    fn floor_nan_inf() {
        assert_eq!(floor_i32(f64::NAN), 0);
        assert_eq!(floor_i32(f64::INFINITY), i32::MAX);
    }

    // --- UT-TRUNC-004: Near-zero negative truncation ---
    #[test]
    fn ut_trunc_004_near_zero_negative() {
        assert_eq!(trunc_i32(-0.5), 0);
    }

    // --- UT-TRUNC-005: Near-zero negative floor ---
    #[test]
    fn ut_trunc_005_near_zero_floor() {
        assert_eq!(floor_i32(-0.5), -1);
    }

    // --- UT-TRUNC-006: Stacking subtraction ---
    #[test]
    fn ut_trunc_006_stacking_subtraction() {
        let a = trunc_i32(1000.3);
        let b = trunc_i32(1002.7);
        assert_eq!(a - b, -2);
    }

    // --- UT-TRUNC-007: Hit window with floor ---
    #[test]
    fn ut_trunc_007_hit_window_floor() {
        let result = (79.5_f64).floor() - 0.5;
        assert!((result - 78.5).abs() < f64::EPSILON);
    }

    // --- UT-TRUNC-008: NaN truncation (saturating) ---
    #[test]
    fn ut_trunc_008_nan() {
        assert_eq!(trunc_i32(f64::NAN), 0);
    }

    // --- UT-TRUNC-009: Overflow truncation (saturating) ---
    #[test]
    fn ut_trunc_009_overflow() {
        assert_eq!(trunc_i32(1e20), i32::MAX);
    }

    // --- UT-TRUNC-010: Exact zero ---
    #[test]
    fn ut_trunc_010_exact_zero() {
        assert_eq!(trunc_i32(0.0), 0);
    }
}
