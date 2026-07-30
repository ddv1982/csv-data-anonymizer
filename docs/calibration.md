# Calibration evidence

Measurements behind the tuned constants in `csv-anonymizer-core`.

These figures were relocated out of doc comments so the source states the *rule*
and this file holds the *evidence*. They are not decoration: each constant below
is unfalsifiable without its table, and anyone retuning one needs to re-run the
named harness rather than reason from first principles.

Every figure is copied verbatim from the doc comment that previously sat above
the constant. If you change a constant, update the table and re-run its harness.

---

# `types.rs` — frequency-inversion constants

Source: the frequency-inversion constants in `types.rs`. Every figure below is copied
verbatim from the doc comments that previously sat above each constant.

## MIN_SAMPLE_COVERAGE

`const MIN_SAMPLE_COVERAGE: f64 = 0.75;`

Sample coverage below which a *sampled* distribution is treated as saying nothing about
the input's distinct count, so the ratio test is skipped.

Good–Turing coverage, `1 - singletons/values`: the estimated share of the column's values
that belong to groups the sample has already seen. Near 1 the sample has found essentially
every group and its distinct count is the column's; near 0 every value seen was new, so the
sample has learned nothing except that there are many.

This gate is what makes the ratio test safe on sampled data, and without it the test is
actively wrong.

### Uniform draws

Simulated 100-value samples drawn evenly across columns of known shape:

| column | coverage | Chao1 / rows | should warn |
| --- | --- | --- | --- |
| 3 statuses in 5k rows | 1.00 | 0.0006 | yes |
| 30 departments in 5k rows | 0.95 | 0.0066 | yes |
| 50 job titles in 100k rows | 0.87 | 0.0005 | yes |
| 200 cities in 5k rows | 0.40 | 0.0412 | no — 25-row groups |
| 1000 names in 5k rows | 0.12 | 0.1478 | no |
| unique in 5k rows | 0.04 | 0.4804 | no |
| **unique in 1M rows** | **0.00** | **0.0051** | **no** |

The last row is the reason this constant exists: a fully unique column in a large file
passes the ratio test outright, because 100 sampled values can never look like a million
distinct ones. Coverage is the statistic that separates it, and it separates every case
above — the data pins this constant only to the interval (0.40, 0.87], and 0.75 sits inside
it with margin at both ends.

### Zipf draws

The table above was measured on *uniform* draws, and the claim that skew only moves coverage
upward was an argument rather than a measurement. Re-measured on Zipf draws with
`zipf_column_file` in `service::tests::cardinality`: 100-value samples of a 5000-row column,
20 draws per cell, worst (lowest) coverage of the 20:

| labels | s=0.5 | s=0.8 | s=1.0 | s=1.2 | s=1.5 | s=2.0 |
| --- | --- | --- | --- | --- | --- | --- |
| 200 | 0.32 | 0.48 | 0.60 | 0.68 | 0.78 | 0.87 |
| 1000 | 0.08 | 0.21 | 0.40 | 0.59 | 0.75 | 0.86 |
| 5000 | 0.00 | 0.10 | 0.24 | 0.47 | 0.74 | 0.88 |

The constant survives: coverage does rise monotonically with skew at every label count, so
the gate opens as a column becomes more invertible, and it stays shut on every diverse column
here — a Zipf-1.0 column over 1000 labels holds around 750 distinct values in 5000 rows and
must not be flagged.

### Dominant value over a unique tail

It does not open in time on its own, though. At 5000 labels a Zipf-1.5 column, whose top
value already takes 39% of the rows, still draws samples below the gate. And the case that
pushed hardest is one the uniform draws could not produce at all: skew raises coverage only
while the skew is in the *body* of the distribution. One dominant value over a long unique
tail is severely skewed and has *low* coverage, because coverage counts singletons and the
tail is all singletons. Measured with `dominant_value_column_file` on one value covering `q`
of a 5000-row column, the rest spread over 5000 others, 20 draws per cell:

| q | 0.2 | 0.3 | 0.4 | 0.5 | 0.6 | 0.8 |
| --- | --- | --- | --- | --- | --- | --- |
| coverage | 0.15–0.28 | 0.23–0.40 | 0.33–0.51 | 0.46–0.59 | 0.54–0.70 | 0.71–0.86 |

At 20 draws per cell every draw up to q=0.6 sat below 0.75. Re-run at 400 draws per cell the
bound is not quite absolute: 0 of 2400 draws at q=0.5 reached the gate, and at q=0.6 six did
— and those six are exactly the six on which the ratio term fired, out of 2400 columns where
one value covered three fifths of the rows.

