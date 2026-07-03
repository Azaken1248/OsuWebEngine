//! Mod set and mod bitmask conversion.
//!
//! osu! mods are stored as a 32-bit bitmask in replay files.
//! This module converts between the bitmask and a typed `ModSet`.
//!
//! ## Reference
//!
//! - BRD §8.6: Mod effect matrix
//! - `osu-reverse-mapper/script.js` L1051–1062

use serde::{Deserialize, Serialize};

/// Typed representation of active osu! mods.
///
/// Each field corresponds to a mod that affects gameplay.
/// Fields match BRD §8.6 mod matrix.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModSet {
    pub no_fail: bool,
    pub easy: bool,
    pub touch_device: bool,
    pub hidden: bool,
    pub hard_rock: bool,
    pub sudden_death: bool,
    pub double_time: bool,
    pub relax: bool,
    pub half_time: bool,
    pub nightcore: bool,
    pub flashlight: bool,
    pub autoplay: bool,
    pub spun_out: bool,
    pub autopilot: bool,
    pub perfect: bool,
    pub cinema: bool,
    pub mirror: bool,
}

/// Mod bitmask constants matching osu!'s binary format.
/// Source: osu! wiki, osu-reverse-mapper/script.js L1051–1062.
pub mod flags {
    pub const NO_FAIL: u32 = 1 << 0;
    pub const EASY: u32 = 1 << 1;
    pub const TOUCH_DEVICE: u32 = 1 << 2;
    pub const HIDDEN: u32 = 1 << 3;
    pub const HARD_ROCK: u32 = 1 << 4;
    pub const SUDDEN_DEATH: u32 = 1 << 5;
    pub const DOUBLE_TIME: u32 = 1 << 6;
    pub const RELAX: u32 = 1 << 7;
    pub const HALF_TIME: u32 = 1 << 8;
    /// Nightcore implies DoubleTime.
    pub const NIGHTCORE: u32 = 1 << 9;
    pub const FLASHLIGHT: u32 = 1 << 10;
    pub const AUTOPLAY: u32 = 1 << 11;
    pub const SPUN_OUT: u32 = 1 << 12;
    pub const AUTOPILOT: u32 = 1 << 13;
    pub const PERFECT: u32 = 1 << 14;
    pub const CINEMA: u32 = 1 << 22;
    pub const MIRROR: u32 = 1 << 30;
}

impl ModSet {
    /// Creates a `ModSet` from the raw 32-bit bitmask stored in `.osr` files.
    pub fn from_bitmask(bits: u32) -> Self {
        Self {
            no_fail: bits & flags::NO_FAIL != 0,
            easy: bits & flags::EASY != 0,
            touch_device: bits & flags::TOUCH_DEVICE != 0,
            hidden: bits & flags::HIDDEN != 0,
            hard_rock: bits & flags::HARD_ROCK != 0,
            sudden_death: bits & flags::SUDDEN_DEATH != 0,
            double_time: bits & flags::DOUBLE_TIME != 0,
            relax: bits & flags::RELAX != 0,
            half_time: bits & flags::HALF_TIME != 0,
            nightcore: bits & flags::NIGHTCORE != 0,
            flashlight: bits & flags::FLASHLIGHT != 0,
            autoplay: bits & flags::AUTOPLAY != 0,
            spun_out: bits & flags::SPUN_OUT != 0,
            autopilot: bits & flags::AUTOPILOT != 0,
            perfect: bits & flags::PERFECT != 0,
            cinema: bits & flags::CINEMA != 0,
            mirror: bits & flags::MIRROR != 0,
        }
    }

