//! `.osr` replay binary parser.
//!
//! ## Binary layout
//!
//! | Type | Field |
//! |---|---|
//! | `u8` | Game mode (0 = osu!Standard) |
//! | `i32` | Game version (e.g. 20230326) |
//! | osu-string | Beatmap MD5 |
//! | osu-string | Player name |
//! | osu-string | Replay MD5 |
//! | `u16` × 6 | 300 / 100 / 50 / geki / katu / miss |
//! | `i32` | Total score |
//! | `u16` | Max combo |
//! | `u8` | Perfect flag |
//! | `i32` | Mod bitmask |
//! | osu-string | Life-bar graph |
//! | `i64` | Timestamp (Windows ticks) |
//! | byte array | LZMA-compressed frame stream |
//! | *version-dependent* | Online score ID — see [`read_score_id`] |
//!
//! ## Behavioral notes
//!
//! Several details here contradict TDD §2 and are implemented per lazer (see
//! the L2 plan, "TDD Divergences"). The frame stream in particular carries four
//! osu!stable quirks that a naive parser gets wrong on essentially every real
//! replay — see [`apply_stable_frame_quirks`].
//!
//! ## Reference
//!
//! - Specification: `osu/osu.Game/Scoring/Legacy/LegacyScoreDecoder.cs`
//! - Frame type: `osu/osu.Game/Replays/Legacy/LegacyReplayFrame.cs`
//! - Byte layout aid: `osu-reverse-mapper/script.js` L862–948

use super::binary::ByteReader;
use super::lzma;
use crate::error::{EngineError, EngineResult};
use crate::model::replay::{ParsedReplay, ReplayFrame};

/// osu! Standard's game mode byte.
const MODE_STANDARD: u8 = 0;

/// Replays at or above this version store the online score ID as an `i64`.
///
/// Source: `LegacyScoreDecoder.cs` L107.
const FIRST_I64_SCORE_ID_VERSION: i32 = 20140721;

/// Replays at or above this version store the online score ID as an `i32`.
/// Below it, the field is absent entirely.
///
/// Source: `LegacyScoreDecoder.cs` L109.
const FIRST_I32_SCORE_ID_VERSION: i32 = 20121008;

/// The delta value marking the seed frame in the stream.
///
/// Compared as a **string**, exactly as lazer does.
///
/// Source: `LegacyScoreDecoder.cs` L282.
const SEED_FRAME_MARKER: &str = "-12345";

/// Maximum coordinate magnitude lazer accepts before throwing.
///
/// Source: `Parsing.cs` L14 (`MAX_COORDINATE_VALUE`).
const MAX_COORDINATE_VALUE: f64 = 131_072.0;

/// The position stable writes for its two leading throwaway frames.
///
/// Source: `LegacyScoreDecoder.cs` L330-337.
const SENTINEL_X: f32 = 256.0;
const SENTINEL_Y: f32 = -500.0;

/// Parses a `.osr` replay file from raw bytes.
///
/// `beatmap_format_version` is the format version of the beatmap this replay
/// targets, so the early-version timing offset (24 ms for format < 5) can be
/// applied to frame times. Pass `None` when the beatmap is unknown; the offset
/// is then zero, which is correct for every beatmap of format >= 5.
///
/// Source: `LegacyScoreDecoder.cs` L98 — the offset is baked into hit object
/// timing by `LegacyBeatmapDecoder`, so replay frames must carry it too or old
/// maps desync by 24 ms.
pub fn parse_osr(data: &[u8], beatmap_format_version: Option<i32>) -> EngineResult<ParsedReplay> {
    let mut r = ByteReader::new(data);

    let mode = r.read_u8("mode")?;
    if mode != MODE_STANDARD {
        return Err(EngineError::InvalidGameMode {
            mode,
            expected: MODE_STANDARD,
        });
    }

    let version = r.read_i32("version")?;

    let beatmap_hash = r.read_osu_string("beatmap_hash")?;
    let player_name = r.read_osu_string("player_name")?;
    let replay_hash = r.read_osu_string("replay_hash")?;

    let count_300 = r.read_u16("count_300")?;
    let count_100 = r.read_u16("count_100")?;
    let count_50 = r.read_u16("count_50")?;
    let count_geki = r.read_u16("count_geki")?;
    let count_katu = r.read_u16("count_katu")?;
    let count_miss = r.read_u16("count_miss")?;

    let total_score = r.read_i32("total_score")?;
    let max_combo = r.read_u16("max_combo")?;
    let perfect = r.read_bool("perfect")?;
    let mods = r.read_i32("mods")?;

    let life_bar = r.read_osu_string("life_bar")?;
    let timestamp = r.read_i64("timestamp")?;

    let compressed = r.read_byte_array("replay_data")?;

    let score_id = read_score_id(&mut r, version)?;

    let offset = early_version_offset(beatmap_format_version);
    let frames = parse_frames(compressed, offset)?;

    Ok(ParsedReplay {
        mode,
        version,
        beatmap_hash,
        player_name,
        replay_hash,
        count_300,
        count_100,
        count_50,
        count_geki,
        count_katu,
        count_miss,
        total_score: total_score.max(0) as u32,
        max_combo,
        perfect,
        mods: mods as u32,
        life_bar,
        timestamp,
        frames,
        score_id,
    })
}

