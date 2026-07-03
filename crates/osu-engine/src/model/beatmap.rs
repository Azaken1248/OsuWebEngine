//! Parsed beatmap data model.
//!
//! `ParsedBeatmap` is the immutable output of the `.osu` parser. It contains
//! all information needed by downstream pipeline stages.

use super::hit_object::HitObject;
use serde::{Deserialize, Serialize};

/// A fully parsed `.osu` beatmap file.
///
/// Immutable after construction — all fields are read-only.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedBeatmap {
    /// Format version (e.g., 14 for `osu file format v14`).
    pub format_version: i32,

    // ── [General] ──────────────────────────────────────────────────────
    /// Audio filename referenced by this beatmap.
    pub audio_filename: String,

    /// Audio lead-in time in milliseconds.
    pub audio_lead_in: f64,

    /// Game mode (must be 0 for Standard).
    pub mode: u8,

    /// Stack leniency (0.0–1.0). Controls stacking behavior.
    /// Source: `StackLeniency` in `[General]`.
    pub stack_leniency: f64,

    // ── [Metadata] ─────────────────────────────────────────────────────
    /// Song title.
    pub title: String,

    /// Song artist.
    pub artist: String,

    /// Beatmap creator username.
    pub creator: String,

    /// Difficulty name (e.g., "Insane", "Expert").
    pub version: String,

    /// MD5 hash of the `.osu` file content.
    pub beatmap_hash: String,

    // ── [Difficulty] ───────────────────────────────────────────────────
    /// HP Drain Rate (0–10).
    pub hp: f64,

    /// Circle Size (0–10). Determines hit circle radius.
    pub cs: f64,

    /// Overall Difficulty (0–10). Determines hit windows.
    pub od: f64,

    /// Approach Rate (0–10). Determines when objects appear.
    pub ar: f64,

    /// Base slider speed multiplier.
    pub slider_multiplier: f64,

    /// Slider tick rate (ticks per beat).
    pub slider_tick_rate: f64,

    // ── [TimingPoints] ─────────────────────────────────────────────────
    /// All timing points (both uninherited and inherited), sorted by time.
    pub timing_points: Vec<TimingPoint>,

    // ── [HitObjects] ───────────────────────────────────────────────────
    /// All hit objects, sorted by start time.
    pub hit_objects: Vec<HitObject>,
}

/// A timing point (red or green line) from the `[TimingPoints]` section.
///
/// - **Uninherited** (red line): Sets BPM via `beat_length` (ms per beat).
/// - **Inherited** (green line): Sets slider velocity multiplier via
///   negative `beat_length` (= `-100 / multiplier`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimingPoint {
    /// Time in milliseconds when this timing point takes effect.
    pub time: f64,

    /// Beat length in milliseconds.
    /// - Positive: BPM = `60000 / beat_length` (uninherited)
    /// - Negative: Velocity multiplier = `-100 / beat_length` (inherited)
    pub beat_length: f64,

    /// Whether this is an uninherited (red) timing point.
    pub uninherited: bool,

    /// Time signature numerator (e.g., 4 for 4/4 time).
    pub meter: u8,

    /// Sample set for this section (0 = auto, 1 = normal, 2 = soft, 3 = drum).
    pub sample_set: u8,

    /// Custom sample index.
    pub sample_index: u8,

    /// Volume percentage (0–100).
    pub volume: u8,

    /// Effect flags (bit 0 = kiai, bit 3 = omit first barline).
    pub effects: u8,
}
