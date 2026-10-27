//! Exact rational numbers for segment parameters (design §7.5).

use std::cmp::Ordering;

/// An exact rational `num / den` in lowest terms with `den > 0`.
///
/// Values are normalized on construction, so derived equality is value
/// equality. Comparison cross-multiplies in `i128`, which cannot overflow for
/// lattice coordinates of any realistic board.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Frac {
    num: i128,
    den: i128,
}

impl Frac {
    /// The value 0.
    pub const ZERO: Frac = Frac { num: 0, den: 1 };
    /// The value 1.
    pub const ONE: Frac = Frac { num: 1, den: 1 };

    /// Returns `num / den` in lowest terms; panics if `den` is zero.
    pub fn new(num: i128, den: i128) -> Frac {
        assert!(den != 0, "Frac denominator must not be zero");
        let (mut a, mut b) = (num.abs(), den.abs());
        while b != 0 {
            (a, b) = (b, a % b);
        }
        let divisor = a * den.signum();
        Frac {
            num: num / divisor,
            den: den / divisor,
        }
    }

    /// Returns the numerator.
    pub const fn numer(self) -> i128 {
        self.num
    }

    /// Returns the denominator, which is always positive.
    pub const fn denom(self) -> i128 {
        self.den
    }

    /// Returns `1 - self`, the parameter of the same point on the reversed segment.
    pub const fn one_minus(self) -> Frac {
        Frac {
            num: self.den - self.num,
            den: self.den,
        }
    }

    /// Returns the greatest integer not above this value.
    pub const fn floor(self) -> i128 {
        self.num.div_euclid(self.den)
    }

    /// Returns the least integer not below this value.
    pub const fn ceil(self) -> i128 {
        -(-self.num).div_euclid(self.den)
    }

    /// Returns `base + self * delta`, the point at this parameter along a segment.
    pub fn affine(self, base: i128, delta: i128) -> Frac {
        Frac::new(base * self.den + self.num * delta, self.den)
    }
}

impl Ord for Frac {
    fn cmp(&self, other: &Frac) -> Ordering {
        (self.num * other.den).cmp(&(other.num * self.den))
    }
}

impl PartialOrd for Frac {
    fn partial_cmp(&self, other: &Frac) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
