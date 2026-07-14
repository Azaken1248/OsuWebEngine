//! `.osu` beatmap text parser.
//!
//! An INI-like format: a version header, then `[Section]` blocks of either
//! `Key: Value` pairs or comma-separated records.
//!
//! ## Behavioral notes
//!
//! Several rules here are absent from or contradict TDD §3, and are
//! implemented per lazer (see the L2 plan, "TDD Divergences"):
//!
//! - An **unknown curve type character defaults to Catmull**, it is not an
//!   error ([`parse_curve_type`]).
//! - Beatmaps of format < 5 get a **+24 ms offset** baked into every hit object
//!   and timing point time ([`EARLY_VERSION_TIMING_OFFSET`]).
//! - Perfect curves are **downgraded at parse time** on legacy maps
//!   ([`downgrade_perfect_curve`]).
//! - **AR defaults to OD** when the `ApproachRate` key is absent, and several
//!   difficulty values are clamped after parsing ([`apply_difficulty_restrictions`]).
//!
//! ## Reference
//!
//! - Specification: `osu/osu.Game/Beatmaps/Formats/LegacyBeatmapDecoder.cs`
//! - Hit objects: `osu/osu.Game/Rulesets/Objects/Legacy/ConvertHitObjectParser.cs`
//! - Numeric limits: `osu/osu.Game/Beatmaps/Formats/Parsing.cs`
//! - Type bitmask: `osu/osu.Game/Beatmaps/Legacy/LegacyHitObjectType.cs`

use crate::error::{EngineError, EngineResult};
use crate::math::curves::CurveType;
use crate::math::vec2::Vec2;
use crate::model::beatmap::{ParsedBeatmap, TimingPoint};
use crate::model::hit_object::{HitObject, HitObjectKind, SliderData, SpinnerData};

/// osu! Standard's mode byte.
const MODE_STANDARD: u8 = 0;

/// Offset applied to beatmaps of format v4 and lower, to correct timing changes
/// that were once applied at the game-client level.
///
/// Baked into hit object *and* timing point times at parse time, exactly as
/// `LegacyBeatmapDecoder` does, so no downstream layer needs to know the format
/// version. The `.osr` parser applies the same offset to replay frames — the
/// two must agree or old maps desync by 24 ms.
///
/// Source: `LegacyBeatmapDecoder.cs` L27-29, L74.
pub const EARLY_VERSION_TIMING_OFFSET: i64 = 24;

/// First beatmap format version authored by lazer.
///
/// Below this, the stable-era perfect-curve downgrades apply.
///
/// Source: `LegacyBeatmapEncoder.cs` L25.
const FIRST_LAZER_VERSION: i32 = 128;

/// Default for HP / CS / OD / AR when the key is absent.
///
/// Source: `BeatmapDifficulty.cs` L14 (`DEFAULT_DIFFICULTY = 5`).
const DEFAULT_DIFFICULTY: f64 = 5.0;

/// Maximum coordinate magnitude lazer accepts.
///
/// Source: `Parsing.cs` L14.
const MAX_COORDINATE_VALUE: f64 = 131_072.0;

/// A slide count above this is rejected outright.
///
/// Source: `ConvertHitObjectParser.cs` L89.
const MAX_SLIDES: i64 = 9000;

/// Epsilon for the collinearity test used by the perfect-curve downgrade.
///
/// Source: `Precision.AlmostEquals` default for `float` (1e-3), as used by
/// `ConvertHitObjectParser.isLinear` L424-426.
const LINEAR_EPSILON: f64 = 1e-3;

// ── Type bitmask (LegacyHitObjectType.cs) ────────────────────────────────────
const TYPE_CIRCLE: u8 = 1;
const TYPE_SLIDER: u8 = 1 << 1;
const TYPE_NEW_COMBO: u8 = 1 << 2;
const TYPE_SPINNER: u8 = 1 << 3;

/// Which section the line lexer is currently inside.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    None,
    General,
    Metadata,
    Difficulty,
    TimingPoints,
    HitObjects,
    /// A section we parse nothing from (Editor, Events, Colours, ...).
    Ignored,
}

/// Parses a `.osu` beatmap from UTF-8 text.
pub fn parse_osu(data: &str) -> EngineResult<ParsedBeatmap> {
    let format_version = parse_format_version(data)?;
    let offset = if format_version < 5 {
        EARLY_VERSION_TIMING_OFFSET as f64
    } else {
        0.0
    };

    let mut b = Builder::new(format_version);
    let mut section = Section::None;

    for raw_line in data.lines() {
        let line = strip_comment(raw_line);
        if line.is_empty() {
            continue;
        }

        if let Some(name) = section_header(line) {
            section = name;
            b.seen_section(section);
            continue;
        }

        match section {
            Section::General => b.general(line)?,
            Section::Metadata => b.metadata(line),
            Section::Difficulty => b.difficulty(line)?,
            Section::TimingPoints => b.timing_point(line, offset)?,
            Section::HitObjects => b.hit_object(line, offset)?,
            Section::None | Section::Ignored => {}
        }
    }

    b.finish()
}

