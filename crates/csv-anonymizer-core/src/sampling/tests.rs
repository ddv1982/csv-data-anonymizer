use super::*;

fn kept_positions(sampler: SpreadSampler<usize>) -> Vec<usize> {
    sampler.into_items()
}

fn sample_spread(total: usize, capacity: usize) -> SpreadSampler<usize> {
    let mut sampler = SpreadSampler::spread(capacity);
    for position in 0..total {
        sampler.push(position);
    }
    sampler
}

#[test]
fn head_keeps_the_opening_items_and_reports_when_it_is_full() {
    let mut sampler = SpreadSampler::head(3);
    assert!(!sampler.is_full());
    for position in 0..10 {
        sampler.push(position);
    }

    assert!(sampler.is_full());
    assert_eq!(sampler.offered(), 10);
    assert_eq!(kept_positions(sampler), vec![0, 1, 2]);
}

#[test]
fn spread_keeps_exactly_its_capacity_in_input_order() {
    let sampler = sample_spread(1_000, 100);

    assert_eq!(sampler.len(), 100);
    assert_eq!(sampler.offered(), 1_000);
    let kept = kept_positions(sampler);
    assert!(
        kept.windows(2).all(|pair| pair[0] < pair[1]),
        "kept items must stay in input order, got {kept:?}"
    );
}

#[test]
fn spread_keeps_everything_from_a_short_stream() {
    let sampler = sample_spread(20, 100);

    assert_eq!(sampler.offered(), 20);
    assert_eq!(kept_positions(sampler), (0..20).collect::<Vec<_>>());
}

#[test]
fn spread_is_deterministic() {
    assert_eq!(
        kept_positions(sample_spread(777, 32)),
        kept_positions(sample_spread(777, 32))
    );
}

#[test]
fn spread_draws_from_every_part_of_the_stream() {
    let kept = kept_positions(sample_spread(1_000, 100));

    for tenth in 0..10 {
        let range = tenth * 100..(tenth + 1) * 100;
        assert!(
            kept.iter().any(|position| range.contains(position)),
            "positions {range:?} contributed nothing to {kept:?}"
        );
    }
}

/// The property a positional rule cannot have. See [`SpreadSampler`].
#[test]
fn spread_does_not_align_with_a_periodic_stream() {
    const CAPACITY: usize = 200;

    for period in [2usize, 3, 4, 5, 8, 16] {
        let kept = kept_positions(sample_spread(period * 500, CAPACITY));

        for phase in 0..period {
            let hits = kept.iter().filter(|item| *item % period == phase).count();
            assert!(
                hits >= CAPACITY / (period * 4),
                "phase {phase} of {period} got {hits} of {CAPACITY} kept items"
            );
        }
    }
}

/// A zero capacity still counts what it is offered. Analyze collects no display
/// values, and it still needs the exact record count.
#[test]
fn zero_capacity_keeps_nothing_but_still_counts() {
    let mut sampler = SpreadSampler::spread(0);
    for position in 0..10 {
        sampler.push(position);
    }

    assert_eq!(sampler.offered(), 10);
    assert_eq!(sampler.len(), 0);
    assert_eq!(sampler.get(0), None);
    assert!(kept_positions(sampler).is_empty());
}

#[test]
fn get_reads_kept_items_by_input_order_index() {
    let sampler = sample_spread(1_000, 10);
    let items = (0..10)
        .map(|index| *sampler.get(index).expect("ten items are kept"))
        .collect::<Vec<_>>();

    assert_eq!(sampler.get(10), None);
    assert_eq!(items, kept_positions(sampler));
}
