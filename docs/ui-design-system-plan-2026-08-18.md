# UI/UX design-system overhaul plan (2026-08-18)

Goal: move the app from "styled scaffold" to a product with a point of view — a dense, calm,
near-black workstation for privacy work, with one restrained accent, a real spacing and type
system, and a column-review table that reads like a ledger instead of a wall of text.

Reference inspiration (not to be copied): the Yuna marketing site — deep near-black surfaces,
warm accent used sparingly, tight display typography, quiet texture behind the fold. That is a
*marketing* aesthetic; what is transferable is the surface stack, the accent discipline, the
typographic contrast, and the sense that spacing was decided rather than accumulated.

## 1. Findings from the current frontend

Verified against `frontend/src` at v1.0.90.

1. **The dark theme has no elevation stack.** In `styles/base.css`, `--card`, `--surface-card`,
   `--surface-raised`, `--surface-inset` and `--surface-app` all resolve to `--background`
   (`222.2 84% 4.9%`). Every panel, card, table and page background is literally the same navy.
   The light theme, by contrast, has bespoke surfaces, gradients and calibrated shadows. That
   asymmetry is why the dark screenshots look unfinished while the light theme looks designed.
2. **The base palette is stock.** The dark values are the untouched shadcn "slate" defaults —
   the blue-on-black look, including the primary blue used for checkboxes and selected rows.
3. **Theme branching lives in component CSS, not in tokens.** 78 blocks across nine files are
   duplicated as `:root[data-resolved-theme='light'] X, :root.theme-resolved-light X`. Component
   rules should be written once and read tokens; only token values should differ per theme.
4. **There is no spacing or type scale.** 142 spacing declarations use raw rem values, including
   off-scale one-offs: `0.35rem`, `0.55rem`, `0.65rem`, `0.2rem`, `0.15rem`, `0.95rem`,
   `0.8125rem`. This is the direct cause of the ragged rhythm in the column table screenshot.
5. **The declared font is not shipped.** `base.css` asks for `Inter` first, but no `@font-face`
   and no font file exist (`frontend/public` holds only `icon.png`), so the app silently renders
   in the platform UI font. Typography is currently unowned.
6. **Elevation is shadow-only.** Shadows barely register on a near-black background; the
   hairline-border + surface-step technique that makes dark UI read as layered is unused.
7. **The column table carries the product but is styled generically.** Uniform `0.75rem 1rem`
   cell padding for cells that hold anything from a checkbox to a four-line evidence stack;
   status pill, confidence text, coverage line and output line all compete at similar weight;
   risk is communicated by color badge alone; the bulk-select buttons are five equal-weight
   buttons with no primary/secondary hierarchy.

None of this is a rewrite case. The token *architecture* is sound — the values and the scales
are the problem, plus where theming is expressed.

## 2. Constraints the design must respect

- Strict CSP (`src-tauri/tauri.conf.json`): no CDN fonts, no remote images. Fonts must be
  self-hosted (`font-src 'self' data:`); texture must be CSS gradients or inline/`data:` SVG.
- No new runtime dependencies: `deadcode:required` runs knip in production mode, and the app is
  offline-first. All work is plain CSS + existing React 19 / lucide-react.
- Accessibility is enforced (`npm run frontend:a11y`, axe via Playwright). WCAG AA contrast, and
  status must never be color-only.
- The app ships two themes and a system mode (`useTheme`); both stay first-class.
- Window minimum is 920×640 with responsive rules down to 320px — density decisions must hold at
  the small end.

## 3. Design direction

**Surfaces.** A neutral (not navy) ramp with four steps and real gaps between them, so depth is
visible without shadows:

```
app background   #0B0B0C     canvas
surface-1        #131316     cards, table frame
surface-2        #1A1A1E     nested panels, table header, inputs
surface-3        #232328     hover, popovers, dropdowns
hairline         rgba(255,255,255,0.07)   default border
hairline-strong  rgba(255,255,255,0.13)   focus/selected edges
```

Light theme keeps its current warm-neutral direction, re-expressed on the same token names.

**Accent discipline.** One accent, on under ~10% of any view: primary action, focus ring, active
tab, selected row edge. Everything else is neutral. Status colors (risk high/medium/low,
success, warning) are a separate, deliberately desaturated family — 10–20% less saturation than
their light-mode equivalents, because saturation reads hotter on near-black.

**Typography.** Ship a variable font locally (Inter Variable, ~110KB woff2, or Geist). Display
sizes get negative tracking (-0.02em) and weight 600 rather than 700; body gets +0.01em tracking
and 1.6 line-height for dark-mode legibility; numbers, IDs, placeholders and coverage counts move
to a mono face with tabular numerals. Hierarchy comes from size and color, not from bolding
everything.

**Texture, sparingly.** A single very low-opacity radial gradient behind the top of the app plus
an optional 24px dot grid at ~2% opacity — enough to stop the canvas reading as a void, far short
of a marketing hero. Zero cost, CSP-safe.

## 4. Phased plan

Each phase ends green on `npm run validate` and leaves the app shippable.

### Phase 0 — Guardrails (half day)
- Add a Playwright screenshot suite covering the six main surfaces in both themes, so every later
  phase has a visual diff instead of vibes.
- Add a small token-lint script (`scripts/check-design-tokens.mjs`) that fails when a raw
  `#hex`/`hsl(...)` or an off-scale rem appears outside the token files, wired into
  `npm run lint`.
- Baseline the axe run so accessibility regressions surface immediately.

### Phase 1 — Token layer (1–2 days)
- Split `styles/base.css` into `styles/tokens/primitives.css`, `semantic.css`, `theme-dark.css`,
  `theme-light.css`.
