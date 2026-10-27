//! PCG32 (PCG-XSH-RR, 64-bit state, 32-bit output) with unbiased bounded sampling.

const MULTIPLIER: u64 = 6_364_136_223_846_793_005;

/// The PCG-XSH-RR 64/32 generator, implemented here so no dependency can change a replay.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pcg32 {
    state: u64,
    inc: u64,
}

impl Pcg32 {
    /// Creates a generator from a seed and a stream selector, matching the reference `pcg32_srandom_r`.
    pub fn new(seed: u64, stream: u64) -> Self {
        let mut rng = Self {
            state: 0,
            inc: (stream << 1) | 1,
        };
        rng.next_u32();
        rng.state = rng.state.wrapping_add(seed);
        rng.next_u32();
        rng
    }

    /// Returns the next 32 random bits.
    pub fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.state = old.wrapping_mul(MULTIPLIER).wrapping_add(self.inc);
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        let rot = (old >> 59) as u32;
        xorshifted.rotate_right(rot)
    }

    /// Returns a uniform value in `0..n` using Lemire's nearly-divisionless method.
    ///
    /// # Panics
    ///
    /// Panics if `n` is zero.
    pub fn bounded(&mut self, n: u32) -> u32 {
        assert!(n > 0, "bounded(0) has no valid result");
        let mut m = u64::from(self.next_u32()) * u64::from(n);
        if (m as u32) < n {
            let threshold = n.wrapping_neg() % n;
            while (m as u32) < threshold {
                m = u64::from(self.next_u32()) * u64::from(n);
            }
        }
        (m >> 32) as u32
    }

    /// Rolls a die with `sides` faces, returning a value in `1..=sides`.
    ///
    /// # Panics
    ///
    /// Panics if `sides` is zero.
    pub fn roll(&mut self, sides: u32) -> u32 {
        self.bounded(sides) + 1
    }
}

#[cfg(test)]
mod tests;