That is why `MIN_INVERTIBLE_DOMINANT_SHARE` is checked *before* this gate rather than behind
it: gating the dominant-value term on coverage would have silenced all but a quarter of a
percent of the shape the term exists for. This constant is right for what it gates — a
distinct-count estimate, which a singleton-heavy sample genuinely cannot make — and carries
no authority over anything else.

### Limits

Still not tested: draws from real production data, non-Zipf skew (bimodal columns, a few
large groups with no tail at all), and columns sitting near the gate itself, where coverage
moves by several hundredths between draws of the same column and the gate's answer is
therefore a coin flip rather than a verdict.

### Harness

`zipf_column_file` and `dominant_value_column_file` in `service::tests::cardinality`.

## MIN_INVERTIBLE_DOMINANT_SHARE

`const MIN_INVERTIBLE_DOMINANT_SHARE: f64 = 1.0 / 3.0;`

Share of a column's values carried by its single most common value, at or above which the
mapping is treated as frequency-invertible.

The failure this closes: a column with thousands of distinct values, one of which covers
most of the rows. Neither other term sees it. `distinct < 10` is nowhere near true, and the
Chao1 ratio is high precisely *because* the column is diverse — so a column where one value
covers 60% of five million rows drew no warning at all, even though inverting that single
pseudonym hands back 60% of the column to anyone who knows which value is the common one.
Measured with the dominant-value generator in `service::tests::cardinality`: across 120 draws
each at every combination of 1000 and 5000 tail values and 1000, 5000 and 100000 rows, the
absolute term fired 0 times and the ratio term fired 0 times for every q from 0.2 to 0.6
inclusive.

A share and not a count. `max_value_occurrences > 5000` is nonsense in a 200-row file and
nearly always true in a 5-million-row one; the share is the quantity that means the same
thing at both sizes, and it is also the quantity with the direct reading — the fraction of
the column one inversion recovers.

### Threshold sweep

Calibrated on the pre-run path, which is the hard case: the post-run report measures every
row and the share is exact, but the preview measures a 100-value sample, so the share is an
estimate with real variance. Fire rate of each candidate threshold, 4000 independent
100-value samples per configuration, given as the range over every column size tested —
200/1000/5000 labels for the Zipf rows, 1000/5000 tail values for the dominant ones, and
1000/5000/100000 rows for both:

| true dominant share | T=0.25 | T=0.30 | **T=1/3** | T=0.35 | T=0.40 | T=0.50 |
| --- | --- | --- | --- | --- | --- | --- |
| Zipf s=1.0 (0.11–0.17) | .000–.031 | .000–.001 | **.000** | .000 | .000 | .000 |
| Zipf s=1.1 (0.16–0.21) | .010–.212 | .000–.028 | **.000–.002** | .000–.001 | .000 | .000 |
| Zipf s=1.2 (0.21–0.26) | .215–.624 | .025–.210 | **.001–.047** | .001–.031 | .000–.002 | .000 |
| one value over 50% | 1.000 | 1.000 | **.999–1.000** | .999–1.000 | .979–.983 | .537–.542 |
| one value over 60% | 1.000 | 1.000 | **1.000** | 1.000 | 1.000 | .980–.985 |

Two requirements pick the constant from that table. A Zipf column with exponent up to 1.1
must stay silent: Zipf with s near 1 is the ordinary shape of real categorical data, its top
value takes a fifth of the rows at most, and a warning that fires there fires on most text
columns in most files — the same noise argument that keeps singleton counts out of the
predicate entirely. A column where one value genuinely covers half the rows must be caught.
The data pins the constant to the interval **[1/3, 0.35]**: at 0.30 a Zipf-1.1 column
false-fires on 2.8% of samples, and at 0.40 a truly 50%-dominant column is missed on 2% of
them. 1/3 rather than 0.35 because the two are indistinguishable on every measurement here
and 1/3 states the rule the warning is making — one value in every three rows.

The interval is narrow because both requirements are strict, and it should be read as what
the measurements happen to admit rather than as a discovered boundary. Relaxing the second
requirement to "caught on 95% of samples" moves its upper end past 0.40, where the rate is
.979, but not as far as 0.45, where it is .859. Tightening either requirement — ten times the
replicates, or demanding the false-positive rate hold at Zipf s=1.2 as well — would narrow
the interval or empty it; that is an extrapolation from the trend across the table, not a
measurement.

### What the measurements establish, and what they do not

