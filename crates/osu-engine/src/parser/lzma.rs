//! LZMA decompression wrapper with safety limits.
//!
//! osu! replay files (`.osr`) compress the cursor frame stream with LZMA.
//! This module wraps the `lzma-rs` crate with a 256 MB output cap
//! to prevent LZMA bomb attacks.
//!
//! ## Security
//!
//! - Maximum decompressed output: 256 MB (BRD §14.1)
//! - Returns `EngineError::DecompressionOutputTooLarge` if exceeded
//! - Returns `EngineError::LzmaDecompressionFailed` for corrupt data
//!
//! ## Status: Stub — implementation in L2

use crate::error::EngineResult;

/// Maximum decompressed output size in bytes (256 MB).
/// Prevents LZMA bomb attacks from crafted .osr files.
pub const MAX_DECOMPRESS_SIZE: usize = 256 * 1024 * 1024;

/// Decompresses LZMA-encoded data with a size limit.
///
/// # Stub
/// Returns `EngineError::LzmaDecompressionFailed` until L2 implementation.
pub fn decompress_lzma(_data: &[u8]) -> EngineResult<Vec<u8>> {
    // TODO(L2): Implement bounded LZMA decompression
    Err(crate::error::EngineError::LzmaDecompressionFailed {
        source: "not implemented".to_string(),
    })
}
