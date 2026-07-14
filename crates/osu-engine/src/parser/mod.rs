//! L2: Serialization — `.osr` replay and `.osu` beatmap parsers.
//!
//! Pipeline Stage 1: converts raw file bytes into typed Rust structures.
//!
//! ## Components
//!
//! - [`binary`]: bounds-checked little-endian reader (ULEB128, osu-strings)
//! - [`lzma`]: LZMA decompression with a 256 MB output cap
//! - [`osr`]: binary `.osr` replay parser (header + compressed cursor stream)
//! - [`osu`]: text `.osu` beatmap parser (INI-like sections)
//!
//! ## Security
//!
//! Both parsers consume untrusted, user-supplied files, so "never panics" is a
//! security property here rather than a robustness nicety (Security Threat
//! Model: untrusted parser input; BRD §14.1). Every failure is a typed
//! `EngineError`. This is enforced three ways:
//!
//! - property tests asserting totality over generated input,
//! - integration tests that truncate and bit-flip real fixture files,
//! - libFuzzer targets, required to run 10 minutes clean (see `fuzz/`).
//!
//! The two allocation-driven attack surfaces — LZMA's unbounded expansion ratio
//! and ULEB128's unbounded declared length — are both explicitly capped.
//!
//! ## Fidelity
//!
//! These parsers reproduce osu!lazer's decoders, including a number of
//! osu!stable quirks that the format itself does not describe. Several details
//! contradict TDD §2–§3; where they do, lazer wins (BRD §7.1) and the divergence
//! is documented at the point of implementation. See the L2 plan for the full
//! list.

pub mod binary;
pub mod lzma;
pub mod osr;
pub mod osu;
