//! Frozen rule data shared by the rules-engine integration tests.
//!
//! Values are transcribed from the design documents so a test failure points at
//! either the implementation or a documented baseline, never at an ad-hoc
//! fixture invented in one test file.

// Each integration-test binary compiles this module separately and uses only
// part of it, so unused items here are expected rather than dead code.
#![allow(dead_code)]

use game_core::rules::ChainPowerParameters;

/// `docs/development/design/chain-power-curve.md`, character A.
pub const CHARACTER_A: ChainPowerParameters = ChainPowerParameters {
    normal_anchor: 440.0,
    normal_tilt: 0.90,
    normal_growth: 0.26,
    fever_anchor: 42.0,
    fever_tilt: 0.95,
};

/// `docs/development/design/chain-power-curve.md`, character B.
pub const CHARACTER_B: ChainPowerParameters = ChainPowerParameters {
    normal_anchor: 380.0,
    normal_tilt: 0.95,
    normal_growth: 0.25,
    fever_anchor: 47.0,
    fever_tilt: 0.92,
};

/// Character A, normal board.
pub const A_NORMAL: [u16; 24] = [
    4, 12, 24, 33, 50, 101, 170, 258, 348, 440, 554, 669, 783, 898, 999, 999, 999, 999, 999, 999,
    999, 999, 999, 999,
];
/// Character A, Fever board.
pub const A_FEVER: [u16; 24] = [
    4, 10, 18, 22, 30, 49, 82, 123, 165, 248, 291, 300, 358, 420, 462, 504, 546, 588, 630, 672,
    714, 756, 798, 840,
];
/// Character B, normal board.
pub const B_NORMAL: [u16; 24] = [
    4, 11, 22, 29, 44, 89, 149, 225, 302, 380, 475, 570, 665, 760, 855, 950, 999, 999, 999, 999,
    999, 999, 999, 999,
];
/// Character B, Fever board.
pub const B_FEVER: [u16; 24] = [
    4, 11, 20, 24, 33, 54, 90, 136, 182, 275, 323, 334, 399, 470, 517, 564, 611, 658, 705, 752,
    799, 846, 893, 940,
];

// Puyo Puyo Fever 2 reference tiers, transcribed from
// <https://puyonexus.com/wiki/List_of_attack_powers>. The design derives both
// shape curves from these and cross-checks each against a second tier.
//
// The wiki's normal table ends in a saturating `21+` column, so entries 22 to
// 24 repeat it — the same tail rule the runtime reader applies. The Fever table
// already runs to a `24+` column.

/// Normal tier `A=400`: Amitie and Lemres (wiki tier 6).
pub const TIER_A400_NORMAL: [u16; 24] = [
    4, 12, 24, 32, 48, 96, 160, 240, 320, 400, 500, 600, 700, 800, 900, 999, 999, 999, 999, 999,
    999, 999, 999, 999,
];
/// Normal cross-check tier `A=360`: Dapper Bones, Hoho and Yu & Rei (wiki tier 9).
pub const TIER_A360_NORMAL: [u16; 24] = [
    4, 11, 22, 29, 43, 86, 144, 216, 288, 360, 450, 540, 630, 720, 810, 900, 990, 999, 999, 999,
    999, 999, 999, 999,
];
/// Fever tier `F=40`: Amitie and Arle (wiki tier 8).
pub const TIER_F40_FEVER: [u16; 24] = [
    4, 10, 18, 22, 30, 48, 80, 120, 160, 240, 280, 288, 342, 400, 440, 480, 520, 560, 600, 640,
    680, 720, 760, 800,
];
/// Fever cross-check tier `F=36`: Accord (wiki tier 12). Its samples 11 to 13
/// are the three non-trivial values that pin the shape curve.
pub const TIER_F36_FEVER: [u16; 24] = [
    4, 9, 16, 20, 27, 43, 72, 108, 144, 216, 252, 259, 308, 360, 396, 432, 468, 504, 540, 576, 612,
    648, 684, 720,
];

/// Fever color bonus from `docs/gameplay.md` §4.1, indexed by color count.
pub fn color_bonus() -> Vec<u16> {
    vec![0, 0, 2, 4, 8, 16]
}

/// Fever group bonus from `docs/gameplay.md` §4.1, indexed by group size.
/// Sizes of 11 and above share the tail value.
pub fn group_bonus() -> Vec<u16> {
    vec![0, 0, 0, 0, 0, 1, 2, 3, 4, 5, 6, 8]
}

/// Fever versus target score before any margin decay.
pub const TARGET_POINTS: u64 = 120;
