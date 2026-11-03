//! A replay as readable text (design §12.6).

use strata_model::Event;

use crate::replay::Replay;

/// Prints `replay`: its setup, then each command with its answers, events and state hash.
///
/// `events` holds what each command committed, as [`PackTests::replay_events`] returns it; leave
/// it empty to print without events.
///
/// [`PackTests::replay_events`]: crate::pack_test::PackTests::replay_events
pub fn dump(replay: &Replay, events: &[Vec<Event>]) -> String {
    let setup = &replay.setup;
    let mut out = format!("engine {}\n", replay.engine_version);
    for pack in &setup.packs {
        let (id, version, hash) = (&pack.id, &pack.version, pack.hash);
        out.push_str(&format!("pack {id} {version} {hash:#018x}\n"));
    }
    out.push_str(&format!(
        "ruleset {:?}  format {:?}  scenario {:?}  seed {:?}\n",
        setup.ruleset, setup.format, setup.scenario, setup.seed
    ));
    for player in &setup.players {
        let (id, slot, team) = (player.id.0, &player.slot, player.team);
        let army = player.army.as_ref().map_or(String::new(), |a| {
            let units: Vec<String> = a
                .units
                .iter()
                .map(|u| format!("{}x{}", u.copies, u.unit))
                .collect();
            units.join(" ")
        });
        out.push_str(&format!("player {id} slot {slot} team {team}: {army}\n"));
    }
    out.push_str(&format!("{} commands\n", replay.commands.len()));
    for (index, record) in replay.commands.iter().enumerate() {
        out.push_str(&format!(
            "\n#{index} player {} {:?}\n",
            record.player.0, record.command
        ));
        for answer in &record.answers {
            out.push_str(&format!("  answer {answer:?}\n"));
        }
        for event in events.get(index).into_iter().flatten() {
            out.push_str(&format!("  event {event:?}\n"));
        }
        out.push_str(&format!("  hash {:#018x}\n", record.state_hash));
    }
    out
}
