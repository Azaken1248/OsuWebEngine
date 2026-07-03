//! L3: Immutable Data Model — type-safe parsed data structures.
//!
//! All types in this module are immutable after construction. They are
//! the shared contract between the parser (L2) and all downstream
//! pipeline stages (L4–L6).

pub mod beatmap;
pub mod hit_object;
pub mod mods;
pub mod replay;
