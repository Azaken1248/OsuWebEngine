//! L5: Timeline Pipelines — judgement, score, and visibility computation.
//!
//! Pipeline Stages 3–5: Three independent pipeline stages that produce
//! sorted timelines consumed by the query engine (L6).
//!
//! - [`judgement`]: Stage 3 — replay scan producing per-object judgements
//! - [`score`]: Stage 4 — score/combo/accuracy accumulation from judgements
//! - [`visibility`]: Stage 5 — object appear/fade timing from AR
//!
//! ## Status: Stubs — implementation in L5

pub mod judgement;
pub mod score;
pub mod visibility;
