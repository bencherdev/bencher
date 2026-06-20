# Bencher Design Guide

This guide captures the visual design language introduced in the redesigned
**landing** ([`pages/index.astro`](./src/pages/index.astro), PR #789) and
**pricing** ([`pages/pricing.astro`](./src/pages/pricing.astro), PR #791) pages.

The goal is a single reference so the same look and feel can be rolled out to the
rest of Bencher: the documentation, the Console app, and the Perf search and
plot pages. When you build or restyle a surface, match the tokens, layout rhythm,
and component patterns described here rather than inventing new ones.

> Writing rule for this repo: do **not** use em dashes anywhere, including this
> doc. Use a colon, comma, parentheses, or two sentences.

## Contents

1. [Design principles](#design-principles)
2. [Foundations](#foundations) (theming, fonts, breakpoints)
3. [Design tokens](#design-tokens) (color, type, space, radius, shadow, motion)
4. [Layout system](#layout-system)
5. [Component patterns](#component-patterns)
6. [Applying it across Bencher](#applying-it-across-bencher) (Docs, Console, Perf)
7. [Known inconsistencies and consolidation](#known-inconsistencies-and-consolidation)
8. [Checklist](#checklist)

---

## Design principles

These are the ideas the landing and pricing redesign encode. Carry them forward.

1. **Flat, border-first surfaces.** Cards are a faint tinted background plus a 1px
   border, not heavy drop shadows. Shadows are reserved for elements that should
   read as "floating above the page" (the terminal mock, the regression chart).
2. **A consistent section rhythm.** Almost every section is eyebrow, then title,
   then a muted subtitle, then content. The repetition is the point: it makes the
   page feel like one system.
3. **Orange is an accent, used sparingly.** Brand orange marks the few things that
   matter: primary calls to action, the "popular" / "recommended" choice, feature
   checkmarks, inline prose links, and hover states. It is almost never a large
   fill.
4. **Monospace signals "technical / measured."** Numbers (stats, step indices),
   terminal output, and inline code use the monospace stack. This ties the visual
   language to what Bencher does: measure things precisely.
5. **Semantic red and green for regressions and improvements.** Red means alert /
   slower, green means healthy / faster. This maps directly onto the product
   domain (alerts vs. passing thresholds) and should stay consistent in data viz.
6. **Muted neutrals for secondary text, full contrast for headings.** Body copy and
   labels are intentionally low-contrast (`--landing-muted`); strong contrast is
   reserved for titles and emphasized words.
7. **Token-driven theming.** Colors come from CSS custom properties that have light
   and dark values. New components read the tokens so dark mode "just works."
8. **Generous, viewport-scaled vertical space.** Sections breathe more as the
   viewport grows. Whitespace is part of the brand.

---

## Foundations

### Theming and dark mode

- Theme is controlled by the `data-theme` attribute on `<html>` (set in
  [`layouts/BaseLayout.astro`](./src/layouts/BaseLayout.astro)), using Bulma's
  scheme system. Dark styles live under `[data-theme="dark"]`.
- Bulma exposes scheme variables that are always safe to use:
  `--bulma-scheme-main`, `--bulma-scheme-main-bis`, `--bulma-background`,
  `--bulma-border`, `--bulma-text`, `--bulma-text-strong`, `--bulma-primary`,
  `--bulma-primary-invert`.
- The redesign adds its own token layer on top (see [Design tokens](#design-tokens)),
  each with a `[data-theme="dark"]` override, declared in
  [`styles/_landing.scss`](./src/styles/_landing.scss).
- In scoped Astro `<style>` blocks, target dark mode with
  `:global([data-theme="dark"]) .my-class { ... }`.

### Fonts

- **Body / UI:** Inter (Bulma's default body font).
- **Monospace:** always use this exact stack:
  ```
  ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, monospace
  ```
  Used for stat numbers, step numbers, terminal mocks, and code. (It is currently
  repeated inline in several components; see
  [consolidation](#known-inconsistencies-and-consolidation).)

### Breakpoints

The redesign uses Bulma's breakpoints plus two custom ones. Keep to this set:

| Name             | Min width | Used for                                  |
| ---------------- | --------- | ----------------------------------------- |
| mobile (default) | 0         | single column                             |
| custom-small     | 600px     | 2-up grids (footer, enterprise bullets)   |
| tablet           | 769px     | 2 to 3 column layouts, larger type        |
| custom-mid       | 900px     | pricing 3-up, FAQ 2-up, stewardship 2-up  |
| desktop          | 1024px    | wider section padding, 5-up harness grid  |
| widescreen       | 1216px    | widest section padding                    |

Note Bulma treats `max-width: 768px` as mobile and `min-width: 769px` as tablet.

---

## Design tokens

Canonical token definitions live in
[`styles/_landing.scss`](./src/styles/_landing.scss). Despite the `landing-`
prefix, these are the app-wide tokens; reuse them everywhere (and see the
[consolidation](#known-inconsistencies-and-consolidation) note about the name).

### Color

**Brand and semantic** (`:root`, same in both themes):

| Token                  | Value     | Meaning                              |
| ---------------------- | --------- | ------------------------------------ |
| `--brand-orange`       | `#ed6704` | Primary accent / CTA fill            |
| `--brand-orange-hover` | `#d25a00` | Hover state for orange fills         |
| `--brand-alert`        | `#ef4444` | Regression / alert / "bad" (red)     |
| `--brand-success`      | `#22c55e` | Improvement / healthy / "good" (green) |

**Neutrals and surfaces** (light value, then dark value):

| Token                          | Light                | Dark                    | Use                          |
| ------------------------------ | -------------------- | ----------------------- | ---------------------------- |
| `--landing-muted`              | `#666`               | `#a0a0a0`               | Body and secondary text      |
| `--landing-eyebrow`            | `#888`               | `#aaa`                  | Eyebrow / micro-label text   |
| `--landing-card-border`        | `rgba(0,0,0,.08)`    | `rgba(255,255,255,.1)`  | Subtle dividers / card edges |
| `--landing-card-border-strong` | `rgba(0,0,0,.25)`    | `rgba(255,255,255,.32)` | Defined card edges           |
| `--landing-card-bg`            | `rgba(0,0,0,.02)`    | `rgba(255,255,255,.025)`| Faint card fill              |

**Orange tints** (used directly, not yet tokens): the "popular / recommended /
highlight" surfaces use `rgba(237, 103, 4, 0.04)` in light mode and
`rgba(237, 103, 4, 0.08)` in dark mode, with `--brand-orange` borders.

**Data-viz palette** (from the hero
[`RegressionChart.astro`](./src/components/landing/RegressionChart.astro), good
defaults for Perf): dark canvas `#1e1e1e`, grid lines `#333`, axis labels `#888`,
data line teal `#6bbfcf`, threshold boundary orange `#ff7a1a`, regression point
and alert badge `#e26000`, healthy point teal `#6bbfcf`.

**Terminal palette** (from
[`InDevelopment.astro`](./src/components/landing/InDevelopment.astro)): background
`#1a1d23` (light theme) / `#0d1117` (dark), prompt and links `#6bbfcf`, command
text `#e6edf3`, flags and bench names `#c9a76a`, numbers green `#7bd88f`, labels
and dim text `#9ba4b0`, alerts `--brand-alert`.

### Typography scale

| Role                | Spec                                                              | Where                          |
| ------------------- | ---------------------------------------------------------------- | ------------------------------ |
| Hero title          | Bulma `.title.is-1`                                               | landing hero `h1`              |
| Section title       | Bulma `.title.is-1` (often `max-width: 900px`)                    | section `h2`                   |
| Big block title     | `2rem`, weight 700, line-height 1.2 (to `2.5rem` at >= 769px)     | FAQ, Stewardship `h2`          |
| Card title          | Bulma `.title.is-5`                                               | card / step `h3`               |
| Subtitle / lede     | Bulma `.subtitle.is-4` or `.is-5`, color `--landing-muted`        | under section titles           |
| **Eyebrow**         | `0.75`-`0.85rem`, weight 600-700, `letter-spacing: 0.14`-`0.15em`, uppercase, `line-height: 1`, color `--landing-eyebrow` | section labels |
| Big stat number     | `2.25rem`, weight 600, monospace                                 | stat cards                     |
| Price number        | `2.5rem`, weight 700, line-height 1                               | pricing cards                  |
| Body                | line-height `1.5`-`1.6`, color `--landing-muted`                  | paragraphs                     |
| Inline code chip    | `0.85`-`0.9em`, padding `~0.1em 0.35em`, radius `4px`, faint bg   | prose `code`                   |

Helper class: `.landing-eyebrow` (in `_landing.scss`) implements the eyebrow.
Prefer it over re-declaring the styles.

### Spacing and section padding

Section padding scales with the viewport. The canonical helper is
`.landing-section`:

| Breakpoint    | Padding (`.landing-section`) |
| ------------- | ---------------------------- |
| mobile        | `4rem 1.5rem`                |
| >= 769px      | `5rem 2rem`                  |
| >= 1024px     | `5rem 4rem`                  |
| >= 1216px     | `5rem 6rem`                  |

The site footer uses the same horizontal scale (`1.5` / `2` / `4` / `6rem`). The
final call-to-action section is intentionally more generous (`6` / `7` / `8rem`
top, `8` / `9` / `10rem` bottom). The hero uses `max(6rem, 15vh)` top padding and
`min-height: 100vh`.

**Content widths:** `1200px` max for grids, pricing, and the footer; `900px` for
hero / section titles; `720px` for prose ledes; `640px` for the CTA subtitle.
Center with `margin: 0 auto`.

### Border radius

| Radius  | Applies to                                                  |
| ------- | ----------------------------------------------------------- |
| `4px`   | Code chips, small badges                                    |
| `6px`   | Chart canvas                                                |
| `8px`   | Buttons / CTAs, terminal card, spec highlight               |
| `12px`  | Standard cards (problem, stat, deploy, testimonial, callout)|
| `14px`  | Large cards (pricing, enterprise, stewardship)              |
| `999px` | Pills / ribbons                                             |

### Borders

- Default card edge: `1px solid var(--landing-card-border)`.
- Emphasized card edge: `1px solid var(--landing-card-border-strong)`.
- Selected / popular: `2px solid var(--brand-orange)` plus an orange tint fill.
- Dashed `1px var(--landing-card-border)` for spec rows; dividers between
  numbered steps use a `1px` `border-left` at >= 769px.

### Shadow

Surfaces are mostly flat (border only). Shadows are used only to lift mocks:

- Regression chart: `0 4px 24px rgba(0,0,0,0.4)`; in dark mode swap to an orange
  glow `0 0 48px rgba(255,122,26,0.18)`.
- Terminal card: `0 8px 32px rgba(0,0,0,0.35)`; dark mode adds a hairline ring
  plus `0 10px 40px rgba(0,0,0,0.7)`.
- Recommended deploy card (dark only): orange glow
  `0 0 0 1px rgba(237,103,4,0.15), 0 0 40px rgba(237,103,4,0.1)`.

### Motion

- Color / background / border transitions: `0.15s ease`.
- Opacity / transform (logo hover, FAQ icon, alert badge): `0.2s ease`.
- Decorative animation (the hero chart) is pure CSS keyframes baked as
  percentages so the loop is stable. Respect `prefers-reduced-motion` for any new
  looping animation.

---

## Layout system

**Standard section skeleton.** Most marketing sections follow this shape:

```astro
<section class="landing-section">
  <p class="landing-eyebrow">SECTION LABEL</p>
  <h2 class="title is-1">Section headline</h2>
  <p class="subtitle is-5 landing-muted">Supporting sentence, max-width ~720px.</p>

  <!-- content: a Bulma columns grid or a CSS grid of cards -->
</section>
```

- Wrap content in Bulma columns (`.columns`, `.column.is-*`) or a CSS `grid`.
- For equal-height card rows, give the card `height: 100%` (the parent column
  stretches by default).
- Center constrained content with `.columns.is-centered > .column.is-10`.
- Card grids commonly use `grid-template-columns: repeat(3, 1fr)` with
  `gap: 1.5rem` at the relevant breakpoint, single column below it.

---

## Component patterns

Each pattern below points at a reference implementation. Reuse the markup and
classes; do not reinvent.

### Eyebrow

Uppercase micro-label above a title. Class `.landing-eyebrow`. The hero variant
`.hero-eyebrow` is the same recipe at a slightly different margin.

### Standard card

Faint fill, 1px border, `1.75rem` padding, `12px` radius. Reference:
`.problem-card` in [`Problem.astro`](./src/components/landing/Problem.astro).

### Stat card

Eyebrow label, a large monospace number colored by tone, then a muted body line.
Use `--brand-alert` for the "bad" value and `--brand-success` for the "good"
value. Reference: [`BareMetal.astro`](./src/components/landing/BareMetal.astro).

### Numbered process steps

Monospace `01` / `02` / `03` indices, a `.title.is-5` step title, and a muted
body. Steps are separated by a `1px` `border-left` at >= 769px. Reference:
[`HowItWorks.astro`](./src/components/landing/HowItWorks.astro).

### Comparison / option cards (with "recommended")

Two cards side by side; the recommended one gets an orange-tinted border, a glow
in dark mode, a small badge, and an orange CTA. The other gets a muted badge and
an outline CTA. Reference:
[`Hosting.astro`](./src/components/landing/Hosting.astro).

### Badge / pill

Small uppercase label. Outline style for muted (`1px` border, `--landing-muted`),
orange-outline style for accented. Radius `4px`. Reference: `.deploy-badge` in
`Hosting.astro`.

### Pricing card

`14px` radius large card. The popular variant adds `2px` orange border, an orange
tint fill, and an absolutely-positioned `.pricing-ribbon` pill. Inside: title,
italic tagline, big price number with a muted unit, an orange or outline CTA,
section labels, a feature list with orange `check` / muted `dash` marks, and a
spec list with dashed dividers. Reference:
[`styles/_pricing.scss`](./src/styles/_pricing.scss) and
[`InnerPricingTable.tsx`](./src/components/pricing/InnerPricingTable.tsx).

### Buttons and CTAs

Two systems coexist; know when to use each.

- **Bulma buttons** (`.button.is-primary.is-medium.is-responsive`) for hero and
  in-flow actions. `is-primary` renders Bulma's primary orange.
- **Custom CTAs** (`.cta-primary`, `.pricing-cta-primary`, `.deploy-cta`): orange
  fill `#fff` text for primary; transparent with a border for secondary. Radius
  `8px`, hover goes to `--brand-orange-hover`. Reference:
  [`CallToAction.astro`](./src/components/landing/CallToAction.astro).

> Caveat: these two systems currently use two slightly different oranges. See
> [consolidation](#known-inconsistencies-and-consolidation).

### FAQ accordion

Native `<details>` / `<summary>` with a custom plus/minus icon drawn from
pseudo-elements, laid out in a 2-up grid, deep-linkable via URL hash. Reference:
[`FaqItem.astro`](./src/components/pricing/FaqItem.astro) plus the inline script
in [`pricing.astro`](./src/pages/pricing.astro).

### Testimonial card

Blockquote, then an avatar (32px circle), name, and `@handle`. Reference:
[`Customers.astro`](./src/components/landing/Customers.astro).

### Trusted-by logo strip

Monochrome logos (color inherits via `currentColor`) at `opacity: 0.5`, going to
`0.8` on hover, `2.5rem` gap. SVGs are inlined at build time. Reference:
[`TrustedBy.astro`](./src/components/landing/TrustedBy.astro).

### Definition / metadata grid

Languages-to-harnesses style grid: an uppercase label per group, then a plain
list. Responsive 2 to 5 columns. Reference:
[`Harnesses.astro`](./src/components/landing/Harnesses.astro).

### Callout

Icon plus a body paragraph in a bordered, faint-fill box. Reference:
`.pricing-callout` in `_pricing.scss`.

### Terminal mock

Dark card with a chrome bar (three traffic-light dots and a title), monospace
body with color-coded tokens (prompt, command, flags, numbers, links, alerts). It
is scaled to fit its column by a small script (`mockScale`). Reference:
[`InDevelopment.astro`](./src/components/landing/InDevelopment.astro).

### Animated data chart

Pure SVG plus CSS keyframes: dark canvas, grid, teal data line that draws in, a
dashed orange threshold, points, and an alert badge at the regression. Reference:
[`RegressionChart.astro`](./src/components/landing/RegressionChart.astro).

### Footer

Full-width brand block (wordmark, tagline, social), a rule, then a responsive
1 to 4 column link grid using `--landing-muted` text and orange hover. Reference:
[`Footer.astro`](./src/components/site/Footer.astro).

---

## Applying it across Bencher

How to bring this language to the surfaces that have not been redesigned yet.

### Docs (`src/content`, `src/layouts/docs`)

- Lead pages and section intros with the eyebrow + title + muted-lede rhythm.
- Replace ad hoc bordered boxes / Bulma `.box` notes with the standard card recipe
  (`--landing-card-bg` + `--landing-card-border`, `12px` radius).
- Inline code chips: faint bg, `4px` radius, `0.85`-`0.9em` (already used in the
  landing prose). Keep code blocks in the dark terminal palette so they match the
  mocks.
- Prose links use `--brand-orange` with underline, hover `--brand-orange-hover`
  (see `.pricing-faq-answer a`).
- Use the FAQ accordion pattern for any expandable Q&A.

### Console app (`src/components/console`)

- Wrap app content in `.landing-section`-equivalent padding and a `1200px` max
  width so it lines up with marketing pages.
- Standardize cards and panels on the card recipe and radius scale instead of
  Bulma `.box` / `.card` defaults.
- Buttons: prefer the custom CTA classes (orange primary, outline secondary) for
  primary actions so the app matches the marketing CTAs; keep Bulma buttons for
  dense / utility actions, but unify the orange (see consolidation).
- Badges and statuses: reuse the badge / pill recipe. Map states to the semantic
  palette: alerts in `--brand-alert`, healthy / passing in `--brand-success`.
- Empty states and tips: use the callout pattern.

### Perf search and plot pages (`src/components/console/perf`)

This is where the data-viz palette matters most.

- Adopt the chart palette as defaults: teal `#6bbfcf` for series lines, dashed
  orange `#ff7a1a` for thresholds / boundaries, `#e26000` plus an alert badge for
  points that cross a boundary, muted `#888` axis labels, `#333` grid lines on a
  dark canvas.
- Keep the red / green semantic meaning consistent: a regression reads red, an
  improvement reads green, matching the stat-card tones.
- Search and result rows: the metadata-grid and card patterns fit project /
  benchmark / testbed listings (uppercase label, muted secondary text, orange
  hover on links).
- Filter and dimension controls: outline buttons and badges from the button /
  badge recipes, orange for the active selection.
- The animated regression chart is a marketing illustration, not a live chart, but
  its color choices are the reference for the real plots.

---

## Known inconsistencies and consolidation

The redesign is internally consistent but left a few rough edges. Fix these as you
extend the system so the rest of Bencher inherits one source of truth.

1. **Three oranges.** Three brand oranges are in play:
   - `#ed6704` as `--brand-orange` (custom CTAs, ribbons, checks) and as
     `$old-orange` in [`styles.scss`](./src/styles/styles.scss).
   - `#e26000` as Bulma `$primary` / `$link` (so `.button.is-primary` and default
     links render this) and as the chart regression point.
   - `#ff7a1a` as the chart threshold line.

   The two primary CTAs on the landing page are different oranges: the hero
   "Benchmark for free" button (Bulma `is-primary`, `#e26000`) and the closing
   "Catch the next regression" button (`.cta-primary`, `#ed6704`). Pick one brand
   orange, set Bulma `$primary` and `--brand-orange` to it, and derive the rest.

2. **Hardcoded eyebrow color.** `#888` is hardcoded for `.hero-eyebrow` /
   `.trusted-by-label` in [`index.astro`](./src/pages/index.astro),
   [`pricing.astro`](./src/pages/pricing.astro), and
   [`TrustedBy.astro`](./src/components/landing/TrustedBy.astro) instead of using
   `--landing-eyebrow`. In dark mode the token is `#aaa`, so these hardcoded spots
   are slightly off. Route them through the token.

3. **Duplicated `.hero-eyebrow` block.** The same style block is copy-pasted in
   `index.astro` and `pricing.astro`. Promote it to a shared class.

4. **Token naming.** The app-wide tokens live in `_landing.scss` and are prefixed
   `--landing-*`, but pricing, the footer, and (going forward) the Console / Perf /
   Docs all depend on them. Consider moving them to a neutral `_tokens.scss`
   (`@use`d first in `styles.scss`) with names that do not imply "landing only,"
   while keeping `--landing-*` as aliases during migration.

5. **Repeated monospace stack.** The monospace font stack is re-declared inline in
   several components. Lift it into a single Sass variable or CSS custom property.

6. **Orange tints are magic numbers.** `rgba(237, 103, 4, 0.04 / 0.08)` appears in
   several places. Promote the popular / highlight tint to a token pair
   (light / dark) so it tracks the chosen brand orange.

---

## Checklist

Before shipping a new or restyled surface, confirm:

- [ ] Colors come from tokens (`--brand-*`, `--landing-*`, `--bulma-*`), not new
      hex literals.
- [ ] It has a `[data-theme="dark"]` story (tokens used, or explicit dark
      overrides via `:global([data-theme="dark"])`).
- [ ] Sections follow eyebrow + title + muted-subtitle, with `.landing-section`
      padding (or the documented scale).
- [ ] Cards use the radius scale (`12px` standard, `14px` large) and the
      border-first (not shadow-heavy) style.
- [ ] Orange is reserved for primary actions, the recommended choice, marks,
      links, and hover. It is not a large background fill.
- [ ] Numbers and code use the monospace stack; regressions read red, improvements
      read green.
- [ ] Motion uses the standard durations and honors `prefers-reduced-motion`.
- [ ] No em dashes in any copy.
