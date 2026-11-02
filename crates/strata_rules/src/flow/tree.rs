//! The phase tree built from module registrations (design §6.4).

use std::collections::BTreeMap;

use strata_kernel::{DefId, Sym};

use crate::registrar::{Anchor, PhaseDef};

/// The implicit parent of top-level phases. Anchor `Within(ROOT)` to add one.
pub const ROOT: &str = "root";

/// The registered phases cannot form a tree.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum FlowError {
    /// Two modules registered the same phase.
    #[error("phase `{0}` is defined twice")]
    Duplicate(Sym),
    /// A phase is anchored to a phase that does not exist.
    #[error("phase `{phase}` is anchored to unknown phase `{anchor}`")]
    UnknownAnchor {
        /// The phase being inserted.
        phase: Sym,
        /// The missing anchor target.
        anchor: Sym,
    },
}

/// Phases in nesting and sibling order.
///
/// A phase's `DefId` in flow events is its position in pre-order, starting at 0.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PhaseTree {
    children: BTreeMap<Sym, Vec<Sym>>,
    parents: BTreeMap<Sym, Sym>,
    order: Vec<Sym>,
}

impl PhaseTree {
    /// Builds the tree. Anchors may name phases registered later, so registration order
    /// does not matter, except between phases inserted at the same anchor.
    pub fn build(phases: &[(PhaseDef, Anchor)]) -> Result<Self, FlowError> {
        let mut tree = PhaseTree::default();
        for (i, (def, _)) in phases.iter().enumerate() {
            if phases[..i].iter().any(|(p, _)| p.id == def.id) {
                return Err(FlowError::Duplicate(def.id.clone()));
            }
        }
        let mut pending: Vec<_> = phases.iter().collect();
        while !pending.is_empty() {
            let before = pending.len();
            pending.retain(|(def, at)| !tree.insert(&def.id, at));
            if pending.len() == before {
                let (def, at) = pending[0];
                let (Anchor::Before(a) | Anchor::After(a) | Anchor::Within(a)) = at;
                return Err(FlowError::UnknownAnchor {
                    phase: def.id.clone(),
                    anchor: a.clone(),
                });
            }
        }
        tree.order = tree.preorder(&Sym::new(ROOT));
        Ok(tree)
    }

    fn insert(&mut self, id: &Sym, at: &Anchor) -> bool {
        let (parent, index) = match at {
            Anchor::Within(p) if p.as_str() == ROOT || self.parents.contains_key(p) => {
                (p.clone(), usize::MAX)
            }
            Anchor::Before(s) | Anchor::After(s) => {
                let Some(parent) = self.parents.get(s) else {
                    return false;
                };
                let pos = self.children[parent].iter().position(|c| c == s);
                let after = usize::from(matches!(at, Anchor::After(_)));
                (parent.clone(), pos.unwrap_or(0) + after)
            }
            Anchor::Within(_) => return false,
        };
        let siblings = self.children.entry(parent.clone()).or_default();
        siblings.insert(index.min(siblings.len()), id.clone());
        self.parents.insert(id.clone(), parent);
        true
    }

    fn preorder(&self, from: &Sym) -> Vec<Sym> {
        let kids = self.children.get(from).into_iter().flatten();
        kids.flat_map(|c| std::iter::once(c.clone()).chain(self.preorder(c)))
            .collect()
    }

    /// Every phase in pre-order, which is also `DefId` order.
    pub fn phases(&self) -> &[Sym] {
        &self.order
    }

    /// The `DefId` that flow events use for `phase`.
    pub fn id(&self, phase: &Sym) -> Option<DefId> {
        let at = self.order.iter().position(|p| p == phase)?;
        Some(DefId(at as u32))
    }

    /// The phase that `id` names.
    pub fn name(&self, id: DefId) -> Option<&Sym> {
        self.order.get(id.0 as usize)
    }

    /// The first child of `phase`, or of the root for `ROOT`.
    pub fn first_child(&self, phase: &Sym) -> Option<&Sym> {
        self.children.get(phase)?.first()
    }

    /// The sibling after `phase`.
    pub fn next_sibling(&self, phase: &Sym) -> Option<&Sym> {
        let siblings = self.children.get(self.parents.get(phase)?)?;
        siblings.get(siblings.iter().position(|c| c == phase)? + 1)
    }

    /// The parent of `phase`; `ROOT` for top-level phases.
    pub fn parent(&self, phase: &Sym) -> Option<&Sym> {
        self.parents.get(phase)
    }
}
