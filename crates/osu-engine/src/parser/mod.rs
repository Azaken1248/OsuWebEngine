//! L2: Serialization — `.osr` replay and `.osu` beatmap parsers.
//!
//! Pipeline Stage 1: Converts raw file bytes into typed Rust structures.
//!
//! ## Components
//!
//! - [`osr`]: Binary `.osr` replay parser (LZMA-compressed cursor stream)
//! - [`osu`]: Text `.osu` beatmap parser (INI-like sections)
//! - [`lzma`]: LZMA decompression wrapper with 256 MB output cap
//!
//! ## Security
//!
//! All parsers return `Result<T, EngineError>` — no panics on malformed
//! input. See BRD §14.1 and Security Threat Model for hardening details.

pub mod lzma;
pub mod osr;
pub mod osu;
