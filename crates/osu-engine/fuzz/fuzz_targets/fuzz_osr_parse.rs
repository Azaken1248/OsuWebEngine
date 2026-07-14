#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Feed arbitrary bytes to the .osr parser.
    // The parser must return Err — never panic — for any input.
    //
    // The first byte doubles as the beatmap format version so the fuzzer also
    // explores the early-version timing offset path (+24 ms for format < 5)
    // instead of always taking the `None` branch. Subtracting 8 puts the
    // v4/v5 boundary inside the range the fuzzer reaches easily.
    let beatmap_version = data.first().map(|b| i32::from(*b) - 8);

    let _ = osu_engine::parser::osr::parse_osr(data, beatmap_version);
});
