# @bencherdev/ui

The design system for the Bencher Console: tokens, the Bencher theme, a stylesheet, the Inter font, and SolidJS components. The console depends on it by path (`"@bencherdev/ui": "file:../../packages/ui"`), so a component and the page that needs it land in one change. It is private and never published.

## Use

Import styles once, in the layout:

```astro
import "@bencherdev/ui/styles.css";
```

Import components from per-component subpaths:

```tsx
import Button from "@bencherdev/ui/Button";
import Token from "@bencherdev/ui/Token";
```

Static Astro templates may use the `ui-*` classes directly when there is no interactivity. Every component forwards `class` and native attributes, and variants render as data attributes (`.ui-button[data-variant="primary"]`), which are public API for app CSS.

## Theme

Every color is a token in [`src/styles/theme.css`](src/styles/theme.css), and no other file may write one; a test fails the build when one does. Dark is the default. Light redefines the same tokens under `:root[data-theme="light"]`, so a component never names a theme. The page sets `data-theme` on `<html>` before it paints, from the reader's stored choice or else the system preference, and **ThemeToggle** flips it.

- **Text:** `--color-text-primary`, `-secondary`, `-muted`, `-eyebrow`, and `-faint`, each at least 4.5:1 on every background. `--color-text-accent` is a link.
- **Backgrounds:** `--color-background-body`, `-surface` (sheets and dialogs), `-card`, `-muted`, `-plot`, `-tag`, and `--color-overlay-hover` over any of them.
- **Accent:** `--color-accent` fills a control that carries `--color-on-accent` text. `--color-stroke-accent` marks a stroke (the current tab, a selected segment) and `--color-focus` rings focus; both hold 3:1 on every background.
- **Borders:** `--color-border` divides, `--color-border-emphasized` outlines a card, and `--color-border-control` outlines a control at 3:1.
- **Feedback:** `--color-success`, `--color-warning`, and `--color-error`, each with a `-muted` background and a `--color-text-*` for text. `--color-marker` and `--color-on-marker` fill an alert marker or a destructive control; `--color-worse` and `--color-better` color a change.
- **Plot:** `--color-data-categorical-1` through `-8` in slot order, each at least 3:1 on the plot; neighbors stay apart under protanopia, deuteranopia, and tritanopia, and a test holds them to it. `--color-data-dim` draws an unfocused line and `--color-data-grid` the grid.

## Fonts

Inter ships in [`src/fonts`](src/fonts) (latin, variable weight) under the SIL Open Font License, so an install without internet access renders the same. `--font-family-code` is the system monospace stack. Preload the font in the document head:

```astro
import inter from "@bencherdev/ui/fonts/inter-latin-wght-normal.woff2?url";
<link rel="preload" href={inter} as="font" type="font/woff2" crossorigin />
```

## Components

