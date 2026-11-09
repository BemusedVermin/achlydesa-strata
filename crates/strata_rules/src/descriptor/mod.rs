//! The native descriptor evaluator (design §6.6, §8.2).
//!
//! A [`Descriptor`](strata_model::Descriptor) is a tagged [`Value`](strata_kernel::Value) map:
//! the tag key is the namespace (`sel`, `cond`, `val` or `eff`) and its value names the
//! descriptor, so `{ sel = "allies", trait = "soldier", adjacent_to = "self" }` selects adjacent
//! soldier allies. Bare literals stand in for the simplest descriptors: a string as a selector
//! (`"self"`), a boolean as a condition and an integer as a value. In Lua, plain data in any logic
//! field is a descriptor and a function is a script. [`compile`] turns the tree into a typed
//! [`Node`] and rejects unknown tags and fields; the loader runs [`check_ability`] on every
//! ability at freeze, so no bad descriptor survives to runtime.
//!
//! Selectors, conditions and values evaluate through [`NativeEvaluator`]. Effects change the game,
//! so they run through [`run_effect`] instead. Every node has a card-text phrase, and
//! [`card_text`] fills a template with them (design §8.4).

use strata_kernel::Sym;

mod card;
mod compile;
mod effect;
mod eval;
#[cfg(test)]
mod tests;
mod text;

pub use card::{CardTextError, card_text};
pub use compile::{
    DescriptorError, LogicFault, check_ability, check_effect, check_logic, compile, native_effect,
};
pub use effect::{EffectCx, EffectHost, run_effect};
pub use eval::{AttackView, NativeEvaluator, World};

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
    /// The figure the logic is aimed at: an action's or effect's target, the figure an event is
    /// about, or a pair query's context.
    Target,
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
    /// Keeps figures that any figure of this selector has line of sight to.
    pub in_los: Option<Box<Sel>>,
    /// Keeps engaged figures when true, and figures not engaged when false.
    pub engaged: Option<bool>,
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
    /// Every selected figure, and at least one, is engaged.
    Engaged(Sel),
    /// Some figure of `from` has line of sight to some figure of `to`.
    InLos {
        /// The figures looking.
        from: Sel,
        /// The figures looked at.
        to: Sel,
    },
    /// An attack is in progress and is of this kind.
    AttackKind(AttackKind),
}

/// Whether an attack reaches past adjacency.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttackKind {
    /// Against an adjacent target.
    Melee,
    /// Against a target beyond adjacency.
    Ranged,
}

/// Something that happens. Each effect acts on the figures of `who`, one at a time in ID order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Eff {
    /// Wounds each figure through the `units` slot.
    Wound {
        /// The figures wounded; the target by default.
        who: Sel,
        /// Wounds each.
        amount: Val,
    },
    /// Removes up to `amount` plain wounds from each figure through the `units` slot.
    Heal {
        /// The figures healed; the target by default.
        who: Sel,
        /// Wounds removed from each.
        amount: Val,
    },
    /// Puts each figure on an empty space within `within` of a figure of `of`, which the
    /// subject's controller chooses. Placing walks no path and provokes nothing.
    Place {
        /// The figures placed; the target by default.
        who: Sel,
        /// The farthest the space may be from `of`.
        within: Val,
        /// The figures the distance is measured from; `self` by default.
        of: Sel,
    },
    /// Removes each figure from the game, without destroying it, through the `units` slot.
    Remove {
        /// The figures removed; the target by default.
        who: Sel,
    },
    /// Moves each figure to an empty space up to `up_to` hexes away, which the subject's
    /// controller chooses. A forced move jumps straight there and provokes nothing.
    Move {
        /// The figures moved; the target by default.
        who: Sel,
        /// The most hexes moved.
        up_to: Val,
    },
    /// Gives each figure in the current activation `count` more of a slot.
    GrantSlot {
        /// The slot, such as `attack`.
        slot: Sym,
        /// How many more.
        count: Val,
        /// The figures; `self` by default.
        who: Sel,
    },
    /// Puts `count` of a marker on each figure, sourced to the subject.
    AddMarker {
        /// The marker's name, such as `stunned`.
        marker: Sym,
        /// How many.
        count: Val,
        /// Rounds it lasts, counting the current one; `None` lasts until removed.
        rounds: Option<u32>,
        /// The figures marked; the target by default.
        who: Sel,
    },
    /// Hands each figure to the subject's controller.
    TakeControl {
        /// The figures taken; the target by default.
        who: Sel,
    },
    /// The attack in progress deals no wounds: its hits drop to its blocks. As a replacement for
    /// `wounds_applied`, the wounds are cancelled outright.
    CancelWounds,
    /// The attacker of the attack in progress takes one wound per block beyond the hits.
    ExcessBlocksWoundAttacker,
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
    /// An effect.
    Eff(Eff),
}

impl Node {
    /// The card-text phrase for this descriptor, such as `adjacent friendly soldier figures`.
    pub fn phrase(&self) -> String {
        match self {
            Node::Sel(s) => text::sel(s),
            Node::Cond(c) => text::cond(c),
            Node::Val(v) => text::val(v),
            Node::Eff(e) => text::eff(e),
        }
    }
}
