//! # osu-engine
//!
//! A Rust behavioral reimplementation of osu! Standard mode game logic,
//! designed to compile to WebAssembly for browser-native replay analysis.
//!
//! ## Architecture
//!
//! The engine is organized as a dependency-layer pipeline (ADR-020):
//!
//! - **L1 — `math`**: Core geometric primitives (Vec2, curves, arc-length tables)
//! - **L2 — `parser`**: `.osr` replay and `.osu` beatmap file parsers
//! - **L3 — `model`**: Immutable data model types (ParsedBeatmap, ParsedReplay, ModSet)
//! - **L4 — `preprocess`**: Mod application, stacking, curve precomputation
//! - **L5 — `pipeline`**: JudgementTimeline, ScoreTimeline, VisibilityTimeline
//! - **L6 — `engine`**: GameEngine façade, `query(t)` → StateSnapshot
//!
//! ## Guiding Principle
//!
//! > This is a behavioral reimplementation of osu!lazer Standard mode.
//! > When documentation, formulas, or community understanding conflict
//! > with observed lazer behavior, **lazer behavior wins**.
//!
//! The C# source code of [ppy/osu](https://github.com/ppy/osu) is the
//! executable specification for this project.

// ── L1: Core Math ──────────────────────────────────────────────────────────
pub mod math;

// ── L2: Serialization (Pipeline Stage 1) ───────────────────────────────────
pub mod parser;

// ── L3: Immutable Data Model ───────────────────────────────────────────────
pub mod model;

// ── L4: Preprocessor (Pipeline Stage 2) ────────────────────────────────────
pub mod preprocess;

// ── L5: Timeline Pipelines (Stages 3–5) ────────────────────────────────────
pub mod pipeline;

// ── L6: Query Engine (Stage 6 — Façade) ────────────────────────────────────
pub mod engine;

// ── Cross-Cutting ──────────────────────────────────────────────────────────
pub mod error;
pub mod version;
