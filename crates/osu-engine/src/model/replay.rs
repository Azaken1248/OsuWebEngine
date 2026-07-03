//! Parsed replay data model.
//!
//! `ParsedReplay` is the immutable output of the `.osr` parser.

use serde::{Deserialize, Serialize};

/// A fully parsed `.osr` replay file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedReplay {
    /// Game mode (must be 0 for Standard).
    pub mode: u8,

    /// Replay format version.
    pub version: i32,

    /// MD5 hash of the beatmap this replay is for.
    pub beatmap_hash: String,

    /// Player name.
    pub player_name: String,

    /// MD5 hash of the replay data.
    pub replay_hash: String,

    /// Count of 300s.
    pub count_300: u16,

    /// Count of 100s.
    pub count_100: u16,

    /// Count of 50s.
    pub count_50: u16,

    /// Count of gekis (300 with all nested objects hit).
    pub count_geki: u16,

    /// Count of katus (100 with some nested objects hit).
    pub count_katu: u16,

    /// Count of misses.
    pub count_miss: u16,

    /// Total score.
    pub total_score: u32,

    /// Maximum combo achieved.
    pub max_combo: u16,

    /// Whether the play was a perfect combo (no combo breaks).
    pub perfect: bool,

    /// Mod bitmask.
    pub mods: u32,

    /// Life bar graph data (typically unused by analysis).
    pub life_bar: String,

    /// Unix timestamp of the play.
    pub timestamp: i64,

    /// Cursor frames, sorted by absolute time.
    pub frames: Vec<ReplayFrame>,

    /// Online score ID (for replays ≥ 2018 format).
    pub score_id: Option<u64>,
}

/// A single cursor frame from the replay's cursor stream.
///
/// Frames are delta-coded in the `.osr` file format:
/// `Δt|x|y|key_flags`, separated by commas.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ReplayFrame {
    /// Absolute time in milliseconds (accumulated from deltas).
    pub time: f64,

    /// Cursor X position in osu! pixels (0–512).
    pub x: f32,

    /// Cursor Y position in osu! pixels (0–384).
    pub y: f32,

    /// Key state bitmask.
    /// - Bit 0 (0x01): M1 (mouse left)
    /// - Bit 1 (0x02): M2 (mouse right)
    /// - Bit 2 (0x04): K1 (keyboard key 1)
    /// - Bit 3 (0x08): K2 (keyboard key 2)
    /// - Bit 4 (0x10): Smoke
    pub keys: u8,
}

impl ReplayFrame {
    /// Returns true if any action key (M1, M2, K1, K2) is pressed.
    pub fn any_key_pressed(&self) -> bool {
        self.keys & 0x0F != 0
    }

    /// Returns true if key K1 is pressed.
    pub fn k1(&self) -> bool {
        self.keys & 0x04 != 0
    }

    /// Returns true if key K2 is pressed.
    pub fn k2(&self) -> bool {
        self.keys & 0x08 != 0
    }

    /// Returns true if mouse M1 is pressed.
    pub fn m1(&self) -> bool {
        self.keys & 0x01 != 0
    }

    /// Returns true if mouse M2 is pressed.
    pub fn m2(&self) -> bool {
        self.keys & 0x02 != 0
    }
}