- **Button** `variant` (`primary` | `secondary` | `ghost` | `danger` | `destructive` | `muted`), `size` (`sm` | `md` | `lg`), `full`, `selected`. Defaults: secondary, md, `type="button"`. A button whose only child is an Icon is square, and every button is 44px on a narrow screen. One `primary` per view. `muted` is the quiet accent for the workhorse action; `selected` makes a toggle chip with `aria-pressed`. `danger` opens the confirmation for what cannot be undone, and `destructive` is the press that confirms it.
- **Dialog** `onDismiss`. A native modal dialog, open while mounted: the browser traps focus, makes the page inert, and returns focus to the opener when it closes. Escape calls `onDismiss`, and the owner unmounts it; without `onDismiss`, Escape does nothing and a close the browser makes on its own is undone, for a dialog that must be answered. Label it with `aria-labelledby`, use `role="alertdialog"` to confirm what cannot be undone, and compose it from `ui-dialog-head` (its first child is the title), `ui-dialog-body`, and `ui-dialog-foot`; wrap them in a `form.ui-dialog-form` to submit with Enter. On a narrow screen it is a sheet from the bottom edge.
- **ThemeToggle** `storageKey`. Switches `data-theme` between light and dark and stores the choice under `storageKey`.
- **Token** `tone` (hue), `variant` (`soft` | `strong`), `dot`. Enumerated metadata only: statuses, roles, parameters (`blue`). `strong` inverts for a high-contrast chip. Never decoration; use text for prose.
- **Banner** `status` (`neutral` | `info` | `success` | `warning` | `error`). Inline feedback in the surface it describes. No toasts.
- **Card** `as` (`article` | `div` | `section` | `form`), `variant` (`soft`). One self-contained unit. Never wraps a list row, never nests. The default is the raised surface; `soft` sits on the page's card ground, for cards laid out side by side on a page.
- **List** / **ListItem** `href` on the item makes the whole row a link. Dense, scannable, edge-to-edge rows with dividers; 48px minimum.
- **Field** `label`, `for`. A label over one control.
- **TextInput** `size`; controlled via `value` and `onInput`. Its font is 14px on a wide screen and 16px on a narrow one, where phones zoom into anything smaller. Every input and select is 44px tall on a narrow screen, whatever its size.
- **TextArea** controlled via `value` and `onInput`.
- **Selector** `size`; controlled via `value` and `onChange`; options are children. `value` is applied through a render effect after the options mount, never on the initial spread: a native select resolves its value against the options present when the property is set, so a value handed in before its `<option>` children would be dropped to the first option. Keep the options in the children, not fetched in after; the effect reasserts the value when either it or the option set changes.
- **Icon** `name` (one of the enumerated glyphs: `alerts`, `check`, `chevron-down`, `close`, `key`, `read-only`, `theme`, `user`, `warning`), `size` (`sm` | `md` | `lg` | `xl`, omit to inherit the surrounding font size). A stroke-drawn glyph on `currentColor`, `aria-hidden` by default. Enumerated meaning only; label the control that wraps it, never the icon. Extend the set in `Icon.tsx` when a screen needs a new glyph.
- **Avatar** `name` (initials) or `label` (literal, e.g. `+3`), `size` (`sm` | `md`). A single-word name reads as its first two letters, a multi-word name as one letter per word (up to two). **AvatarGroup** overlaps its children.
- **EmptyState** `title`, children as one quiet sentence. For regions with nothing to show.
- **Stack** `direction`, `gap` (spacing step), `align`, `justify`, `wrap`. The only spacing app code needs: no margin utilities exist, on purpose.
- **Table** `fold`. Columns under a header row, with dividers, in a bordered card; scrolls inside its own track on a narrow screen so the page never does. Compose with the semantic table elements (`thead`, `tbody`, `tr`, `th`, `td`); numeric cells are tabular by default and `data-align="end"` right-aligns a column. With `fold` (or `data-fold`), each row folds into two lines on a narrow screen: a cell's `data-fold` places it (`l1`, `l2` on the left, `n1`, `n2` (the numbers) on the right, `act` in a column of its own, `hide` nowhere), and a `ui-fold-label` span inside a cell shows only when folded, to say what the missing header said. For dense records that are one line each, prefer List instead.
- **Segmented** `name`, `options` (`value`, `label`), `value`, `onChange`, `full`. One choice among a few, as joined positions. Each position is a native radio, so the arrow keys move the choice and Tab enters at the checked one (the first when none is checked); label the group with `aria-label` or `aria-labelledby`. 44px positions on a narrow screen.
- **Chip** `label` (a quiet key before the value), `on` (it narrows something), `caret` (it opens a menu or a sheet). A rounded button showing a setting; 44px on a narrow screen.
- **Menu** `label`, `anchor`, `onClose`, `header`, `align`; **MenuItemRadio** `checked`, `onSelect`. A menu under the control that opened it, inside an element with `position: relative`: it takes the focus on opening (a search box in `header`, else the checked item) and scrolls itself into view, the arrow keys, Home, and End move through the items, and Escape, Tab, or a press outside close it, returning the focus to `anchor`. In a Sheet it opens in the flow below its control, and the sheet grows to hold it.
- **Sheet** `open`, `onClose`, `title`, `done`, `side`. A modal native dialog rising from the bottom of the screen, named by its title, closed by its Done button, Escape, or a press on the dimmed page. With `side` it docks to the right on wide screens.
- **Popover** `summary`, `summaryLabel`, `summaryVariant`, `summarySize`, `shape` (`menu` | `pill`). A details/summary disclosure; close via `ref` by setting `open = false`.
- **Heading** `level` (semantic), `size` (visual): the two are set separately.
- **Text** `as`, `size`, `tone`. Secondary-tone small text is the hint style.
- **Timestamp** `title` (full time), `href` (permalink), `datetime`. Children carry the formatted relative time.
- **Skeleton** `size` (`text` | `row` | `card`). A placeholder for data never fetched: it holds its place at once and shows only after 400ms, so a quick answer never flashes it. `text` is one line of the surrounding type, as wide as the caller makes it.

## Rules

- A component is added when a screen needs it, with the smallest prop surface that serves. Do not invent props; extend the component here instead.
- No hardcoded colors, spacing, or sizes anywhere: tokens only, in the package and in app code alike.
- The plot reads its series colors from the tokens at draw time, in slot order; its chrome uses core tokens.
- This package depends on `solid-js` alone and references nothing outside its directory. The console's tests run its component tests in real Chromium (`services/console`, `*.chromium.test.tsx`).
