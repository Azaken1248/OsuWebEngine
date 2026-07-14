# Business Requirements Document  
## osu!-engine-wasm — Browser-Native Replay & Beatmap Engine

| | |
|---|---|
| **Document ID** | ENG-BRD-0042 |
| **Version** | 0.10 — DRAFT FOR REVIEW |
| **Author** | Systems Engineering |
| **Status** | Pre-approval |
| **Audience** | Engineering Leads, Product, QA |
| **Last Revised** | 2026-06-25 |

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Background & Current State](#2-background--current-state)
3. [Problem Statement](#3-problem-statement)
4. [Goals & Non-Goals](#4-goals--non-goals)
5. [Options Analysis](#5-options-analysis)
6. [Proposed Architecture](#6-proposed-architecture)
7. [Behavioral Compatibility](#7-behavioral-compatibility)
8. [Component Specifications](#8-component-specifications)
9. [Public API Contract](#9-public-api-contract)
10. [Data Flow & State Machine](#10-data-flow--state-machine)
11. [Performance Requirements](#11-performance-requirements)
12. [Build & Release Pipeline](#12-build--release-pipeline)
13. [Testing Strategy](#13-testing-strategy)
14. [Security Considerations](#14-security-considerations)
15. [Milestones & Timeline](#15-milestones--timeline)
16. [Risks & Mitigations](#16-risks--mitigations)
17. [Reference Repositories](#17-reference-repositories)
18. [Dependencies & Prerequisite Work](#18-dependencies--prerequisite-work)
19. [Open Questions](#19-open-questions)
20. [Glossary](#20-glossary)

---

## 1. Executive Summary

The OsuRender platform currently delegates all game-logic computation (replay parsing, curve resolution, object visibility, judging) to a server-side Python layer backed by the `slider` library, and delegates the actual rendering to Danser-Go — a compiled Go binary running on GPU-equipped Modal cloud instances. This architecture creates hard latency floors (minimum 60–120 seconds per render), prevents interactive replay scrubbing entirely, and makes the analysis feature a second-class citizen tied permanently to server availability and cost.

This document specifies **`osu-engine-wasm`**: a Rust-based **behavioral reimplementation** of the osu! Standard game core, compiled to WebAssembly, exposing a clean TypeScript-typed API surface that allows any web client to:

1. Parse `.osr` replay files and `.osu` beatmap files in the browser
2. Compute exact game state (cursor position, object visibility, hit windows, combo, accuracy) at arbitrary time `t` with sub-millisecond latency
3. Drive a WebGL2 renderer that reproduces osu! Standard's visual output at 60 fps without any server round-trip

The target output is a single `.wasm` binary (≤ 800 KB gzipped) and an accompanying TypeScript bindings package (`@osurender/engine`), deployable on any CDN and usable by any web application — including the existing `view_player.html` page and future clients.

> **Guiding Principle**: `osu-engine-wasm` is not an independent implementation of osu! rules. It is a behavioral reimplementation of osu!lazer Standard mode. Whenever documentation, formulas, or community understanding conflict with observed lazer behavior, **lazer behavior wins**. The C# source code of osu!lazer is the executable specification for this project.

---

## 2. Background & Current State

### 2.1 The Existing Pipeline

```
User uploads .osr
    │
    ▼
FastAPI worker (Python)
  • osrparse → extract cursor frames → upload frames.json.gz
  • osu! API → resolve beatmap hash → get set_id
  • (new) slider → parse .osz → upload beatmap.json.gz
    │
    ▼
Modal GPU worker
  • Danser-Go (Go binary + OpenGL) → renders .mp4 at 1080p/4K
  • ffmpeg → encodes, uploads to S3
    │
    ▼
view_player.html
  • Streams video from CDN
  • Fetches frames.json.gz + beatmap.json.gz
  • Canvas-based analyzer (JS) — simplified visuals only
```

### 2.2 What's Wrong With This

| Problem | Impact |
|---|---|
| Render latency: 60–180 seconds minimum | No interactive analysis; user must wait before seeing anything |
| Danser-Go dependency | Modal GPU required; local/dev setup is painful; binary updates are manual |
| Python `slider` library for curve math | Not performance-optimised; no wasm export; limited control |
| JS canvas analyzer re-implements game logic | Accuracy gap vs. the real game; sliders approximated; mod handling incomplete |
| Beatmap download required server-side | Cost + latency; user can't bring their own `.osz` |
| No playback seek | Users get a static video; can't scrub to a specific note |
| Server cost per render | GPU worker costs scale linearly with users; analysis adds extra CPU cost |

### 2.3 The Opportunity

osu!lazer (the canonical modern client) is open-source C# under MIT. A Rust behavioral reimplementation of Standard mode is tractable in scope — Standard has only three object types (circle, slider, spinner). Once compiled to WASM, every computation that currently requires a server call becomes a sub-millisecond browser call.

**Critical framing**: The wiki, formulas, and community documentation are *informational references only*. The actual C# source code of osu!lazer is the ground truth. osu! has accumulated twenty years of gameplay behavior — including historical quirks, accidental behaviors, stable-compatibility hacks, and edge cases where "the bug became part of the standard." This is analogous to how browser engines are validated against millions of compatibility tests rather than the specification alone, or how Game Boy emulators must reproduce hardware quirks that contradict datasheets. The project's correctness is defined by behavioral equivalence to lazer, not by adherence to documented formulas.

---

## 3. Problem Statement

**We cannot provide real-time, frame-accurate, fully-faithful replay analysis in the browser because all game logic lives on the server or in Danser-Go's opaque binary.**

Specifically:

- **Slider curves** (Bézier, Catmull-Rom, perfect arc) are computed server-side and approximated as 24-point polylines in the client. This approximation fails for low-CS maps and snake-in/out sliders.
- **Judging** (300/100/50/miss thresholds) is never computed at all in the current client; accuracy shown in the analyzer is from the replay header, not recomputed per-note.
- **Mods** are partially handled (AR/CS scaling) but miss cases: Flashlight viewport, Spun-out spinner physics, Half-Time pitch.
- **Stacking** (where overlapping notes offset toward the previous object) is not applied to the JS analyzer canvas, causing visible misalignment.
- **Real-time scrubbing** requires the user to wait for both the `.mp4` render and the analytics pipeline to complete.

---

## 4. Goals & Non-Goals

### 4.1 Goals (In Scope)

**P0 — Must ship in v1.0**

- G1: Parse `.osr` replay binary (LZMA cursor stream, header fields, all versions ≥ 20131216)
- G2: Parse `.osu` beatmap text format (all hit object types, timing points, difficulty params)
- G3: Apply mod transformations: EZ, HR, HT, DT, NC, HD, SD, PF, NF, Relax, Autopilot, Mirror
- G4: Resolve all three curve types: Bézier (including multi-segment), Catmull-Rom, perfect circular arc
- G5: Apply stacking algorithm to hit objects (pre-empt and distance threshold, standard and Peppy's v2)
- G6: Compute object visibility window (appear time, fade-in, fade-out) given effective AR
- G7: Compute hit windows (300/100/50/miss) given effective OD
- G8: Evaluate replay accuracy per-note (assign 300/100/50/miss to each hit object from frame data)
- G9: Cursor interpolation between frames (linear, matching osu!'s own approach)
- G10: Expose WASM bindings with TypeScript types (via `wasm-bindgen`)
- G11: Deliver `@osurender/engine` as an NPM package
- G12: ≤ 800 KB gzipped WASM binary

**P1 — v1.1 target**

- G13: Spinner physics (RPM computation from cursor angular velocity)
- G14: HP drain model (all objects, breaks, drain rate)
- G15: Combo color cycling from skin or beatmap
- G16: Streaming parse API (process `.osu` / `.osr` as `ReadableStream`, not requiring full buffer in memory)

**P2 — Future / v2.0**

- G17: osu!taiko, osu!catch, osu!mania game modes
- G18: Difficulty calculation (star rating) via rosu-pp integration
- G19: Skin texture atlas compilation

### 4.2 Non-Goals

- NG1: Audio playback (handled by the host application via Web Audio API)
- NG2: Video encoding or `.mp4` output (Danser-Go remains the render path for that)
- NG3: Storyboard/SB rendering
- NG4: osu!direct API integration
- NG5: Online score submission or ranking
- NG6: Full osu!lazer UI reproduction
- NG7: Multiplayer / spectator protocol

---

## 5. Options Analysis

Three credible approaches were evaluated.

### Option A — Compile osu!lazer to WASM via .NET WASM Runtime

osu!lazer is MIT-licensed C# targeting .NET 8. .NET ships a WASM runtime (`dotnet.wasm`) that can run Blazor or headless .NET code in the browser.

| | |
|---|---|
| **Pros** | Exact game logic; maintained by ppy; no reimplementation risk |
| **Cons** | dotnet.wasm runtime is 6–12 MB gzipped before any app code; startup time 3–8 seconds; osu!framework depends on Veldrid/OpenGL — no clean headless mode; full lazer has ~350 NuGet dependencies; no control over binary size or API surface |
| **Verdict** | Rejected — startup cost and runtime size are disqualifying for an inline browser component |

### Option B — Emscripten Port of osu!-stable (C++)

osu!-stable's renderer was DirectX; a partial C++ reimplementation exists in the community but is incomplete and unmaintained.

| | |
|---|---|
| **Pros** | Closest to the "real" stable game logic |
| **Cons** | C++ source is not cleanly separated from Win32 GDI/DirectX; Emscripten build would require extensive shim work; no meaningful community codebase to pull from |
| **Verdict** | Rejected — insufficient foundation to build on |

### Option C — Rust Reimplementation Targeting `wasm32-unknown-unknown` RECOMMENDED

Write `osu-engine-wasm` from scratch in Rust, implementing only Standard mode game rules (not the renderer). Compile to WASM. Ship as an NPM package with TypeScript bindings.

| | |
|---|---|
| **Pros** | Full control over API surface, binary size, and performance; Rust's `wasm-bindgen` + `wasm-pack` ecosystem is mature; existing Rust osu! crates exist (rosu-pp, osu-db, libosu) for reference; WASM binary can be < 500 KB; correctness can be validated against replay scorefiles; no runtime startup penalty |
| **Cons** | Requires reimplementing game rules in a different paradigm (C# → Rust), introducing risk of behavioral divergence; C#/Rust paradigm differences (GC vs. ownership, inheritance vs. traits, mutable objects vs. explicit mutability, floating-point evaluation order) may cause subtle output differences even when both implementations are "correct"; team needs Rust expertise |
| **Verdict** | Selected — the constraints (binary size, API cleanliness, no runtime overhead) only Option C can meet |

---

## 6. Proposed Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           BROWSER (Host Application)                        │
│                                                                             │
│  ┌──────────────────────────┐     ┌──────────────────────────────────────┐  │
│  │   @osurender/engine      │     │   @osurender/renderer (separate pkg) │  │
│  │   (WASM + TS bindings)   │     │   (TypeScript + WebGL2)              │  │
│  │                          │     │                                      │  │
│  │  ┌────────────────────┐  │     │  ┌──────────┐  ┌──────────────────┐ │  │
│  │  │  OsuParser         │  │     │  │ CircleGL │  │  SliderGL        │ │  │
│  │  │  .osr + .osu       │  │     │  │ shader   │  │  shader          │ │  │
│  │  └────────┬───────────┘  │     │  └──────────┘  └──────────────────┘ │  │
│  │           │              │     │  ┌──────────┐  ┌──────────────────┐ │  │
│  │  ┌────────▼───────────┐  │     │  │ CursorGL │  │  HUD / Overlay   │ │  │
│  │  │  GameState         │◄─┼─────┼─►│          │  │  (Canvas 2D)     │ │  │
│  │  │  queryAt(t: f64)   │  │     │  └──────────┘  └──────────────────┘ │  │
│  │  └────────────────────┘  │     └──────────────────────────────────────┘  │
│  │                          │                                               │
│  │  ┌────────────────────┐  │     ┌──────────────────────────────────────┐  │
│  │  │  ModEngine         │  │     │   view_player.html / host app        │  │
│  │  │  CurveResolver     │  │     │   • Scrubber                         │  │
│  │  │  JudgeEngine       │  │     │   • Controls                         │  │
│  │  │  StackSolver       │  │     │   • Skin loader (.osk)               │  │
│  │  └────────────────────┘  │     └──────────────────────────────────────┘  │
│  └──────────────────────────┘                                               │
└─────────────────────────────────────────────────────────────────────────────┘

        compile target: wasm32-unknown-unknown
        toolchain: wasm-pack + wasm-bindgen
        package: @osurender/engine (NPM)
```

### 6.1 Separation of Concerns

The engine and renderer are **deliberately separate packages**. The engine is pure logic with zero rendering code — it has no dependency on the DOM, WebGL, or Canvas. This matters for three reasons:

1. The engine can be tested in Node.js or in native Rust (`cargo test`) without a browser
2. A different renderer (e.g. PixiJS, Three.js, or a native desktop renderer via FFI) can consume the same engine
3. The WASM binary stays small because it contains no shader code, image data, or font data

### 6.2 Threading Model

WASM threads require `SharedArrayBuffer` + `COOP`/`COEP` headers. We will ship a single-threaded WASM binary by default and provide an opt-in threaded build for hosts that can set the required response headers.

- **Default build**: `wasm32-unknown-unknown` — synchronous, single-threaded
- **Threaded build**: `wasm32-unknown-unknown` with `atomics` + `bulk-memory` features — offloads parsing to a WebWorker, exposes a `Promise`-based API

Parse time for a typical 3-minute map (.osu ~50 KB, .osr ~300 KB) must be ≤ 50 ms on a mid-range laptop in single-threaded mode. If this target is not met, the threaded build becomes the default.

---

## 7. Behavioral Compatibility

This section addresses the single largest technical risk in the project: **behavioral divergence from osu!lazer**.

The challenge of reimplementing osu! in Rust is not a language or paradigm problem. C# and Rust differ in meaningful ways (GC vs. ownership, classes vs. structs/enums, inheritance vs. traits, LINQ vs. iterators, exceptions vs. `Result`, reference types vs. borrowing), but most of these differences do not affect gameplay semantics. The differences that *do* matter are:

1. **Floating-point behavior** — evaluation order, intermediate precision, and rounding can differ between C# and Rust compilers, producing cumulative drift in position calculations
2. **State mutation order** — C# class-based mutation patterns may produce different intermediate states than Rust's explicit-mutation approach
3. **Update ordering** — the sequence in which objects are processed per-frame may differ if not explicitly matched
4. **Object lifetime assumptions** — GC'd objects in C# may linger in ways that affect game logic (e.g., objects remaining hittable longer than expected)
5. **Accumulated drift** — even mathematically equivalent approaches (e.g., incremental `position += velocity * dt` vs. analytical `curve.position_at(t)`) can diverge after thousands of frames

### 7.1 Source of Truth

For every behavioral category, the source of truth is the osu!lazer C# implementation — not the wiki, not community formulas, not documentation.

| Category | Source of Truth | Reference Code (osu!lazer) |
|---|---|---|
| AR/OD/CS/HP scaling | osu!lazer | `BeatmapDifficulty`, `DifficultyApplier` |
| Hit windows (300/100/50/miss thresholds) | osu!lazer | `HitWindows`, `OsuHitWindows` |
| Stacking algorithm (v1 and v2) | osu!lazer | `OsuBeatmapProcessor` |
| Slider path resolution | osu!lazer | `SliderPath`, `PathApproximator` |
| Slider ball position at time t | osu!lazer | `Slider.ProgressAt()` |
| Slider velocity and inherited timing | osu!lazer | `ControlPointInfo`, `DifficultyControlPoint` |
| Note lock behavior | osu!lazer | `OsuHitPolicy` |
| Slider leniency and slider end judgement | osu!lazer | `SliderTailCircle`, `DrawableSlider` |
| Object fade-in/fade-out timing | osu!lazer | `DrawableOsuHitObject` |
| Judgement assignment logic | osu!lazer | `OsuScoreProcessor`, `DrawableHitCircle` |
| Combo counting and reset | osu!lazer | `ScoreProcessor` |
| HP drain model | osu!lazer | `DrainingHealthProcessor` |
| Score calculation (ScoreV2) | osu!lazer | `ScoreProcessor` |
| Spinner completion thresholds | osu!lazer | `SpinnerSpmCalculator` |

When any ambiguity arises during implementation, the resolution process is:

1. Read the relevant osu!lazer C# source code
2. Write a test that captures the observed lazer behavior
3. Implement the Rust version to match the test
4. Verify via the differential test harness (§13.5)

### 7.2 Paradigm-Specific Risks

The following table documents specific C# → Rust translation risks and their mitigations:

| C# Pattern | Rust Equivalent | Risk | Mitigation |
|---|---|---|---|
| `position += velocity * dt` (incremental) | `curve.position_at(t)` (analytical) | Accumulated floating-point drift diverges after many frames | Match lazer's approach exactly: if lazer accumulates, we accumulate; if lazer computes analytically, we compute analytically |
| Mutable object graph with shared references | Owned structs with explicit mutation | State mutation order may differ | Mirror lazer's update sequence in the game loop; document deviations |
| C# `double` IEEE 754 | Rust `f64` IEEE 754 | Generally identical, but compiler optimizations (FMA, reordering) can differ | Use `#[inline(never)]` on critical math paths during validation; compare outputs at ≤ 0.01 osu!px tolerance |
| LINQ `.OrderBy().Where().Select()` chains | Iterator `.sorted_by().filter().map()` chains | Evaluation order and sort stability | Use stable sort; verify intermediate results match |
| `null` checks / nullable references | `Option<T>` | Missing-value semantics may differ at boundaries | Audit all nullable fields in lazer's hit object model |

### 7.3 Behavioral Compliance Statement

> `osu-engine-wasm` is not an independent implementation of osu! rules. It is a behavioral reimplementation of osu!lazer Standard mode. Whenever documentation, formulas, or community understanding conflict with observed lazer behavior, lazer behavior wins.

---

## 8. Component Specifications

### 8.1 `osu-engine-wasm` Crate Layout

```
crates/
  osu-engine/          ← Pure Rust, no WASM deps, unit-tested natively
    src/
      lib.rs
      parser/
        osr.rs         ← .osr binary parser (LZMA + header)
        osu.rs         ← .osu text parser (hit objects, timing, difficulty)
      beatmap/
        hit_object.rs  ← Circle, Slider, Spinner types
        timing.rs      ← TimingPoint, inherited velocity
        curve.rs       ← BezierCurve, CatmullCurve, PerfectArc
        stacking.rs    ← Stack offset computation (v1 and v2)
      replay/
        frame.rs       ← ReplayFrame (t, x, y, keys)
        cursor.rs      ← Cursor interpolation at time t
      mods/
        mod_set.rs     ← Bitmask-to-ModSet conversion
        applicator.rs  ← AR/CS/OD/HP transformations, time scaling
      judge/
        windows.rs     ← Hit window computation from OD
        evaluator.rs   ← Per-note 300/100/50/miss from frame data
      game_state.rs    ← GameState: combines above, queryAt(t) → StateSnapshot
    tests/
      fixtures/        ← .osr + .osu test files (committed, small maps)
      osr_parse.rs
      osu_parse.rs
      curve.rs
      judge.rs
      regression.rs    ← Golden-output regression tests vs. known scores

  osu-engine-wasm/     ← WASM bindings only, thin wrapper
    src/
      lib.rs           ← #[wasm_bindgen] exports
    Cargo.toml
    build.rs           ← wasm-pack build hook
```

### 8.2 Parser: `.osr` Replay

The `.osr` binary format has been stable since 2013. The parser must handle:

- **Header fields**: game mode, version, beatmap hash, player name, replay hash, hit counts (300/100/50/miss/geki/katu), total score, max combo, perfect flag, mods bitmask, life bar graph, timestamp, replay length
- **LZMA-compressed cursor stream**: delta-coded `(Δt, x, y, key_flags)` frames, separated by commas, ending with a `-12345` **seed** frame
- **Online score ID**: **the field width depends on the replay version** — `i64` at `>= 20140721`, **`i32`** at `>= 20121008`, and **absent** below that (TDD §2.1)

The parser must **bound the LZMA payload**: output is capped at 256 MB, enforced *during* decompression rather than after, so a decompression bomb never commits the memory in the first place (§14.1).

The cursor stream additionally carries **four osu!stable frame quirks** — two leading `(256, -500)` sentinel frames, two out-of-order repairs, and a backwards-time rejection — plus an **integer** delta rule. These are not described by the file format; they are behaviors of stable's `ReplayWatcher` that lazer reproduces. Without them, frame timing is wrong at the start of essentially every stable-recorded replay. See TDD §2.4.

**Error contract**: Any parse error returns a typed `EngineError` (never a panic). The WASM binding maps these to typed JS exceptions.

> **Correction (2026-07-14):** this section previously described the score ID as *"(≥ 2018 format): score ID (u64)"*. The 2018 threshold and the fixed width are both wrong — see TDD §2.1. It also called for streaming the decompressed payload "for replays > 10 MB"; the actual requirement is a hard output cap enforced during decompression, which is a stronger and simpler guarantee.

### 8.3 Parser: `.osu` Beatmap

The `.osu` format is INI-like with named sections. The parser must handle:

| Section | Key fields |
|---|---|
| `[General]` | AudioFilename, AudioLeadIn, Mode (must be 0 for Standard) |
| `[Difficulty]` | HPDrainRate, CircleSize, OverallDifficulty, ApproachRate, SliderMultiplier, SliderTickRate |
| `[TimingPoints]` | Time, BeatLength (negative = inherited), Meter, SampleSet, SampleIndex, Volume, Uninherited, Effects |
| `[HitObjects]` | x, y, time, type, hitSound, objectParams, hitSample |

Hit object type byte is a bitmask: bit 0 = circle, bit 1 = slider, bit 3 = spinner, bit 2 = new combo, bits 4-6 = combo color skip, bit 7 = osu!mania hold.

**Timing note**: Inherited timing points (negative BeatLength) set slider velocity multiplier. The effective BPM and beat length for any time `t` must trace back to the last uninherited point. This is the most common source of slider-duration bugs in third-party implementations.

**Behaviors the format does not document** (all required, all verified against lazer — see TDD §3.3, §3.5, §3.6):

- **Early-version offset**: beatmaps of format < 5 get **+24 ms** added to every hit object *and* timing point time. The same offset must be applied to replay frame times, or old maps desync.
- **Perfect-curve downgrades at parse time** (legacy maps): a `P` curve with a control point count != 3 becomes **Bézier**; one with 3 **collinear** points becomes **Linear**.
- **Unknown curve type characters fall back to Catmull**, and are not an error. lazer's switch is `default: case 'C':`.
- **AR defaults to OD** when the `ApproachRate` key is absent (maps predating the field).
- **Difficulty values are clamped after parsing** — a map declaring `OverallDifficulty: 99` is accepted by osu! and silently becomes 10.
- The slider `repeat` field is really the **slide count** (`1` = no repeat), and a value above 9000 is rejected.

### 8.4 Curve Resolver

The curve system is the most technically complex component.

#### 8.4.1 Bézier Curves

osu! uses **composite Bézier** curves: a single slider can chain multiple Bézier segments. The boundary between segments is a **repeated control point** (two consecutive identical points in the control point list). Each segment is an independent Bézier of arbitrary degree.

Implementation requirements:
- De Casteljau algorithm for evaluation (numerically stable; avoids binomial coefficient overflow at high degree)
- Arc-length parameterization: pre-compute a lookup table of (arc_length → parameter) pairs at N=100 samples, then linear-interpolate within the table for O(log N) arc-length-to-point evaluation
- The arc-length parameterization must be within 0.5 osu!px of the real value for the slider ball position computation

#### 8.4.2 Catmull-Rom Curves

osu! uses a specific Catmull-Rom variant where each set of 4 consecutive control points defines one cubic segment, with no knot customization. Arc-length parameterization applies here too.

#### 8.4.3 Perfect Circular Arc

Three non-collinear points define a unique circle. The implementation must:
1. Compute the circumcenter of the three control points
2. Compute the signed arc from point 1 to point 3 via point 2
3. Clamp the arc if `req_length` is shorter than the full arc
4. Handle every degenerate case by falling back to the **Bézier** approximation

**Fallbacks** — all three route to Bézier, *not* to a straight line:

| Condition | Source |
|---|---|
| Control point count != 3 | `SliderPath.cs` L345 |
| Collinear / invalid arc | `SliderPath.cs` L351 |
| Arc would need >= 1000 points | `SliderPath.cs` L359 |

> **Correction (2026-07-14)**: This section previously read *"If the three points
> are nearly collinear (circumradius > 500 osu!px), fall back to linear
> interpolation. This matches osu!lazer's behavior."* **No such 500px rule exists
> anywhere in osu!lazer**, and the degenerate fallback is Bézier, not linear.
> The claim was written without access to `PathApproximator.cs`, which lives in
> osu-framework and was not vendored into `references/` at the time. See
> TDD §4.3 and [ADR-021](./ADR_Registry.md#adr-021-vendor-osu-framework-as-the-curve-specification).

#### 8.4.4 Curve Output Contract

All curve types must expose:

```rust
pub trait Curve {
    /// Position at arc-length fraction t ∈ [0.0, 1.0]
    fn position_at(&self, t: f64) -> Position;
    
    /// Total arc length in osu! pixels
    fn length(&self) -> f64;
    
    /// Pre-computed point buffer for rendering (N evenly-spaced arc-length samples)
    fn render_points(&self, n: usize) -> Vec<Position>;
}
```

### 8.5 Stacking Algorithm

osu! applies "stacking" to prevent notes from being perfectly overlaid. Objects that are within `stack_distance` of each other AND within `stack_threshold` ms are offset diagonally.

Two algorithms exist:

| Version | Applies to | Used in |
|---|---|---|
| v1 (legacy) | Beatmap format < 6 | Older maps |
| v2 (Peppy's) | Beatmap format ≥ 6 | All modern maps |

The algorithm runs once during beatmap load (not per-frame). Output: each hit object gets a `stack_offset: i32` (an integer multiple of `stack_distance / stack_height`).

This is frequently the reason third-party analyzers show circles in the wrong position. **Must be implemented correctly.**

### 8.6 Mod Engine

```rust
pub struct ModSet {
    pub easy: bool,
    pub no_fail: bool,
    pub half_time: bool,
    pub hard_rock: bool,
    pub sudden_death: bool,
    pub double_time: bool,
    pub hidden: bool,
    pub flashlight: bool,
    pub relax: bool,
    pub autopilot: bool,
    pub spun_out: bool,
    pub perfect: bool,
    pub nightcore: bool,       // implies double_time
    pub cinema: bool,
}
```

Mod effect matrix:

| Mod | AR | CS | OD | HP | Time | Visual |
|---|---|---|---|---|---|---|
| EZ | ×0.5 | ×0.5 | ×0.5 | ×0.5 | – | – |
| HR | min(AR×1.4, 10) | min(CS×1.3, 10) | min(OD×1.4, 10) | min(HP×1.4, 10) | – | Y-flip |
| DT/NC | Recompute from adjusted preempt/300window | | | | ×0.667 | – |
| HT | Recompute from adjusted preempt/300window | | | | ×0.75 | – |

**DT/HT note**: AR and OD are not multiplied directly. Instead, the preempt time and hit windows (computed from the base AR/OD) are divided or multiplied by the time factor, and the "effective AR" displayed to the user is back-computed from the adjusted preempt. This is the canonical behavior and avoids the "AR 11 doesn't exist in the formula" class of bugs.

### 8.7 Judge Engine

The judge engine replicates osu!lazer's hit detection per note from replay frame data. **The implementation must match lazer's `OsuHitPolicy`, `DrawableHitCircle`, and `DrawableSlider` behavior exactly** — not a generalized interpretation of the rules.

**Circle judging:**
1. For each hit frame (key state transition → pressed), find the nearest unhit circle within the frame's (x, y)
2. Distance check: `dist(cursor, object_center + stack_offset) ≤ circle_radius`
3. Timing check: `|frame.t - object.t| ≤ hit_window_50`
4. Assign 300/100/50 based on timing; assign miss if object time + miss_window passes with no qualifying hit

**Note lock (resolved — see OQ-2):**
The engine must implement osu!lazer's note lock behavior as defined in `OsuHitPolicy`. When an earlier object is still within its hit window, clicks are consumed by the earlier object even if a later object is closer to the cursor. This is not a "nearest object" heuristic — it is a strict temporal ordering constraint. The exact logic must be traced from lazer's source, as it differs from both stable's behavior and from a naïve "closest object wins" implementation.

**Slider judging:**
1. Head: judged as circle (position + timing)
2. Body: no position check — only key-held check during the slider duration
3. Tail: additional leniency of `ms_per_beat × 0.25` beyond the 50 window (verify exact value against lazer's `SliderTailCircle`)

**Miss conditions:**
- No click within ±miss_window of the object
- Hit outside circle_radius for position-based check
- Key released during slider body (causes slider break, not full miss)

### 8.8 Game State Machine

`GameState` is the central facade. It holds immutable parsed beatmap + replay data and exposes `query_at(t: f64) → StateSnapshot`.

```rust
pub struct StateSnapshot {
    // Cursor
    pub cursor: Position,
    pub keys: KeyFlags,
    
    // Active objects (visible at time t)
    pub visible_circles: Vec<CircleState>,
    pub visible_sliders: Vec<SliderState>,
    pub visible_spinners: Vec<SpinnerState>,
    
    // Score / HUD
    pub combo: u32,
    pub max_combo_so_far: u32,
    pub accuracy: f64,   // 0.0–1.0
    pub hp: f64,         // 0.0–1.0
    pub score: u64,
    
    // Judging (events that occurred at or before t)
    pub recent_judgements: Vec<JudgementEvent>,
    
    // Frame metadata
    pub frame_index: usize,
    pub duration_ms: f64,
}
```

`query_at` must be pure (no mutation of internal state). It must be callable at any `t` in any order (seek backward is O(log n) in the frame list, not O(n) replay).

---

## 9. Public API Contract

The WASM bindings expose a minimal, versioned API. All types crossing the WASM boundary are serialized to plain JS objects via `wasm-bindgen`. Internal `JsValue` use is prohibited — only typed structs with `#[wasm_bindgen]` or `serde_wasm_bindgen`.

```typescript
// @osurender/engine — public TypeScript API

export interface EngineVersion {
  major: number;
  minor: number;
  patch: number;
  git_hash: string;
}

export function version(): EngineVersion;

// ── Loading ──────────────────────────────────────────────────────────────────

export class OsuBeatmap {
  static parse(bytes: Uint8Array): OsuBeatmap;
  
  readonly title: string;
  readonly artist: string;
  readonly version: string;           // difficulty name
  readonly beatmap_hash: string;      // MD5 of source .osu file
  readonly base_ar: number;
  readonly base_cs: number;
  readonly base_od: number;
  readonly base_hp: number;
  readonly object_count: number;
  readonly slider_count: number;
  readonly max_combo: number;
  readonly drain_time_seconds: number;
  
  free(): void;                       // explicit WASM memory release
}

export class OsuReplay {
  static parse(bytes: Uint8Array): OsuReplay;
  
  readonly player_name: string;
  readonly beatmap_hash: string;
  readonly mods: number;              // bitmask
  readonly mod_names: string[];       // ["HardRock", "DoubleTime", …]
  readonly score: number;
  readonly max_combo: number;
  readonly count_300: number;
  readonly count_100: number;
  readonly count_50: number;
  readonly count_miss: number;
  readonly frame_count: number;
  readonly duration_ms: number;
  
  free(): void;
}

// ── Engine ───────────────────────────────────────────────────────────────────

export class GameEngine {
  /** Primary constructor — load both files and build game state machine. */
  static create(beatmap: OsuBeatmap, replay: OsuReplay): GameEngine;
  
  /** Query game state at any time t (milliseconds). O(log n). */
  query(t: number): StateSnapshot;
  
  /** Pre-render slider curve points for all sliders.
   *  Call once after creation; results are cached internally.
   *  n: number of points per curve segment (16–64 recommended). */
  precompute_curves(n: number): void;
  
  /** Get curve render points for slider at object_index.
   *  Returns flat [x0,y0, x1,y1, …] f32 buffer — zero-copy from WASM heap. */
  slider_curve_buffer(object_index: number): Float32Array;
  
  /** Total duration of the replay in milliseconds. */
  readonly duration_ms: number;
  
  free(): void;
}

// ── State Snapshot ────────────────────────────────────────────────────────────

export interface StateSnapshot {
  t: number;

  cursor: { x: number; y: number };
  keys: { k1: boolean; k2: boolean; m1: boolean; m2: boolean; smoke: boolean };

  visible_objects: VisibleObject[];   // sorted by draw order (furthest first)

  combo: number;
  max_combo: number;
  score: number;
  accuracy: number;                   // 0–1
  hp: number;                         // 0–1
  
  recent_judgements: JudgementEvent[];
  
  frame_index: number;
  effective_ar: number;
  effective_cs: number;
  effective_od: number;
  preempt_ms: number;
  fade_in_ms: number;
  circle_radius: number;              // in osu! pixels
}

export type VisibleObject =
  | VisibleCircle
  | VisibleSlider
  | VisibleSpinner;

export interface VisibleCircle {
  kind: "circle";
  object_index: number;
  x: number;                         // stack-adjusted osu!px
  y: number;
  hit_time: number;
  alpha: number;                     // 0–1 fade computed by engine
  approach_scale: number;            // 1 = at hit time, >1 = approaching
  combo_color_index: number;
  combo_number: number;
}

export interface VisibleSlider {
  kind: "slider";
  object_index: number;
  x: number;
  y: number;
  hit_time: number;
  end_time: number;
  repeat: number;
  alpha: number;
  approach_scale: number;
  ball_position: { x: number; y: number } | null;  // null before hit_time
  ball_progress: number;             // 0–1 within current repeat
  combo_color_index: number;
  combo_number: number;
}

export interface VisibleSpinner {
  kind: "spinner";
  object_index: number;
  hit_time: number;
  end_time: number;
  alpha: number;
  completion: number;                // 0–1
}

export interface JudgementEvent {
  object_index: number;
  t: number;
  result: "300" | "100" | "50" | "miss" | "slider_break";
  x: number;
  y: number;
  delta_ms: number;                  // hit timing relative to object time (negative = early)
}
```

**API stability contract**: The TypeScript API above is the v1.0 public surface. All additions in minor versions are additive (new optional fields, new methods). Breaking changes require a major version bump and 6-month deprecation notice.

---

## 10. Data Flow & State Machine

### 10.1 Initialization Flow

```
Host receives .osu bytes + .osr bytes
    │
    ├─► OsuBeatmap.parse(bytes)
    │     • Lexes INI sections
    │     • Validates Mode === 0
    │     • Parses timing points → builds BPM+velocity timeline
    │     • Parses hit objects → applies stacking algorithm
    │     • Returns immutable OsuBeatmap handle
    │
    ├─► OsuReplay.parse(bytes)
    │     • Parses header fields
    │     • LZMA-decompresses cursor stream
    │     • Parses delta-coded frames into sorted Vec<ReplayFrame>
    │     • Returns immutable OsuReplay handle
    │
    └─► GameEngine.create(beatmap, replay)
          • Applies mod transformations (AR/CS/OD/HP, time scaling)
          • Applies stacking (if mod set changes it — HR flips Y first)
          • Builds judge timeline: pre-assigns judgements via full replay scan
          • Returns GameEngine
```

### 10.2 Per-Frame Query Flow

```
host calls engine.query(t)
    │
    ├─► Binary search cursor frame list → interpolate cursor at t
    │
    ├─► Binary search object list for visibility window [t - preempt, t + 300]
    │     • For each visible object:
    │         compute alpha, approach_scale
    │         for sliders: compute ball_position from t and curve buffer
    │
    ├─► Binary search judgement timeline for events ≤ t
    │     • Compute combo, score, accuracy from pre-built timeline
    │
    └─► Return StateSnapshot (stack-allocated, no heap allocation per call)
```

`query` must be **allocation-free** for the common case. The `StateSnapshot` is returned by value. `visible_objects` is a `Vec` allocated once and reused (the engine internally maintains a scratch buffer). On the JS side, the binding returns a JS object assembled from the snapshot; this crossing involves one allocation per call. For 60-fps rendering this is acceptable; for tight loops (e.g. bulk analysis), a `query_raw` method returning a shared memory view will be provided in v1.1.

**Note on testability**: The deterministic `query(t)` design is critical for behavioral validation. Because any game state can be reconstructed from an arbitrary `t` without frame-by-frame replay, it becomes possible to compare `lazer_snapshot(t)` vs `rust_snapshot(t)` at thousands of time points per map. This would be far harder with an `advance_frame()` architecture.

### 10.3 Memory Layout

The WASM linear memory contains:

| Segment | Size estimate | Notes |
|---|---|---|
| Parsed hit objects | ~200 bytes × N objects | Typical map: 500–2000 objects |
| Slider curve buffers | ~32 pts × 8 bytes × S sliders | Typical: 500 sliders |
| Replay frames | ~16 bytes × F frames | Typical: 10,000–50,000 frames |
| Judgement timeline | ~24 bytes × N objects | |
| Scratch / stack | 64 KB | |

For a typical 3-minute map with 300 BPM streams: total WASM heap usage ≈ 4–8 MB. Well within the 256 MB WebAssembly default page limit.

---

## 11. Performance Requirements

These targets were derived from the requirement that the analyzer renders at 60 fps in a browser tab, on a 2022-generation mid-range laptop (Apple M2 / AMD Ryzen 5 6600H equivalent).

| Operation | Target | Measurement method |
|---|---|---|
| `.osr` parse (300 KB, 20K frames) | ≤ 20 ms | `performance.now()` in integration test |
| `.osu` parse (100 KB, 1500 objects) | ≤ 15 ms | `performance.now()` in integration test |
| `GameEngine.create()` including stacking | ≤ 30 ms | `performance.now()` |
| `precompute_curves(32)` for 500 sliders | ≤ 10 ms | `performance.now()` |
| `engine.query(t)` single call | ≤ 0.1 ms | 10K-iteration benchmark in Criterion (native) |
| WASM binary size (gzipped) | ≤ 800 KB | `wasm-pack build --release` + gzip -9 |
| Initial WASM instantiation | ≤ 300 ms | Cold load including streaming compile |
| Memory peak (during parse) | ≤ 30 MB | Chrome DevTools memory snapshot |

---

## 12. Build & Release Pipeline

### 12.1 Repository Layout

```
osu-engine-wasm/
  ├── crates/
  │     ├── osu-engine/          ← pure Rust, no WASM
  │     └── osu-engine-wasm/     ← wasm-bindgen shim
  ├── pkg/                       ← wasm-pack output (gitignored)
  ├── npm/
  │     └── @osurender/engine/   ← TS wrapper, package.json
  ├── benches/                   ← Criterion benchmarks
  ├── fuzz/                      ← cargo-fuzz targets
  ├── tests/
  │     └── fixtures/            ← committed .osr + .osu files
  ├── .github/
  │     └── workflows/
  │           ├── ci.yml
  │           ├── bench.yml
  │           └── release.yml
  └── Cargo.toml                 ← workspace
```

### 12.2 CI Workflow (`ci.yml`)

Triggered on every PR and push to `main`.

```
Steps:
  1. cargo fmt --check
  2. cargo clippy -- -D warnings
  3. cargo test --workspace (native)
  4. wasm-pack build --target web --release (crates/osu-engine-wasm)
  5. Check WASM binary size: fail if > 820 KB gzipped
  6. npm ci && npm run type-check (npm/@osurender/engine)
  7. npm test (Node.js integration tests via Jest + @wasm-pack/core)
  8. cargo bench --no-run (compile check only, not run)
```

### 12.3 Benchmark Workflow (`bench.yml`)

Triggered on pushes to `main` only. Posts performance regression comments to PR via `github-actions-benchmark`.

```
Steps:
  1. cargo bench (Criterion, native target)
  2. Upload benchmark results as JSON artifact
  3. Compare against main branch baseline
  4. Fail and annotate PR if any benchmark regresses > 10%
```

### 12.4 Release Workflow (`release.yml`)

Triggered by pushing a semver tag (`v*`).

```
Steps:
  1. Full CI passes (reuse)
  2. wasm-pack build --target bundler + --target web + --target nodejs
  3. Assemble npm package from pkg/ + npm/
  4. npm publish --access public (@osurender/engine)
  5. Create GitHub Release with WASM binary as attached asset
  6. Update CDN edge (CloudFront invalidation for jsDelivr / unpkg)
```

### 12.5 Toolchain Pinning

```toml
# rust-toolchain.toml
[toolchain]
channel = "1.79.0"        # pin to known-good; update via PR
components = ["rustfmt", "clippy"]
targets = ["wasm32-unknown-unknown"]
```

`wasm-pack` and `wasm-bindgen-cli` versions are pinned in `package.json` `devDependencies`. **No floating version ranges** on build toolchain — this is non-negotiable for reproducible WASM binaries.

---

## 13. Testing Strategy

### 13.1 Unit Tests (Rust, native)

Each module has its own `#[cfg(test)]` block:

- **Parser**: round-trip property tests via `proptest` — generate random-ish valid headers, verify parse-then-serialize produces identical bytes
- **Curve math**: compare against known reference outputs from osu!lazer's `SliderPath` for a suite of 50 hand-crafted sliders (control points + expected positions at t=0, 0.25, 0.5, 0.75, 1.0)
- **Stacking**: reference outputs from osu!lazer; test maps chosen to exercise boundary conditions (maps with ≥3 stacked objects, mixed combo skip, spinner resets)
- **Mod engine**: exhaustive table tests for all mod combinations' effects on AR/CS/OD, compared against the osu! wiki formulas
- **Judge engine**: replays where the exact score is known (from the replay header), verify that our judgements reproduce the header's count_300/count_100/count_50/count_miss
- **Game state**: `query(t)` returns consistent combo/accuracy that matches cumulative judgement timeline at all t

Target: **≥ 90% line coverage** on `osu-engine/src`. Coverage is measured via `cargo-tarpaulin` and reported to Codecov on every CI run.

### 13.2 Fuzz Testing

```
fuzz/
  fuzz_targets/
    fuzz_osr_parse.rs    ← arbitrary bytes → OsuReplay::parse, must not panic
    fuzz_osu_parse.rs    ← arbitrary bytes → OsuBeatmap::parse, must not panic
```

Fuzzing runs in CI for 60 seconds per target per PR. Any panic, `unwrap()` failure, or OOM is a P0 bug. The parsers must return `Err(ParseError::…)` for all malformed input — **no panics in production code paths**.

The fuzz corpus is maintained in `fuzz/corpus/` and seeds are committed (the osu! file format is known; crafting interesting seeds is tractable).

### 13.3 Integration Tests (WASM in Node.js)

The NPM package ships a `test/` directory with Jest tests that load the real WASM binary (not mocked) in Node.js via `@wasm-pack/core`:

```typescript
test("query returns correct cursor interpolation", async () => {
  const engine = await GameEngine.create(beatmap, replay);
  const snap0 = engine.query(0);
  expect(snap0.cursor.x).toBeCloseTo(256, 1);
  // … etc
});
```

Each known-score replay in `tests/fixtures/` has a corresponding integration test that verifies the final state at `t = replay.duration_ms` matches the replay header's hit counts exactly.

### 13.4 Regression Baseline

A set of **20 public replays** (spanning Easy through 8-star maps, all major mod combinations, short and marathon-length maps) are committed as binary fixtures. Their expected `StateSnapshot` at 10 evenly-spaced time points is committed as JSON golden outputs. Any commit that changes any golden output must explicitly update the golden file AND include a comment explaining why the behavior changed.

### 13.5 Differential Test Harness (Lazer Comparison)

This is the **single most important engineering task** for ensuring behavioral correctness. The differential test harness validates that the Rust engine produces identical outputs to osu!lazer for the same inputs.

#### 13.5.1 Architecture

```
beatmap (.osu) + replay (.osr)
        │
        ├──────────────────────┐
        ▼                      ▼
  osu!lazer (C#)         osu-engine-wasm (Rust)
        │                      │
        ▼                      ▼
  state dump (JSON)      state dump (JSON)
        │                      │
        └──────┬───────────────┘
               ▼
         diff comparator
               │
               ▼
         pass / fail report
```

#### 13.5.2 State Dump Format

Both engines produce identical JSON snapshots at each sampled time point:

```json
{
  "t": 15000,
  "cursor": { "x": 256.42, "y": 191.87 },
  "visible_object_indices": [42, 43, 44],
  "combo": 127,
  "max_combo": 127,
  "accuracy": 0.9987,
  "hp": 0.82,
  "score": 458920,
  "judgements_so_far": {
    "300": 120,
    "100": 5,
    "50": 2,
    "miss": 0,
    "slider_break": 0
  }
}
```

#### 13.5.3 Sampling Strategy

For each beatmap+replay pair, state is compared at:

- Every 100 ms throughout the replay (thousands of checkpoints per map)
- Every hit object's exact target time ± 1 ms
- Every combo break point
- Start and end of every slider
- First and last frame of the replay

#### 13.5.4 Tolerance Thresholds

| Field | Tolerance | Rationale |
|---|---|---|
| `cursor.x`, `cursor.y` | ≤ 0.01 osu!px | Interpolation should be identical |
| `combo` | exact (0) | Integer — must match |
| `score` | exact (0) | Integer — must match |
| `accuracy` | ≤ 0.0001 | Floating-point accumulation |
| `hp` | ≤ 0.001 | Drain model floating-point |
| `visible_object_indices` | exact set match | Visibility window must be identical |
| `judgements` | exact match | Any judgement difference is a P0 bug |

#### 13.5.5 Golden Data Generation

A headless osu!lazer build (using `osu.Game.Tests` infrastructure) generates golden state dumps:

1. Load beatmap + replay
2. Drive the game clock to each sample time `t`
3. Extract `ScoreProcessor`, `Playfield`, and `DrawableRuleset` state
4. Serialize to the JSON format above
5. Commit as `tests/golden/{beatmap_hash}_{replay_hash}.json.gz`

The golden data corpus covers:

- **50 beatmaps** spanning all difficulty tiers (Easy → 10 stars+)
- **50 replays** covering all mod combinations in the P0 set
- Maps specifically chosen to exercise edge cases: extreme AR, low CS, high-degree Bézier, snake-in/out sliders, dense stacking, 2B-style overlaps, marathon length, break sections, speed changes

This approach mirrors how browser engines validate against Web Platform Tests and how console emulators validate against ROM test suites.

#### 13.5.6 CI Integration

The differential test suite runs as a CI workflow:

```
Steps:
  1. Build osu-engine-wasm (Rust, native target)
  2. For each golden dataset:
     a. Load beatmap + replay
     b. Run engine.query(t) at all sample points
     c. Compare against golden JSON
     d. Report per-field deltas
  3. Fail CI if any field exceeds tolerance
  4. Generate visual diff report (HTML artifact) for manual review
```

The golden data is updated only when osu!lazer itself changes behavior (tracked via lazer release tags). Any golden data update requires a linked lazer commit hash and an explanation of why the behavior changed.

---

## 14. Security Considerations

The engine consumes untrusted user-provided bytes (`.osr` and `.osu` files). All of the following must hold:

### 14.1 Parser Hardening

- **No `unwrap()` or `expect()` in parser code paths.** Every potential failure returns `Result<T, ParseError>`.
- **Bounded allocations**: the LZMA decompressor must cap output at 256 MB before accepting more input. A crafted `.osr` could otherwise trigger OOM via LZMA bomb.
- **Integer overflow**: all arithmetic on untrusted timing values (delta-t, object times) uses `checked_add` / `saturating_add`. No silent wrapping.
- **String handling**: player name and beatmap metadata are validated as UTF-8 and length-limited to 512 bytes before being exposed to JS.

### 14.2 WASM Sandboxing

WASM itself provides memory isolation — the engine cannot access host memory outside its linear heap. No additional sandboxing is required beyond what the browser provides.

### 14.3 Supply Chain

- Cargo dependencies are pinned to exact versions in `Cargo.lock` (committed)
- `cargo deny check` runs in CI: no known vulnerabilities, no GPL-incompatible licenses
- Only 4 external crates are permitted: `lzma-rs` (decompression), `wasm-bindgen`, `serde` + `serde_wasm_bindgen`. All other functionality is implemented in-house.

### 14.4 Content

The engine processes beatmap coordinate data. There is no user-generated text rendered as HTML. No XSS surface.

---

## 15. Milestones & Timeline

Estimated for a team of 2 engineers (1 Rust-primary, 1 TypeScript-primary) with availability to dedicate 70% of time to this project.

| Milestone | Deliverable | Target |
|---|---|---|
| **M0 — Foundation** | Repo structure, CI skeleton, parser stubs with fuzz targets, `lzma-rs` integration | Week 2 |
| **M1 — Parsers done** | `.osr` + `.osu` parsers pass all unit tests + fuzz clean for 10 min; binary committed to fixtures | Week 5 |
| **M1.5 — Golden Data Pipeline** | Headless lazer golden data generator operational; initial 50-map golden corpus committed | Week 6 |
| **M2 — Curves done** | All three curve types passing reference-output tests AND differential tests against lazer SliderPath; `precompute_curves` tested | Week 8 |
| **M3 — Stacking + Mods** | Stacking v1+v2 passing reference tests; full mod matrix tested; effective AR/CS/OD computation verified against lazer | Week 10 |
| **M4 — Judge Engine** | Per-note 300/100/50/miss matches replay headers for all 20 fixture replays; note lock behavior verified against lazer | Week 13 |
| **M5 — Game State + WASM + Differential Tests** | `GameEngine.create` + `query(t)` wired end-to-end; differential test harness passing for all 50 golden datasets within tolerance | Week 16 |
| **M6 — Integration** | `@osurender/engine` NPM package integrated into `view_player.html` analyzer tab; visual output verified side-by-side against Danser-Go reference video | Week 18 |
| **M7 — Performance** | All benchmarks within targets; WASM binary ≤ 800 KB gzipped; 60 fps on reference hardware | Week 19 |
| **M8 — Hardening** | Fuzz runs extended to 1 hour; coverage ≥ 90%; all 50 golden datasets passing differential tests; edge case maps validated | Week 21 |
| **v1.0 Release** | NPM publish; GitHub release; CDN deployed; differential test dashboard public | Week 23 |

Total estimate: **23 weeks** from kickoff to v1.0. The additional 2 weeks over the original estimate account for the golden data pipeline and differential testing infrastructure.

---

## 16. Risks & Mitigations

| Risk | Probability | Impact | Mitigation |
|---|---|---|---|
| **Behavioral divergence from lazer** — The Rust engine unknowingly implements documented rules instead of actual lazer behavior. osu! has 20 years of accumulated quirks, historical edge cases, and accidental behaviors that are effectively part of the specification. This is the single biggest threat to the project. | Very High | Critical | Differential test harness (§13.5) comparing engine output against golden data from lazer at thousands of time points per map; 50-map golden corpus; CI enforcement; lazer source code is the specification, not the wiki |
| **Floating-point drift** — C# and Rust compilers may produce different floating-point results for mathematically equivalent operations, causing cumulative slider ball drift or judgement window differences | High | High | Match lazer's computational approach exactly (incremental vs. analytical); validate with ≤ 0.01 osu!px tolerance; use differential harness to detect drift over long maps |
| **Curve math divergence** — Our Bézier arc-length approximation differs from lazer's at high curvature | Medium | High | Pre-commit reference tests against lazer's SliderPath output; use same sample count (100) as lazer; differential test with slider-heavy maps |
| **Note lock / hit policy divergence** — Naïve "closest object" heuristic differs from lazer's strict temporal ordering | High | Critical | Trace exact logic from `OsuHitPolicy` in lazer source; dedicated test maps with overlapping objects; differential test comparison |
| **Stacking algorithm divergence** — Two different algorithms (v1/v2) and the boundary between them has subtle version-dependent behavior | High | High | Fixture tests cover 5 maps per stacking version; visual check tool (overlay on reference screenshot); differential harness comparison |
| **WASM binary size creep** | Medium | Low | CI gate at 820 KB; `cargo bloat` run on each release to identify top contributors |
| **LZMA decompressor quality** — `lzma-rs` is community-maintained; may have edge cases | Low | High | Fuzz with format-aware seeds; fall back to `xz2` (C bindings via Emscripten) if critical bugs found |
| **Performance miss** — `query(t)` is too slow for 60 fps rendering | Low | Medium | Early benchmark at M5; if over target, profile with `perf` + LLVM-MCA, consider moving combo/accuracy to pre-computed table |
| **Team Rust skill gap** | Medium | Medium | Rust onboarding: 2 weeks of paired programming; existing crates (`libosu`, `osu-db`) used as reference implementations |
| **wasm-bindgen breaking change** | Low | Medium | Toolchain pin; dedicated upgrade PR process |
| **Lazer behavior changes upstream** — osu!lazer is actively developed; behavior may change between releases | Medium | Medium | Pin golden data to specific lazer release tags; monitor lazer changelog; re-generate golden data on each lazer stable release |

---

## 17. Reference Repositories

Three external codebases serve as the primary reference material for reconstructing osu! Standard game logic. All three are cloned locally under `references/` in the project workspace.

### 17.1 Repository Overview

| Repository | Language | License | Role | Local Path |
|---|---|---|---|---|
| [ppy/osu](https://github.com/ppy/osu) | C# (.NET 8) | MIT | **Primary specification** — the canonical osu!lazer client. All behavioral questions are resolved by reading this source. | `references/osu/` |
| [Wieku/danser-go](https://github.com/Wieku/danser-go) | Go | GPL-3.0 | **Secondary reference** — a mature, battle-tested third-party osu! renderer/replay player. Valuable for its independent reimplementation of game rules (stacking, curves, judging, note lock) and as a cross-validation source. Already in use as the OsuRender video pipeline. | `references/danser-go/` |
| [andrewli336/osu-reverse-mapper](https://github.com/andrewli336/osu-reverse-mapper) | JS (browser) | MIT | **Format reference** — a browser-native tool that generates `.osu` beatmap files and `.osr` replay files from cursor input. Contains working implementations of `.osr` binary encoding (LZMA, ULEB128, osu string format), `.osu` text parsing, beat-snapping, coordinate transforms, and replay frame interpolation — all in plain JS. | `references/osu-reverse-mapper/` |

### 17.2 ppy/osu (osu!lazer) — Primary Specification

The osu!lazer codebase is the **executable specification** for this project (see §7). The relevant code lives across two top-level project directories:

#### Core Framework (`osu.Game/`)

Shared infrastructure used by all rulesets:

| Component | Key File(s) | Relevance |
|---|---|---|
| Beatmap parsing | `Beatmaps/Formats/LegacyBeatmapDecoder.cs` | `.osu` text format parser — the definitive implementation of section parsing, timing point decoding, hit object type bitmask interpretation |
| Timing & control points | `Beatmaps/ControlPoints/ControlPointInfo.cs`, `TimingControlPoint.cs`, `DifficultyControlPoint.cs` | BPM timeline, inherited velocity multipliers — the most common source of slider-duration bugs |
| Hit object base | `Rulesets/Objects/HitObject.cs` | Base class: start time, nested objects, stacking, samples |
| Slider path | `Rulesets/Objects/SliderPath.cs` (524 lines) | Arc-length parameterized slider curves — Bézier, Catmull, PerfectArc, Linear. **Critical reference for §8.4** |
| Path control points | `Rulesets/Objects/PathControlPoint.cs` | Control point type enum (Bézier, Catmull, PerfectCurve, Linear) |
| Hit windows base | `Rulesets/Scoring/HitWindows.cs` (120 lines) | Abstract hit window computation framework |
| Score processor | `Rulesets/Scoring/ScoreProcessor.cs` (667 lines) | Combo, accuracy, score calculation — **the source of truth for §8.7 and §8.8** |
| Health processor | `Rulesets/Scoring/DrainingHealthProcessor.cs` (217 lines) | HP drain model with passive drain rate and recovery — **reference for P1 goal G14** |
| Replay frames | `Replays/Legacy/LegacyReplayFrame.cs`, `Rulesets/Replays/ReplayFrame.cs` | Replay frame structure: cursor position, key state, timestamps |
| Legacy types | `Beatmaps/Legacy/LegacyHitObjectType.cs`, `LegacyControlPointInfo.cs` | Bitmask constants, backward-compatible timing logic |

#### osu! Standard Ruleset (`osu.Game.Rulesets.Osu/`)

Standard-mode-specific game logic:

| Component | Key File(s) | Relevance |
|---|---|---|
| **Stacking algorithm** | `Beatmaps/OsuBeatmapProcessor.cs` (283 lines) | Both v1 and v2 stacking algorithms — **critical for §8.5** |
| **Hit windows** | `Scoring/OsuHitWindows.cs` (67 lines) | OD → hit window (300/100/50) mapping specific to Standard mode |
| **Score processor** | `Scoring/OsuScoreProcessor.cs` | Standard-specific scoring adjustments |
| **Health processor** | `Scoring/OsuHealthProcessor.cs`, `OsuLegacyHealthProcessor.cs` | HP drain/recovery specific to Standard mode |
| **Hit policy (note lock)** | `UI/StartTimeOrderedHitPolicy.cs` (100 lines), `UI/AnyOrderHitPolicy.cs`, `UI/LegacyHitPolicy.cs` | **Critical for note lock behavior (§7.1, §8.7)** — `StartTimeOrderedHitPolicy` is lazer's default |
| **Hit circle** | `Objects/HitCircle.cs`, `Objects/Drawables/DrawableHitCircle.cs` | Circle judging, hit detection, approach circle timing |
| **Slider** | `Objects/Slider.cs`, `Objects/Drawables/DrawableSlider.cs`, `DrawableSliderBall.cs` | Slider duration, repeat count, ball position, body tracking |
| **Slider components** | `Objects/SliderTailCircle.cs`, `SliderHeadCircle.cs`, `SliderEndCircle.cs`, `SliderRepeat.cs`, `SliderTick.cs` | Slider sub-object judging and leniency |
| **Spinner** | `Objects/Spinner.cs`, `Objects/Drawables/DrawableSpinner.cs` | RPM calculation, completion thresholds |
| **Mod implementations** | `Mods/OsuModHardRock.cs`, `OsuModEasy.cs`, `OsuModHidden.cs`, `OsuModFlashlight.cs`, etc. | Mod-specific transformations (HR Y-flip, HD fade timing, etc.) |
| **Judgements** | `Judgements/OsuJudgement.cs`, `OsuHitCircleJudgementResult.cs`, `OsuSliderJudgementResult.cs` | Result types, combo logic |
| **Replay** | `Replays/OsuReplayFrame.cs`, `OsuAutoGenerator.cs` | Standard-specific replay frame structure, auto-play generation (useful for golden data) |
| **Playfield & drawing** | `UI/OsuPlayfield.cs`, `UI/DrawableOsuRuleset.cs` | Object draw order, playfield coordinate system |

### 17.3 Wieku/danser-go — Secondary Reference

danser-go is a full osu! renderer written in Go, with its own independent implementation of game rules. It supports stable, lazer, and ScoreV2 scoring modes. Key value: it provides a **third point of comparison** for behavioral validation and has solved many of the same translation problems we face (different language reimplementing C# logic).

#### Game Logic (`app/rulesets/osu/`)

| Component | Key File(s) | Lines | Relevance |
|---|---|---|---|
| **Ruleset core** | `ruleset.go` | 879 | Central game loop, `Update()`, `UpdateClickFor()`, `CanBeHit()` — implements **both stable and lazer note lock** (`CanBeHitStable()` vs `CanBeHitLazer()`) |
| **Circle judging** | `circle.go` | — | Circle hit detection with position+timing checks |
| **Slider judging** | `slider.go` | — | Slider head/body/tail/tick judging, ball tracking |
| **Spinner** | `spinner.go` | — | RPM computation, completion scoring |
| **Hit results** | `hitresult.go` | — | HitResult enum, score values |
| **Score v1/v2/v3** | `scorev1.go`, `scorev2.go`, `scorev3.go` | — | All three scoring modes implemented independently |
| **Health processor** | `healthprocessor.go`, `healthprocessorv2.go` | — | HP drain for stable and lazer modes |

#### Curve Math (`framework/math/curves/`)

| Component | Key File(s) | Relevance |
|---|---|---|
| **Bézier curves** | `bezier.go`, `bezierapproximator.go` | Bézier evaluation and flattening to line segments |
| **Catmull-Rom** | `catmull.go` | Catmull-Rom spline segments |
| **Circular arc** | `cirarc.go` | Perfect circular arc from 3 points |
| **Multi-curve** | `multicurve.go` (389 lines) | Composite curve assembly with arc-length parameterization — **directly comparable to our §8.4** |
| **B-spline** | `bspline.go` | B-spline support (newer lazer feature) |
| **Linear** | `linear.go` | Linear interpolation fallback |

#### Beatmap Parsing (`app/beatmap/`)

| Component | Key File(s) | Relevance |
|---|---|---|
| **Parser** | `parser.go` (395 lines) | `.osu` file parser — Go implementation, good for cross-validating our parser |
| **Hit objects** | `objects/circle.go`, `objects/slider.go`, `objects/spinner.go`, `objects/hitobject.go` | Object types with timing, position, combo logic |
| **Stacking** | `stackleniency.go` (177 lines) | Go implementation of stacking — **cross-validation reference for §8.5** |
| **Timing** | `objects/timing.go` | Timing point handling |
| **Difficulty** | `difficulty/difficulty.go`, `difficulty/mods.go`, `difficulty/utils.go` | AR/OD/CS/HP calculations, mod effects |

### 17.4 andrewli336/osu-reverse-mapper — Format Reference

A single-file browser app (`script.js`, 1062 lines) that creates `.osu` and `.osr` files from scratch. While simple, it contains working implementations of several format-level concerns:

| Component | Location in `script.js` | Relevance |
|---|---|---|
| **`.osu` parsing** | `parseOsuFile()` (L258–281) | Minimal INI-like section parser |
| **`.osu` generation** | `buildModifiedOsuFile()` (L283–373) | Hit object line format: `x,y,time,type,hitSound,hitSample` — demonstrates type bitmask (bit 0 = circle, bit 2 = new combo) |
| **`.osr` binary encoding** | `generateOsrFile()` (L862–897), `BinaryEncoder` class (L899–948) | Complete `.osr` binary format: game mode byte, version int, osu-strings (0x0B prefix + ULEB128 length), header fields, LZMA-compressed replay data, score ID |
| **Replay frame format** | `recordReplayFrame()` (L849–860) | Delta-coded frames: `[Δt, x, y, buttonMask]` separated by pipes, terminated with `-12345|0|0|seed` |
| **Replay frame interpolation** | `getInterpolatedReplayPosition()` (L375–395) | Linear interpolation between replay frames at arbitrary time — comparable to our cursor interpolation (§8.2) |
| **Beat snapping** | `getBeatSnapper()` (L397–419) | BPM extraction from timing points, beat subdivision calculation |
| **CS → radius** | `csToRadius()` (L54–61) | `radius = 64 * (1 - 0.7 * (cs - 5) / 5)` — the canonical formula |
| **Coordinate transform** | `screenToOsuCoords()` (L251–256) | Screen pixels ↔ osu! playfield (512×384) coordinate mapping |
| **Mod bitmask** | `getSelectedModsValue()` (L1051–1062) | Mod checkbox → bitmask OR accumulation |

### 17.5 Cross-Reference Map: BRD Components → Source Files

This table maps each BRD component to the specific source files across all three reference repositories that should be consulted during implementation:

| BRD Component | ppy/osu (Primary) | danser-go (Secondary) | osu-reverse-mapper |
|---|---|---|---|
| **§8.2 `.osr` parser** | `Replays/Legacy/LegacyReplayFrame.cs` | — | `generateOsrFile()`, `BinaryEncoder` class |
| **§8.3 `.osu` parser** | `Beatmaps/Formats/LegacyBeatmapDecoder.cs` | `beatmap/parser.go` | `parseOsuFile()` |
| **§8.4 Curve resolver** | `Rulesets/Objects/SliderPath.cs` | `framework/math/curves/` (all files) | — |
| **§8.5 Stacking** | `Beatmaps/OsuBeatmapProcessor.cs` | `beatmap/stackleniency.go` | — |
| **§8.6 Mod engine** | `Mods/OsuMod*.cs` files | `beatmap/difficulty/mods.go` | `getSelectedModsValue()` |
| **§8.7 Judge engine** | `UI/StartTimeOrderedHitPolicy.cs`, `Scoring/OsuHitWindows.cs`, `Objects/Drawables/DrawableHitCircle.cs` | `ruleset.go` (`CanBeHit*`, `GetResultForDelta`) | — |
| **§8.8 Game state** | `Scoring/ScoreProcessor.cs`, `Scoring/OsuScoreProcessor.cs` | `score.go`, `scorev1.go`–`scorev3.go` | — |
| **HP drain (P1)** | `Scoring/DrainingHealthProcessor.cs` | `healthprocessor.go`, `healthprocessorv2.go` | — |
| **Replay frames** | `Replays/Legacy/LegacyReplayFrame.cs` | — | `recordReplayFrame()`, `insertReplayFrameAtTime()` |
| **Coordinate system** | `UI/OsuPlayfield.cs` | — | `screenToOsuCoords()` |

### 17.6 Key Implementation Insights from Reference Analysis

From analyzing the three codebases, the following non-obvious implementation details have been identified:

1. **Note lock has three distinct implementations**: lazer uses `StartTimeOrderedHitPolicy` (time-ordered blocking), stable uses stack-aware blocking (danser's `CanBeHitStable`), and there's also `AnyOrderHitPolicy` (no lock). Our engine must support at least the lazer variant.

2. **danser-go implements both stable and lazer judging**: The `CanBeHit()` method in `ruleset.go:610–633` dispatches between `CanBeHitStable()` and `CanBeHitLazer()` based on the Lazer mod flag. This dual implementation is a valuable reference for understanding the behavioral differences.

3. **Hit window computation differs between stable and lazer**: danser's `GetResultForDelta()` (L692–712) shows that lazer uses `<=` comparisons with float windows, while stable uses `<` comparisons with integer-truncated windows. This is exactly the kind of subtle divergence §7 warns about.

4. **The `.osr` format uses osu-strings**: The binary encoder in osu-reverse-mapper shows the format is `0x0B` prefix byte + ULEB128-encoded string length + UTF-8 bytes (or `0x00` for null/empty). This matches the `.osr` spec but is easy to get wrong.

5. **danser-go's multi-curve assembly** (`multicurve.go`) handles the same arc-length parameterization challenges described in §8.4 and is a Go implementation of the same logic in lazer's `SliderPath.cs`.

6. **Stacking leniency is recalculated per-difficulty**: danser's `CalculateStackLeniency()` is called per-player difficulty (L148 in `ruleset.go`), not once per beatmap. This means mods that change preempt (AR) affect stacking.

---

## 18. Dependencies & Prerequisite Work

### 18.1 Blocking Prerequisites

| Item | Owner | Status |
|---|---|---|
| `.osr` fixture collection (50 replays, diverse mods/maps, spanning Easy → 10 stars+) | QA | Not started |
| `.osu` fixture collection (50 maps matching each replay, chosen for edge case coverage) | QA | Not started |
| **Headless osu!lazer golden data generator** — builds headless lazer, drives replays to sample points, extracts state dumps | Eng | Not started — **critical path** |
| Golden state dump corpus (50 maps × thousands of time points each, in JSON format) | Eng | Not started — depends on headless lazer build |
| Decision on CDN hosting for `@osurender/engine` WASM binary | Infra | Not started |
| `view_player.html` tab scaffolding (Analyze tab, canvas, controls) | FE Eng | In progress (current sprint) |

### 18.2 External Crates Under Evaluation

| Crate | Version | Purpose | Risk |
|---|---|---|---|
| `lzma-rs` | 0.3.0 | LZMA stream decompression | Medium — check max-output limits |
| `wasm-bindgen` | 0.2.92 | JS/WASM interop | Low — mature, well maintained |
| `serde` + `serde_wasm_bindgen` | 1.0 / 0.6 | Type serialization at WASM boundary | Low |
| `wasm-pack` | 0.12.1 | Build tooling | Low |

**Deliberate exclusions:**

- `rosu-pp` — star rating calculation is out of scope for v1.0; re-evaluate at v1.1
- `web-sys` / `js-sys` — engine crate must have zero DOM or browser API dependencies
- `rayon` — no multi-threading in default build; use feature flag for the threaded variant

---

## 19. Open Questions

| # | Question | Decision needed by | Owner |
|---|---|---|---|
| OQ-1 | Should `slider_curve_buffer` return a zero-copy view into WASM memory, or always copy to a JS `Float32Array`? Zero-copy is faster but requires the host to not hold the reference across a `query()` call. | M5 | Eng Lead |
| ~~OQ-2~~ | ~~Do we need to reproduce osu!lazer's "note lock" behavior?~~ **RESOLVED**: Yes. Per the behavioral compatibility principle (§7), lazer behavior is the specification. Note lock must be implemented exactly as `OsuHitPolicy` defines it. This is not optional. | ~~M4~~ | ~~Product~~ |
| OQ-3 | Should the NPM package include a reference WebGL2 renderer, or is that always a separate repo? Bundling it increases package size but simplifies integration. | M6 | Eng Lead |
| OQ-4 | Multi-replay support (overlaying two replays' cursors on one playfield) — P2 or defer indefinitely? | Pre-M0 | Product |
| OQ-5 | How are skin textures (hit circle, approach circle, slider body) handled? Loading `.osk` in the browser is feasible but adds complexity to the renderer. For v1.0, can we use a bundled default skin? | M6 | Design / Eng |
| OQ-6 | Do we expose a synchronous `query` API and a `Promise`-based async API, or only one? Sync is simpler but blocks the main thread during `create()`. | M0 | Eng Lead |
| OQ-7 | What osu!lazer release tag should we pin the golden data corpus to? Should we track lazer `master` or only stable releases? | M1.5 | Eng Lead |
| OQ-8 | Should we implement a phased approach where the C# lazer reference server validates judgements initially, and we gradually replace pieces with the Rust implementation as differential tests pass? This reduces risk but adds integration complexity. | M0 | Eng Lead |

---

## 20. Glossary

| Term | Definition |
|---|---|
| **AR (Approach Rate)** | Difficulty parameter controlling how early hit objects appear on screen. Converted to a `preempt` time in milliseconds. |
| **CS (Circle Size)** | Difficulty parameter controlling the radius of hit circles and slider bodies. |
| **OD (Overall Difficulty)** | Difficulty parameter controlling the hit window sizes (300/100/50/miss thresholds). |
| **HP (HP Drain Rate)** | Difficulty parameter controlling HP drain speed and passive drain rate. |
| **Stacking** | Visual offset applied to overlapping objects to prevent them from being hidden behind each other. |
| **Combo** | Count of consecutive successful hits. Resets on a miss. |
| **Preempt** | Time in milliseconds before a hit object's target time when it first becomes visible. Determined by AR. |
| **Approach circle** | Expanding/shrinking ring around a hit circle that reaches the circle's edge at the exact moment the player should hit it. |
| **Slider ball** | Visual indicator of the current position along a slider's path during active gameplay. |
| **Perfect arc** | A slider curve type defined by three control points that lies on a circular arc. |
| **Composite Bézier** | A slider curve type made of multiple chained Bézier segments, delimited by repeated control points. |
| **Judgement** | The result assigned to a player's attempt to hit an object: 300, 100, 50, or miss. |
| **Hit window** | The time range around an object's target time within which a hit is registered. Narrower = higher OD. |
| **LZMA** | Compression algorithm used to encode the cursor frame stream in `.osr` files. |
| **`wasm-bindgen`** | Rust toolchain crate that generates JS glue code for Rust→WASM type crossing. |
| **`wasm-pack`** | Build tool that invokes `cargo build --target wasm32-unknown-unknown` and wasm-bindgen, then assembles an NPM package. |
| **Arc-length parameterization** | Re-mapping a curve's parameter so that equal parameter steps correspond to equal distances along the curve. Required for smooth slider ball movement. |
| **Behavioral reimplementation** | An implementation that aims to reproduce the exact observable behavior of a reference system (osu!lazer), as opposed to an independent implementation from documentation alone. Analogous to how hardware emulators reproduce quirks rather than just documented specifications. |
| **Differential test harness** | A testing system that compares outputs of two implementations (lazer and Rust engine) given identical inputs, identifying behavioral divergences at fine granularity. |
| **Golden data** | Pre-computed reference outputs from the authoritative implementation (osu!lazer), committed as test fixtures. The Rust engine's outputs must match these within defined tolerances. |
| **Note lock** | osu!'s hit policy where earlier objects consume clicks even when a later object is closer to the cursor. Defined by `OsuHitPolicy` in lazer. |

---

*End of BRD. Related: [ADD](./Architecture_Design_Document.md) · [TDD](./Technical_Design_Document.md) · [Test Plan](./Test_Plan.md) · [API Spec](./API_Specification.md) · [Impl Plan](./Implementation_Plan.md) · [ADR Registry](./ADR_Registry.md) · [Security Threat Model](./Security_Threat_Model.md) · [Developer Guide](./Developer_Guide.md)*