/// Reads `osu file format vN` from the first non-empty line.
fn parse_format_version(data: &str) -> EngineResult<i32> {
    for line in data.lines().take(8) {
        // A BOM may precede the header on files written by Windows tooling.
        let line = line.trim_start_matches('\u{feff}').trim();
        if line.is_empty() {
            continue;
        }

        if let Some(rest) = line.strip_prefix("osu file format v") {
            return rest
                .trim()
                .parse::<i32>()
                .map_err(|_| EngineError::MalformedField {
                    field: "format_version",
                    value: rest.to_string(),
                });
        }

        // The header must be the first thing in the file.
        break;
    }

    Err(EngineError::InvalidMagic {
        expected: "osu file format v",
        found: data.bytes().take(16).collect(),
    })
}

/// Strips a trailing `//` comment and surrounding whitespace.
fn strip_comment(line: &str) -> &str {
    match line.find("//") {
        Some(i) => line[..i].trim(),
        None => line.trim(),
    }
}

/// Recognises a `[Section]` header.
fn section_header(line: &str) -> Option<Section> {
    let name = line.strip_prefix('[')?.strip_suffix(']')?;

    Some(match name {
        "General" => Section::General,
        "Metadata" => Section::Metadata,
        "Difficulty" => Section::Difficulty,
        "TimingPoints" => Section::TimingPoints,
        "HitObjects" => Section::HitObjects,
        _ => Section::Ignored,
    })
}

/// Splits `Key: Value` (whitespace around the colon is optional).
fn key_value(line: &str) -> Option<(&str, &str)> {
    let (k, v) = line.split_once(':')?;
    Some((k.trim(), v.trim()))
}

/// Accumulates parsed state, then validates it in [`Builder::finish`].
struct Builder {
    format_version: i32,

    audio_filename: String,
    audio_lead_in: f64,
    mode: u8,
    stack_leniency: f64,

    title: String,
    artist: String,
    creator: String,
    version: String,

    hp: f64,
    cs: f64,
    od: f64,
    ar: f64,
    /// AR falls back to OD unless an explicit `ApproachRate` key appeared.
    /// Source: `LegacyBeatmapDecoder.cs` L410-411.
    has_approach_rate: bool,
    slider_multiplier: f64,
    slider_tick_rate: f64,

    timing_points: Vec<TimingPoint>,
    hit_objects: Vec<HitObject>,

    saw_timing_points: bool,
    saw_hit_objects: bool,
}

impl Builder {
    fn new(format_version: i32) -> Self {
        Builder {
            format_version,
            audio_filename: String::new(),
            audio_lead_in: 0.0,
            mode: MODE_STANDARD,
            stack_leniency: 0.7,
            title: String::new(),
            artist: String::new(),
            creator: String::new(),
            version: String::new(),
            hp: DEFAULT_DIFFICULTY,
            cs: DEFAULT_DIFFICULTY,
            od: DEFAULT_DIFFICULTY,
            ar: DEFAULT_DIFFICULTY,
            has_approach_rate: false,
            slider_multiplier: 1.4,
            slider_tick_rate: 1.0,
            timing_points: Vec::new(),
            hit_objects: Vec::new(),
            saw_timing_points: false,
            saw_hit_objects: false,
        }
    }

    fn seen_section(&mut self, s: Section) {
        match s {
            Section::TimingPoints => self.saw_timing_points = true,
            Section::HitObjects => self.saw_hit_objects = true,
            _ => {}
        }
    }

    fn general(&mut self, line: &str) -> EngineResult<()> {
        let Some((k, v)) = key_value(line) else {
            return Ok(());
        };

        match k {
            "AudioFilename" => self.audio_filename = v.to_string(),
            "AudioLeadIn" => self.audio_lead_in = parse_f64(v, "AudioLeadIn")?,
            "StackLeniency" => self.stack_leniency = parse_f64(v, "StackLeniency")?,
            "Mode" => {
                let mode = parse_f64(v, "Mode")? as u8;
                if mode != MODE_STANDARD {
                    return Err(EngineError::InvalidGameMode {
                        mode,
                        expected: MODE_STANDARD,
                    });
                }
                self.mode = mode;
            }
            _ => {}
        }

        Ok(())
    }

    fn metadata(&mut self, line: &str) {
        let Some((k, v)) = key_value(line) else {
            return;
        };

        match k {
            "Title" => self.title = v.to_string(),
            "Artist" => self.artist = v.to_string(),
            "Creator" => self.creator = v.to_string(),
            "Version" => self.version = v.to_string(),
            _ => {}
        }
    }

    fn difficulty(&mut self, line: &str) -> EngineResult<()> {
        let Some((k, v)) = key_value(line) else {
            return Ok(());
        };

        match k {
            "HPDrainRate" => self.hp = parse_f64(v, "HPDrainRate")?,
            "CircleSize" => self.cs = parse_f64(v, "CircleSize")?,
            "OverallDifficulty" => {
                self.od = parse_f64(v, "OverallDifficulty")?;
                // AR mirrors OD until an explicit ApproachRate is seen. Old
                // maps predate the AR field entirely and rely on this.
                if !self.has_approach_rate {
                    self.ar = self.od;
                }
            }
            "ApproachRate" => {
                self.ar = parse_f64(v, "ApproachRate")?;
                self.has_approach_rate = true;
            }
            "SliderMultiplier" => self.slider_multiplier = parse_f64(v, "SliderMultiplier")?,
            "SliderTickRate" => self.slider_tick_rate = parse_f64(v, "SliderTickRate")?,
            _ => {}
        }

        Ok(())
    }