    /// Converts back to the raw 32-bit bitmask.
    pub fn to_bitmask(&self) -> u32 {
        let mut bits = 0u32;
        if self.no_fail {
            bits |= flags::NO_FAIL;
        }
        if self.easy {
            bits |= flags::EASY;
        }
        if self.touch_device {
            bits |= flags::TOUCH_DEVICE;
        }
        if self.hidden {
            bits |= flags::HIDDEN;
        }
        if self.hard_rock {
            bits |= flags::HARD_ROCK;
        }
        if self.sudden_death {
            bits |= flags::SUDDEN_DEATH;
        }
        if self.double_time {
            bits |= flags::DOUBLE_TIME;
        }
        if self.relax {
            bits |= flags::RELAX;
        }
        if self.half_time {
            bits |= flags::HALF_TIME;
        }
        if self.nightcore {
            bits |= flags::NIGHTCORE;
        }
        if self.flashlight {
            bits |= flags::FLASHLIGHT;
        }
        if self.autoplay {
            bits |= flags::AUTOPLAY;
        }
        if self.spun_out {
            bits |= flags::SPUN_OUT;
        }
        if self.autopilot {
            bits |= flags::AUTOPILOT;
        }
        if self.perfect {
            bits |= flags::PERFECT;
        }
        if self.cinema {
            bits |= flags::CINEMA;
        }
        if self.mirror {
            bits |= flags::MIRROR;
        }
        bits
    }

    /// Returns the time rate multiplier for this mod set.
    ///
    /// - DT/NC: 1.5× (play at 150% speed, time compressed to 66.7%)
    /// - HT: 0.75× (play at 75% speed, time expanded to 133%)
    /// - Default: 1.0×
    pub fn time_rate(&self) -> f64 {
        if self.double_time || self.nightcore {
            1.5
        } else if self.half_time {
            0.75
        } else {
            1.0
        }
    }

    /// Returns a list of human-readable mod names.
    pub fn mod_names(&self) -> Vec<&'static str> {
        let mut names = Vec::new();
        if self.no_fail {
            names.push("NoFail");
        }
        if self.easy {
            names.push("Easy");
        }
        if self.hidden {
            names.push("Hidden");
        }
        if self.hard_rock {
            names.push("HardRock");
        }
        if self.sudden_death {
            names.push("SuddenDeath");
        }
        if self.double_time && !self.nightcore {
            names.push("DoubleTime");
        }
        if self.nightcore {
            names.push("Nightcore");
        }
        if self.half_time {
            names.push("HalfTime");
        }
        if self.flashlight {
            names.push("Flashlight");
        }
        if self.relax {
            names.push("Relax");
        }
        if self.autopilot {
            names.push("Autopilot");
        }
        if self.spun_out {
            names.push("SpunOut");
        }
        if self.perfect {
            names.push("Perfect");
        }
        if self.mirror {
            names.push("Mirror");
        }
        names
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_bitmask_is_no_mods() {
        let mods = ModSet::from_bitmask(0);
        assert_eq!(mods, ModSet::default());
    }

    #[test]
    fn round_trip_bitmask() {
        let bits = flags::HARD_ROCK | flags::DOUBLE_TIME | flags::HIDDEN;
        let mods = ModSet::from_bitmask(bits);
        assert!(mods.hard_rock);
        assert!(mods.double_time);
        assert!(mods.hidden);
        assert!(!mods.easy);
        assert_eq!(mods.to_bitmask(), bits);
    }

    #[test]
    fn nightcore_implies_double_time() {
        let bits = flags::NIGHTCORE | flags::DOUBLE_TIME;
        let mods = ModSet::from_bitmask(bits);
        assert!(mods.nightcore);
        assert!(mods.double_time);
        assert_eq!(mods.time_rate(), 1.5);
    }

    #[test]
    fn half_time_rate() {
        let mods = ModSet::from_bitmask(flags::HALF_TIME);
        assert_eq!(mods.time_rate(), 0.75);
    }

    #[test]
    fn no_mods_rate_is_one() {
        let mods = ModSet::default();
        assert_eq!(mods.time_rate(), 1.0);
    }

    #[test]
    fn mod_names() {
        let mods = ModSet::from_bitmask(flags::HARD_ROCK | flags::HIDDEN);
        let names = mods.mod_names();
        assert!(names.contains(&"Hidden"));
        assert!(names.contains(&"HardRock"));
        assert_eq!(names.len(), 2);
    }
}