A 100-value sample cannot reliably separate a 26%-dominant column from a 40%-dominant one —
at 1/3 the first fires on up to 4.7% of samples and the second on 90% of them — and no choice
of constant makes it able to. What the measurements do establish is that 1/3 separates the
shapes at the two *ends* — ordinary skew and one-value dominance — with a false-positive rate
at or below 0.2% and a miss rate at or below 0.1%.

Not tested: real production columns, tails that are not uniform, columns whose second value
is nearly as common as the first (where inverting the top pseudonym is a coin flip rather
than a certainty, so this term over-warns by construction), samples larger than 100 values —
the "Sample rows" setting can only raise that figure, which shrinks the variance above and so
can only move the fire rates toward the exact post-run answer — and the interaction with a
column that is *also* low-cardinality, which the absolute term answers first.

### Harness

The dominant-value generator in `service::tests::cardinality`.

## Sample share vs. distinct count

Supporting measurement cited by `ColumnValueDistribution::frequency_inversion_risk_in` for
why the dominant-value term needs neither a population figure nor a coverage gate:

Over 2400 draws, a 100-value spread sample of a column whose top value covers half the rows
reported a share between 0.36 and 0.69, while the same samples' distinct counts
under-reported the columns' by one to two orders of magnitude. That asymmetry is why the
distinct-count term needs both a population figure and a coverage gate while this one needs
neither.

### Harness

`service::tests::cardinality`.

---

# `strategies/state.rs` — mapping memory budget

Relocated verbatim from `crates/csv-anonymizer-core/src/strategies/state.rs`
(doc comments on `TransformState`'s memory-budget constants).

Harness for every figure below: `strategies::tests::mapping_budget`
(`crates/csv-anonymizer-core/src/strategies/tests/mapping_budget.rs`), whose
ignored tests print these figures and say how to re-run them.

## approximate-bytes-per-mapping-entry

Constant: `TransformState::APPROXIMATE_BYTES_PER_MAPPING_ENTRY = 160`
— bytes of resident memory one mapping entry costs, measured.

Method: measured on Linux with `VmHWM` read at the end of a one-column,
1,000,000-row transform. The harness is `strategies::tests::mapping_budget`,
whose ignored tests print these figures and say how to re-run them. The
streaming floor is subtracted, so what remains is the mapping's own cost:

| Run | Peak RSS | Entries | Bytes per entry |
| --- | --- | --- | --- |
| Redact, all distinct | 11 MiB | 0 | — (floor) |
| Label, all distinct | 162 MiB | 1,000,000 | 158 |
| Pseudonymize, 250,000 distinct | 127 MiB | 750,000 | 162 |
| Pseudonymize, all distinct | 477 MiB | 3,000,000 | 163 |

160 is the middle of that 158–163 band. The band is narrow across two
structures with different value types — a ledger entry is a `String` key with
two `usize`s, a mapper entry is a `String` key with a `String` value — because
at these sizes the cost is dominated by the allocator and hash-table overhead
per entry rather than by the payload, which is what makes one figure per
*entry* meaningful at all.

Range the data supports: keys of about 16 bytes, entry counts from 750,000 to
3,000,000, on 64-bit Linux with the system allocator. Not tested: other
platforms or allocators, 32-bit targets, long values (a 200-byte cell pays its
own bytes on top of this overhead, twice over on a pseudonymizing strategy
since the value is also a key of the reverse map), or entry counts far above
3,000,000, where the figure could drift with hash-table growth steps.

## mapping-entry-ceiling

Constant: `TransformState::MAPPING_ENTRY_CEILING = 32_000_000`
— mapping entries a single run may hold before it is refused.

At `APPROXIMATE_BYTES_PER_MAPPING_ENTRY` this is about 5.1 GB, and it is chosen
from both ends:

- It must not refuse work the app does today. The largest run this project has
  measured is four all-distinct columns of a 63 MB input at about 1.9 GB, which
  is 4 × 1,000,000 × 3 = 12,000,000 entries. The ceiling sits 2.7× above that.
  A single all-distinct pseudonymized column of 1,000,000 rows — the README's
  worst measured single-column case, 477 MiB — is 3,000,000 entries, under a
  tenth of it.
- It must fire before the machine dies. 5.1 GB of mapping still leaves room on
  the 8 GB floor of a current desktop, where the alternative is the OOM killer
  taking the process with no message at all.

Not tested: machines with less than 8 GB of RAM, and 32-bit builds, where 5.1 GB
is unreachable and this ceiling can never be the thing that fires.