    /// `time, beatLength, meter, sampleSet, sampleIndex, volume, uninherited, effects`
    ///
    /// Only the first two fields are guaranteed present on very old maps.
    fn timing_point(&mut self, line: &str, offset: f64) -> EngineResult<()> {
        let f: Vec<&str> = line.split(',').collect();
        if f.len() < 2 {
            return Ok(());
        }

        let time = parse_f64(f[0], "timing_point_time")? + offset;
        let beat_length = parse_f64(f[1], "beat_length")?;

        // `uninherited` is absent on old maps; a positive beat length means an
        // uninherited (red) line, which is the correct fallback.
        let uninherited = match f.get(6) {
            Some(v) => parse_f64(v, "uninherited")? != 0.0,
            None => beat_length > 0.0,
        };

        self.timing_points.push(TimingPoint {
            time,
            beat_length,
            uninherited,
            meter: opt_u8(&f, 2, 4)?,
            sample_set: opt_u8(&f, 3, 0)?,
            sample_index: opt_u8(&f, 4, 0)?,
            volume: opt_u8(&f, 5, 100)?,
            effects: opt_u8(&f, 7, 0)?,
        });

        Ok(())
    }

    /// `x, y, time, type, hitSound, [objectParams], [hitSample]`
    fn hit_object(&mut self, line: &str, offset: f64) -> EngineResult<()> {
        let f: Vec<&str> = line.split(',').collect();
        if f.len() < 4 {
            return Ok(());
        }

        let x = parse_coordinate(f[0], "hit_object_x")?;
        let y = parse_coordinate(f[1], "hit_object_y")?;
        let time = parse_f64(f[2], "hit_object_time")? + offset;
        let type_flags = parse_f64(f[3], "hit_object_type")? as u8;
        let hit_sound = f
            .get(4)
            .map_or(Ok(0u8), |v| Ok(parse_f64(v, "hit_sound")? as u8))?;

        let new_combo = type_flags & TYPE_NEW_COMBO != 0;
        let combo_color_skip = (type_flags >> 4) & 0x07;

        let index = self.hit_objects.len();

        // Circle is checked first, then slider, then spinner — matching lazer's
        // precedence. The bits are not mutually exclusive in malformed files.
        let kind = if type_flags & TYPE_CIRCLE != 0 {
            HitObjectKind::Circle
        } else if type_flags & TYPE_SLIDER != 0 {
            HitObjectKind::Slider(self.parse_slider(&f, Vec2::new(x, y))?)
        } else if type_flags & TYPE_SPINNER != 0 {
            let end_time = f
                .get(5)
                .ok_or(EngineError::MalformedField {
                    field: "spinner_end_time",
                    value: line.to_string(),
                })
                .and_then(|v| parse_f64(v, "spinner_end_time"))?;

            HitObjectKind::Spinner(SpinnerData {
                // lazer: `Math.Max(startTime, endTime)`.
                end_time: end_time.max(time - offset) + offset,
            })
        } else {
            return Err(EngineError::MalformedField {
                field: "hit_object_type",
                value: type_flags.to_string(),
            });
        };

        self.hit_objects.push(HitObject {
            index,
            x,
            y,
            time,
            type_flags,
            hit_sound,
            new_combo,
            combo_color_skip,
            kind,
            stack_height: 0,
        });

        Ok(())
    }

    /// `...,type,hitSound, curveType|p1|p2|..., slides, length, ...`
    fn parse_slider(&self, f: &[&str], start: Vec2) -> EngineResult<SliderData> {
        let curve_field = f.get(5).ok_or(EngineError::MalformedField {
            field: "slider_curve",
            value: String::new(),
        })?;

        let (mut curve_type, control_points) = parse_curve(curve_field, start)?;

        // `slides` is the field name; 1 means "no repeat". lazer stores
        // `repeatCount = slides - 1` internally, but the L3 model documents the
        // `slides` convention, so that is what is kept here.
        let slides = f
            .get(6)
            .ok_or(EngineError::MalformedField {
                field: "slider_slides",
                value: String::new(),
            })
            .and_then(|v| {
                v.trim()
                    .parse::<i64>()
                    .map_err(|_| EngineError::MalformedField {
                        field: "slider_slides",
                        value: v.to_string(),
                    })
            })?;

        if slides > MAX_SLIDES {
            return Err(EngineError::ValueOutOfRange {
                field: "slider_slides",
                value: slides as f64,
                limit: MAX_SLIDES as f64,
            });
        }

        let repeat_count = slides.max(1) as u32;

        let pixel_length = match f.get(7) {
            Some(v) => parse_f64(v, "slider_length")?.max(0.0),
            None => 0.0,
        };

        if pixel_length > MAX_COORDINATE_VALUE {
            return Err(EngineError::ValueOutOfRange {
                field: "slider_length",
                value: pixel_length,
                limit: MAX_COORDINATE_VALUE,
            });
        }

        downgrade_perfect_curve(self.format_version, &mut curve_type, &control_points);

        Ok(SliderData {
            curve_type,
            control_points,
            repeat_count,
            pixel_length,
            // Computed in L4 once timing is resolved.
            end_time: 0.0,
        })
    }

