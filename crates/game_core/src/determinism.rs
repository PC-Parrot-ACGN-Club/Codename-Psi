//! Versioned deterministic primitives shared by match construction and checks.

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

/// Version of the random stream derivation.
///
/// Upgrading the derivation must bump this: two runs are only comparable when
/// their streams were derived the same way.
pub const RNG_ALGORITHM_VERSION: u32 = 1;

/// Version of the canonical state encoding behind [`StateChecksum`].
pub const STATE_CODEC_VERSION: u32 = 1;

/// Stable names for independent match random streams.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamName {
    Color,
    Nuisance,
    FeverPuzzle,
}

impl StreamName {
    const fn tag(self) -> u64 {
        match self {
            Self::Color => 0x0000_0063_6f6c_6f72,
            Self::Nuisance => 0x6e75_6973_616e_6365,
            Self::FeverPuzzle => 0x6665_7665_722d_7075,
        }
    }
}

/// Deterministically derived, independently advancing random stream.
#[derive(Debug, Clone)]
pub struct MatchRng(ChaCha8Rng);

impl MatchRng {
    /// Derives a stream solely from immutable match/round identifiers.
    #[must_use]
    pub fn derive(
        root_seed: u64,
        round_index: u32,
        draw_attempt: u32,
        player_slot: u8,
        stream: StreamName,
    ) -> Self {
        let mut state = root_seed;
        for value in [
            u64::from(round_index),
            u64::from(draw_attempt),
            u64::from(player_slot),
            stream.tag(),
        ] {
            state ^= value;
            state = state.wrapping_mul(0x9e37_79b9_7f4a_7c15).rotate_left(27);
        }
        let mut seed = [0_u8; 32];
        for chunk in seed.chunks_exact_mut(8) {
            state ^= state >> 30;
            state = state.wrapping_mul(0xbf58_476d_1ce4_e5b9);
            state ^= state >> 27;
            state = state.wrapping_mul(0x94d0_49bb_1331_11eb);
            state ^= state >> 31;
            chunk.copy_from_slice(&state.to_le_bytes());
        }
        Self(ChaCha8Rng::from_seed(seed))
    }

    /// Takes the next deterministic value from this stream only.
    pub fn next_u32(&mut self) -> u32 {
        self.0.next_u32()
    }
}

/// Versioned fixed-width state checksum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StateChecksum(pub u64);

/// FNV-1a checksum over the caller's explicitly encoded state bytes.
#[must_use]
pub fn checksum_v1(bytes: impl IntoIterator<Item = u8>) -> StateChecksum {
    let value = bytes
        .into_iter()
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
        });
    StateChecksum(value)
}
