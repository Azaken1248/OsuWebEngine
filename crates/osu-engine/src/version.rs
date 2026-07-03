//! Engine version tracking with 5-dimension versioning.
//!
//! Each dimension tracks a different aspect of the engine that can
//! change independently. See API Specification §15.2.

/// 5-dimension versioning system.
///
/// Each dimension tracks a different aspect of the engine:
/// - `api`: SemVer of the engine API
/// - `snapshot_schema`: StateSnapshot format version (monotonically increasing)
/// - `golden_dataset`: Golden data corpus version tag (lazer release pin)
/// - `beatmap_parser`: Beatmap parser version (monotonically increasing)
/// - `replay_parser`: Replay parser version (monotonically increasing)
/// - `git_hash`: Short git commit hash (set at build time)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineVersion {
    /// SemVer of the engine API (from Cargo.toml).
    pub api: &'static str,

    /// Snapshot schema version (monotonically increasing integer).
    /// Incremented when `StateSnapshot` fields are added/changed.
    pub snapshot_schema: u32,

    /// Golden dataset version tag (e.g., `"lazer-2024.1115.0-r3"`).
    /// Tracks which osu!lazer release the golden data was generated from.
    pub golden_dataset: &'static str,

    /// Beatmap parser version (monotonically increasing integer).
    /// Incremented when `.osu` parsing behavior changes.
    pub beatmap_parser: u32,

    /// Replay parser version (monotonically increasing integer).
    /// Incremented when `.osr` parsing behavior changes.
    pub replay_parser: u32,

    /// Short git commit hash. Set at build time, `"dev"` for local builds.
    pub git_hash: &'static str,
}

/// Current engine version. Updated at each release.
pub const ENGINE_VERSION: EngineVersion = EngineVersion {
    api: env!("CARGO_PKG_VERSION"),
    snapshot_schema: 1,
    golden_dataset: "none",
    beatmap_parser: 1,
    replay_parser: 1,
    git_hash: "dev",
};

/// Returns the current engine version.
pub fn version() -> &'static EngineVersion {
    &ENGINE_VERSION
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_populated() {
        let v = version();
        assert!(!v.api.is_empty(), "API version must not be empty");
        assert_eq!(v.api, "0.1.0");
        assert_eq!(v.snapshot_schema, 1);
        assert_eq!(v.golden_dataset, "none");
        assert_eq!(v.beatmap_parser, 1);
        assert_eq!(v.replay_parser, 1);
    }

    #[test]
    fn version_is_cloneable() {
        let v = version().clone();
        assert_eq!(v, ENGINE_VERSION);
    }
}
