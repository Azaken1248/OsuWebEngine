//! Integration tests: parse the committed fixtures as real files off disk.
//!
//! The unit tests build their inputs in-process, which risks the parser and its
//! test fixtures agreeing on a format that no file actually has. These tests
//! read the bytes from `tests/fixtures/` through the public API, so a mistake in
//! the layout shows up here even if the unit tests are self-consistent.

use osu_engine::error::EngineError;
use osu_engine::math::curves::CurveType;
use osu_engine::model::hit_object::HitObjectKind;
use osu_engine::parser::{osr::parse_osr, osu::parse_osu};

use std::path::PathBuf;

fn fixture(rel: &str) -> PathBuf {
    // CARGO_MANIFEST_DIR is crates/osu-engine; fixtures live at the repo root.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(rel)
}

fn read(rel: &str) -> Vec<u8> {
    std::fs::read(fixture(rel)).unwrap_or_else(|e| panic!("missing fixture {rel}: {e}"))
}

fn read_text(rel: &str) -> String {
    String::from_utf8(read(rel)).expect("fixture should be valid UTF-8")
}

// ── .osu ─────────────────────────────────────────────────────────────────────

#[test]
fn parses_normal_beatmap_from_disk() {
    let b = parse_osu(&read_text("beatmaps/normal.osu")).expect("normal.osu should parse");

    assert_eq!(b.format_version, 14);
    assert_eq!(b.title, "Test Song");
    assert_eq!(b.mode, 0);
    assert_eq!(b.cs, 4.0);
    assert_eq!(b.ar, 9.5);

    assert_eq!(b.timing_points.len(), 2);
    assert!(b.timing_points[0].uninherited, "1500 is a red line");
    assert!(!b.timing_points[1].uninherited, "3000 is a green line");

    // circle, bezier, perfect, catmull, linear, spinner
    assert_eq!(b.hit_objects.len(), 6);

    let kinds: Vec<&str> = b
        .hit_objects
        .iter()
        .map(|o| match &o.kind {
            HitObjectKind::Circle => "circle",
            HitObjectKind::Slider(_) => "slider",
            HitObjectKind::Spinner(_) => "spinner",
        })
        .collect();

    assert_eq!(
        kinds,
        ["circle", "slider", "slider", "slider", "slider", "spinner"]
    );

    // Objects come out sorted and re-indexed.
    for (i, o) in b.hit_objects.iter().enumerate() {
        assert_eq!(o.index, i);
        if i > 0 {
            assert!(b.hit_objects[i - 1].time <= o.time);
        }
    }
}

#[test]
fn every_curve_type_survives_a_round_trip_from_disk() {
    let b = parse_osu(&read_text("beatmaps/normal.osu")).unwrap();

    let curves: Vec<CurveType> = b
        .hit_objects
        .iter()
        .filter_map(|o| match &o.kind {
            HitObjectKind::Slider(s) => Some(s.curve_type),
            _ => None,
        })
        .collect();

    assert_eq!(
        curves,
        [
            CurveType::Bezier,
            CurveType::PerfectArc,
            CurveType::CatmullRom,
            CurveType::Linear,
        ]
    );
}

/// The +24 ms early-version offset, verified through a real v4 file.
#[test]
fn early_version_beatmap_carries_the_24ms_offset() {
    let b = parse_osu(&read_text("beatmaps/early_v4.osu")).unwrap();

    assert_eq!(b.format_version, 4);
    assert_eq!(b.hit_objects[0].time, 1024.0, "object time missing +24ms");
    assert_eq!(b.timing_points[0].time, 1024.0, "timing missing +24ms");
}

/// A collinear `P` curve is rewritten to Linear at parse time on legacy maps.
#[test]
fn collinear_perfect_curve_is_linear_on_disk() {
    let b = parse_osu(&read_text("beatmaps/collinear_p.osu")).unwrap();

    let HitObjectKind::Slider(s) = &b.hit_objects[0].kind else {
        panic!("expected a slider");
    };

    assert_eq!(s.curve_type, CurveType::Linear);
}

// ── .osr ─────────────────────────────────────────────────────────────────────

