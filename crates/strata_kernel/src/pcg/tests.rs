use super::*;

/// Reference vectors from the PCG C `pcg32-demo` (`pcg32_srandom_r(&rng, 42u, 54u)`),
/// https://www.pcg-random.org/using-pcg-c.html and `pcg-c/test-high/expected/check-pcg32.out`.
#[test]
fn matches_reference_vectors() {
    let mut rng = Pcg32::new(42, 54);
    let expected = [
        0xa15c02b7u32,
        0x7b47f409,
        0xba1d3330,
        0x83d2f293,
        0xbfa4784b,
        0xcbed606e,
    ];
    for want in expected {
        assert_eq!(rng.next_u32(), want);
    }
}

#[test]
fn bounded_stays_in_range() {
    let mut rng = Pcg32::new(7, 1);
    for n in [1u32, 2, 3, 6, 7, 100, 1 << 31, u32::MAX] {
        for _ in 0..1000 {
            assert!(rng.bounded(n) < n);
        }
    }
    for _ in 0..1000 {
        assert!((1..=6).contains(&rng.roll(6)));
    }
}

#[test]
fn bounded_one_is_always_zero() {
    let mut rng = Pcg32::new(1, 1);
    assert!((0..100).all(|_| rng.bounded(1) == 0));
}

#[test]
#[should_panic]
fn bounded_zero_panics() {
    Pcg32::new(1, 1).bounded(0);
}

#[test]
fn bounded_six_passes_chi_square() {
    let mut rng = Pcg32::new(12345, 6789);
    let samples = 60_000u64;
    let mut counts = [0u64; 6];
    for _ in 0..samples {
        counts[rng.bounded(6) as usize] += 1;
    }
    let expected = samples / 6;
    // Integer chi-square statistic scaled by `expected`: sum((o - e)^2) / e.
    let stat: u64 = counts
        .iter()
        .map(|&o| o.abs_diff(expected).pow(2))
        .sum::<u64>()
        / expected;
    // 5 degrees of freedom: the 0.1% critical value is 20.5.
    assert!(
        stat < 21,
        "chi-square {stat} too large for counts {counts:?}"
    );
}
