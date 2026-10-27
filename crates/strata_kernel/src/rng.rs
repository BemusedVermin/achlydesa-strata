//! Per-command random streams.

use crate::pcg::Pcg32;

/// The random stream for one command, derived from the game seed and the command's index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandRng(Pcg32);

impl CommandRng {
    /// Derives the stream for command `command_index` of a game with seed `game_seed`.
    ///
    /// The key is the 12 bytes `game_seed` (u64 LE) then `command_index` (u32 LE). The PCG seed
    /// is `xxh3_64(key)` and the PCG stream is `xxh3_64_with_seed(key, 1)`.
    pub fn for_command(game_seed: u64, command_index: u32) -> Self {
        let mut key = [0u8; 12];
        key[..8].copy_from_slice(&game_seed.to_le_bytes());
        key[8..].copy_from_slice(&command_index.to_le_bytes());
        let seed = xxhash_rust::xxh3::xxh3_64(&key);
        let stream = xxhash_rust::xxh3::xxh3_64_with_seed(&key, 1);
        Self(Pcg32::new(seed, stream))
    }

    /// Returns the next 32 random bits.
    pub fn next_u32(&mut self) -> u32 {
        self.0.next_u32()
    }

    /// Returns a uniform value in `0..n`; see [`Pcg32::bounded`].
    pub fn bounded(&mut self, n: u32) -> u32 {
        self.0.bounded(n)
    }

    /// Rolls a die with `sides` faces, returning `1..=sides`.
    pub fn roll(&mut self, sides: u32) -> u32 {
        self.0.roll(sides)
    }
}

#[cfg(test)]
mod tests;
