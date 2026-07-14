//! Little-endian binary reader primitives for the `.osr` format.
//!
//! A cursor over an in-memory `&[u8]`. Every read is bounds-checked and
//! returns `EngineError::UnexpectedEof` rather than panicking — the whole
//! point of this module is that untrusted bytes cannot take the engine down.
//!
//! ## Why a slice cursor and not `impl Read`
//!
//! TDD §2.2–2.3 sketches these against `impl Read`. In practice the bytes
//! always arrive fully in memory (the WASM boundary hands us a `Uint8Array`),
//! so a slice cursor is simpler, allocation-free, and cannot block or
//! partially fill. See L2 plan, Design Decision 1.
//!
//! ## References
//!
//! - `osu/osu.Game/IO/Legacy/SerializationReader.cs` — the reader lazer uses
//! - `osu-reverse-mapper/script.js` L925–943 — osu-string byte layout

use crate::error::{EngineError, EngineResult};

/// Maximum accepted length of an osu-string, in bytes.
///
/// Player names and hashes are far below this; the life-bar graph is the
/// only field that gets long. The cap exists so a crafted ULEB128 length
/// cannot make us allocate wildly. TDD §2.2 suggests 512, which is too small
/// for real life-bar graphs (they run to several KB on long maps), so this is
/// set to a value that admits real replays while still bounding the allocation.
pub const MAX_STRING_LEN: usize = 1 << 20; // 1 MiB

/// ULEB128 shift ceiling. `usize` is 64-bit on our targets and 32-bit on
/// wasm32; 10 groups of 7 bits covers 64 bits, and we reject beyond that.
const MAX_ULEB_SHIFT: u32 = 63;

