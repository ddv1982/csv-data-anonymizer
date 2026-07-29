//! Bounded sampling of a stream whose length is not known in advance.

/// Keeps `capacity` of however many items are offered.
///
/// Every item is given a priority derived from its position in the stream, and the
/// `capacity` lowest-priority items seen so far are the ones kept. Which priority
/// function is used decides what kind of window that produces:
///
/// - [`SpreadSampler::head`] scores by the position itself, so the first `capacity`
///   items win. Once the buffer is full no later item can beat it, which
///   [`SpreadSampler::is_full`] lets a caller use to stop reading.
/// - [`SpreadSampler::spread`] scores by a fixed hash of the position, which draws a
///   sample from the whole stream: unbiased, reproducible for a given input, and
///   bounded to `capacity` items however long the stream turns out to be.
///
/// Anything that classifies data has to spread. A head window is a window on the
/// input's opening, and the transforms here process every item, so a value the
/// classifier never saw is a value whose PII is never detected, never selected, and
/// copied verbatim into output the user believes is anonymized.
///
/// The spread priorities have to be pseudorandom rather than positional. Keeping
/// every nth item is the obvious bounded-memory scheme and it reads as the fairest
/// one, but a fixed period samples a periodic input at a fixed phase, and real
/// inputs are periodic: a flattened export that writes one logical record per four
/// rows, with the email address always on the fourth, is either sampled entirely on
/// its email rows or entirely off them. Measured on a 400-row file of that shape, a
/// four-row stride saw none of the 100 email addresses in it, classified the column
/// `String` at Low risk, and so left it out of the auto-selection. Pseudorandom
/// priorities have no period to align with; all they cost is even spacing, which
/// nothing depends on.
pub(crate) struct SpreadSampler<T> {
    capacity: usize,
    priority: fn(usize) -> u64,
    offered: usize,
    /// Kept items, always ordered by position: see [`SpreadSampler::push`].
    kept: Vec<Kept<T>>,
    /// Index into `kept` of the highest-priority item — the one a better candidate
    /// evicts. Only meaningful once `kept` is full.
    worst: usize,
}

struct Kept<T> {
    priority: u64,
    item: T,
}

impl<T> SpreadSampler<T> {
    /// A sample of the whole stream. Use this for anything that classifies.
    pub(crate) fn spread(capacity: usize) -> Self {
        Self::new(capacity, spread_priority)
    }

    /// The stream's first `capacity` items. Only for display, and for reading back
    /// small outputs.
    pub(crate) fn head(capacity: usize) -> Self {
        Self::new(capacity, position_priority)
    }

    fn new(capacity: usize, priority: fn(usize) -> u64) -> Self {
        Self {
            capacity,
            priority,
            offered: 0,
            kept: Vec::new(),
            worst: 0,
        }
    }

    /// Offers one item, keeping it if it beats what is already held.
    pub(crate) fn push(&mut self, item: T) {
        self.push_with(|| item);
    }

    /// Offers one item, building it only if it is kept.
    ///
    /// For streams whose items cost an allocation and are mostly dropped — a field
    /// of a large paste is offered to two samplers, each of which keeps a small
    /// fraction of what it sees.
    pub(crate) fn push_with(&mut self, item: impl FnOnce() -> T) {
        let position = self.offered;
        self.offered += 1;
        if self.capacity == 0 {
            return;
        }

        let priority = (self.priority)(position);
        if self.kept.len() >= self.capacity && priority >= self.kept[self.worst].priority {
            return;
        }
        let candidate = Kept {
            priority,
            item: item(),
        };
        if self.kept.len() < self.capacity {
            self.kept.push(candidate);
            if self.kept.len() == self.capacity {
                self.worst = self.worst_kept();
            }
            return;
        }
        // Positions only ever rise, so dropping one entry and appending the
        // candidate leaves `kept` ordered by position without a sort.
        self.kept.remove(self.worst);
        self.kept.push(candidate);
        self.worst = self.worst_kept();
    }

    /// Rescans for the highest priority held. Linear, but only reached when an item
    /// is actually kept, and that becomes rare as the stream grows: past the first
    /// `capacity` items an item is kept with probability `capacity / offered`, so a
    /// `capacity`-item sample of n items keeps about `capacity * ln(n / capacity)`
    /// of them in total.
    fn worst_kept(&self) -> usize {
        self.kept
            .iter()
            .enumerate()
            .max_by_key(|(_, kept)| kept.priority)
            .map(|(index, _)| index)
            .unwrap_or(0)
    }

    /// How many items have been offered, kept or not — the stream's exact length
    /// once it is exhausted.
    pub(crate) fn offered(&self) -> usize {
        self.offered
    }

    /// Whether a head sampler has everything it will ever keep.
    pub(crate) fn is_full(&self) -> bool {
        self.kept.len() >= self.capacity
    }

    pub(crate) fn len(&self) -> usize {
        self.kept.len()
    }

    /// The `index`th kept item in input order.
    pub(crate) fn get(&self, index: usize) -> Option<&T> {
        self.kept.get(index).map(|kept| &kept.item)
    }

    /// The kept items, in input order.
    pub(crate) fn into_items(self) -> Vec<T> {
        self.kept.into_iter().map(|kept| kept.item).collect()
    }
}

fn position_priority(position: usize) -> u64 {
    position as u64
}

/// A position's spread-sampling priority: SplitMix64 over the position.
///
/// Any fixed hash whose output rank is uncorrelated with its input works. What
/// matters is that it is a pure function of the position — the same input always
/// yields the same sample, so a preview and the run that follows it classify on the
/// same values — and that its ordering bears no relation to the position's residue
/// modulo anything. See [`SpreadSampler`].
fn spread_priority(position: usize) -> u64 {
    let mut state = (position as u64).wrapping_add(0x9e37_79b9_7f4a_7c15);
    state = (state ^ (state >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    state = (state ^ (state >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    state ^ (state >> 31)
}

#[cfg(test)]
mod tests;
