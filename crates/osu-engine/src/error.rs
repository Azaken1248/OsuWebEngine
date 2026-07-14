//! Error taxonomy for all engine operations.
//!
//! Categorized per API Specification §17 into 6 domains:
//! - **Parse** (`P-*`): Invalid or corrupt input files
//! - **Decompression** (`D-*`): LZMA decompression failures
//! - **Validation** (`V-*`): Semantically invalid but parseable data
//! - **Engine** (`E-*`): Runtime query failures
//! - **Memory** (`M-*`): WASM memory allocation failures
//! - **Internal** (`I-*`): Bug in engine code (should never happen)

/// Unified error type for all engine operations.
#[derive(Debug, Clone, PartialEq)]
pub enum EngineError {
    // ── Parse errors (P-*) ─────────────────────────────────────────────
    /// Expected magic bytes not found at the start of input.
    InvalidMagic {
        expected: &'static str,
        found: Vec<u8>,
    },

    /// Unexpected end of input during parsing.
    UnexpectedEof {
        context: &'static str,
        offset: usize,
    },

    /// Invalid UTF-8 sequence in a string field.
    InvalidUtf8 {
        context: &'static str,
        offset: usize,
    },

    /// A parsed field has an invalid or unrecognized value.
    MalformedField { field: &'static str, value: String },

    /// File format version is not supported.
    UnsupportedVersion { version: i32 },

    /// A ULEB128-encoded length used more continuation bytes than can fit
    /// in the target integer. Guards against a crafted `.osr` spinning the
    /// decode loop or wrapping the accumulator.
    UlebOverflow { context: &'static str },

    /// An osu-string declared a length beyond the accepted maximum.
    StringTooLong { context: &'static str, len: usize },

    /// An osu-string began with a byte other than `0x00` (empty) or
    /// `0x0B` (string follows).
    InvalidStringMarker { context: &'static str, marker: u8 },

    /// A required `.osu` section was absent.
    MissingSection { section: &'static str },

    /// A numeric field parsed successfully but fell outside the range lazer
    /// accepts (e.g. a coordinate beyond `MAX_COORDINATE_VALUE`).
    ///
    /// Source: `Parsing.cs` — lazer throws rather than clamping.
    ValueOutOfRange {
        field: &'static str,
        value: f64,
        limit: f64,
    },

    // ── Decompression errors (D-*) ─────────────────────────────────────
    /// LZMA decompression failed.
    LzmaDecompressionFailed { source: String },

    /// Decompressed output exceeds the safety limit.
    DecompressionOutputTooLarge {
        limit_bytes: usize,
        actual_bytes: usize,
    },

    /// Decompression timed out (anti-LZMA-bomb).
    DecompressionTimeout { elapsed_ms: u64, limit_ms: u64 },

    // ── Validation errors (V-*) ────────────────────────────────────────
    /// Game mode is not osu! Standard (mode 0).
    InvalidGameMode { mode: u8, expected: u8 },

    /// Timestamp is outside the valid range for this replay/beatmap.
    TimestampOutOfRange { timestamp_ms: f64, duration_ms: f64 },

    /// Replay has no cursor frames after decompression.
    EmptyReplayFrames,

    /// Beatmap has no hit objects.
    EmptyHitObjects,

    // ── Engine errors (E-*) ────────────────────────────────────────────
    /// Engine was not initialized before querying.
    EngineNotInitialized,

    /// Query time is outside the replay's time range.
    QueryOutOfRange { t: f64, min: f64, max: f64 },

    /// Batch query exceeds the maximum sample count.
    BatchTooLarge { count: usize, limit: usize },

    // ── Memory errors (M-*) ────────────────────────────────────────────
    /// WASM linear memory allocation failed.
    WasmAllocationFailed { requested_bytes: usize },

    /// Handle ID does not refer to a valid object.
    HandleInvalid { handle: u32 },

    /// Handle was already freed (use-after-free).
    HandleAlreadyFreed { handle: u32 },

    // ── Internal errors (I-*) ──────────────────────────────────────────
    /// Internal invariant violation — this is a bug.
    InternalError { message: String },
}

impl EngineError {
    /// Returns the error category string.
    pub fn category(&self) -> &'static str {
        match self {
            Self::InvalidMagic { .. }
            | Self::UnexpectedEof { .. }
            | Self::InvalidUtf8 { .. }
            | Self::MalformedField { .. }
            | Self::UnsupportedVersion { .. }
            | Self::UlebOverflow { .. }
            | Self::StringTooLong { .. }
            | Self::InvalidStringMarker { .. }
            | Self::MissingSection { .. }
            | Self::ValueOutOfRange { .. } => "parse",

            Self::LzmaDecompressionFailed { .. }
            | Self::DecompressionOutputTooLarge { .. }
            | Self::DecompressionTimeout { .. } => "decompression",

            Self::InvalidGameMode { .. }
            | Self::TimestampOutOfRange { .. }
            | Self::EmptyReplayFrames
            | Self::EmptyHitObjects => "validation",

            Self::EngineNotInitialized
            | Self::QueryOutOfRange { .. }
            | Self::BatchTooLarge { .. } => "engine",

            Self::WasmAllocationFailed { .. }
            | Self::HandleInvalid { .. }
            | Self::HandleAlreadyFreed { .. } => "memory",

            Self::InternalError { .. } => "internal",
        }
    }

    /// Returns the specific error code (e.g., `"P-001"`).
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidMagic { .. } => "P-001",
            Self::UnexpectedEof { .. } => "P-002",
            Self::InvalidUtf8 { .. } => "P-003",
            Self::MalformedField { .. } => "P-004",
            Self::UnsupportedVersion { .. } => "P-005",
            Self::UlebOverflow { .. } => "P-006",
            Self::StringTooLong { .. } => "P-007",
            Self::InvalidStringMarker { .. } => "P-008",
            Self::MissingSection { .. } => "P-009",
            Self::ValueOutOfRange { .. } => "P-010",
            Self::LzmaDecompressionFailed { .. } => "D-001",
            Self::DecompressionOutputTooLarge { .. } => "D-002",
            Self::DecompressionTimeout { .. } => "D-003",
            Self::InvalidGameMode { .. } => "V-001",
            Self::TimestampOutOfRange { .. } => "V-002",
            Self::EmptyReplayFrames => "V-003",
            Self::EmptyHitObjects => "V-004",
            Self::EngineNotInitialized => "E-001",
            Self::QueryOutOfRange { .. } => "E-002",
            Self::BatchTooLarge { .. } => "E-003",
            Self::WasmAllocationFailed { .. } => "M-001",
            Self::HandleInvalid { .. } => "M-002",
            Self::HandleAlreadyFreed { .. } => "M-003",
            Self::InternalError { .. } => "I-001",
        }
    }
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidMagic { expected, found } => {
                write!(
                    f,
                    "[P-001] Expected magic {:?}, found {:?}",
                    expected, found
                )
            }
            Self::UnexpectedEof { context, offset } => {
                write!(
                    f,
                    "[P-002] Unexpected EOF in {} at offset {}",
                    context, offset
                )
            }
            Self::InvalidUtf8 { context, offset } => {
                write!(
                    f,
                    "[P-003] Invalid UTF-8 in {} at offset {}",
                    context, offset
                )
            }
            Self::MalformedField { field, value } => {
                write!(f, "[P-004] Malformed field '{}': {:?}", field, value)
            }
            Self::UnsupportedVersion { version } => {
                write!(f, "[P-005] Unsupported format version: {}", version)
            }
            Self::UlebOverflow { context } => {
                write!(f, "[P-006] ULEB128 length overflow in {}", context)
            }
            Self::StringTooLong { context, len } => {
                write!(f, "[P-007] String in {} too long: {} bytes", context, len)
            }
            Self::InvalidStringMarker { context, marker } => {
                write!(
                    f,
                    "[P-008] Invalid osu-string marker {:#04x} in {}",
                    marker, context
                )
            }
            Self::MissingSection { section } => {
                write!(f, "[P-009] Missing required section [{}]", section)
            }
            Self::ValueOutOfRange {
                field,
                value,
                limit,
            } => {
                write!(
                    f,
                    "[P-010] Field '{}' value {} exceeds limit {}",
                    field, value, limit
                )
            }
            Self::LzmaDecompressionFailed { source } => {
                write!(f, "[D-001] LZMA decompression failed: {}", source)
            }
            Self::DecompressionOutputTooLarge {
                limit_bytes,
                actual_bytes,
            } => {
                write!(
                    f,
                    "[D-002] Decompressed output {} bytes exceeds limit {} bytes",
                    actual_bytes, limit_bytes
                )
            }
            Self::DecompressionTimeout {
                elapsed_ms,
                limit_ms,
            } => {
                write!(
                    f,
                    "[D-003] Decompression timed out at {}ms (limit {}ms)",
                    elapsed_ms, limit_ms
                )
            }
            Self::InvalidGameMode { mode, expected } => {
                write!(
                    f,
                    "[V-001] Game mode {} is not {} (Standard)",
                    mode, expected
                )
            }
            Self::TimestampOutOfRange {
                timestamp_ms,
                duration_ms,
            } => {
                write!(
                    f,
                    "[V-002] Timestamp {}ms outside range [0, {}ms]",
                    timestamp_ms, duration_ms
                )
            }
            Self::EmptyReplayFrames => write!(f, "[V-003] Replay has no cursor frames"),
            Self::EmptyHitObjects => write!(f, "[V-004] Beatmap has no hit objects"),
            Self::EngineNotInitialized => write!(f, "[E-001] Engine not initialized"),
            Self::QueryOutOfRange { t, min, max } => {
                write!(
                    f,
                    "[E-002] Query time {}ms outside range [{}, {}]ms",
                    t, min, max
                )
            }
            Self::BatchTooLarge { count, limit } => {
                write!(f, "[E-003] Batch size {} exceeds limit {}", count, limit)
            }
            Self::WasmAllocationFailed { requested_bytes } => {
                write!(
                    f,
                    "[M-001] WASM allocation failed for {} bytes",
                    requested_bytes
                )
            }
            Self::HandleInvalid { handle } => {
                write!(f, "[M-002] Invalid handle: {}", handle)
            }
            Self::HandleAlreadyFreed { handle } => {
                write!(f, "[M-003] Handle {} already freed", handle)
            }
            Self::InternalError { message } => {
                write!(f, "[I-001] Internal error: {}", message)
            }
        }
    }
}

