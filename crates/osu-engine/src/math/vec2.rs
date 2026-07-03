//! 2D vector type for osu! coordinate space.
//!
//! osu! uses a 512×384 pixel playfield. All positions, distances, and
//! interpolations operate in this coordinate space using `f64` to match
//! C#'s `double` precision (BRD §7.2).

use serde::{Deserialize, Serialize};
use std::ops::{Add, Mul, Sub};

/// A 2D point/vector in osu! pixel coordinates.
///
/// Uses `f64` to match C# `double` and avoid floating-point divergence
/// during behavioral comparison with osu!lazer.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}

impl Vec2 {
    /// Creates a new `Vec2`.
    #[inline]
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// Returns the Euclidean distance to another point.
    #[inline]
    pub fn distance(self, other: Self) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }

    /// Returns the squared Euclidean distance (avoids sqrt for comparisons).
    #[inline]
    pub fn distance_sq(self, other: Self) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        dx * dx + dy * dy
    }

    /// Linear interpolation between `self` and `other` at parameter `t`.
    ///
    /// `t = 0.0` returns `self`, `t = 1.0` returns `other`.
    #[inline]
    pub fn lerp(self, other: Self, t: f64) -> Self {
        Self {
            x: self.x + (other.x - self.x) * t,
            y: self.y + (other.y - self.y) * t,
        }
    }

    /// Returns the length (magnitude) of this vector.
    #[inline]
    pub fn length(self) -> f64 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    /// Returns a unit-length vector in the same direction, or zero vector
    /// if the length is zero.
    #[inline]
    pub fn normalized(self) -> Self {
        let len = self.length();
        if len == 0.0 {
            Self::default()
        } else {
            Self {
                x: self.x / len,
                y: self.y / len,
            }
        }
    }

    /// Dot product of two vectors.
    #[inline]
    pub fn dot(self, other: Self) -> f64 {
        self.x * other.x + self.y * other.y
    }
}

impl Add for Vec2 {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl Sub for Vec2 {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl Mul<f64> for Vec2 {
    type Output = Self;
    #[inline]
    fn mul(self, scalar: f64) -> Self {
        Self {
            x: self.x * scalar,
            y: self.y * scalar,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_between_same_point_is_zero() {
        let p = Vec2::new(100.0, 200.0);
        assert_eq!(p.distance(p), 0.0);
    }

    #[test]
    fn distance_3_4_5_triangle() {
        let a = Vec2::new(0.0, 0.0);
        let b = Vec2::new(3.0, 4.0);
        assert!((a.distance(b) - 5.0).abs() < 1e-10);
    }

    #[test]
    fn lerp_at_zero_returns_start() {
        let a = Vec2::new(10.0, 20.0);
        let b = Vec2::new(30.0, 40.0);
        let result = a.lerp(b, 0.0);
        assert_eq!(result, a);
    }

    #[test]
    fn lerp_at_one_returns_end() {
        let a = Vec2::new(10.0, 20.0);
        let b = Vec2::new(30.0, 40.0);
        let result = a.lerp(b, 1.0);
        assert_eq!(result, b);
    }

    #[test]
    fn lerp_at_half_returns_midpoint() {
        let a = Vec2::new(0.0, 0.0);
        let b = Vec2::new(10.0, 20.0);
        let result = a.lerp(b, 0.5);
        assert!((result.x - 5.0).abs() < 1e-10);
        assert!((result.y - 10.0).abs() < 1e-10);
    }

    #[test]
    fn normalized_unit_vector() {
        let v = Vec2::new(3.0, 4.0);
        let n = v.normalized();
        assert!((n.length() - 1.0).abs() < 1e-10);
        assert!((n.x - 0.6).abs() < 1e-10);
        assert!((n.y - 0.8).abs() < 1e-10);
    }

    #[test]
    fn normalized_zero_vector_is_zero() {
        let v = Vec2::new(0.0, 0.0);
        let n = v.normalized();
        assert_eq!(n, Vec2::default());
    }

    #[test]
    fn arithmetic_operations() {
        let a = Vec2::new(1.0, 2.0);
        let b = Vec2::new(3.0, 4.0);

        let sum = a + b;
        assert_eq!(sum, Vec2::new(4.0, 6.0));

        let diff = b - a;
        assert_eq!(diff, Vec2::new(2.0, 2.0));

        let scaled = a * 3.0;
        assert_eq!(scaled, Vec2::new(3.0, 6.0));
    }
}
