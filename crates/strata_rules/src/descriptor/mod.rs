//! The native descriptor evaluator (design §6.6, §8.2).
//!
//! A [`Descriptor`](strata_model::Descriptor) is a tagged [`Value`](strata_kernel::Value) map:
//! the tag key is the namespace (`sel`, `cond` or `val`) and its value names the descriptor, so
//! `{ sel = "allies", trait = "soldier", adjacent_to = "self" }` selects adjacent soldier allies.
//! Bare literals stand in for the simplest descriptors: `"self"` as a selector, a boolean as a
//! condition and an integer as a value. [`compile`] turns the tree into a typed [`Node`] and
//! rejects unknown tags and fields, so loaders call it at freeze and no bad descriptor survives
//! to runtime. [`check_ability`] does so for every native logic of an ability.

use strata_kernel::Sym;

mod compile;
mod eval;
#[cfg(test)]
mod tests;
mod text;

pub use compile::{DescriptorError, check_ability, compile};
pub use eval::{NativeEvaluator, World};

/// Where a selector starts before its filters narrow it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Base {
    /// The figure being asked about.
    Own,
    /// Other figures on the subject's team.
    Allies,
    /// Figures on other teams.
    Enemies,
    /// Figures the subject's controller controls, including the subject.
    MyUnits,
    /// The figure standing on the subject's space.
    Occupant,
}

/// Which figures a descriptor talks about. Results are always sorted by figure ID.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sel {
    /// The starting set.
    pub base: Base,
    /// Keeps figures with this trait value.
    pub trait_value: Option<Sym>,
    /// Keeps figures whose unit has this kind.
    pub kind: Option<Sym>,
    /// Keeps figures next to any figure of this selector.
    pub adjacent_to: Option<Box<Sel>>,
    /// Keeps figures within this many hexes of any figure of `of`.
    pub within: Option<u32>,
    /// The reference for `within`; `None` means `self`.
    pub of: Option<Box<Sel>>,
}

/// A comparison operator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cmp {
    /// Equal.
    Eq,
    /// Not equal.
    Ne,
    /// Less than.
    Lt,
    /// Less than or equal.
    Lte,
    /// Greater than.
    Gt,
    /// Greater than or equal.
    Gte,
}

/// A yes-or-no question about the board.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Cond {
    /// Always the same answer.
    Const(bool),
    /// Every condition holds.
    All(Vec<Cond>),
    /// At least one holds.
    Any(Vec<Cond>),
    /// None holds.
    None(Vec<Cond>),
    /// The condition does not hold.
    Not(Box<Cond>),
    /// Two values compare as given.
    Compare(Cmp, Box<Val>, Box<Val>),
    /// The selector matches a number of figures within the bounds.
    Count {
        /// The figures counted.
        of: Sel,
        /// Fewest figures.
        min: i64,
        /// Most figures, if capped.
        max: Option<i64>,
    },
    /// The game is in this phase.
    Phase(Sym),
    /// Every selected figure, and at least one, stands on this terrain.
    OnTerrain {
        /// The terrain name.
        terrain: Sym,
        /// The figures checked.
        who: Sel,
    },
    /// The subject stands above every figure selected, by at least `by` levels.
    HigherThan {
        /// The figures compared against.
        than: Sel,
        /// The least difference.
        by: i64,
    },
}

/// A number.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Val {
    /// A literal.
    Lit(i64),
    /// A stat of the first selected figure.
    Stat {
        /// The stat name.
        stat: Sym,
        /// The figure asked.
        of: Sel,
    },
    /// How many figures are selected.
    Count(Sel),
    /// Hexes from the subject to the first selected figure.
    Distance(Sel),
    /// The subject's level minus the first selected figure's.
    LevelDifference(Sel),
    /// The total of the values.
    Sum(Vec<Val>),
    /// The smallest value.
    Min(Vec<Val>),
    /// The largest value.
    Max(Vec<Val>),
}

/// A compiled descriptor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Node {
    /// A selector.
    Sel(Sel),
    /// A condition.
    Cond(Cond),
    /// A value.
    Val(Val),
}

impl Node {
    /// The card-text phrase for this descriptor, such as `adjacent friendly soldier figures`.
    pub fn phrase(&self) -> String {
        match self {
            Node::Sel(s) => text::sel(s),
            Node::Cond(c) => text::cond(c),
            Node::Val(v) => text::val(v),
        }
    }
}