/// A bounds-checked, little-endian cursor over a byte slice.
pub struct ByteReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> ByteReader<'a> {
    /// Wraps a byte slice.
    pub fn new(data: &'a [u8]) -> Self {
        ByteReader { data, pos: 0 }
    }

    /// Current byte offset.
    pub fn position(&self) -> usize {
        self.pos
    }

    /// Bytes remaining.
    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    /// True when the cursor has consumed the whole slice.
    pub fn is_empty(&self) -> bool {
        self.remaining() == 0
    }

    /// Takes `n` bytes, or fails with `UnexpectedEof`.
    pub fn take(&mut self, n: usize, context: &'static str) -> EngineResult<&'a [u8]> {
        let end = self.pos.checked_add(n).ok_or(EngineError::UnexpectedEof {
            context,
            offset: self.pos,
        })?;

        if end > self.data.len() {
            return Err(EngineError::UnexpectedEof {
                context,
                offset: self.pos,
            });
        }

        let slice = &self.data[self.pos..end];
        self.pos = end;
        Ok(slice)
    }

    /// Reads a `u8`.
    pub fn read_u8(&mut self, context: &'static str) -> EngineResult<u8> {
        Ok(self.take(1, context)?[0])
    }

    /// Reads a `.osr` boolean (any non-zero byte is `true`).
    pub fn read_bool(&mut self, context: &'static str) -> EngineResult<bool> {
        Ok(self.read_u8(context)? != 0)
    }

    /// Reads a little-endian `u16`.
    pub fn read_u16(&mut self, context: &'static str) -> EngineResult<u16> {
        let b = self.take(2, context)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    /// Reads a little-endian `i32`.
    pub fn read_i32(&mut self, context: &'static str) -> EngineResult<i32> {
        let b = self.take(4, context)?;
        Ok(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// Reads a little-endian `i64`.
    pub fn read_i64(&mut self, context: &'static str) -> EngineResult<i64> {
        let b = self.take(8, context)?;
        Ok(i64::from_le_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }

    /// Reads a ULEB128-encoded length.
    ///
    /// Rejects an encoding that shifts past 63 bits rather than silently
    /// wrapping. A crafted `.osr` can otherwise supply an unbounded run of
    /// continuation bytes.
    ///
    /// Source: TDD §2.3 (corrected — the TDD's `shift > 35` ceiling is both
    /// arbitrary and too low to represent the values it then accepts).
    pub fn read_uleb128(&mut self, context: &'static str) -> EngineResult<usize> {
        let mut result: u64 = 0;
        let mut shift: u32 = 0;

        loop {
            let byte = self.read_u8(context)?;

            let chunk = u64::from(byte & 0x7F);
            result |= chunk
                .checked_shl(shift)
                .ok_or(EngineError::UlebOverflow { context })?;

            if byte & 0x80 == 0 {
                break;
            }

            shift += 7;
            if shift > MAX_ULEB_SHIFT {
                return Err(EngineError::UlebOverflow { context });
            }
        }

        usize::try_from(result).map_err(|_| EngineError::UlebOverflow { context })
    }

    /// Reads an osu-string: a `0x00` / `0x0B` marker, then (for `0x0B`) a
    /// ULEB128 length and that many UTF-8 bytes.
    ///
    /// `0x00` yields an empty string rather than `None` — every caller in the
    /// `.osr` header wants a string, and the distinction between "absent" and
    /// "empty" is not one the format actually makes use of.
    ///
    /// Source: `osu-reverse-mapper/script.js` L925–943.
    pub fn read_osu_string(&mut self, context: &'static str) -> EngineResult<String> {
        let marker = self.read_u8(context)?;

        match marker {
            0x00 => Ok(String::new()),
            0x0B => {
                let len = self.read_uleb128(context)?;

                if len > MAX_STRING_LEN {
                    return Err(EngineError::StringTooLong { context, len });
                }

                let offset = self.pos;
                let bytes = self.take(len, context)?;

                String::from_utf8(bytes.to_vec())
                    .map_err(|_| EngineError::InvalidUtf8 { context, offset })
            }
            marker => Err(EngineError::InvalidStringMarker { context, marker }),
        }
    }

    /// Reads a length-prefixed byte array: an `i32` length, then that many
    /// bytes. A negative length means "absent" and yields an empty slice.
    ///
    /// Source: `SerializationReader.ReadByteArray()`, used by
    /// `LegacyScoreDecoder` L105 for the compressed replay payload.
    pub fn read_byte_array(&mut self, context: &'static str) -> EngineResult<&'a [u8]> {
        let len = self.read_i32(context)?;

        if len <= 0 {
            return Ok(&[]);
        }

        self.take(len as usize, context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_little_endian_integers() {
        let data = [0x01, 0x02, 0x03, 0x00, 0x00, 0x00];
        let mut r = ByteReader::new(&data);

        assert_eq!(r.read_u8("t").unwrap(), 0x01);
        assert_eq!(r.read_u16("t").unwrap(), 0x0302);
        assert_eq!(r.remaining(), 3);
    }

    #[test]
    fn read_past_end_is_eof_not_panic() {
        let mut r = ByteReader::new(&[0x01]);
        assert!(r.read_u8("t").is_ok());

        match r.read_u8("t") {
            Err(EngineError::UnexpectedEof { .. }) => {}
            other => panic!("expected UnexpectedEof, got {:?}", other),
        }
    }

    #[test]
    fn empty_input_is_eof() {
        let mut r = ByteReader::new(&[]);
        assert!(matches!(
            r.read_i32("t"),
            Err(EngineError::UnexpectedEof { .. })
        ));
    }

    // --- UT-OSR-007: osu-string null handling ---
    #[test]
    fn ut_osr_007_osu_string_markers() {
        // 0x00 → empty
        let mut r = ByteReader::new(&[0x00]);
        assert_eq!(r.read_osu_string("t").unwrap(), "");

        // 0x0B + ULEB len 5 + "hello"
        let data = [0x0B, 0x05, b'h', b'e', b'l', b'l', b'o'];
        let mut r = ByteReader::new(&data);
        assert_eq!(r.read_osu_string("t").unwrap(), "hello");

        // Any other marker → error
        let mut r = ByteReader::new(&[0x42]);
        assert!(matches!(
            r.read_osu_string("t"),
            Err(EngineError::InvalidStringMarker { marker: 0x42, .. })
        ));
    }

    // --- UT-OSR-008: ULEB128 overflow ---
    #[test]
    fn ut_osr_008_uleb_overflow() {
        // An unbroken run of continuation bytes must terminate with an error,
        // not loop forever and not wrap the accumulator.
        let data = [0xFF; 32];
        let mut r = ByteReader::new(&data);

        assert!(matches!(
            r.read_uleb128("t"),
            Err(EngineError::UlebOverflow { .. })
        ));
    }

    #[test]
    fn uleb128_multibyte() {
        // 300 = 0b100101100 → 0xAC 0x02
        let mut r = ByteReader::new(&[0xAC, 0x02]);
        assert_eq!(r.read_uleb128("t").unwrap(), 300);

        // 0 → single zero byte
        let mut r = ByteReader::new(&[0x00]);
        assert_eq!(r.read_uleb128("t").unwrap(), 0);
    }

    // --- UT-OSR-014: UTF-8 validation ---
    #[test]
    fn ut_osr_014_invalid_utf8() {
        // 0x0B, len 2, then an invalid continuation sequence
        let data = [0x0B, 0x02, 0xFF, 0xFE];
        let mut r = ByteReader::new(&data);

        assert!(matches!(
            r.read_osu_string("player_name"),
            Err(EngineError::InvalidUtf8 { .. })
        ));
    }

    #[test]
    fn string_length_is_capped() {
        // Declares a huge length with no payload behind it. Must reject on the
        // cap rather than attempt a 4 GB allocation.
        let mut data = vec![0x0B];
        // ULEB128 for 2^40
        let mut n: u64 = 1 << 40;
        while n >= 0x80 {
            data.push((n as u8 & 0x7F) | 0x80);
            n >>= 7;
        }
        data.push(n as u8);

        let mut r = ByteReader::new(&data);
        assert!(matches!(
            r.read_osu_string("t"),
            Err(EngineError::StringTooLong { .. })
        ));
    }

    #[test]
    fn byte_array_negative_length_is_empty() {
        let data = [0xFF, 0xFF, 0xFF, 0xFF]; // -1
        let mut r = ByteReader::new(&data);
        assert_eq!(r.read_byte_array("t").unwrap(), &[] as &[u8]);
    }

    #[test]
    fn byte_array_truncated_is_eof() {
        // Declares 100 bytes, supplies 2.
        let data = [100, 0, 0, 0, 0xAA, 0xBB];
        let mut r = ByteReader::new(&data);
        assert!(matches!(
            r.read_byte_array("t"),
            Err(EngineError::UnexpectedEof { .. })
        ));
    }
}
