---
version: "alpha"
name: "Token Use Console"
description: "A dense dark design system for a token usage analytics dashboard, with a shared root and two tracks: terminal (TUI) and desktop (Tauri + Svelte)."
colors:
  primary: "#FF8F40"
  secondary: "#62A6FF"
  tertiary: "#4CF2A0"
  surface: "#202438"
  neutral: "#25293D"
  on-surface: "#CBD4F2"
  muted: "#A1A7C3"
  warning: "#FFD60A"
  error: "#FF5F6D"
  cyan: "#4DF3E8"
  magenta: "#F05AF2"
  bar-empty: "#292D42"
typography:
  display:
    fontFamily: "JetBrains Mono, SFMono-Regular, Menlo, Consolas, monospace"
    fontSize: "18px"
    fontWeight: 700
    lineHeight: 1.2
  panel-title:
    fontFamily: "JetBrains Mono, SFMono-Regular, Menlo, Consolas, monospace"
    fontSize: "16px"
    fontWeight: 700
    lineHeight: 1.25
  body:
    fontFamily: "JetBrains Mono, SFMono-Regular, Menlo, Consolas, monospace"
    fontSize: "15px"
    fontWeight: 500
    lineHeight: 1.35
  label:
    fontFamily: "JetBrains Mono, SFMono-Regular, Menlo, Consolas, monospace"
    fontSize: "14px"
    fontWeight: 600
    lineHeight: 1.25
rounded:
  none: "0px"
  sm: "2px"
  md: "4px"
spacing:
  xs: "1px"
  sm: "2px"
  md: "4px"
  lg: "8px"
  xl: "12px"
desktop:
  typography:
    ui:
      fontFamily: "Inter, system-ui, -apple-system, Segoe UI, sans-serif"
      fontSize: "13px"
      fontWeight: 500
      lineHeight: 1.4
    heading:
      fontFamily: "Inter, system-ui, -apple-system, Segoe UI, sans-serif"
      fontSize: "13px"
      fontWeight: 600
      lineHeight: 1.3
      letterSpacing: "0.01em"
    label:
      fontFamily: "Inter, system-ui, -apple-system, Segoe UI, sans-serif"
      fontSize: "11px"
      fontWeight: 600
      lineHeight: 1.2
      letterSpacing: "0.06em"
      textTransform: "uppercase"
    body:
      fontFamily: "Inter, system-ui, -apple-system, Segoe UI, sans-serif"
      fontSize: "13px"
      fontWeight: 400
      lineHeight: 1.5
    numeric:
      fontFamily: "JetBrains Mono, SFMono-Regular, Menlo, Consolas, monospace"
      fontFeatureSettings: "'tnum' 1, 'zero' 1"
  rounded:
    none: "0px"
    sm: "3px"
    md: "8px"
  spacing:
    xs: "2px"
    sm: "4px"
    md: "8px"
    lg: "12px"
    xl: "16px"
    "2xl": "24px"
    "3xl": "32px"
  elevation:
    popover: "0 1px 0 rgba(255,255,255,0.04) inset, 0 8px 24px rgba(0,0,0,0.35)"
  motion:
    durations:
      fast: "120ms"
      base: "180ms"
      slow: "280ms"
    easings:
      standard: "cubic-bezier(.2,.8,.2,1)"
      accel: "cubic-bezier(.4,0,1,1)"
      decel: "cubic-bezier(0,0,.2,1)"