impl std::error::Error for EngineError {}

/// Convenience result alias for all engine operations.
pub type EngineResult<T> = Result<T, EngineError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_are_correct() {
        let err = EngineError::InvalidMagic {
            expected: "osr",
            found: vec![0x00],
        };
        assert_eq!(err.code(), "P-001");
        assert_eq!(err.category(), "parse");
    }

    #[test]
    fn error_display_is_human_readable() {
        let err = EngineError::UnexpectedEof {
            context: "osr header",
            offset: 42,
        };
        let msg = format!("{}", err);
        assert!(msg.contains("P-002"));
        assert!(msg.contains("osr header"));
        assert!(msg.contains("42"));
    }

    #[test]
    fn all_categories_covered() {
        // Verify each category has at least one variant
        let categories: Vec<&str> = vec![
            EngineError::InvalidMagic {
                expected: "",
                found: vec![],
            }
            .category(),
            EngineError::LzmaDecompressionFailed {
                source: String::new(),
            }
            .category(),
            EngineError::InvalidGameMode {
                mode: 0,
                expected: 0,
            }
            .category(),
            EngineError::EngineNotInitialized.category(),
            EngineError::WasmAllocationFailed { requested_bytes: 0 }.category(),
            EngineError::InternalError {
                message: String::new(),
            }
            .category(),
        ];
        assert_eq!(
            categories,
            vec![
                "parse",
                "decompression",
                "validation",
                "engine",
                "memory",
                "internal"
            ]
        );
    }
}