    fn finish(mut self) -> EngineResult<ParsedBeatmap> {
        if !self.saw_timing_points {
            return Err(EngineError::MissingSection {
                section: "TimingPoints",
            });
        }
        if !self.saw_hit_objects {
            return Err(EngineError::MissingSection {
                section: "HitObjects",
            });
        }

        apply_difficulty_restrictions(&mut self);

        // Stable sort: equal-time objects must keep file order (Dev Guide §10.1).
        self.timing_points.sort_by(|a, b| a.time.total_cmp(&b.time));
        self.hit_objects.sort_by(|a, b| a.time.total_cmp(&b.time));

        // Re-index after sorting so `index` stays authoritative downstream.
        for (i, o) in self.hit_objects.iter_mut().enumerate() {
            o.index = i;
        }

        Ok(ParsedBeatmap {
            format_version: self.format_version,
            audio_filename: self.audio_filename,
            audio_lead_in: self.audio_lead_in,
            mode: self.mode,
            stack_leniency: self.stack_leniency,
            title: self.title,
            artist: self.artist,
            creator: self.creator,
            version: self.version,
            beatmap_hash: String::new(),
            hp: self.hp,
            cs: self.cs,
            od: self.od,
            ar: self.ar,
            slider_multiplier: self.slider_multiplier,
            slider_tick_rate: self.slider_tick_rate,
            timing_points: self.timing_points,
            hit_objects: self.hit_objects,
        })
    }
}

/// Clamps difficulty values to the ranges lazer enforces after parsing.
///
/// Not in the TDD. A map declaring `OverallDifficulty: 99` is accepted by osu!
/// and silently clamped to 10; rejecting or honouring it would both be wrong.
///
/// Source: `LegacyBeatmapDecoder.cs` L117-131.
fn apply_difficulty_restrictions(b: &mut Builder) {
    b.hp = b.hp.clamp(0.0, 10.0);
    b.cs = b.cs.clamp(0.0, 10.0);
    b.od = b.od.clamp(0.0, 10.0);
    b.ar = b.ar.clamp(0.0, 10.0);
    b.slider_multiplier = b.slider_multiplier.clamp(0.4, 3.6);
    b.slider_tick_rate = b.slider_tick_rate.clamp(0.5, 8.0);
}

/// Parses `B|100:100|200:200` into a curve type and its control points.
///
/// The hit object's own position is prepended as the first control point —
/// the file omits it, but every curve algorithm expects the path to start there.
fn parse_curve(field: &str, start: Vec2) -> EngineResult<(CurveType, Vec<Vec2>)> {
    let mut parts = field.split('|');

    let type_str = parts.next().unwrap_or("");
    let curve_type = parse_curve_type(type_str);

    let mut points = Vec::with_capacity(field.len() / 8 + 1);
    points.push(start);

    for p in parts {
        let (xs, ys) = p.split_once(':').ok_or(EngineError::MalformedField {
            field: "slider_control_point",
            value: p.to_string(),
        })?;

        points.push(Vec2::new(
            parse_coordinate(xs, "slider_control_point_x")?,
            parse_coordinate(ys, "slider_control_point_y")?,
        ));
    }

    Ok((curve_type, points))
}

/// Maps the curve type character.
///
/// **An unrecognised character yields Catmull, not an error.** lazer's `switch`
/// has `default: case 'C':` — a parser that rejected unknown types would fail
/// on files the game happily loads. TDD §3.3's `UnknownCurveType` error is
/// wrong.
///
/// `B` followed by a digit selects a B-spline of that degree (a lazer-era
/// extension). Legacy maps never use it, and our curve model has no B-spline
/// variant, so it is treated as Bézier — the closest available and what the
/// degree collapses to when it meets or exceeds the control point count.
///
/// Source: `ConvertHitObjectParser.cs` L237-257.
fn parse_curve_type(s: &str) -> CurveType {
    match s.as_bytes().first() {
        Some(b'B') => CurveType::Bezier,
        Some(b'L') => CurveType::Linear,
        Some(b'P') => CurveType::PerfectArc,
        // Includes 'C' and every unrecognised character.
        _ => CurveType::CatmullRom,
    }
}

/// Applies stable's parse-time perfect-curve downgrades.
///
/// On legacy maps (format < 128) a `P` curve is rewritten before it ever
/// reaches the flattener:
///
/// - not exactly 3 control points → **Bézier**
/// - 3 collinear control points   → **Linear**
///
/// The second rule is why the "collinear perfect curve behaves as a line"
/// folklore exists. It is a *parser* rule, and it does not contradict the
/// flattener's own collinear → Bézier fallback (ADR-021): on legacy maps the
/// flattener simply never sees a collinear `P`.
///
/// Source: `ConvertHitObjectParser.cs` L366-383.
fn downgrade_perfect_curve(format_version: i32, curve_type: &mut CurveType, points: &[Vec2]) {
    if *curve_type != CurveType::PerfectArc {
        return;
    }

    if format_version < FIRST_LAZER_VERSION {
        if points.len() != 3 {
            *curve_type = CurveType::Bezier;
        } else if is_linear(points[0], points[1], points[2]) {
            *curve_type = CurveType::Linear;
        }
    } else if points.len() > 3 {
        // lazer permits perfect curves with fewer than 3 points and collinear
        // points; only an over-long control set is downgraded.
        *curve_type = CurveType::Bezier;
    }
}

