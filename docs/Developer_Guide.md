# Developer Onboarding & Contribution Guide
## osu-engine-wasm — Getting Started

| | |
|---|---|
| **Document ID** | ENG-DEV-0048 |
| **Version** | 1.0 |
| **Last Revised** | 2026-06-26 |

---

## Table of Contents

1. [Project Overview](#1-project-overview)
2. [Prerequisites](#2-prerequisites)
3. [Repository Structure](#3-repository-structure)
4. [Development Setup](#4-development-setup)
5. [Build Commands](#5-build-commands)
6. [Testing Commands](#6-testing-commands)
7. [Documentation Map](#7-documentation-map)
8. [Reference Repositories](#8-reference-repositories)
9. [Development Workflow](#9-development-workflow)
10. [Coding Standards](#10-coding-standards)
11. [Common Tasks](#11-common-tasks)
12. [Troubleshooting](#12-troubleshooting)

---

## 1. Project Overview

`osu-engine-wasm` is a **Rust → WebAssembly** game logic engine that behaviorally reimplements osu! Standard mode. It parses `.osr` replay files and `.osu` beatmap files in the browser and provides a pure `query(t)` API returning the complete game state at any time.

> **Core Principle**: This is a behavioral reimplementation of osu!lazer. When our code conflicts with lazer's behavior, **lazer wins**. The C# source code is the executable specification.

---

## 2. Prerequisites

| Tool | Version | Install |
|---|---|---|
| Rust | 1.79.0 (pinned) | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| wasm-pack | 0.12.1 | `cargo install wasm-pack@0.12.1` |
| Node.js | 20 LTS | [nodejs.org](https://nodejs.org) |
| cargo-fuzz | latest | `cargo install cargo-fuzz` |
| cargo-tarpaulin | latest | `cargo install cargo-tarpaulin` |

After cloning, the pinned Rust version will be auto-installed via `rust-toolchain.toml`.

---

## 3. Repository Structure

```
osu-engine-wasm/
├── crates/
│   ├── osu-engine/           ← Pure Rust library (no WASM deps)
│   │   ├── src/
│   │   │   ├── parser/       ← .osr and .osu parsers
│   │   │   ├── beatmap/      ← Hit objects, timing, difficulty
│   │   │   ├── curve/        ← Bézier, Catmull, Arc, SliderPath
│   │   │   ├── stacking/     ← Stack offset algorithms v1/v2
│   │   │   ├── replay/       ← Replay frames, cursor interpolation
│   │   │   ├── mods/         ← Mod bitmask, difficulty transforms
│   │   │   ├── judge/        ← Hit windows, note lock, evaluator
│   │   │   ├── scoring/      ← Combo, accuracy, score
│   │   │   └── engine/       ← GameEngine, query(t), StateSnapshot
│   │   └── tests/
│   └── osu-engine-wasm/      ← Thin WASM binding layer
├── npm/@osurender/engine/    ← NPM package wrapper
├── tests/
│   ├── fixtures/             ← .osr + .osu test files
│   ├── golden/               ← Differential test baselines
│   └── fuzz/                 ← Fuzz corpus and artifacts
├── benches/                  ← Criterion benchmarks
├── docs/                     ← All documentation (you are here)
├── references/               ← Cloned reference repositories
│   ├── osu/                  ← ppy/osu (osu!lazer)
│   ├── danser-go/            ← Wieku/danser-go
│   └── osu-reverse-mapper/   ← andrewli336/osu-reverse-mapper
└── Cargo.toml                ← Workspace root
```

---

## 4. Development Setup

```bash
# Clone the repository
git clone <repo-url> osu-engine-wasm
cd osu-engine-wasm

# Rust toolchain will auto-install from rust-toolchain.toml
rustup show

# Build the workspace
cargo build --workspace

# Run all tests
cargo test --workspace

# Build WASM
wasm-pack build crates/osu-engine-wasm --target web --release

# Install NPM dependencies (for integration tests)
cd npm/@osurender/engine && npm ci && cd -
```

---

## 5. Build Commands

| Command | Purpose |
|---|---|
| `cargo build --workspace` | Build all crates (native) |
| `cargo build --release --workspace` | Release build (native) |
| `wasm-pack build crates/osu-engine-wasm --target web` | Build WASM (browser target) |
| `wasm-pack build crates/osu-engine-wasm --target nodejs` | Build WASM (Node.js target) |
| `wasm-pack build crates/osu-engine-wasm --target bundler` | Build WASM (Webpack/Vite) |

---

## 6. Testing Commands

| Command | Purpose |
|---|---|
| `cargo test --workspace` | All unit tests (native) |
| `cargo test -p osu-engine -- --test-threads=1` | Single-threaded (for debugging) |
| `cargo fuzz run fuzz_osr_parse -- -max_total_time=60` | Fuzz .osr parser for 60s |
| `cargo fuzz run fuzz_osu_parse -- -max_total_time=60` | Fuzz .osu parser for 60s |
| `cargo tarpaulin --workspace --out html` | Generate coverage report |
| `cargo bench` | Run Criterion benchmarks |
| `cd npm/@osurender/engine && npm test` | Jest integration tests |
| `cargo clippy --workspace -- -D warnings` | Lint check |
| `cargo fmt --check` | Format check |
| `cargo deny check` | License + vulnerability check |

---

## 7. Documentation Map

| Document | Path | Purpose |
|---|---|---|
| **BRD** | [BRD.md](./BRD.md) | Business requirements, goals, component specs |
| **ADD** | [Architecture_Design_Document.md](./Architecture_Design_Document.md) | System architecture, component design, data flow |
| **TDD** | [Technical_Design_Document.md](./Technical_Design_Document.md) | Algorithm details, pseudocode, math formulas |
| **Test Plan** | [Test_Plan.md](./Test_Plan.md) | Testing strategy, test cases, CI integration |
| **API Spec** | [API_Specification.md](./API_Specification.md) | Public TypeScript API reference |
| **Impl Plan** | [Implementation_Plan.md](./Implementation_Plan.md) | Phased roadmap, task breakdown, schedule |
| **This Guide** | [Developer_Guide.md](./Developer_Guide.md) | Setup, workflow, coding standards |

**When implementing a component**, read in this order:
1. BRD section for the component (requirements)
2. TDD section (algorithm details + reference code locations)
3. Implementation Plan phase (tasks + acceptance criteria)
4. The actual lazer/danser-go source files listed in the reference table

---

## 8. Reference Repositories

**Four** repos are cloned under `references/` (gitignored — clone them yourself).
**Always consult these when implementing game logic.**

| Repo | Language | When to Use |
|---|---|---|
| `references/osu/` | C# | **Primary**: game rules, decoders, judgement, scoring |
| `references/osu-framework/` | C# | **Primary**: curve math (`PathApproximator`), `Precision` |
| `references/danser-go/` | Go | **Secondary**: cross-validation, already-solved translation patterns |
| `references/osu-reverse-mapper/` | JS | **Format aid only**: binary layout of `.osr` |

> [!WARNING]
> **`ppy/osu` and `ppy/osu-framework` are separate repositories.** Curve
> flattening (`PathApproximator.cs`) lives in **osu-framework**, not osu. L1 was
> originally written believing it lived in `osu/`, implemented against danser-go
> instead, and shipped five divergences from lazer as a result. See
> [ADR-021](./ADR_Registry.md#adr-021-vendor-osu-framework-as-the-curve-specification).
>
> **Before citing a lazer file in code or docs, confirm it resolves on disk.**
> A citation to a file we have not vendored is how fiction enters the spec.

### Cloning

```bash
cd references

git clone --depth 1 https://github.com/ppy/osu.git
git clone --depth 1 https://github.com/Wieku/danser-go.git
git clone --depth 1 https://github.com/andrewli336/osu-reverse-mapper.git

# osu-framework: sparse checkout — we only need Utils/ (a full clone is ~500 MB)
git clone --depth 1 --filter=blob:none --sparse https://github.com/ppy/osu-framework.git
cd osu-framework && git sparse-checkout set osu.Framework/Utils && cd -
```

Quick reference lookup table (see [Implementation Plan §19](./Implementation_Plan.md#19-source-code-reference-guide) for full table):

```
Parser (.osu):   references/osu/osu.Game/Beatmaps/Formats/LegacyBeatmapDecoder.cs
                 references/osu/osu.Game/Beatmaps/Formats/Parsing.cs
Parser (.osr):   references/osu/osu.Game/Scoring/Legacy/LegacyScoreDecoder.cs   ← spec
                 references/osu/osu.Game/Replays/Legacy/LegacyReplayFrame.cs
                 references/osu-reverse-mapper/script.js L862–948               ← format aid
Hit windows:     references/osu/osu.Game.Rulesets.Osu/Scoring/OsuHitWindows.cs
Note lock:       references/osu/osu.Game.Rulesets.Osu/UI/StartTimeOrderedHitPolicy.cs
Stacking:        references/osu/osu.Game.Rulesets.Osu/Beatmaps/OsuBeatmapProcessor.cs
Curve flattening: references/osu-framework/osu.Framework/Utils/PathApproximator.cs
Slider path:     references/osu/osu.Game/Rulesets/Objects/SliderPath.cs
                 references/danser-go/framework/math/curves/
```

> [!NOTE]
> `.osr` parsing previously listed `osu-reverse-mapper/script.js` as its primary
> reference. lazer has its own decoder — `LegacyScoreDecoder.cs` — and BRD §7.1
> makes lazer the specification. Use the third-party JS only to disambiguate raw
> byte layout, never to settle a behavioral question.

---

## 9. Development Workflow

```mermaid
graph LR
    A["1. Read lazer source"] --> B["2. Write Rust code"]
    B --> C["3. Write tests"]
    C --> D["4. Run cargo test"]
    D --> E["5. Submit PR"]
    E --> F["6. CI checks"]
    F --> G["7. Code review"]
    G --> H["8. Merge"]
```

### Commit Convention

```
<type>(<scope>): <description>

Types: feat, fix, test, perf, docs, refactor, ci, chore
Scope: parser, curves, stacking, mods, judge, scoring, engine, wasm, ci
```

### PR Requirements

- [ ] All CI checks pass
- [ ] New/modified code has tests
- [ ] Coverage hasn't decreased
- [ ] Code follows standards (§10)
- [ ] At least one approval

---

## 10. Coding Standards

### 10.1 Rust

- **No `unwrap()` or `expect()` in parser/engine code paths.** Always use `Result` or `Option` handling.
- **No panics reachable from user input.** The only acceptable panics are logic errors (bugs) that should never occur.
- All public items have `///` doc comments.
- `cargo fmt` and `cargo clippy -- -D warnings` must pass.
- Use `f64` for all game math (match C# `double`).
- Use stable sort (`sort_by` not `sort_unstable_by`) for object ordering.
- Prefer `enum` dispatch over `dyn Trait` in hot paths.
- Mark performance-critical functions with `#[inline]`.

### 10.2 Constants

```rust
// Always define constants with the source reference:

/// Maximum distance for stacking (osu!px)
/// Source: OsuBeatmapProcessor.cs L22
const STACK_DISTANCE: f64 = 3.0;

/// Fixed miss window (ms)
/// Source: OsuHitWindows.cs L19
const MISS_WINDOW: f64 = 400.0;
```

### 10.3 Error Handling

```rust
// Correct
fn parse_something(input: &[u8]) -> Result<Thing, ParseError> {
    let value = input.get(0).ok_or(ParseError::TruncatedInput)?;
    // ...
}

// Wrong
fn parse_something(input: &[u8]) -> Thing {
    let value = input[0]; // panics on empty input!
    // ...
}
```

---

## 11. Common Tasks

### Adding a New Test Fixture

1. Place `.osu` file in `tests/fixtures/beatmaps/`
2. Place `.osr` file in `tests/fixtures/replays/`
3. Update fixture manifest (JSON)
4. Add corresponding test in the relevant module

### Updating Golden Data

1. Update `LAZER_VERSION.txt` with the new lazer release tag
2. Re-run golden data generation script
3. Commit updated `.json.gz` files
4. Document why behavior changed in the PR description

### Adding a New Module

1. Create `src/module_name/mod.rs`
2. Add `pub mod module_name;` to `src/lib.rs`
3. Add `#[cfg(test)] mod tests { }` in the module
4. Update the Architecture_Design_Document if it changes component boundaries
5. If the module introduces an architectural decision, create a new entry in [ADR_Registry.md](./ADR_Registry.md)

### Adding a New Dependency

> **Strict policy**: The engine has a **maximum of 4 direct production dependencies**. Any new dependency requires an ADR.

1. Justify the dependency in a PR description
2. Check license with `cargo deny check licenses`
3. Check advisories with `cargo deny check advisories`
4. Pin the exact version in `Cargo.toml` (use `=x.y.z`)
5. Commit the updated `Cargo.lock`
6. Verify WASM binary size hasn't exceeded 800 KB gzipped

---

## 12. Deterministic Build & Release

### 12.1 Pinned Toolchain

| Component | Version | Pin File |
|---|---|---|
| Rust compiler | 1.79.0 | `rust-toolchain.toml` |
| wasm-pack | 0.12.1 | CI workflow |
| binaryen (wasm-opt) | 116 | CI workflow |
| Node.js | 20 LTS | `.nvmrc` |

### 12.2 Dependency Policy

- All production deps use **exact version pinning** (`=x.y.z`)
- `Cargo.lock` is committed and reviewed on every change
- `cargo deny check` runs on every CI build (advisories + licenses)
- Any `Cargo.lock` diff in a PR triggers a dedicated dependency review

### 12.3 Reproducible Build Verification

```bash
# Build twice from clean state, compare hashes
wasm-pack build --release --target web crates/osu-engine-wasm
sha256sum pkg/osu_engine_wasm_bg.wasm > build1.sha
cargo clean
wasm-pack build --release --target web crates/osu-engine-wasm
sha256sum pkg/osu_engine_wasm_bg.wasm > build2.sha
diff build1.sha build2.sha  # Must match
```

### 12.4 Release Checklist

1. All CI green on release commit
2. `cargo deny check` clean
3. WASM binary ≤ 800 KB gzipped
4. SHA-256 of `.wasm` published in GitHub Release notes
5. NPM publish with `--provenance` flag
6. Tag follows SemVer: `v{major}.{minor}.{patch}`

---

## 13. Browser Capability Quick Reference

| Feature | Required? | Minimum Browser | Fallback |
|---|---|---|---|
| WebAssembly MVP | **Yes** | Chrome 57, Firefox 52, Safari 11 | None — hard requirement |
| BigInt | **Yes** | Chrome 67, Firefox 68, Safari 14 | None — required for score IDs |
| SharedArrayBuffer | No | Chrome 68+, Firefox 79+ (w/ COOP/COEP headers) | Single-threaded mode |
| WASM SIMD | No | Chrome 91+, Firefox 89+ | Scalar fallback |
| FinalizationRegistry | No | Chrome 84+, Firefox 79+ | Manual `.free()` only |
| OffscreenCanvas | No | Chrome 69+, Firefox 105+ | Main-thread rendering |

**Required HTTP headers for threaded build**:
```
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
```

---

## 14. Troubleshooting

| Issue | Solution |
|---|---|
| `wasm-pack build` fails "target not installed" | `rustup target add wasm32-unknown-unknown` |
| LZMA decompression panic | Check `lzma-rs` version matches `Cargo.lock` pin |
| CI fails "WASM too large" | Run `cargo bloat --release -n 20` to identify size contributors |
| Fuzz crash found | `cargo fuzz tmin <target> <artifact>` to minimize, commit to `fuzz/artifacts/` |
| Coverage below 90% | `cargo tarpaulin --workspace --out html`, open `tarpaulin-report.html` |
| Golden data mismatch | Check `LAZER_VERSION.txt` — has lazer changed behavior? |
| `INVALID_HANDLE` error in JS | You're using an object after `.free()`. Check lifecycle. See [ADR-007](./ADR_Registry.md#adr-007-handle-based-ownership-model). |
| `BATCH_TOO_LARGE` error | Reduce batch to ≤ 65,536 samples per call. Use `query_range()` for large ranges. |
| `Float32Array` is detached | You held a zero-copy view across an engine call. Copy it first: `new Float32Array(view)`. See [API Spec §19](./API_Specification.md#19-zero-copy-buffer-lifetime-guarantees). |
| Performance regression in CI | Check `cargo bench` output. If >10%, investigate with `cargo flamegraph`. |
| Different results on ARM vs x86 | May be floating-point divergence. Run cross-platform benchmark suite. See [ADR-013](./ADR_Registry.md#adr-013-measure-first-floating-point-strategy). |
| `cargo deny` advisory failure | Check if CVE applies to our usage. If not, add exception to `deny.toml` with justification. |

---

## 15. Documentation Map

| Document | Path | Purpose |
|---|---|---|
| **BRD** | [BRD.md](./BRD.md) | Business requirements, goals, component specs |
| **ADD** | [Architecture_Design_Document.md](./Architecture_Design_Document.md) | System architecture, component design, data flow, cache, lifecycle |
| **TDD** | [Technical_Design_Document.md](./Technical_Design_Document.md) | Algorithm details, pseudocode, math formulas |
| **Test Plan** | [Test_Plan.md](./Test_Plan.md) | Testing strategy, test cases, CI, benchmark corpus |
| **API Spec** | [API_Specification.md](./API_Specification.md) | TypeScript API reference, batch API, error taxonomy |
| **Impl Plan** | [Implementation_Plan.md](./Implementation_Plan.md) | Phased roadmap, task breakdown, schedule |
| **ADR Registry** | [ADR_Registry.md](./ADR_Registry.md) | 16 formalized architecture decision records |
| **Security Threat Model** | [Security_Threat_Model.md](./Security_Threat_Model.md) | Trust boundaries, STRIDE analysis, supply chain |
| **This Guide** | [Developer_Guide.md](./Developer_Guide.md) | Setup, workflow, coding standards, troubleshooting |

**When implementing a component**, read in this order:
1. BRD section for the component (requirements)
2. Relevant ADRs (architectural constraints)
3. TDD section (algorithm details + reference code locations)
4. Implementation Plan phase (tasks + acceptance criteria)
5. The actual lazer/danser-go source files listed in the reference table

---

*End of Developer Guide. Related: [BRD](./BRD.md) · [ADD](./Architecture_Design_Document.md) · [TDD](./Technical_Design_Document.md) · [Test Plan](./Test_Plan.md) · [API Spec](./API_Specification.md) · [Impl Plan](./Implementation_Plan.md) · [ADR Registry](./ADR_Registry.md) · [Security Threat Model](./Security_Threat_Model.md)*