- Add the missing scales: space (4/8/12/16/24/32/48/64), radius, type sizes and line-heights,
  motion durations and easings, elevation (border + surface + shadow triples), z-index.
- Re-cut both palettes per section 3; give every surface token a distinct value in dark.
- Delete the 78 duplicated light-theme component blocks as their values move into theme tokens.
  This is the single biggest simplification in the plan — expect the CSS to shrink, not grow.

### Phase 2 — Component primitives (2 days)
Rebuild as component-token driven CSS (`.button { --btn-bg: … }`, variants override tokens only):
Button, Card, Badge/StatusPill, Field/Input/Select, Tabs, Table frame, Popover, Toast, Progress.
No React API changes; components keep their current props and class names where possible.

### Phase 3 — App shell and rhythm (1 day)
- Sticky hairline topbar with translucent surface (`backdrop-filter: blur(20px) saturate(180%)`),
  brand mark, and the two controls grouped rather than floating.
- Consistent page rhythm: 32px page inset, 24px section gaps, 8px base everywhere; container
  width and card padding driven by tokens.
- Numbered workflow sections get a real header pattern (small uppercase eyebrow + title + help
  affordance) instead of an `h2` with a digit prefix.
- Quiet background texture per section 3.

### Phase 4 — The column review table (2–3 days) — the centerpiece
This is the screen the product lives or dies on, and the one in the screenshot.
- Retune the `<col>` widths: give Privacy Meaning the space it needs, tighten `#` and checkbox.
- Cell rhythm: 12px vertical row padding; inside the evidence cell, a strict 4/8 stack —
  pill, then one line of meaning + confidence, then metadata at 12px muted, then output on a
  mono line. Consistent baselines across all cells in a row.
- Sticky table header on `surface-2`; hairline dividers; no zebra striping.
- Risk gets a 3px left-edge row marker in addition to the badge, so risk is not color-only.
- Selected rows: tinted surface + accent left edge; custom checkbox styling replacing the stock
  blue control.
- One `StatusPill` treatment for Resolved/Uncertain/Review — 11px, uppercase, tracked, 12% tinted
  fill, no borders — instead of today's three visually different chips.
- Bulk actions become a proper toolbar: a primary action, secondary actions grouped, a live
  "N of M columns selected" count, and stickiness once the table scrolls.
- Selects sized and aligned to the row baseline at a single control height.
- Mobile/narrow layout re-verified: the existing `.mobile-cell-label` card fallback gets the same
  rhythm treatment.

### Phase 5 — Motion and feedback (1 day)
Hover/press/focus states from motion tokens (120ms micro, 180ms standard), a single focus-ring
token, restyled progress and toast, CSS-only view transition on tab switch, and a full
`prefers-reduced-motion` path. No animation library.

### Phase 6 — Typography rollout (half day)
Self-host the variable font under `frontend/public/fonts`, add `@font-face` with
`font-display: swap`, apply the type scale across headings, table text, labels and mono data.

### Phase 7 — Documentation and governance (half day)
`docs/design-system.md`: token reference, the do/don't list, the accent rule, density rules, and
screenshots. Keeps future changes inside the system rather than around it.

## 5. Status (2026-08-18)

Landed in this pass, with `npm run validate`, `npm run frontend:e2e` and `npm run frontend:a11y`
green:

- **Phase 1 complete.** Tokens moved to `frontend/src/styles/tokens/` (`scales.css`,
  `theme-dark.css`, `theme-light.css`); spacing, type, radius, motion, control-height and
  z-index scales added; the dark palette re-cut on a neutral ramp with four distinct surface
  steps and a single cool accent. All 78 duplicated light-theme component blocks are gone —
  component CSS no longer branches on the theme (net -457 lines across the stylesheets).
- **Phase 2 partial.** Button, status pill, risk badge, alert, switch, tabs, select and evidence
  chip are now component-token driven (`--btn-bg`, `--pill-tint`, `--risk-tint`, `--alert-tint`),
  so variants override values instead of repeating rules. The mode switcher is one segmented
  control in both themes instead of two different components.
- **Phase 4 partial.** The column table is now a ledger: top-aligned cells on a single rhythm,
  sticky header, retuned column widths, uppercase tracked headers, one pill treatment for
  status, a 3px risk edge on the row so risk is not colour-only, and the bulk actions and
  section help sharing one toolbar row. The evidence help trigger sits inline with the coverage
  line instead of taking its own row.
- **Accessibility fixes found along the way:** the blanket `opacity` on not-yet-ready sections
  dropped text under the contrast floor (it now reads as a recessed surface), and light-theme
  tertiary text at 46% lightness failed 4.5:1 on white (now 40%).

Still open: Phase 0 guardrails (screenshot baselines, token lint), the rest of Phase 2/3
(app-shell header pattern, section eyebrows), Phase 5 motion polish, Phase 6 (the variable font
is declared but no file is shipped yet, so the app still renders in the platform UI font), and
Phase 7 documentation.

## 6. Sequencing note

Phases 0–2 are the investment; phases 3–6 are where the app visibly changes. If the goal is to
see the new look sooner, Phase 4 can be pulled forward to run directly after Phase 1 — the table
is where most of the perceived quality lives.

## 7. Decisions taken (2026-08-18)

- **Accent:** cool brand, warm risk. The brand accent is a single cool hue used on primary
  actions, focus rings, active tabs and selected rows; amber and red stay reserved for risk,
  warnings and destructive states so the warm colors keep meaning something.
- **Sequencing:** tokens first. Phases 0–2 (guardrails, token layer, primitives) run before the
  column-table redesign in Phase 4.
