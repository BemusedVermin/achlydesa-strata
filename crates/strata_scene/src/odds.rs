//! Exact wound odds: the two dice pools convolved, with no sampling (design §13.6).
//!
//! Each die of a pool counts when it lands on one of `success` of its `faces` faces, so a pool's
//! count is binomial. The wounds are the attack's hits minus the defense's blocks, never below
//! zero. Every probability is an integer weight over one common total, so the distribution is
//! exact and sums to exactly one.
//!
//! The common total is capped at [`MAX_TOTAL`], `i64::MAX`. `Frac` compares by cross-multiplying
//! in `i128`, so with every numerator and denominator at most `i64::MAX` any two chances or
//! expectations these odds give can be compared, or added, without overflow. Pools beyond the
//! cap get no exact odds.

use strata_kernel::Frac;

/// The largest common total, and the largest numerator, of the fractions [`WoundOdds`] gives:
/// small enough that `Frac`'s `i128` cross-multiplication of any two of them cannot overflow.
pub const MAX_TOTAL: i128 = i64::MAX as i128;

/// A pool of identical dice, each counting on `success` of its `faces` faces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DicePool {
    /// How many dice are rolled.
    pub dice: u32,
    /// How many faces count, such as the `hit` faces of an attack die.
    pub success: u32,
    /// How many faces the die has.
    pub faces: u32,
}

/// The exact distribution of the number of wounds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WoundOdds {
    /// The weight of each wound count from zero up, over `total`.
    weights: Vec<i128>,
    /// The sum of the weights.
    total: i128,
}

/// A pool's count distribution as integer weights over a total, or `None` when the die is
/// malformed or the total overflows.
fn counts(pool: DicePool) -> Option<(Vec<i128>, i128)> {
    if pool.faces == 0 || pool.success > pool.faces {
        return None;
    }
    // Reduce the per-die odds first, so a 3-in-6 die weighs like a coin.
    let divisor = gcd(pool.success, pool.faces);
    let (yes, all) = (pool.success / divisor, pool.faces / divisor);
    let (yes, no) = (i128::from(yes), i128::from(all - yes));
    let total = i128::from(all).checked_pow(pool.dice)?;
    // Pascal's rule with weights: each die splits every count into a miss and a hit. Every entry
    // is at most `total`, so nothing here can overflow once `total` fits.
    let mut weights = vec![1i128];
    for _ in 0..pool.dice {
        let mut next = vec![0i128; weights.len() + 1];
        for (n, w) in weights.iter().enumerate() {
            next[n] += w * no;
            next[n + 1] += w * yes;
        }
        weights = next;
    }
    Some((weights, total))
}

fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a.max(1)
}

impl WoundOdds {
    /// The wounds `attack`'s hits minus `defense`'s blocks deal, or `None` when a die is
    /// malformed (no faces, or more counting faces than faces) or the pools are too large for
    /// exact odds: their common total would pass [`MAX_TOTAL`].
    pub fn of(attack: DicePool, defense: DicePool) -> Option<Self> {
        let (hits, hit_total) = counts(attack)?;
        let (blocks, block_total) = counts(defense)?;
        let total = hit_total.checked_mul(block_total)?;
        if total > MAX_TOTAL {
            return None;
        }
        // Each product is at most `total`, and the sums never exceed it.
        let mut weights = vec![0i128; hits.len()];
        for (h, hw) in hits.iter().enumerate() {
            for (b, bw) in blocks.iter().enumerate() {
                weights[h.saturating_sub(b)] += hw * bw;
            }
        }
        while weights.len() > 1 && weights.last() == Some(&0) {
            weights.pop();
        }
        Some(Self { weights, total })
    }

    /// The most wounds with a chance above zero.
    pub fn max(&self) -> usize {
        self.weights.len() - 1
    }

    /// The chance of exactly `wounds` wounds.
    pub fn chance(&self, wounds: usize) -> Frac {
        let weight = self.weights.get(wounds).copied().unwrap_or(0);
        Frac::new(weight, self.total)
    }

    /// The chance of at least `wounds` wounds.
    pub fn at_least(&self, wounds: usize) -> Frac {
        let weight = self.weights.iter().skip(wounds).sum();
        Frac::new(weight, self.total)
    }

    /// The expected number of wounds, exactly, or `None` if its numerator would pass
    /// [`MAX_TOTAL`].
    pub fn expected(&self) -> Option<Frac> {
        let mut sum = 0i128;
        for (n, w) in self.weights.iter().enumerate() {
            sum = sum.checked_add(i128::try_from(n).ok()?.checked_mul(*w)?)?;
        }
        (sum <= MAX_TOTAL).then(|| Frac::new(sum, self.total))
    }

    /// The weight of each wound count from zero, with the common total they are over.
    pub fn weights(&self) -> (&[i128], i128) {
        (&self.weights, self.total)
    }
}

/// `chance` as a percentage, for display only.
pub fn percent(chance: Frac) -> f64 {
    chance.numer() as f64 * 100.0 / chance.denom() as f64
}

#[cfg(test)]
mod tests;
