//! Replays as pack-test material: the events each command committed, and a regression test
//! that starts from the state after any command (design §15.4).

use std::collections::BTreeMap;

use strata_kernel::Value;
use strata_model::{
    Answer, Command, DieDef, DieKind, Event, FigureId, FigureStatus, GameState, RulesetDef,
    ScenarioDef, UnitDef,
};
use strata_rules::event_kind;

use super::run::{event_figure, status_name};
use super::{MatchRig, PackTests};
use crate::replay::{Replay, ReplayError, ReplayRecord};
use crate::setup::SetupError;

/// A replay could not be replayed against the loaded packs, or cannot be written as a test.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ExportError {
    /// The replay does not reproduce.
    #[error(transparent)]
    Replay(#[from] ReplayError),
    /// The match could not be built from the replay's setup.
    #[error(transparent)]
    Setup(#[from] SetupError),
    /// The replay uses something a pack test cannot express.
    #[error("{0}")]
    Unsupported(String),
}

fn unsupported<T>(message: &str) -> Result<T, ExportError> {
    Err(ExportError::Unsupported(message.to_owned()))
}

impl PackTests {
    /// Re-resolves every command of `replay` against these packs and checks each state hash.
    /// Returns the events each command committed. The first mismatch names its command.
    pub fn replay_events(&self, replay: &Replay) -> Result<Vec<Vec<Event>>, ExportError> {
        replay.verify(&self.registry)?;
        let rig = MatchRig::new(self, replay.setup.clone())?;
        Ok(replay.play_events(&mut rig.engine())?)
    }

    /// A pack test, as Lua source, that places the figures as they stand after `at` commands,
    /// runs the next command with its dice rigged, and expects what the replay recorded.
    ///
    /// Only the figures' places are carried over: wounds, markers, tokens and the turn flow
    /// start fresh.
    pub fn to_test(&self, replay: &Replay, at: usize) -> Result<String, ExportError> {
        let Some(record) = replay.commands.get(at) else {
            let count = replay.commands.len();
            let message = format!("the replay has {count} commands, so --at must be below that");
            return Err(ExportError::Unsupported(message));
        };
        replay.verify(&self.registry)?;
        let rig = MatchRig::new(self, replay.setup.clone())?;
        let mut engine = rig.engine();
        replay.check_start(&engine)?;
        for (index, earlier) in replay.commands[..at].iter().enumerate() {
            earlier.apply(&mut engine, index)?;
        }
        let before = engine.state().clone();
        let events = record.apply(&mut engine, at)?;
        let handles = self.handles(&before);
        let handle = |id: FigureId| handles.get(&id).cloned().unwrap_or_default();

        let mut steps = vec![self.command_step(&rig, record, &events, &handle)?];
        for answer in &record.answers {
            steps.push(answer_step(answer)?);
        }
        steps.extend(expectations(&events, engine.state(), &handle));

        let header = self.header(replay, at)?;
        let mut text = header;
        for line in self.places(&before)? {
            text.push_str(&format!("    {line},\n"));
        }
        text.push_str("  },\n  steps = {\n");
        for step in steps {
            text.push_str(&format!("    {step},\n"));
        }
        text.push_str("  },\n}\n");
        Ok(text)
    }

    /// The opening of the test: its names, ruleset and map.
    fn header(&self, replay: &Replay, at: usize) -> Result<String, ExportError> {
        let registry = &self.registry;
        let ruleset = registry.name::<RulesetDef>(replay.setup.ruleset);
        let scenario = registry.get::<ScenarioDef>(replay.setup.scenario);
        let full = registry.name::<ScenarioDef>(replay.setup.scenario);
        let (Some(ruleset), Some(scenario), Some(full)) = (ruleset, scenario, full) else {
            return unsupported("the replay's ruleset or scenario is not loaded");
        };
        let pack = full.split_once(':').map_or("", |(pack, _)| pack);
        let map = scenario.map.as_str();
        let map = if map.contains(':') {
            map.to_owned()
        } else {
            format!("{pack}:{map}")
        };
        let (ruleset, map) = (lua_str(ruleset), lua_str(&map));
        Ok(format!(
            "test {{\n  id = 'replay_at_{at}', name = 'replay command {at}',\n  ruleset = {ruleset}, map = {map},\n  place = {{\n"
        ))
    }

    /// `p0.unit.figure#n` for every figure, numbered the way a test's `place` will number them.
    fn handles(&self, state: &GameState) -> BTreeMap<FigureId, String> {
        let mut seen: BTreeMap<(u8, u32, &str), usize> = BTreeMap::new();
        let mut handles = BTreeMap::new();
        for (index, figure) in (0u32..).zip(&state.figures) {
            let (player, entry) = (figure.controller.0, figure.def.figure.as_str());
            let nth = seen.entry((player, figure.def.unit.0, entry)).or_default();
            *nth += 1;
            let unit = self.registry.name::<UnitDef>(figure.def.unit);
            let handle = format!("p{player}.{}.{entry}#{nth}", unit.unwrap_or_default());
            handles.insert(FigureId(index), handle);
        }
        handles
    }

    /// One `place` entry per unit, in unit order so figure IDs stay the same. A unit entirely in
    /// reserve is placed with no spaces.
    fn places(&self, state: &GameState) -> Result<Vec<String>, ExportError> {
        let mut lines = Vec::new();
        for unit in &state.units {
            let name = self.registry.name::<UnitDef>(unit.def).unwrap_or_default();
            let figures: Vec<_> = unit
                .figures
                .iter()
                .map(|f| &state.figures[f.index()])
                .collect();
            let on_board = figures.iter().all(|f| f.status == FigureStatus::OnBoard);
            let reserve = figures.iter().all(|f| f.status == FigureStatus::Reserve);
            if !on_board && !reserve {
                let owner = unit.owner.0;
                return Err(ExportError::Unsupported(format!(
                    "unit `{name}` of player {owner} is partly deployed or has lost figures, which `place` cannot express"
                )));
            }
            let spaces = figures.iter().filter_map(|f| f.location);
            let spaces: Vec<String> = spaces
                .map(|s| format!("{{ q = {}, r = {}, z = {} }}", s.q, s.r, s.z))
                .collect();
            let (name, owner) = (lua_str(name), unit.owner.0);
            let at = spaces.join(", ");
            let at = if at.is_empty() { at } else { format!(" {at} ") };
            lines.push(format!(
                "{{ unit = {name}, player = {owner}, at = {{{at}}} }}"
            ));
        }
        Ok(lines)
    }

    /// The step that submits `record`, with the dice it rolled forced.
    fn command_step(
        &self,
        rig: &MatchRig,
        record: &ReplayRecord,
        events: &[Event],
        handle: &dyn Fn(FigureId) -> String,
    ) -> Result<String, ExportError> {
        let opts = self.rig_options(events)?;
        match &record.command {
            Command::Custom { id, payload } => {
                let (player, id) = (record.player.0, lua_str(id.as_str()));
                Ok(format!(
                    "act.command({player}, {id}, {}{opts})",
                    lua_value(payload)?
                ))
            }
            Command::Action {
                figure,
                action,
                args,
            } => {
                let actions = rig.runtime().actions();
                let Some(found) = actions.get(action.0 as usize) else {
                    return unsupported("the command names an unknown action");
                };
                let (figure, name) = (lua_str(&handle(*figure)), lua_str(found.id.as_str()));
                Ok(format!(
                    "act.action({figure}, {name}, {}{opts})",
                    lua_value(args)?
                ))
            }
            _ => unsupported("only actions and pack commands can be written as test steps"),
        }
    }

    /// The `, { rig = ... }` argument that forces the faces `events` rolled, or nothing.
    fn rig_options(&self, events: &[Event]) -> Result<String, ExportError> {
        let mut rolls = Vec::new();
        for event in events {
            let Event::DiceRolled { die, results } = event else {
                continue;
            };
            let Some(DieKind::Faces(faces)) = self.registry.get::<DieDef>(*die).map(|d| &d.kind)
            else {
                return unsupported("only dice with named faces can be rigged");
            };
            let mut names = Vec::new();
            for result in results {
                let Some(face) = faces.get(*result as usize) else {
                    return unsupported("a roll is outside its die");
                };
                names.push(lua_str(face.as_str()));
            }
            rolls.push(format!("{{ {} }}", names.join(", ")));
        }
        if rolls.is_empty() {
            return Ok(String::new());
        }
        Ok(format!(", {{ rig = {{ {} }} }}", rolls.join(", ")))
    }
}

/// `expect` steps for each distinct event the command committed, then every figure's status.
fn expectations(
    events: &[Event],
    after: &GameState,
    handle: &dyn Fn(FigureId) -> String,
) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for event in events {
        let kind = lua_str(event_kind(event));
        let line = match event_figure(event) {
            Some(figure) => {
                let figure = lua_str(&handle(figure));
                format!("expect.event({kind}, {{ figure = {figure} }})")
            }
            None => format!("expect.event({kind})"),
        };
        if !lines.contains(&line) {
            lines.push(line);
        }
    }
    for (index, figure) in (0u32..).zip(&after.figures) {
        let (handle, status) = (
            lua_str(&handle(FigureId(index))),
            status_name(figure.status),
        );
        lines.push(format!("expect.figure({handle}, {})", lua_str(status)));
    }
    lines
}

