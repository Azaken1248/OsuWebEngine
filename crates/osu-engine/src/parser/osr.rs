//! `.osr` replay binary parser.
//!
//! Parses the osu! replay file format into a structured `ParsedReplay`.
//! Handles all versions ≥ 20131216.
//!
//! ## Binary Format
//!
//! The `.osr` format consists of:
//! 1. Header fields (game mode, version, hashes, player name, hit counts)
//! 2. LZMA-compressed cursor stream (delta-coded frames)
//! 3. Optional additional data (score ID for ≥ 2018 formats)
//!
//! ## Reference
//!
//! - Primary: `osu-reverse-mapper/script.js` L862–948 (binary encoding)
//! - Format spec: osu! wiki `.osr` format page
//!
//! ## Status: Stub — implementation in L2

use crate::error::EngineResult;
use crate::model::replay::ParsedReplay;

/// Parses a `.osr` replay file from raw bytes.
///
/// Returns `EngineError` for any parse failure — never panics.
///
/// # Stub
/// Returns `EngineError::UnsupportedVersion` until L2 implementation.
pub fn parse_osr(_data: &[u8]) -> EngineResult<ParsedReplay> {
    // TODO(L2): Implement .osr binary parser
    Err(crate::error::EngineError::UnsupportedVersion { version: 0 })
}
