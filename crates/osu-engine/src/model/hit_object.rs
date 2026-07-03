//! Hit object types: Circle, Slider, Spinner.
//!
//! osu! Standard mode has three hit object types, distinguished by a
//! bitmask in the type byte of the `[HitObjects]` section:
//! - Bit 0 (0x01): Hit circle
//! - Bit 1 (0x02): Slider
//! - Bit 3 (0x08): Spinner
//! - Bit 2 (0x04): New combo flag
//! - Bits 4–6: Combo color skip count

use crate::math::curves::CurveType;
use crate::math::vec2::Vec2;
use serde::{Deserialize, Serialize};

/// A parsed hit object from the `[HitObjects]` section.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HitObject {
    /// Zero-based index in the beatmap's hit object list.
    pub index: usize,

    /// Position in osu! pixel coordinates (0–512 × 0–384).
    pub x: f64,
    pub y: f64,

    /// Start time in milliseconds.
    pub time: f64,

    /// Raw type bitmask byte.
    pub type_flags: u8,

    /// Hit sound flags.
    pub hit_sound: u8,

    /// Whether this object starts a new combo.
    pub new_combo: bool,

    /// Number of combo colors to skip (from type byte bits 4–6).
    pub combo_color_skip: u8,

    /// Object-specific data.
    pub kind: HitObjectKind,

    // ── Computed during preprocessing (L4) ────────────────────────────
    /// Stack height assigned by the stacking algorithm.
    /// Set to 0 initially; computed during L4 preprocessing.
    pub stack_height: i32,
}

/// Type-specific data for each hit object kind.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HitObjectKind {
    /// A hit circle — tap at the right time.
    Circle,

    /// A slider — follow the ball along a curve.
    Slider(SliderData),

    /// A spinner — spin the cursor around the center.
    Spinner(SpinnerData),
}

/// Slider-specific data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SliderData {
    /// Curve type for this slider's path.
    pub curve_type: CurveType,

    /// Control points (including the hit object's position as the first point).
    pub control_points: Vec<Vec2>,

    /// Number of repeats (1 = no repeat, 2 = one repeat, etc.).
    pub repeat_count: u32,

    /// Pixel length of the slider path.
    pub pixel_length: f64,

    /// Computed end time (set during preprocessing).
    pub end_time: f64,
}

/// Spinner-specific data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpinnerData {
    /// End time of the spinner in milliseconds.
    pub end_time: f64,
}

impl HitObject {
    /// Returns the position as a `Vec2`.
    pub fn position(&self) -> Vec2 {
        Vec2::new(self.x, self.y)
    }

    /// Returns true if this is a hit circle.
    pub fn is_circle(&self) -> bool {
        matches!(self.kind, HitObjectKind::Circle)
    }

    /// Returns true if this is a slider.
    pub fn is_slider(&self) -> bool {
        matches!(self.kind, HitObjectKind::Slider(_))
    }

    /// Returns true if this is a spinner.
    pub fn is_spinner(&self) -> bool {
        matches!(self.kind, HitObjectKind::Spinner(_))
    }

    /// Returns the end time of this object.
    /// For circles, this is the same as the start time.
    pub fn end_time(&self) -> f64 {
        match &self.kind {
            HitObjectKind::Circle => self.time,
            HitObjectKind::Slider(s) => s.end_time,
            HitObjectKind::Spinner(s) => s.end_time,
        }
    }
}
