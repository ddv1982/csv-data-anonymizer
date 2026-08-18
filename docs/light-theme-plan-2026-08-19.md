# Light theme rebuild plan (2026-08-19)

Follow-up to [`ui-design-system-plan-2026-08-18.md`](ui-design-system-plan-2026-08-18.md). The
dark theme was re-cut in that pass; the light theme kept its pre-existing values and now looks
like the weaker sibling. This plan rebuilds it on the same token layer.

Also fixed on the way in (already landed): the row `Action` dropdowns clipped their text. The
short control box inherited the body line-height, so the line box overflowed a 2rem select and
cut the glyphs. Selects now set their own line box and vertical padding.

## 1. What is wrong with the current light theme

Verified against `frontend/src/styles/tokens/theme-light.css`.

1. **The neutrals and the brand do not agree.** The canvas is `216 33% 97%` (cool blue-grey), the
   brand is `211 89% 39%`, the accent container is `204 86% 94%` — three different blues. Premium
   light UIs derive every neutral from the brand hue at 2–4% saturation so the accent lands in a
   field that already leans the same direction; ours reads as three unrelated palettes.
2. **Surfaces are nearly identical, so shadows do all the work.** Canvas 97% vs card 100% is a
   3-point gap; the card only separates because of a black-tinted shadow. Black shadows on a
   tinted surface read as an effect layered on top rather than a property of the surface.
3. **Borders are heavier than the surfaces they separate.** `214 26% 84%` against a 97–100%
   field is a strong line — every card, panel, input and table cell draws one, so the page reads
   as a grid of boxes instead of a document.
4. **Too many earned boundaries.** Cards contain panels which contain framed tables; three
   nested bordered levels where spacing alone would group. This is the "boxes on boxes" effect,
   and it is much more visible in light than in dark.
5. **Component tints were tuned for dark.** After the unification pass, pills, chips, badges and
   hover states use `hsl(var(--token) / 0.12)` alpha fills. On near-black that reads as a solid
   tint; on white it washes out into pastel. Light needs opaque tint steps, not alpha.
6. **Text weight is not compensated.** Light text on dark renders thin, so the dark theme was
   tuned down in weight; the same values on white now read slightly light and low-contrast.

## 2. Direction (from the research)

- **Tinted neutrals.** Every light neutral carries 2–4% of the brand hue. No pure `#fff`, no
  hue-less grey. The canvas leans the same direction as the accent.
- **Temperature-carrying elevation.** Shadows built from the brand's dark tone, not black. Each
  element gets one edge treatment: either a hairline border or a shadow ring, never both, so
  cards stop reading as double-outlined.
- **Earn the boundary.** Prefer spacing, alignment and density before a border or a box. Remove
  one whole level of nesting rather than restyling all three.
- **Explicit step roles** (Radix's 12-step model, which we mirror without adopting the library):
  steps 1–2 backgrounds, 3–5 component background / hover / selected, 6–8 borders (6 static,
  7 interactive, 8 focus), 9–10 solid fills, 11–12 low- and high-contrast text. Every current
  token gets mapped to a role so light stops improvising.
- **Restrained colour.** Neutrals plus one accent; status colour only where it means something,
  always paired with a non-colour cue (we already have the risk row edge).

## 3. Phases

Each phase ends green on `npm run validate`, `npm run frontend:e2e` and `npm run frontend:a11y`.

### Phase L0 — Baselines (half day)
Capture light-theme screenshots of the six main surfaces (file workflow, column review, preview,
privacy report, paste, quick) and a contrast matrix of every text-on-surface pair currently in
use, so the rebuild is measured rather than eyeballed.

### Phase L1 — The light palette (1 day)
Build a 12-step brand-tinted neutral ramp plus brand, success, warning and danger ramps as
primitives, and re-point every light semantic token at a step by role. Deliverable is
`theme-light.css` only — no component CSS changes, so the diff is reviewable as a palette swap.

### Phase L2 — Elevation and borders (half day)
Replace black shadows with brand-temperature shadows carrying an inline hairline layer; lift
border tokens to step 6/7 so lines sit just above the surface instead of dominating it; remove
the double treatment where an element draws both a border and a shadow ring.

### Phase L3 — Fewer surfaces (1 day)
Flatten a nesting level: the table frame loses its own border inside a card, settings and Local
AI panels become spaced sections with a single hairline rather than filled boxes, and section
headers carry the grouping. This is the change that will make the biggest visual difference.

### Phase L4 — Light-mode component states (1 day)
Give pills, chips, badges, hover, selected and focus states opaque step 3–5 fills in light while
keeping the alpha tints in dark, expressed as tokens (`--pill-fill`, `--row-selected`, …) so the
component CSS still has no theme branch. Verify all six microstates per control.

### Phase L5 — Typographic compensation (half day)
Nudge weight and colour for light (body one step heavier than dark, headings unchanged), confirm
tabular numerals in tables, and re-check the small 11–12px metadata text against the new
surfaces.

### Phase L6 — Verification and documentation (half day)
Re-run the contrast matrix and axe, refresh the screenshots, and write the light half of
`docs/design-system.md` with the step-role table and the do/don't list.

## 4. Status (2026-08-19)

Decisions: cool, brand-tinted light theme; implement L1-L3 first.

Landed, with `npm run validate`, `npm run frontend:e2e` and `npm run frontend:a11y` green:

- **L1.** `theme-light.css` rebuilt on a brand-tinted neutral ramp with step roles named in the
  file: canvas, raised surface, component background, hover, selected, static border,
  interactive border, and the two text steps. Brand, status and container colours re-cut to sit
  on those surfaces.
- **L2.** Shadows now carry the brand temperature (`hsl(214 44% 24%)`) instead of black, and
  border tokens moved up the ramp so hairlines sit just above the surface rather than
  dominating it.
- **L3.** One level of nesting removed: the column ledger runs full-bleed to its card edges
  instead of being a framed table inside a framed card, and the settings, Local AI and preview
  panels became recessed wells with no border of their own. `.card-content` publishes
  `--card-inset` so full-bleed children can cancel the padding without hard-coding it.

Remaining: L4 (opaque light-mode tints for pills, chips and states, currently still alpha fills
tuned for dark), L5 (typographic compensation), L6 (contrast matrix, screenshots, design-system
documentation).

## 5. Open decision

Resolved on 2026-08-19: option 1, cool and brand-tinted. Recorded here with the alternatives
that were weighed.

1. **Cool, brand-tinted** (chosen) — neutrals carry the same cool hue as the dark theme's accent. The two
   themes read as siblings; safest fit for a data tool.
2. **Warm paper** — a warm off-white canvas with cool accent kept for action. More editorial and
   distinctive, and the strongest contrast with the dark theme, but it makes the two themes feel
   like two products unless the dark theme also warms slightly.
3. **Near-white minimal** — an almost paper-free canvas where structure comes from spacing and
   hairlines only, in the Vercel direction. The most restrained, and the least forgiving of any
   remaining layout looseness.