#[test]
fn parses_nomod_replay_from_disk() {
    let r = parse_osr(&read("replays/nomod_fc.osr"), Some(14)).expect("nomod_fc.osr should parse");

    assert_eq!(r.mode, 0);
    assert_eq!(r.version, 20230326);
    assert_eq!(r.player_name, "PlayerOne");
    assert_eq!(r.beatmap_hash, "d41d8cd98f00b204e9800998ecf8427e");
    assert_eq!(r.count_300, 300);
    assert_eq!(r.count_miss, 3);
    assert_eq!(r.max_combo, 850);
    assert!(r.perfect);
    assert_eq!(r.mods, 0);
    assert_eq!(r.score_id, Some(555_000_111));

    // The fixture's stream is: 2 sentinels + 3 real frames + seed.
    // Sentinels and seed must all be gone.
    assert_eq!(r.frames.len(), 3, "sentinel or seed frames leaked through");
    assert_eq!(r.frames[0].x, 100.0);
    assert_eq!(r.frames[2].keys, 5);

    for w in r.frames.windows(2) {
        assert!(w[0].time <= w[1].time, "frame times must be monotonic");
    }
}

#[test]
fn parses_mod_bitmask_from_disk() {
    let r = parse_osr(&read("replays/dthr_fc.osr"), Some(14)).unwrap();
    // Hidden (8) | HardRock (16) | DoubleTime (64)
    assert_eq!(r.mods, 88);
}

/// The i32 score-ID window. A parser that assumed a fixed `i64` — as TDD §2.1
/// specifies — would read past the end of this file.
#[test]
fn old_2013_replay_uses_an_i32_score_id() {
    let r = parse_osr(&read("replays/old_format_2013.osr"), Some(14)).unwrap();

    assert_eq!(r.version, 20130101);
    assert_eq!(r.score_id, Some(4242));
    assert_eq!(r.frames.len(), 2);
}

/// Pre-2012 replays carry no score ID at all.
#[test]
fn ancient_replay_has_no_score_id() {
    let r = parse_osr(&read("replays/ancient_2011.osr"), Some(14)).unwrap();

    assert_eq!(r.version, 20111001);
    assert_eq!(r.score_id, None);
    assert_eq!(r.frames.len(), 1);
}

/// A v4 beatmap shifts replay frames by the same +24 ms as its hit objects.
/// If these two ever disagree, old maps desync — so pin them together.
#[test]
fn replay_and_beatmap_offsets_agree_on_early_versions() {
    let beatmap = parse_osu(&read_text("beatmaps/early_v4.osu")).unwrap();
    let replay = parse_osr(
        &read("replays/ancient_2011.osr"),
        Some(beatmap.format_version),
    )
    .unwrap();

    // The fixture's single frame has delta 16; with the v4 offset it lands at 40.
    assert_eq!(replay.frames[0].time, 40.0);

    // Without the offset it would be 16 — confirming the offset is what moved it.
    let no_offset = parse_osr(&read("replays/ancient_2011.osr"), Some(14)).unwrap();
    assert_eq!(no_offset.frames[0].time, 16.0);
}

// ── Robustness on real bytes ────────────────────────────────────────────────

#[test]
fn truncating_a_real_replay_never_panics() {
    let full = read("replays/nomod_fc.osr");

    // Every prefix of a valid replay must parse or error — never panic.
    for n in 0..full.len() {
        let _ = parse_osr(&full[..n], None);
    }
}

#[test]
fn truncating_a_real_beatmap_never_panics() {
    let full = read_text("beatmaps/normal.osu");

    for n in 0..full.len() {
        // Slice on a char boundary to keep the input valid UTF-8.
        if full.is_char_boundary(n) {
            let _ = parse_osu(&full[..n]);
        }
    }
}

#[test]
fn bit_flips_in_a_real_replay_never_panic() {
    let full = read("replays/nomod_fc.osr");

    for i in 0..full.len() {
        for bit in 0..8u32 {
            let mut corrupt = full.clone();
            corrupt[i] ^= 1 << bit;
            let _ = parse_osr(&corrupt, None);
        }
    }
}

#[test]
fn a_taiko_replay_is_rejected() {
    let mut data = read("replays/nomod_fc.osr");
    data[0] = 1; // mode 1 = taiko

    assert!(matches!(
        parse_osr(&data, None),
        Err(EngineError::InvalidGameMode { mode: 1, .. })
    ));
}
