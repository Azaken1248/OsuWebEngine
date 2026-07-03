#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Feed arbitrary bytes to the .osr parser.
    // The parser must return Err — never panic — for any input.
    let _ = osu_engine::parser::osr::parse_osr(data);
});
