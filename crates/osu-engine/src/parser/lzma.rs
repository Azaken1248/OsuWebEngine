//! LZMA decompression wrapper with safety limits.
//!
//! osu! replay files (`.osr`) compress the cursor frame stream with raw LZMA
//! (the "alone" format: a 13-byte header, then the stream). This module wraps
//! `lzma-rs` with an output cap so a crafted `.osr` cannot exhaust memory.
//!
//! ## The threat
//!
//! LZMA's compression ratio is unbounded in principle — a few hundred bytes of
//! input can declare gigabytes of output. Both the declared size and the actual
//! produced output must be bounded, and the declared size (which is
//! attacker-controlled) must never be used to pre-allocate.
//!
//! ## Security
//!
//! - Output capped at 256 MB (BRD §14.1)
//! - Returns `EngineError::DecompressionOutputTooLarge` when exceeded
//! - Returns `EngineError::LzmaDecompressionFailed` for corrupt data
//! - Never panics

use crate::error::{EngineError, EngineResult};
use std::io::Write;

/// Maximum decompressed output size in bytes (256 MB).
///
/// Source: BRD §14.1, ADR-018.
pub const MAX_DECOMPRESS_SIZE: usize = 256 * 1024 * 1024;

/// A sink that refuses to accept more than `limit` bytes.
///
/// This is the load-bearing piece: `lzma-rs` streams into a `Write`, so
/// enforcing the cap here stops decompression *while it runs* rather than
/// after the fact. Checking the output size afterwards would be too late —
/// the memory would already be committed.
struct BoundedWriter {
    buf: Vec<u8>,
    limit: usize,
    overflowed: bool,
}

impl BoundedWriter {
    fn new(limit: usize) -> Self {
        BoundedWriter {
            buf: Vec::new(),
            limit,
            overflowed: false,
        }
    }
}

impl Write for BoundedWriter {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        if self.buf.len().saturating_add(data.len()) > self.limit {
            self.overflowed = true;
            // Abort the stream. lzma-rs surfaces this as a decode error, which
            // is translated back into DecompressionOutputTooLarge below.
            return Err(std::io::Error::new(
                std::io::ErrorKind::WriteZero,
                "decompression output limit exceeded",
            ));
        }

        self.buf.extend_from_slice(data);
        Ok(data.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Decompresses raw LZMA data with a 256 MB output cap.
pub fn decompress_lzma(data: &[u8]) -> EngineResult<Vec<u8>> {
    decompress_lzma_with_limit(data, MAX_DECOMPRESS_SIZE)
}

/// Decompresses raw LZMA data with an explicit output cap.
///
/// Exposed separately so tests can drive the bomb path with a small limit
/// instead of having to actually produce 256 MB of output.
pub fn decompress_lzma_with_limit(data: &[u8], limit: usize) -> EngineResult<Vec<u8>> {
    if data.is_empty() {
        return Ok(Vec::new());
    }

    let mut reader = std::io::Cursor::new(data);
    let mut writer = BoundedWriter::new(limit);

    match lzma_rs::lzma_decompress(&mut reader, &mut writer) {
        Ok(()) => Ok(writer.buf),

        Err(e) => {
            // A cap hit and genuine corruption both surface as a decode error;
            // the flag tells them apart so the caller gets an actionable code.
            if writer.overflowed {
                Err(EngineError::DecompressionOutputTooLarge {
                    limit_bytes: limit,
                    actual_bytes: writer.buf.len(),
                })
            } else {
                Err(EngineError::LzmaDecompressionFailed {
                    source: e.to_string(),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_yields_empty_output() {
        assert_eq!(decompress_lzma(&[]).unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn corrupt_data_is_an_error_not_a_panic() {
        let garbage = [0xFF, 0x00, 0xDE, 0xAD, 0xBE, 0xEF, 0x42, 0x13, 0x37];

        match decompress_lzma(&garbage) {
            Err(EngineError::LzmaDecompressionFailed { .. }) => {}
            Err(EngineError::DecompressionOutputTooLarge { .. }) => {}
            other => panic!("expected a decompression error, got {:?}", other),
        }
    }

    #[test]
    fn round_trips_real_lzma() {
        let original = b"-12345|0|0|0,16|256|192|0,16|257|193|5";

        let mut compressed = Vec::new();
        lzma_rs::lzma_compress(&mut std::io::Cursor::new(&original[..]), &mut compressed)
            .expect("compression should succeed");

        let decompressed = decompress_lzma(&compressed).expect("decompression should succeed");
        assert_eq!(decompressed, original);
    }

    // --- UT-OSR-012: LZMA bomb rejection ---
    #[test]
    fn ut_osr_012_lzma_bomb_is_rejected() {
        // 1 MiB of zeroes compresses to a tiny payload — a miniature bomb.
        let payload = vec![0u8; 1024 * 1024];

        let mut compressed = Vec::new();
        lzma_rs::lzma_compress(&mut std::io::Cursor::new(&payload[..]), &mut compressed)
            .expect("compression should succeed");

        // Sanity-check the fixture actually expands: assert the ratio rather
        // than an absolute size, so this does not become a brittle assertion
        // about lzma-rs's compressor tuning.
        let ratio = payload.len() / compressed.len().max(1);
        assert!(
            ratio >= 20,
            "fixture does not expand enough to exercise the cap ({}:1)",
            ratio
        );

        // Under a 64 KiB cap this must be rejected...
        match decompress_lzma_with_limit(&compressed, 64 * 1024) {
            Err(EngineError::DecompressionOutputTooLarge { limit_bytes, .. }) => {
                assert_eq!(limit_bytes, 64 * 1024);
            }
            other => panic!("expected DecompressionOutputTooLarge, got {:?}", other),
        }

        // ...and accepted under a cap that accommodates it, proving the
        // rejection above came from the cap and not from a broken fixture.
        let ok = decompress_lzma_with_limit(&compressed, 2 * 1024 * 1024)
            .expect("should decompress under a sufficient cap");
        assert_eq!(ok.len(), payload.len());
    }

    #[test]
    fn output_exactly_at_the_limit_is_accepted() {
        let payload = vec![7u8; 1000];

        let mut compressed = Vec::new();
        lzma_rs::lzma_compress(&mut std::io::Cursor::new(&payload[..]), &mut compressed).unwrap();

        let out = decompress_lzma_with_limit(&compressed, 1000).expect("limit is inclusive");
        assert_eq!(out.len(), 1000);
    }
}