/// Collinearity via the 2D cross product, matching `Precision.AlmostEquals`.
///
/// Source: `ConvertHitObjectParser.cs` L424-426.
fn is_linear(p0: Vec2, p1: Vec2, p2: Vec2) -> bool {
    let cross = (p1.y - p0.y) * (p2.x - p0.x) - (p1.x - p0.x) * (p2.y - p0.y);
    cross.abs() < LINEAR_EPSILON
}

// ── Field helpers ────────────────────────────────────────────────────────────

fn parse_f64(s: &str, field: &'static str) -> EngineResult<f64> {
    let v = s
        .trim()
        .parse::<f64>()
        .map_err(|_| EngineError::MalformedField {
            field,
            value: s.to_string(),
        })?;

    if !v.is_finite() {
        return Err(EngineError::MalformedField {
            field,
            value: s.to_string(),
        });
    }

    Ok(v)
}

fn parse_coordinate(s: &str, field: &'static str) -> EngineResult<f64> {
    let v = parse_f64(s, field)?;

    if v.abs() > MAX_COORDINATE_VALUE {
        return Err(EngineError::ValueOutOfRange {
            field,
            value: v,
            limit: MAX_COORDINATE_VALUE,
        });
    }

    Ok(v)
}

/// Optional comma-separated field with a default.
fn opt_u8(f: &[&str], i: usize, default: u8) -> EngineResult<u8> {
    match f.get(i) {
        Some(v) if !v.trim().is_empty() => {
            let n = parse_f64(v, "timing_point_field")?;
            Ok(n.clamp(0.0, 255.0) as u8)
        }
        _ => Ok(default),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal well-formed beatmap; `extra` is appended into `[HitObjects]`.
    fn beatmap(difficulty: &str, timing: &str, objects: &str) -> String {
        format!(
            "osu file format v14\n\n\
             [General]\nAudioFilename: audio.mp3\nMode: 0\nStackLeniency: 0.7\n\n\
             [Metadata]\nTitle: Test Song\nArtist: Test Artist\nCreator: mapper\nVersion: Insane\n\n\
             [Difficulty]\n{difficulty}\n\n\
             [TimingPoints]\n{timing}\n\n\
             [HitObjects]\n{objects}\n"
        )
    }

    fn simple(objects: &str) -> ParsedBeatmap {
        parse_osu(&beatmap(
            "HPDrainRate:6\nCircleSize:4\nOverallDifficulty:8\nApproachRate:9.5\nSliderMultiplier:1.8\nSliderTickRate:1",
            "1500,500,4,2,0,100,1,0",
            objects,
        ))
        .expect("fixture should parse")
    }

    // --- UT-OSU-001: complete beatmap ---
    #[test]
    fn ut_osu_001_complete_beatmap() {
        let b = simple("256,192,1500,1,0,0:0:0:0:");

        assert_eq!(b.format_version, 14);
        assert_eq!(b.audio_filename, "audio.mp3");
        assert_eq!(b.mode, 0);
        assert_eq!(b.title, "Test Song");
        assert_eq!(b.artist, "Test Artist");
        assert_eq!(b.creator, "mapper");
        assert_eq!(b.version, "Insane");
        assert_eq!(b.timing_points.len(), 1);
        assert_eq!(b.hit_objects.len(), 1);
    }

    // --- UT-OSU-002: circle ---
    #[test]
    fn ut_osu_002_circle() {
        let b = simple("256,192,1500,1,0,0:0:0:0:");
        let o = &b.hit_objects[0];

        assert!(o.is_circle());
        assert_eq!(o.x, 256.0);
        assert_eq!(o.y, 192.0);
        assert_eq!(o.time, 1500.0);
        assert!(!o.new_combo);
    }

    // --- UT-OSU-003: Bezier slider ---
    #[test]
    fn ut_osu_003_bezier_slider() {
        let b = simple("100,200,2000,2,0,B|200:200|300:100,1,200");
        let HitObjectKind::Slider(s) = &b.hit_objects[0].kind else {
            panic!("expected a slider");
        };

        assert_eq!(s.curve_type, CurveType::Bezier);
        // The object's own position is prepended as control point 0.
        assert_eq!(s.control_points.len(), 3);
        assert_eq!(s.control_points[0], Vec2::new(100.0, 200.0));
        assert_eq!(s.control_points[2], Vec2::new(300.0, 100.0));
        assert_eq!(s.repeat_count, 1);
        assert_eq!(s.pixel_length, 200.0);
    }

    // --- UT-OSU-004: Catmull slider ---
    #[test]
    fn ut_osu_004_catmull_slider() {
        let b = simple("100,200,2000,2,0,C|200:200|300:100,1,200");
        let HitObjectKind::Slider(s) = &b.hit_objects[0].kind else {
            panic!("expected a slider");
        };
        assert_eq!(s.curve_type, CurveType::CatmullRom);
    }

    // --- UT-OSU-005: PerfectArc slider (3 non-collinear points) ---
    #[test]
    fn ut_osu_005_perfect_arc_slider() {
        let b = simple("100,100,2000,2,0,P|200:150|300:100,1,200");
        let HitObjectKind::Slider(s) = &b.hit_objects[0].kind else {
            panic!("expected a slider");
        };

        assert_eq!(s.curve_type, CurveType::PerfectArc);
        assert_eq!(s.control_points.len(), 3);
    }

    // --- UT-OSU-006: spinner ---
    #[test]
    fn ut_osu_006_spinner() {
        let b = simple("256,192,3000,8,0,5000,0:0:0:0:");
        let o = &b.hit_objects[0];

        assert!(o.is_spinner());
        assert_eq!(o.time, 3000.0);
        assert_eq!(o.end_time(), 5000.0);
    }

    // --- UT-OSU-007: new combo ---
    #[test]
    fn ut_osu_007_new_combo() {
        // type 5 = circle (1) | new combo (4)
        let b = simple("256,192,1500,5,0,0:0:0:0:");
        assert!(b.hit_objects[0].new_combo);
        assert!(b.hit_objects[0].is_circle());
    }

    // --- UT-OSU-008: combo color skip ---
    #[test]
    fn ut_osu_008_combo_color_skip() {
        // type = circle | new combo | (3 << 4) = 1 | 4 | 48 = 53
        let b = simple("256,192,1500,53,0,0:0:0:0:");
        assert_eq!(b.hit_objects[0].combo_color_skip, 3);
    }

    // --- UT-OSU-009: red (uninherited) timing point ---
    #[test]
    fn ut_osu_009_red_timing_point() {
        let b = parse_osu(&beatmap(
            "OverallDifficulty:8",
            "1500,500,4,2,0,100,1,0",
            "256,192,1500,1,0",
        ))
        .unwrap();

        let tp = &b.timing_points[0];
        assert!(tp.uninherited);
        assert_eq!(tp.beat_length, 500.0);
        // 60000 / 500 = 120 BPM
        assert_eq!(60_000.0 / tp.beat_length, 120.0);
    }

    // --- UT-OSU-010: green (inherited) timing point ---
    #[test]
    fn ut_osu_010_green_timing_point() {
        let b = parse_osu(&beatmap(
            "OverallDifficulty:8",
            "1500,500,4,2,0,100,1,0\n3000,-50,4,2,0,100,0,0",
            "256,192,1500,1,0",
        ))
        .unwrap();

        let green = &b.timing_points[1];
        assert!(!green.uninherited);
        assert_eq!(green.beat_length, -50.0);
        // Velocity multiplier = -100 / beat_length = 2.0
        assert_eq!(-100.0 / green.beat_length, 2.0);
    }

    // --- UT-OSU-011: difficulty section ---
    #[test]
    fn ut_osu_011_difficulty_section() {
        let b = simple("256,192,1500,1,0");

        assert_eq!(b.cs, 4.0);
        assert_eq!(b.ar, 9.5);
        assert_eq!(b.od, 8.0);
        assert_eq!(b.hp, 6.0);
        assert_eq!(b.slider_multiplier, 1.8);
    }

    // --- UT-OSU-012: format version ---
    #[test]
    fn ut_osu_012_format_version() {
        assert_eq!(parse_format_version("osu file format v14").unwrap(), 14);
        assert_eq!(parse_format_version("osu file format v3").unwrap(), 3);
        // A UTF-8 BOM must not defeat the header check.
        assert_eq!(
            parse_format_version("\u{feff}osu file format v9").unwrap(),
            9
        );

        assert!(matches!(
            parse_format_version("not a beatmap"),
            Err(EngineError::InvalidMagic { .. })
        ));
    }

    // --- UT-OSU-013: missing section ---
    #[test]
    fn ut_osu_013_missing_section() {
        let no_timing = "osu file format v14\n\n[HitObjects]\n256,192,1500,1,0\n";
        assert!(matches!(
            parse_osu(no_timing),
            Err(EngineError::MissingSection {
                section: "TimingPoints"
            })
        ));

        let no_objects = "osu file format v14\n\n[TimingPoints]\n1500,500,4,2,0,100,1,0\n";
        assert!(matches!(
            parse_osu(no_objects),
            Err(EngineError::MissingSection {
                section: "HitObjects"
            })
        ));
    }

    // --- UT-OSU-014: non-standard mode ---
    #[test]
    fn ut_osu_014_non_standard_mode_rejected() {
        let taiko = "osu file format v14\n\n[General]\nMode: 1\n\n[TimingPoints]\n1500,500\n\n[HitObjects]\n256,192,1500,1,0\n";

        assert!(matches!(
            parse_osu(taiko),
            Err(EngineError::InvalidGameMode { mode: 1, .. })
        ));
    }

    // --- UT-OSU-015: composite Bezier (repeated control point) ---
    #[test]
    fn ut_osu_015_composite_bezier_split() {
        // The repeated point (100,50) delimits two segments.
        let b = simple("0,0,2000,2,0,B|50:100|100:50|100:50|150:0|200:50,1,400");
        let HitObjectKind::Slider(s) = &b.hit_objects[0].kind else {
            panic!("expected a slider");
        };

        assert_eq!(s.curve_type, CurveType::Bezier);
        assert_eq!(s.control_points.len(), 6);

        // The duplicate is preserved for the flattener, which splits on it.
        assert_eq!(s.control_points[2], s.control_points[3]);
    }

    // ── Divergences (see L2 plan) ────────────────────────────────────────

    // --- D8: unknown curve type defaults to Catmull, not an error ---
    #[test]
    fn d8_unknown_curve_type_defaults_to_catmull() {
        for ch in ["X", "Z", "9", ""] {
            let line = format!("100,200,2000,2,0,{ch}|200:200|300:100,1,200");
            let b = simple(&line);

            let HitObjectKind::Slider(s) = &b.hit_objects[0].kind else {
                panic!("expected a slider");
            };
            assert_eq!(
                s.curve_type,
                CurveType::CatmullRom,
                "curve type {ch:?} should fall back to Catmull"
            );
        }
    }

    /// `B` with a degree suffix is a lazer B-spline extension; treated as Bézier.
    #[test]
    fn d8_bspline_degree_suffix_is_bezier() {
        let b = simple("100,200,2000,2,0,B3|200:200|300:100,1,200");
        let HitObjectKind::Slider(s) = &b.hit_objects[0].kind else {
            panic!("expected a slider");
        };
        assert_eq!(s.curve_type, CurveType::Bezier);
    }

    // --- D9: slides > 9000 rejected ---
    #[test]
    fn d9_excessive_slide_count_rejected() {
        let line = "100,200,2000,2,0,B|200:200,9001,200";
        let src = beatmap("OverallDifficulty:8", "1500,500,4,2,0,100,1,0", line);

        assert!(matches!(
            parse_osu(&src),
            Err(EngineError::ValueOutOfRange {
                field: "slider_slides",
                ..
            })
        ));
    }

    // --- Early-version timing offset (+24 ms on format < 5) ---
    #[test]
    fn early_version_offset_applied_to_objects_and_timing() {
        let v4 = "osu file format v4\n\n[TimingPoints]\n1500,500,4,2,0,100,1,0\n\n[HitObjects]\n256,192,1500,1,0\n";
        let b = parse_osu(v4).unwrap();

        assert_eq!(b.hit_objects[0].time, 1524.0, "+24ms not applied to object");
        assert_eq!(
            b.timing_points[0].time, 1524.0,
            "+24ms not applied to timing"
        );

        // v5 and later get no offset.
        let v5 = "osu file format v5\n\n[TimingPoints]\n1500,500,4,2,0,100,1,0\n\n[HitObjects]\n256,192,1500,1,0\n";
        let b = parse_osu(v5).unwrap();
        assert_eq!(b.hit_objects[0].time, 1500.0);
        assert_eq!(b.timing_points[0].time, 1500.0);
    }

    // --- Perfect-curve downgrades at parse time ---
    #[test]
    fn perfect_curve_with_wrong_point_count_downgrades_to_bezier() {
        // 4 control points (incl. the prepended start) -> Bezier on legacy maps.
        let b = simple("100,100,2000,2,0,P|200:150|300:100|400:200,1,200");
        let HitObjectKind::Slider(s) = &b.hit_objects[0].kind else {
            panic!("expected a slider");
        };

        assert_eq!(s.control_points.len(), 4);
        assert_eq!(s.curve_type, CurveType::Bezier);
    }

    #[test]
    fn collinear_perfect_curve_downgrades_to_linear() {
        // (0,0) (50,0) (100,0) are collinear -> stable rewrites P to Linear.
        let b = simple("0,0,2000,2,0,P|50:0|100:0,1,100");
        let HitObjectKind::Slider(s) = &b.hit_objects[0].kind else {
            panic!("expected a slider");
        };

        assert_eq!(
            s.curve_type,
            CurveType::Linear,
            "collinear P must downgrade to Linear on legacy maps"
        );
    }

    // --- AR defaults to OD when absent ---
    #[test]
    fn ar_defaults_to_od_when_absent() {
        let b = parse_osu(&beatmap(
            "OverallDifficulty:7.5\nCircleSize:4",
            "1500,500,4,2,0,100,1,0",
            "256,192,1500,1,0",
        ))
        .unwrap();

        assert_eq!(b.ar, 7.5, "AR must mirror OD when ApproachRate is absent");
    }

    #[test]
    fn explicit_ar_wins_regardless_of_key_order() {
        // ApproachRate listed *before* OverallDifficulty must not be clobbered.
        let b = parse_osu(&beatmap(
            "ApproachRate:9\nOverallDifficulty:5",
            "1500,500,4,2,0,100,1,0",
            "256,192,1500,1,0",
        ))
        .unwrap();

        assert_eq!(b.ar, 9.0);
        assert_eq!(b.od, 5.0);
    }

    // --- Difficulty clamping ---
    #[test]
    fn difficulty_values_are_clamped() {
        let b = parse_osu(&beatmap(
            "HPDrainRate:99\nCircleSize:-5\nOverallDifficulty:50\nApproachRate:20\n\
             SliderMultiplier:99\nSliderTickRate:99",
            "1500,500,4,2,0,100,1,0",
            "256,192,1500,1,0",
        ))
        .unwrap();

        assert_eq!(b.hp, 10.0);
        assert_eq!(b.cs, 0.0);
        assert_eq!(b.od, 10.0);
        assert_eq!(b.ar, 10.0);
        assert_eq!(b.slider_multiplier, 3.6);
        assert_eq!(b.slider_tick_rate, 8.0);
    }

    // ── Robustness ───────────────────────────────────────────────────────

    #[test]
    fn comments_and_blank_lines_are_ignored() {
        let src = "osu file format v14\n\n// a comment\n\n[TimingPoints]\n\
                   1500,500,4,2,0,100,1,0 // trailing comment\n\n\
                   [HitObjects]\n256,192,1500,1,0\n";
        let b = parse_osu(src).unwrap();

        assert_eq!(b.timing_points.len(), 1);
        assert_eq!(b.hit_objects.len(), 1);
    }

    #[test]
    fn unknown_sections_are_skipped() {
        let src = "osu file format v14\n\n[Events]\n0,0,\"bg.jpg\"\n\n\
                   [Colours]\nCombo1: 255,128,0\n\n\
                   [TimingPoints]\n1500,500,4,2,0,100,1,0\n\n\
                   [HitObjects]\n256,192,1500,1,0\n";

        assert!(parse_osu(src).is_ok());
    }

    #[test]
    fn objects_are_sorted_and_reindexed() {
        let b = simple("256,192,3000,1,0\n100,100,1000,1,0\n200,200,2000,1,0");

        assert_eq!(b.hit_objects.len(), 3);
        assert_eq!(b.hit_objects[0].time, 1000.0);
        assert_eq!(b.hit_objects[1].time, 2000.0);
        assert_eq!(b.hit_objects[2].time, 3000.0);

        for (i, o) in b.hit_objects.iter().enumerate() {
            assert_eq!(o.index, i, "index must be reassigned after sorting");
        }
    }

    #[test]
    fn out_of_range_coordinate_rejected() {
        let src = beatmap(
            "OverallDifficulty:8",
            "1500,500,4,2,0,100,1,0",
            "999999999,192,1500,1,0",
        );

        assert!(matches!(
            parse_osu(&src),
            Err(EngineError::ValueOutOfRange { .. })
        ));
    }

    #[test]
    fn unknown_object_type_is_rejected() {
        // type 0: none of the circle/slider/spinner bits set.
        let src = beatmap(
            "OverallDifficulty:8",
            "1500,500,4,2,0,100,1,0",
            "256,192,1500,0,0",
        );

        assert!(matches!(
            parse_osu(&src),
            Err(EngineError::MalformedField {
                field: "hit_object_type",
                ..
            })
        ));
    }

    #[test]
    fn old_timing_point_without_uninherited_field() {
        // Very old maps omit trailing fields; a positive beat length implies red.
        let b = parse_osu(&beatmap(
            "OverallDifficulty:8",
            "1500,500\n3000,-50",
            "256,192,1500,1,0",
        ))
        .unwrap();

        assert!(b.timing_points[0].uninherited);
        assert!(!b.timing_points[1].uninherited);
    }
}

/// Property tests: the parser must be total over arbitrary input.
#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(1000))]

        /// Arbitrary text must produce a `Result`, never a panic.
        #[test]
        fn never_panics_on_arbitrary_text(text in ".{0,1024}") {
            let _ = parse_osu(&text);
        }

        /// A valid header plus arbitrary body must not panic. Random text
        /// almost always fails the magic check, so this is what actually
        /// exercises the section lexer and the record parsers.
        #[test]
        fn never_panics_with_valid_header(body in ".{0,1024}") {
            let src = format!("osu file format v14\n{body}");
            let _ = parse_osu(&src);
        }

        /// Arbitrary hit object records must not panic.
        #[test]
        fn never_panics_on_arbitrary_hit_objects(records in prop::collection::vec(".{0,64}", 0..16)) {
            let src = format!(
                "osu file format v14\n[TimingPoints]\n1500,500,4,2,0,100,1,0\n[HitObjects]\n{}\n",
                records.join("\n")
            );
            let _ = parse_osu(&src);
        }

        /// Whatever parses must come out sorted and correctly indexed — the
        /// contract every later layer relies on.
        #[test]
        fn parsed_objects_are_sorted_and_indexed(
            times in prop::collection::vec(0i32..100_000, 1..32),
        ) {
            let objects: Vec<String> = times
                .iter()
                .map(|t| format!("256,192,{t},1,0"))
                .collect();

            let src = format!(
                "osu file format v14\n[TimingPoints]\n1500,500,4,2,0,100,1,0\n[HitObjects]\n{}\n",
                objects.join("\n")
            );

            let b = parse_osu(&src).expect("generated beatmap must parse");

            prop_assert_eq!(b.hit_objects.len(), times.len());

            for (i, o) in b.hit_objects.iter().enumerate() {
                prop_assert_eq!(o.index, i);
                if i > 0 {
                    prop_assert!(b.hit_objects[i - 1].time <= o.time);
                }
            }
        }

        /// Difficulty values always land in their legal ranges, whatever the
        /// file claims.
        #[test]
        fn difficulty_always_within_range(
            hp in -1000.0f64..1000.0,
            cs in -1000.0f64..1000.0,
            od in -1000.0f64..1000.0,
            ar in -1000.0f64..1000.0,
        ) {
            let src = format!(
                "osu file format v14\n[Difficulty]\n\
                 HPDrainRate:{hp}\nCircleSize:{cs}\nOverallDifficulty:{od}\nApproachRate:{ar}\n\
                 [TimingPoints]\n1500,500,4,2,0,100,1,0\n[HitObjects]\n256,192,1500,1,0\n"
            );

            let b = parse_osu(&src).expect("generated beatmap must parse");

            prop_assert!((0.0..=10.0).contains(&b.hp), "hp out of range: {}", b.hp);
            prop_assert!((0.0..=10.0).contains(&b.cs), "cs out of range: {}", b.cs);
            prop_assert!((0.0..=10.0).contains(&b.od), "od out of range: {}", b.od);
            prop_assert!((0.0..=10.0).contains(&b.ar), "ar out of range: {}", b.ar);
        }
    }
}
