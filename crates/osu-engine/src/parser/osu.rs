//! `.osu` beatmap text parser.
//!
//! Parses the osu! beatmap file format (INI-like sections) into
//! a structured `ParsedBeatmap`.
//!
//! ## Sections Parsed
//!
//! | Section | Key fields |
//! |---|---|
//! | `[General]` | AudioFilename, AudioLeadIn, Mode |
//! | `[Metadata]` | Title, Artist, Creator, Version |
//! | `[Difficulty]` | AR, CS, OD, HP, SliderMultiplier, SliderTickRate |
//! | `[TimingPoints]` | Time, BeatLength, Uninherited, Velocity |
//! | `[HitObjects]` | x, y, time, type bitmask, slider params |
//!
//! ## Reference
//!
//! - Primary: `osu/Beatmaps/Formats/LegacyBeatmapDecoder.cs`
//! - Secondary: `danser-go/beatmap/parser.go` (395 lines)
//!
//! ## Status: Stub — implementation in L2

use crate::error::EngineResult;
use crate::model::beatmap::ParsedBeatmap;

/// Parses a `.osu` beatmap file from a UTF-8 string.
///
/// Returns `EngineError` for any parse failure — never panics.
///
/// # Stub
/// Returns `EngineError::UnsupportedVersion` until L2 implementation.
pub fn parse_osu(_data: &str) -> EngineResult<ParsedBeatmap> {
    // TODO(L2): Implement .osu text parser
    Err(crate::error::EngineError::UnsupportedVersion { version: 0 })
}