components:
  app-surface:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.on-surface}"
    typography: "{typography.body}"
    rounded: "{rounded.none}"
    padding: "{spacing.sm}"
  desktop-topbar:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.primary}"
    typography: "{desktop.typography.heading}"
    rounded: "{desktop.rounded.md}"
    padding: "{desktop.spacing.lg}"
  brand-title:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.primary}"
    typography: "{typography.display}"
    rounded: "{rounded.none}"
    padding: "{spacing.xs}"
  dashboard-panel:
    backgroundColor: "{colors.neutral}"
    textColor: "{colors.on-surface}"
    typography: "{typography.body}"
    rounded: "{rounded.sm}"
    padding: "{spacing.md}"
  desktop-panel:
    backgroundColor: "{colors.neutral}"
    textColor: "{colors.on-surface}"
    typography: "{desktop.typography.body}"
    rounded: "{desktop.rounded.md}"
    padding: "{desktop.spacing.lg}"
  summary-panel:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.primary}"
    typography: "{typography.display}"
    rounded: "{rounded.sm}"
    padding: "{spacing.md}"
  desktop-kpi-tile:
    backgroundColor: "{colors.neutral}"
    textColor: "{colors.on-surface}"
    typography: "{desktop.typography.numeric}"
    rounded: "{desktop.rounded.md}"
    padding: "{desktop.spacing.lg}"
  desktop-status-pill:
    backgroundColor: "{colors.neutral}"
    textColor: "{colors.muted}"
    typography: "{desktop.typography.label}"
    rounded: "999px"
    padding: "{desktop.spacing.sm} {desktop.spacing.lg}"
  info-panel-title:
    backgroundColor: "{colors.neutral}"
    textColor: "{colors.secondary}"
    typography: "{typography.panel-title}"
    rounded: "{rounded.sm}"
    padding: "{spacing.sm}"
  success-panel-title:
    backgroundColor: "{colors.neutral}"
    textColor: "{colors.tertiary}"
    typography: "{typography.panel-title}"
    rounded: "{rounded.sm}"
    padding: "{spacing.sm}"
  warning-panel-title:
    backgroundColor: "{colors.neutral}"
    textColor: "{colors.warning}"
    typography: "{typography.panel-title}"
    rounded: "{rounded.sm}"
    padding: "{spacing.sm}"
  danger-panel-title:
    backgroundColor: "{colors.neutral}"
    textColor: "{colors.error}"
    typography: "{typography.panel-title}"
    rounded: "{rounded.sm}"
    padding: "{spacing.sm}"
  cyan-panel-title:
    backgroundColor: "{colors.neutral}"
    textColor: "{colors.cyan}"
    typography: "{typography.panel-title}"
    rounded: "{rounded.sm}"
    padding: "{spacing.sm}"
  magenta-panel-title:
    backgroundColor: "{colors.neutral}"
    textColor: "{colors.magenta}"
    typography: "{typography.panel-title}"
    rounded: "{rounded.sm}"
    padding: "{spacing.sm}"
  muted-label:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.muted}"
    typography: "{typography.label}"
    rounded: "{rounded.none}"
    padding: "{spacing.xs}"
  heatbar-empty:
    backgroundColor: "{colors.bar-empty}"
    textColor: "{colors.on-surface}"
    typography: "{typography.label}"
    rounded: "{rounded.none}"
    padding: "{spacing.xs}"
---

## Overview

Token Use should feel like a compact operator console for people who care about token spend, model behavior, and workflow efficiency. The interface is dark, dense, and calm, with bright terminal accents used as structural signposts rather than decoration. The desired response is quick orientation: the user should be able to scan costs, calls, hot spots, and command hints without leaving the keyboard.

The brand mark is the orange bars symbol from `desktop/tokenusebars.svg`. Desktop chrome should pair the bars-only mark with the product name `Token Use`; reserve `tokenuse` for command names, package identifiers, URLs, and other literal technical strings.

This system has a shared root and two tracks:

- **TUI track** — the terminal renderer. Strict monospace, square corners, flat depth, no animation. Everything below this overview that is not explicitly under "Desktop Track" applies here.
- **Desktop track** — the Tauri + Svelte app. Inherits the same color palette and brand identity, but allows a sans-serif UI font for chrome and labels, an expanded spacing scale, 8px corners on outer surfaces, and a defined motion vocabulary. See the **Desktop Track** section below.

The two tracks must stay visually related: same accent colors, same data-dense tables, same brand mark. They diverge only where the desktop surface needs to feel like a calm desktop app instead of a literal terminal copy.