/// Reads the online score ID, whose **width depends on the replay version**.
///
/// TDD §2.1 describes this as a plain `i64` present for "≥ 2018 format". Both
/// halves are wrong: the `i64` threshold is 20140721, there is an `i32` form
/// for 20121008..20140720, and older replays omit the field entirely. Reading a
/// fixed `i64` misparses every replay in the `i32` window and runs off the end
/// of anything older.
///
/// Source: `LegacyScoreDecoder.cs` L107-110.
fn read_score_id(r: &mut ByteReader, version: i32) -> EngineResult<Option<u64>> {
    let id = if version >= FIRST_I64_SCORE_ID_VERSION {
        r.read_i64("score_id")?
    } else if version >= FIRST_I32_SCORE_ID_VERSION {
        i64::from(r.read_i32("score_id")?)
    } else {
        return Ok(None);
    };

    // lazer normalises 0 to -1 ("no online ID"); absence is modelled as `None`.
    if id <= 0 {
        Ok(None)
    } else {
        Ok(Some(id as u64))
    }
}

/// The 24 ms offset applied to beatmaps of format < 5.
///
/// Source: `LegacyBeatmapDecoder.cs` L27-29, `LegacyScoreDecoder.cs` L98.
fn early_version_offset(beatmap_format_version: Option<i32>) -> i64 {
    match beatmap_format_version {
        Some(v) if v < 5 => super::osu::EARLY_VERSION_TIMING_OFFSET,
        _ => 0,
    }
}

/// Decompresses and parses the delta-coded frame stream.
fn parse_frames(compressed: &[u8], offset: i64) -> EngineResult<Vec<ReplayFrame>> {
    if compressed.is_empty() {
        return Ok(Vec::new());
    }

    let raw = lzma::decompress_lzma(compressed)?;

    let text = std::str::from_utf8(&raw).map_err(|_| EngineError::InvalidUtf8 {
        context: "replay_frames",
        offset: 0,
    })?;

    let mut frames = parse_frame_text(text, offset)?;
    apply_stable_frame_quirks(&mut frames);

    Ok(frames)
}

/// Parses `Δt|x|y|keys,...` into absolute-timed frames.
///
/// **Deltas are integers.** stable's format does not permit fractional deltas,
/// and lazer parses them as `int` precisely to avoid floating-point
/// accumulation error across thousands of frames — it only falls back to
/// rounding a float because a window of lazer builds briefly emitted fractional
/// values. TDD §2.4's `f64` accumulation reintroduces exactly the drift the
/// integer path exists to prevent.
///
/// Source: `LegacyScoreDecoder.cs` L275-317.
fn parse_frame_text(text: &str, offset: i64) -> EngineResult<Vec<ReplayFrame>> {
    let mut frames = Vec::with_capacity(text.len() / 20);
    let mut last_time: i64 = offset;

    for segment in text.split(',') {
        let mut parts = segment.split('|');

        let (Some(t), Some(x), Some(y), Some(k)) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            // Fewer than 4 fields. lazer skips these silently, which also
            // absorbs the empty segment left by a trailing comma.
            continue;
        };

        // The seed frame. lazer *continues* past it rather than breaking, and
        // never accumulates its delta. TDD §2.4's `break` happens to work only
        // because the frame is conventionally last.
        if t == SEED_FRAME_MARKER {
            continue;
        }

        let delta = parse_delta(t)?;

        let px = parse_coordinate(x, "frame_x")?;
        let py = parse_coordinate(y, "frame_y")?;

        let keys = k
            .trim()
            .parse::<i64>()
            .map_err(|_| EngineError::MalformedField {
                field: "frame_keys",
                value: k.to_string(),
            })?;

        last_time = last_time.saturating_add(delta);

        frames.push(ReplayFrame {
            time: last_time as f64,
            x: px,
            y: py,
            keys: (keys & 0xFF) as u8,
        });
    }

    Ok(frames)
}

