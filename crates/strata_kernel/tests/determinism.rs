//! Same seed, same draws and hashes, twice (T-003).

use strata_kernel::{CommandRng, StableHash};

fn run(seed: u64) -> Vec<u64> {
    (0..20)
        .map(|index| {
            let mut rng = CommandRng::for_command(seed, index);
            let rolls: Vec<u32> = (0..5).map(|_| rng.roll(6)).collect();
            rolls.stable_hash_value()
        })
        .collect()
}

#[test]
fn replay_hashes_are_identical() {
    assert_eq!(run(99), run(99));
    assert_ne!(run(99), run(100));
}
