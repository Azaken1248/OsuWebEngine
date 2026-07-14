//! Generates the `.osr` test fixtures and fuzz seeds.
//!
//! Run: `cargo run -p osu-engine --example gen_osr_fixture -- <out_dir>`
//!
//! `.osr` is a binary format, so the fixtures are generated rather than
//! committed as opaque hex blobs: the layout stays legible, reviewable, and in
//! sync with the parser. No copyrighted beatmap or replay content is involved —
//! every field is synthetic.

use std::io::Cursor;

fn osu_string(buf: &mut Vec<u8>, s: &str) {
    buf.push(0x0B);
    let mut len = s.len();
    while len >= 0x80 {
        buf.push((len as u8 & 0x7F) | 0x80);
        len >>= 7;
    }
    buf.push(len as u8);
    buf.extend_from_slice(s.as_bytes());
}

fn build(version: i32, mods: i32, frames: &str, score_id: i64) -> Vec<u8> {
    let mut b = Vec::new();

    b.push(0u8); // mode: osu!Standard
    b.extend_from_slice(&version.to_le_bytes());

    osu_string(&mut b, "d41d8cd98f00b204e9800998ecf8427e");
    osu_string(&mut b, "PlayerOne");
    osu_string(&mut b, "0123456789abcdef0123456789abcdef");

    for n in [300u16, 10, 5, 42, 7, 3] {
        b.extend_from_slice(&n.to_le_bytes());
    }

    b.extend_from_slice(&1_234_567i32.to_le_bytes()); // total score
    b.extend_from_slice(&850u16.to_le_bytes()); // max combo
    b.push(1); // perfect
    b.extend_from_slice(&mods.to_le_bytes());

    osu_string(&mut b, "0|1,1000|0.9"); // life bar
    b.extend_from_slice(&638_000_000_000_000_000i64.to_le_bytes()); // timestamp

    let mut compressed = Vec::new();
    lzma_rs::lzma_compress(&mut Cursor::new(frames.as_bytes()), &mut compressed)
        .expect("compression should succeed");

    b.extend_from_slice(&(compressed.len() as i32).to_le_bytes());
    b.extend_from_slice(&compressed);

    // Score ID width depends on the replay version (LegacyScoreDecoder.cs L107-110).
    if version >= 20140721 {
        b.extend_from_slice(&score_id.to_le_bytes());
    } else if version >= 20121008 {
        b.extend_from_slice(&(score_id as i32).to_le_bytes());
    }

    b
}

fn main() {
    let out = std::env::args()
        .nth(1)
        .expect("usage: gen_osr_fixture <out_dir>");

    let dir = std::path::Path::new(&out);
    std::fs::create_dir_all(dir).expect("output directory should be creatable");

    // A stable-style stream: two (256,-500) sentinel frames, real frames, then
    // the -12345 seed frame. Exercises every frame quirk at once.
    let stable_frames =
        "0|256|-500|0,10|256|-500|0,16|100|200|0,16|110|210|1,16|120|220|5,-12345|0|0|8721";

    let cases: [(&str, i32, i32, &str, i64); 4] = [
        ("nomod_fc.osr", 20230326, 0, stable_frames, 555_000_111),
        // Hidden (8) | HardRock (16) | DoubleTime (64) = 88
        ("dthr_fc.osr", 20230326, 88, stable_frames, 555_000_222),
        (
            "old_format_2013.osr",
            20130101,
            0,
            "16|100|200|0,16|110|210|1",
            4242,
        ),
        ("ancient_2011.osr", 20111001, 0, "16|100|200|0", 0),
    ];

    for (name, version, mods, frames, score_id) in cases {
        let bytes = build(version, mods, frames, score_id);
        std::fs::write(dir.join(name), &bytes).expect("fixture should be writable");
        println!("{name}: {} bytes", bytes.len());
    }
}