## Colors

- **Primary (#FF8F40):** active period/provider states, summary borders, brand title text, and command keys.
- **Secondary (#62A6FF):** informational panels such as Daily Activity.
- **Tertiary (#4CF2A0):** successful or efficient usage signals.
- **Warning (#FFD60A):** money, token savings, and metric values that need attention.
- **Error (#FF5F6D):** risk, high severity optimization items, and Top Sessions.
- **Cyan (#4DF3E8) and Magenta (#F05AF2):** secondary category accents for tools, models, and MCP-like surfaces.
- **Surface and Neutral:** layered dark blue-gray backgrounds; hierarchy comes from borders and color, not shadows.
- **Brand bars:** the icon may use a warm orange gradient within the primary family, from pale amber through orange to coral, but this gradient is limited to the app icon and bars mark.

## Typography

The TUI uses a monospace stack everywhere. Labels and values should align cleanly in columns, with bold reserved for the brand title, panel titles, active navigation, and important numeric values. Avoid display-scale type inside the dashboard; the interface should stay compact enough for repeated terminal use.

The Desktop track splits typography into a sans-serif UI track (Inter) for chrome and prose and a monospace numeric track (JetBrains Mono) for values; see **Desktop Track** below.

## Layout

The layout is a high-density grid with thin gaps, fixed-height summary/nav/footer bands, and proportional two-column content rows. Panels should preserve predictable column alignment at common terminal widths, degrading by truncating long labels before hiding key metrics.

Desktop topbars should stay quiet: bars mark, `Token Use`, centered navigation, and compact icon buttons. Do not spend topbar space on version labels, live/source badges, or explanatory copy; put status in the status line and data provenance in Config.

## Elevation & Depth

In the TUI, depth is flat. Borders, foreground color, and background tone provide hierarchy; shadows do not belong in the terminal implementation. Gradients are reserved for the brand bars asset only; heat bars should use stepped color ramps to imply magnitude.

In the Desktop track, in-flow panels remain flat — no drop-shadow halos. A single subtle elevation token (`desktop.elevation.popover`) is reserved for transient surfaces: dropdowns, popovers, modals, the tray popover. Tiles, tables, status bars, and inline chips stay flat.

## Shapes

TUI panels use square or nearly square corners. Any rounded interpretation should stay at 2-4px in the TUI. The bars inside the brand mark may be softly rounded, but the app icon background stays solid and square-cornered.

In the Desktop track, outer panels and KPI tiles may use 8px corners (`desktop.rounded.md`). Inner chips, badges, pills, and inline controls stay at 3px (`desktop.rounded.sm`). Status pills are fully rounded (`999px`).

## Iconography

Use `desktop/tokenusebars.svg` as the source asset for generated app icons. The full icon keeps its dark square background; in app chrome, use only the four orange bars next to `Token Use` so the header stays compact and recognizable.

## Components

Dashboard panels use one-pixel borders, a colored title, and dense table content. The summary panel uses the primary border and brighter numeric emphasis. Footer commands are inline key/value pairs with orange keys and muted labels.

Desktop topbars use the primary border and surface background. The brand area is a tight horizontal group: bars mark first, `Token Use` second, with no version or source chip beside it.

## Desktop Track

The Desktop track inherits every shared token above and adds the following.

### Typography

A dual-font system. Inter (variable, weights 400/500/600/700) is the UI font; JetBrains Mono is reserved for numbers, IDs, paths, code blocks, and table data cells where column alignment matters.

- **`desktop.typography.ui`** — base 13px Inter for nav, panel titles (when not branded as a TUI panel title), button text, dropdown labels, prose.
- **`desktop.typography.heading`** — 13px Inter 600 for panel titles and section headings on Config, Insights, and modals.
- **`desktop.typography.label`** — 11px Inter 600 uppercase with letter-spacing for KPI labels, table headers, status pill text.
- **`desktop.typography.body`** — 13px Inter 400 for advice prose, descriptions, modal copy.
- **`desktop.typography.numeric`** — JetBrains Mono with `font-variant-numeric: tabular-nums slashed-zero` for KPI values, costs, call counts, token counts, durations, session IDs, file paths, and code blocks.

Use the `.mono` utility class (or set `font-family: var(--font-mono)` directly) on any cell or span containing a number or identifier.

### Spacing

The desktop spacing scale expands the lower end and adds higher-end tokens for vertical rhythm between groups: `xs 2 / sm 4 / md 8 / lg 12 / xl 16 / 2xl 24 / 3xl 32`. Use `xl` between sibling panels in a grid; use `2xl` between unrelated sections in a page (e.g. between the filter strip and the page content); use `3xl` only for top-level page padding.

### Shapes

Outer panels and KPI tiles use `desktop.rounded.md` (8px). Inner chips, badges, segmented controls, and inline pills use `desktop.rounded.sm` (3px). Status pills and inline notice pills use `999px` for full rounding. Tables remain flat — no rounded rows.

### Elevation

Only transient surfaces (dropdown menus, modals, popovers, tray popover) may use `desktop.elevation.popover`. In-flow panels, KPI tiles, table rows, and the status pill stay flat. Never stack a card inside a card.

### Motion

The desktop motion vocabulary is built on the existing `motion` library (npm `motion` v12+) in `desktop/src/motion.ts`. All animations must respect `prefers-reduced-motion: reduce` via the helper already in that module.

**Durations**

- `fast 120ms` — hover transitions, focus rings, status pill enter/exit.
- `base 180ms` — segmented-control indicator slide, panel reveal, modal scrim fade, inline notice enter/exit.
- `slow 280ms` — bar fills (gauges, rank bars), chart refresh.

**Easings**

- `standard cubic-bezier(.2,.8,.2,1)` — default for most state changes.
- `accel cubic-bezier(.4,0,1,1)` — exit transitions (modal close, notice dismiss).
- `decel cubic-bezier(0,0,.2,1)` — entrance transitions (reveal, pill in).

**Patterns**

- **Page transition** — cross-fade 120ms between view changes triggered by nav click.
- **Panel reveal** — staggered (25ms stagger, 220ms each) using `staggeredReveal` on page mount.
- **Segmented indicator** — a positioned underline `<div>` whose `x`/`width` tween 180ms standard when the active tab changes.
- **Status pill** — enter (translateY 4 → 0, opacity 0 → 1, 180ms decel) and exit (opacity 1 → 0, 120ms accel).
- **Hover lift** — subtle background tint tween via a CSS custom property, 120ms standard. No layout reads in JS.

## Do's and Don'ts

- Do keep the first screen as the actual dashboard, not a splash or marketing page.
- Do keep metric values right-aligned and labels left-aligned for fast scanning.
- Do use color to group panels and severity, but keep the dark surface dominant.
- Do use `Token Use` for product-facing desktop labels and `tokenuse` only for literal technical identifiers.
- Do generate desktop app icons from `desktop/tokenusebars.svg` and use the bars-only mark in app chrome.
- Do prefer native TUI widgets and layout primitives over custom terminal drawing.
- Do use Inter for UI chrome on the Desktop track and JetBrains Mono for numbers, IDs, and table data cells with `tabular-nums`.
- Do bundle Inter via `@fontsource-variable/inter` so the app never reaches a font CDN at runtime.
- Don't add decorative backgrounds, oversized type, or large empty hero areas.
- Don't add drop-shadow halos, stacked cards, or rounded card styling that makes the desktop feel like a generic web mockup. The 8px corners on outer Desktop panels are the cap, paired with hairline borders and flat depth.
- Don't show version numbers or live/source badges in the primary topbar.
- Don't hide keyboard commands behind help text; keep the footer visible (on Desktop it may be downsized and muted, but it must remain on-screen).
- Don't introduce a third font family. The Desktop track is strictly Inter + JetBrains Mono; the TUI track is strictly JetBrains Mono.
