//! L6: Query Engine — GameEngine façade and `query(t)` → StateSnapshot.
//!
//! The query engine is the top-level interface. It chains all pipeline
//! stages during `create()` and delegates to `SnapshotBuilder` during
//! `query(t)`.
//!
//! ## Architecture
//!
//! - `GameEngine::create()` chains L2 → L4 → L5 (construction phase)
//! - `GameEngine::query(t)` delegates to `SnapshotBuilder` (query phase)
//! - `query(t)` is a **pure function** — no mutation, O(log n), allocation-free
//!
//! ## Status: Stubs — implementation in L6

pub mod index;
pub mod query;
pub mod snapshot;
