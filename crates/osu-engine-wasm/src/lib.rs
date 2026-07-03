//! WASM bindings for osu-engine.
//!
//! This crate is a thin wrapper that exposes `osu-engine`'s API
//! via `wasm-bindgen` for browser consumption.
//!
//! ## Design
//!
//! - No game logic lives here — all logic is in `osu-engine`
//! - Types crossing the WASM boundary use `serde-wasm-bindgen`
//! - Opaque handles are managed via the HandleArena pattern (ADR-007)
//!
//! ## Status: Stub — implementation in L7

use wasm_bindgen::prelude::*;

/// Returns the engine version as a JSON string.
#[wasm_bindgen]
pub fn engine_version() -> String {
    let v = osu_engine::version::version();
    format!(
        r#"{{"api":"{}","snapshot_schema":{},"golden_dataset":"{}","beatmap_parser":{},"replay_parser":{},"git_hash":"{}"}}"#,
        v.api, v.snapshot_schema, v.golden_dataset, v.beatmap_parser, v.replay_parser, v.git_hash
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_returns_valid_json() {
        let json = engine_version();
        assert!(json.contains("\"api\""));
        assert!(json.contains("0.1.0"));
    }
}