/// Parses a frame delta as an integer, falling back to a rounded float.
///
/// Source: `LegacyScoreDecoder.cs` L304-306.
fn parse_delta(s: &str) -> EngineResult<i64> {
    let s = s.trim();

    if let Ok(v) = s.parse::<i64>() {
        return Ok(v);
    }

    let f = s.parse::<f64>().map_err(|_| EngineError::MalformedField {
        field: "frame_delta",
        value: s.to_string(),
    })?;

    if !f.is_finite() {
        return Err(EngineError::MalformedField {
            field: "frame_delta",
            value: s.to_string(),
        });
    }

    Ok(f.round() as i64)
}

/// Parses a coordinate, enforcing lazer's `MAX_COORDINATE_VALUE` bound.
///
/// Source: `Parsing.cs` L18-28 — lazer throws rather than clamping.
fn parse_coordinate(s: &str, field: &'static str) -> EngineResult<f32> {
    let v = s
        .trim()
        .parse::<f64>()
        .map_err(|_| EngineError::MalformedField {
            field,
            value: s.to_string(),
        })?;

    if !v.is_finite() || v.abs() > MAX_COORDINATE_VALUE {
        return Err(EngineError::ValueOutOfRange {
            field,
            value: v,
            limit: MAX_COORDINATE_VALUE,
        });
    }

    Ok(v as f32)
}

/// Applies the four osu!stable replay quirks, in lazer's order.
///
/// None of these are in the TDD, and without them frame timing is wrong at the
/// start of essentially every stable-recorded replay. They exist because
/// stable's `ReplayWatcher` wrote frames the format does not really describe.
///
/// Source: `LegacyScoreDecoder.cs` L319-351, which in turn cites
/// `osu-stable-reference/ReplayWatcher.cs` L62-71.
fn apply_stable_frame_quirks(frames: &mut Vec<ReplayFrame>) {
    // 1. A second frame earlier than the first: hoist the first to zero.
    if frames.len() >= 2 && frames[1].time < frames[0].time {
        frames[1].time = frames[0].time;
        frames[0].time = 0.0;
    }

    // 2. A first frame later than the third: flatten the first two onto it.
    if frames.len() >= 3 && frames[0].time > frames[2].time {
        let t = frames[2].time;
        frames[0].time = t;
        frames[1].time = t;
    }

    // 3. Drop stable's two leading sentinel frames, both at (256, -500).
    //    Index 1 goes first, so removing index 0 does not shift it underneath us.
    if frames.len() >= 2 && is_sentinel(&frames[1]) {
        frames.remove(1);
    }
    if !frames.is_empty() && is_sentinel(&frames[0]) {
        frames.remove(0);
    }

    // 4. Never allow time to run backwards. Frames with a negative delta are
    //    dropped rather than reordered — lazer notes this differs slightly from
    //    stable (which interpolates an intermediate frame), and we match lazer.
    let mut current: Option<f64> = None;
    frames.retain(|f| match current {
        Some(t) if f.time < t => false,
        _ => {
            current = Some(f.time);
            true
        }
    });
}

