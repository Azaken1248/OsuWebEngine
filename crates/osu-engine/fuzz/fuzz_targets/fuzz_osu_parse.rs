#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Feed arbitrary bytes to the .osu parser.
    // The parser must return Err — never panic — for any input.
    if let Ok(text) = std::str::from_utf8(data) {
        let _ = osu_engine::parser::osu::parse_osu(text);
    }
});
