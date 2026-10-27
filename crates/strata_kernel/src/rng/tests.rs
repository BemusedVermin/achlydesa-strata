use super::*;

fn draw(mut rng: CommandRng) -> Vec<u32> {
    (0..8).map(|_| rng.next_u32()).collect()
}

#[test]
fn same_inputs_same_stream() {
    assert_eq!(
        draw(CommandRng::for_command(9, 3)),
        draw(CommandRng::for_command(9, 3))
    );
}

#[test]
fn inputs_select_distinct_streams() {
    let base = draw(CommandRng::for_command(9, 3));
    assert_ne!(base, draw(CommandRng::for_command(9, 4)));
    assert_ne!(base, draw(CommandRng::for_command(10, 3)));
}

#[test]
fn golden_stream() {
    let mut rng = CommandRng::for_command(0, 0);
    let got: Vec<u32> = (0..3).map(|_| rng.next_u32()).collect();
    assert_eq!(got, GOLDEN);
}

const GOLDEN: [u32; 3] = [1328264953, 2897752262, 4057153102];