/// True when a frame sits at stable's throwaway sentinel position.
fn is_sentinel(f: &ReplayFrame) -> bool {
    f.x == SENTINEL_X && f.y == SENTINEL_Y
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frames_of(text: &str) -> Vec<ReplayFrame> {
        let mut f = parse_frame_text(text, 0).expect("frame text should parse");
        apply_stable_frame_quirks(&mut f);
        f
    }

    // ── Frame stream ──────────────────────────────────────────────────────

    // --- UT-OSR-005 / UT-OSR-006: frame count + monotonicity ---
    #[test]
    fn ut_osr_005_006_frames_parse_and_are_monotonic() {
        let f = frames_of("10|100|200|0,16|110|210|1,16|120|220|5");

        assert_eq!(f.len(), 3);
        assert_eq!(f[0].time, 10.0);
        assert_eq!(f[1].time, 26.0);
        assert_eq!(f[2].time, 42.0);

        for w in f.windows(2) {
            assert!(w[0].time <= w[1].time, "frame times must be non-decreasing");
        }

        assert_eq!(f[0].x, 100.0);
        assert_eq!(f[2].keys, 5);
    }

    // --- D4: the seed frame is skipped, not terminating ---
    #[test]
    fn d4_seed_frame_is_skipped_not_terminating() {
        // A frame *after* the seed marker must still be parsed. TDD §2.4's
        // `break` would silently drop it.
        let f = frames_of("10|100|200|0,-12345|0|0|9876,16|110|210|1");

        assert_eq!(f.len(), 2, "frame after the seed marker was dropped");
        assert_eq!(f[0].time, 10.0);
        assert_eq!(f[1].time, 26.0, "seed delta leaked into frame timing");
    }

    // --- D3: deltas are integers ---
    #[test]
    fn d3_fractional_deltas_are_rounded_to_integers() {
        let f = frames_of("16.7|100|200|0,16.2|110|210|0");

        assert_eq!(f[0].time, 17.0);
        assert_eq!(f[1].time, 33.0); // 17 + 16
    }

    #[test]
    fn d3_integer_deltas_do_not_accumulate_drift() {
        // 10_000 frames of delta 1 must land exactly on 10_000.
        let text = "1|0|0|0,".repeat(10_000);
        let f = frames_of(text.trim_end_matches(','));

        assert_eq!(f.len(), 10_000);
        assert_eq!(f.last().map(|fr| fr.time), Some(10_000.0));
    }

    // --- D5: early-version beatmap offset ---
    #[test]
    fn d5_early_version_offset_shifts_frame_times() {
        let f = parse_frame_text("10|100|200|0", 24).unwrap();
        assert_eq!(f[0].time, 34.0, "24 ms early-version offset not applied");

        assert_eq!(early_version_offset(Some(4)), 24);
        assert_eq!(early_version_offset(Some(5)), 0);
        assert_eq!(early_version_offset(Some(14)), 0);
        assert_eq!(early_version_offset(None), 0);
    }

    // --- D7.3: sentinel frame removal ---
    #[test]
    fn d7_sentinel_frames_are_removed() {
        let f = frames_of("0|256|-500|0,10|256|-500|0,16|100|200|1");

        assert_eq!(f.len(), 1, "sentinel frames were not removed");
        assert_eq!(f[0].x, 100.0);
    }

    #[test]
    fn d7_single_leading_sentinel_is_removed() {
        let f = frames_of("0|256|-500|0,16|100|200|1");
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].x, 100.0);
    }

    // --- D7.4: backwards time traversal is dropped ---
    #[test]
    fn d7_backwards_frames_are_dropped() {
        // The backwards frame is placed at index 3 so that quirk 2 (which only
        // inspects indices 0..2) does not fire first and repair the timeline —
        // this isolates quirk 4.
        //
        // Times: 100, 200, 300, 150, 250. The last two both fall behind 300.
        let f = frames_of("100|10|10|0,100|20|20|0,100|30|30|0,-150|40|40|0,100|50|50|0");

        for w in f.windows(2) {
            assert!(w[0].time <= w[1].time, "time ran backwards: {:?}", f);
        }
        assert!(
            !f.iter().any(|fr| fr.x == 40.0),
            "backwards frame was retained"
        );
        assert_eq!(f.len(), 3, "expected the two trailing frames to be dropped");
    }

    /// Quirk 2 repairs a leading out-of-order run rather than dropping it, so
    /// no frame is lost in that case. Pins the interaction that made the first
    /// draft of the test above wrong.
    #[test]
    fn d7_quirk2_repairs_rather_than_drops() {
        // Times: 100, 200, 50 -> quirk 2 flattens the first two onto 50.
        let f = frames_of("100|10|10|0,100|20|20|0,-150|30|30|0");

        assert_eq!(f.len(), 3, "quirk 2 must repair, not drop");
        assert_eq!(f[0].time, 50.0);
        assert_eq!(f[1].time, 50.0);
        assert_eq!(f[2].time, 50.0);
    }

    #[test]
    fn malformed_segments_are_skipped_not_fatal() {
        let f = frames_of("10|100|200|0,garbage,16|110|210|0,");
        assert_eq!(f.len(), 2);
    }

    #[test]
    fn out_of_range_coordinate_is_rejected() {
        assert!(matches!(
            parse_frame_text("10|999999999|200|0", 0),
            Err(EngineError::ValueOutOfRange { .. })
        ));
    }

    // ── Score ID width (D1) ───────────────────────────────────────────────

    #[test]
    fn d1_score_id_width_depends_on_version() {
        // >= 20140721 -> i64
        let data = 999i64.to_le_bytes();
        let mut r = ByteReader::new(&data);
        assert_eq!(read_score_id(&mut r, 20140721).unwrap(), Some(999));
        assert_eq!(r.remaining(), 0, "i64 form must consume 8 bytes");

        // 20121008..20140720 -> i32
        let data = 777i32.to_le_bytes();
        let mut r = ByteReader::new(&data);
        assert_eq!(read_score_id(&mut r, 20130101).unwrap(), Some(777));
        assert_eq!(r.remaining(), 0, "i32 form must consume 4 bytes");

        // < 20121008 -> absent, consumes nothing
        let data = [0xAAu8; 8];
        let mut r = ByteReader::new(&data);
        assert_eq!(read_score_id(&mut r, 20120101).unwrap(), None);
        assert_eq!(r.remaining(), 8, "old versions must not consume a score ID");
    }

    #[test]
    fn zero_score_id_is_absent() {
        let data = 0i64.to_le_bytes();
        let mut r = ByteReader::new(&data);
        assert_eq!(read_score_id(&mut r, 20140721).unwrap(), None);
    }

    // ── Full-header round trip ────────────────────────────────────────────

    /// Builds a synthetic but byte-accurate `.osr`, so the header tests
    /// exercise the real reader rather than a hand-waved slice.
    pub(super) struct OsrBuilder {
        buf: Vec<u8>,
    }

    impl OsrBuilder {
        pub(super) fn new(version: i32, mods: i32) -> Self {
            let mut b = OsrBuilder { buf: Vec::new() };
            b.buf.push(MODE_STANDARD);
            b.buf.extend_from_slice(&version.to_le_bytes());
            b.string("beatmap-md5-hash");
            b.string("PlayerOne");
            b.string("replay-md5-hash");
            for n in [300u16, 10, 5, 42, 7, 3] {
                b.buf.extend_from_slice(&n.to_le_bytes());
            }
            b.buf.extend_from_slice(&1_234_567i32.to_le_bytes()); // total score
            b.buf.extend_from_slice(&850u16.to_le_bytes()); // max combo
            b.buf.push(1); // perfect
            b.buf.extend_from_slice(&mods.to_le_bytes());
            b.string("0|1,1000|0.9");
            b.buf
                .extend_from_slice(&638_000_000_000_000_000i64.to_le_bytes());
            b
        }

        fn string(&mut self, s: &str) {
            self.buf.push(0x0B);
            let mut len = s.len();
            while len >= 0x80 {
                self.buf.push((len as u8 & 0x7F) | 0x80);
                len >>= 7;
            }
            self.buf.push(len as u8);
            self.buf.extend_from_slice(s.as_bytes());
        }

        /// Appends the LZMA-compressed frame stream and the version-appropriate
        /// score ID.
        pub(super) fn finish(mut self, frames: &str, version: i32, score_id: i64) -> Vec<u8> {
            let mut compressed = Vec::new();
            lzma_rs::lzma_compress(
                &mut std::io::Cursor::new(frames.as_bytes()),
                &mut compressed,
            )
            .expect("compression should succeed");

            self.buf
                .extend_from_slice(&(compressed.len() as i32).to_le_bytes());
            self.buf.extend_from_slice(&compressed);

            if version >= FIRST_I64_SCORE_ID_VERSION {
                self.buf.extend_from_slice(&score_id.to_le_bytes());
            } else if version >= FIRST_I32_SCORE_ID_VERSION {
                self.buf.extend_from_slice(&(score_id as i32).to_le_bytes());
            }

            self.buf
        }
    }

    // --- UT-OSR-001: valid nomod replay, all header fields ---
    #[test]
    fn ut_osr_001_valid_nomod_replay() {
        let version = 20230326;
        let data = OsrBuilder::new(version, 0).finish("10|100|200|0,16|110|210|1", version, 555);

        let r = parse_osr(&data, None).expect("replay should parse");

        assert_eq!(r.mode, 0);
        assert_eq!(r.version, version);
        assert_eq!(r.beatmap_hash, "beatmap-md5-hash");
        assert_eq!(r.player_name, "PlayerOne");
        assert_eq!(r.replay_hash, "replay-md5-hash");
        assert_eq!(r.count_300, 300);
        assert_eq!(r.count_100, 10);
        assert_eq!(r.count_50, 5);
        assert_eq!(r.count_geki, 42);
        assert_eq!(r.count_katu, 7);
        assert_eq!(r.count_miss, 3);
        assert_eq!(r.total_score, 1_234_567);
        assert_eq!(r.max_combo, 850);
        assert!(r.perfect);
        assert_eq!(r.mods, 0);
        assert_eq!(r.life_bar, "0|1,1000|0.9");
        assert_eq!(r.score_id, Some(555));
        assert_eq!(r.frames.len(), 2);
    }

    // --- UT-OSR-002: mod bitmask decoding ---
    #[test]
    fn ut_osr_002_mods_decoded() {
        // Hidden (8) | HardRock (16) | DoubleTime (64) = 88
        let version = 20230326;
        let data = OsrBuilder::new(version, 88).finish("10|100|200|0", version, 1);

        let r = parse_osr(&data, None).unwrap();
        assert_eq!(r.mods, 88);
    }

    // --- UT-OSR-003: old format (no score ID field) ---
    #[test]
    fn ut_osr_003_old_format_2012_has_no_score_id() {
        let version = 20111001; // < 20121008
        let data = OsrBuilder::new(version, 0).finish("10|100|200|0", version, 0);

        let r = parse_osr(&data, None).expect("old replay should parse");

        assert_eq!(r.version, version);
        assert_eq!(r.score_id, None);
        assert_eq!(r.frames.len(), 1);
    }

    /// The i32 score-ID window. A parser that assumed a fixed i64 (as TDD §2.1
    /// specifies) would read 4 bytes past the end here.
    #[test]
    fn ut_osr_003b_i32_score_id_window() {
        let version = 20130101; // in [20121008, 20140721)
        let data = OsrBuilder::new(version, 0).finish("10|100|200|0", version, 4242);

        let r = parse_osr(&data, None).expect("replay in the i32 window should parse");
        assert_eq!(r.score_id, Some(4242));
    }

    // --- UT-OSR-004: new format (i64 score ID) ---
    #[test]
    fn ut_osr_004_new_format_i64_score_id() {
        let version = 20240101;
        let big = 5_000_000_000i64; // beyond i32 range
        let data = OsrBuilder::new(version, 0).finish("10|100|200|0", version, big);

        let r = parse_osr(&data, None).unwrap();
        assert_eq!(r.score_id, Some(big as u64));
    }

    // --- UT-OSR-011: LZMA frame stream decompresses ---
    #[test]
    fn ut_osr_011_lzma_frames_decompress() {
        let version = 20230326;
        let data = OsrBuilder::new(version, 0).finish(
            "0|256|-500|0,10|256|-500|0,16|100|200|1,16|110|210|5,-12345|0|0|777",
            version,
            1,
        );

        let r = parse_osr(&data, None).unwrap();

        // The two sentinel frames and the seed frame are all removed.
        assert_eq!(r.frames.len(), 2);
        assert_eq!(r.frames[0].x, 100.0);
        assert_eq!(r.frames[1].keys, 5);
    }

    /// The early-version offset must reach frame times through the public entry
    /// point, not just the internal helper.
    #[test]
    fn early_version_offset_applies_through_parse_osr() {
        let version = 20230326;
        let data = OsrBuilder::new(version, 0).finish("10|100|200|0", version, 1);

        let r = parse_osr(&data, Some(4)).unwrap();
        assert_eq!(
            r.frames[0].time, 34.0,
            "24ms offset not applied for a v4 beatmap"
        );

        let r = parse_osr(&data, Some(14)).unwrap();
        assert_eq!(r.frames[0].time, 10.0);
    }

    /// A replay whose declared payload length runs past the buffer must be an
    /// error, not a panic.
    #[test]
    fn truncated_replay_payload_is_an_error() {
        let version = 20230326;
        let mut data = OsrBuilder::new(version, 0).finish("10|100|200|0", version, 1);

        // Chop the payload in half.
        data.truncate(data.len() / 2);

        assert!(
            parse_osr(&data, None).is_err(),
            "a truncated replay must not parse"
        );
    }

    // ── Header ────────────────────────────────────────────────────────────

    // --- UT-OSR-010: empty file ---
    #[test]
    fn ut_osr_010_empty_file() {
        assert!(matches!(
            parse_osr(&[], None),
            Err(EngineError::UnexpectedEof { .. })
        ));
    }

    // --- UT-OSR-013: non-standard game mode ---
    #[test]
    fn ut_osr_013_non_standard_mode_rejected() {
        let data = [1u8, 0, 0, 0, 0]; // mode 1 = taiko
        assert!(matches!(
            parse_osr(&data, None),
            Err(EngineError::InvalidGameMode { mode: 1, .. })
        ));
    }

    // --- UT-OSR-009: truncated input ---
    #[test]
    fn ut_osr_009_truncated_input() {
        let data = [0u8, 0x01, 0x02]; // valid mode, truncated version
        assert!(matches!(
            parse_osr(&data, None),
            Err(EngineError::UnexpectedEof { .. })
        ));
    }
}

