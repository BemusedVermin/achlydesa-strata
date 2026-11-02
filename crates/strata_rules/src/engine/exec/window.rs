//! Windows: named pauses where eligible reactions are offered as decisions (design §6.7).

use std::collections::BTreeSet;

use strata_kernel::{Sym, WindowId};
use strata_model::{Answer, Decision, DecisionKind, PlayerId};

use super::Exec;
use crate::engine::ctx::Ctx;
use crate::engine::halt::Halt;

impl Exec<'_> {
    /// Opens window `id`. Rejects an undeclared window, or one already open further up.
    pub(super) fn open_window(&mut self, id: &WindowId) -> Result<(), Halt> {
        if !self.rt.windows().contains(id) {
            return Err(Halt::reject(format!("window `{id}` is not declared")));
        }
        if self.open_windows.contains(id) {
            return Err(Halt::reject(format!("window `{id}` is already open")));
        }
        self.open_windows.push(id.clone());
        let result = self.offer_reactions(id);
        self.open_windows.pop();
        result
    }

    /// Offers reactions until nobody eligible is left. The active player is asked first; a
    /// player who passes is not asked again, and each reaction is used at most once.
    fn offer_reactions(&mut self, id: &WindowId) -> Result<(), Halt> {
        let (rt, hooks) = (self.rt, self.hooks);
        let mut used = BTreeSet::new();
        let mut passed: BTreeSet<PlayerId> = BTreeSet::new();
        loop {
            let mut eligible = Vec::new();
            for (i, def) in rt.reactions().iter().enumerate() {
                if def.window != *id || used.contains(&i) {
                    continue;
                }
                if let Some(p) = hooks.reactor(def, &self.state)?
                    && !passed.contains(&p)
                {
                    eligible.push((i, p));
                }
            }
            let active = self.state.flow.active;
            let Some(player) = eligible
                .iter()
                .map(|e| e.1)
                .min_by_key(|p| (*p != active, *p))
            else {
                return Ok(());
            };
            let mine: Vec<usize> = eligible
                .iter()
                .filter(|e| e.1 == player)
                .map(|e| e.0)
                .collect();
            let labels = std::iter::once(Sym::new("pass"))
                .chain(mine.iter().map(|i| Sym::new(&format!("reaction:{i}"))))
                .collect();
            let decision = Decision {
                prompt: Sym::new(&format!("window:{id}")),
                kind: DecisionKind::ChooseOption(labels),
            };
            match self.choose(player, decision)? {
                Answer::Option(0) => {
                    passed.insert(player);
                }
                Answer::Option(k) => {
                    let i = mine[k as usize - 1];
                    used.insert(i);
                    hooks.react(&rt.reactions()[i], self)?;
                }
                _ => return Err(Halt::reject("answer does not fit the decision")),
            }
        }
    }
}