fn answer_step(answer: &Answer) -> Result<String, ExportError> {
    match answer {
        Answer::Figure(id) => Ok(format!("act.answer.figure({})", id.0)),
        Answer::Option(i) => Ok(format!("act.answer.option({i})")),
        Answer::YesNo(yes) => Ok(format!("act.answer.yes_no({yes})")),
        Answer::Number(n) => Ok(format!("act.answer.number({n})")),
        Answer::Space(_) | Answer::Order(_) => {
            unsupported("space and order answers have no test step")
        }
    }
}

/// A single-quoted Lua string.
fn lua_str(text: &str) -> String {
    let escaped = text
        .replace('\\', "\\\\")
        .replace('\'', "\\'")
        .replace('\n', "\\n");
    format!("'{escaped}'")
}

fn lua_value(value: &Value) -> Result<String, ExportError> {
    Ok(match value {
        Value::Nil => "nil".into(),
        Value::Bool(b) => b.to_string(),
        Value::Int(i) => i.to_string(),
        Value::Str(s) => lua_str(s.as_str()),
        Value::Id(_) => return unsupported("a definition handle cannot be written as Lua"),
        Value::List(items) => {
            let items: Result<Vec<_>, _> = items.iter().map(lua_value).collect();
            format!("{{ {} }}", items?.join(", "))
        }
        Value::Map(map) => {
            let mut fields = Vec::new();
            for (key, value) in map {
                fields.push(format!(
                    "[{}] = {}",
                    lua_str(key.as_str()),
                    lua_value(value)?
                ));
            }
            format!("{{ {} }}", fields.join(", "))
        }
    })
}