/// Property tests: the parser must be total over arbitrary input.
///
/// The `.osr` parser is the engine's most exposed attack surface — it consumes
/// an untrusted binary file and drives an LZMA decompressor. "Never panics" is
/// a security property here, not just a robustness nicety (Security Threat
/// Model: untrusted parser input), so it is asserted over generated input
/// rather than only over the hand-written cases above.
#[cfg(test)]
mod property_tests {
    use super::tests::OsrBuilder;
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(1000))]

        /// Arbitrary bytes must produce a `Result`, never a panic.
        #[test]
        fn never_panics_on_arbitrary_bytes(data in prop::collection::vec(any::<u8>(), 0..2048)) {
            let _ = parse_osr(&data, None);
        }

        /// A valid header followed by arbitrary trailing bytes must not panic.
        /// This reaches deeper into the parser than random noise, which usually
        /// dies at the mode byte.
        #[test]
        fn never_panics_on_valid_header_with_garbage_tail(
            tail in prop::collection::vec(any::<u8>(), 0..512),
        ) {
            let mut data = vec![MODE_STANDARD];
            data.extend_from_slice(&20230326i32.to_le_bytes());
            data.extend_from_slice(&tail);

            let _ = parse_osr(&data, None);
        }

        /// Arbitrary frame text must not panic.
        #[test]
        fn never_panics_on_arbitrary_frame_text(text in ".{0,512}") {
            let _ = parse_frame_text(&text, 0);
        }

        /// Frame times are non-decreasing for every input that parses at all.
        /// This is the invariant every downstream binary search depends on.
        #[test]
        fn parsed_frames_are_always_monotonic(
            deltas in prop::collection::vec(-2000i32..2000, 0..64),
        ) {
            let text: Vec<String> = deltas
                .iter()
                .map(|d| format!("{d}|100|100|0"))
                .collect();

            let mut frames = parse_frame_text(&text.join(","), 0).unwrap_or_default();
            apply_stable_frame_quirks(&mut frames);

            for w in frames.windows(2) {
                prop_assert!(
                    w[0].time <= w[1].time,
                    "frame times not monotonic: {} then {}",
                    w[0].time,
                    w[1].time
                );
            }
        }

        // --- UT-OSR-015: header round-trip ---
        /// Serialising a header and parsing it back must reproduce every field.
        #[test]
        fn ut_osr_015_header_round_trip(
            version in 20140721i32..30000000,
            mods in 0i32..1024,
            score_id in 1i64..1_000_000,
        ) {
            let data = OsrBuilder::new(version, mods)
                .finish("10|100|200|0", version, score_id);

            let r = parse_osr(&data, None).expect("generated replay must parse");

            prop_assert_eq!(r.version, version);
            prop_assert_eq!(r.mods, mods as u32);
            prop_assert_eq!(r.score_id, Some(score_id as u64));
            prop_assert_eq!(r.player_name, "PlayerOne");
            prop_assert_eq!(r.count_300, 300);
            prop_assert_eq!(r.max_combo, 850);
            prop_assert!(r.perfect);
        }
    }
}
