---
version: "beta"
name: "Token Use Console"
description: "A dense dark design system for a token usage analytics dashboard, with a shared root and two deliberately diverged tracks: terminal (TUI) and desktop (Tauri + Svelte)."
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
  providers:
    anthropic: "#D97757"
    openai: "#9FB8AD"
    google: "#8AB4F8"
    github: "#8B949E"
    cursor: "#7A7FEE"
    xai: "#C8CDD8"
    other: "#A1A7C3"
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
    display-lg:
      fontFamily: "JetBrains Mono, SFMono-Regular, Menlo, Consolas, monospace"
      fontSize: "20px"
      fontWeight: 700
      lineHeight: 1.15
      fontFeatureSettings: "'tnum' 1, 'zero' 1"
    display-xl:
      fontFamily: "JetBrains Mono, SFMono-Regular, Menlo, Consolas, monospace"
      fontSize: "28px"
      fontWeight: 700
      lineHeight: 1.1
      fontFeatureSettings: "'tnum' 1, 'zero' 1"
  sidebar:
    width: "200px"
    width-collapsed: "64px"
    item-height: "32px"
    icon-size: "16px"
  lists:
    panel-max-height: "480px"
  charts:
    grid: "#292D42"
    axis-label-size: "10px"
    series:
      - "{colors.primary}"
      - "{colors.cyan}"
      - "{colors.secondary}"
      - "{colors.tertiary}"
      - "{colors.magenta}"
      - "{colors.warning}"
      - "{colors.error}"
      - "{colors.muted}"
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
  desktop-sidebar:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.on-surface}"
    typography: "{desktop.typography.ui}"
    rounded: "{desktop.rounded.none}"
    padding: "{desktop.spacing.md}"
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
    typography: "{desktop.typography.display-xl}"
    rounded: "{desktop.rounded.md}"
    padding: "{desktop.spacing.lg}"
  desktop-status-bar:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.muted}"
    typography: "{desktop.typography.label}"
    rounded: "{desktop.rounded.none}"
    padding: "{desktop.spacing.xs} {desktop.spacing.lg}"
  desktop-status-toast:
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

Token Use is a local-only console for people who care about token spend, model behavior, and rate-limit headroom. The interface is dark, dense, and calm, with bright terminal accents used as structural signposts rather than decoration. The desired response is quick orientation: costs, calls, hot spots, and utilisation readable at a glance.

The system has a shared root and **two deliberately diverged tracks**:

- **TUI track** — the terminal renderer. A dense single-dashboard operator console: strict monospace, square corners, flat depth, no animation, three tabs (Overview, Deep Dive, Usage) plus Config and Session sub-pages. Everything below this overview that is not explicitly under "Desktop Track" applies to the TUI.
- **Desktop track** — the Tauri + Svelte app. A modern sidebar-rail analytics application with more screens and more room: Overview, Analytics, Coach, Tools, Models, Projects, and Config, each a scrollable page. Graphs and utilisation are the hero; provider icons identify models at a glance. The desktop is not a terminal copy — it shares the palette, the brand mark, and the data-dense table DNA, and diverges everywhere a desktop app earns the space.

The Rust core is the data and query layer both tracks render from. It owns filters, aggregation, the model registry (canonical model names, providers, families), and memoized query results.

The brand mark is the orange bars symbol from `desktop/tokenusebars.svg`. Desktop chrome pairs the bars-only mark with the product name `Token Use`; reserve `tokenuse` for command names, package identifiers, URLs, and other literal technical strings.

## Shared Foundation

### Colors

- **Primary (#FF8F40):** active states, summary borders, brand title text, command keys, and the spend series.
- **Secondary (#62A6FF):** informational panels and secondary series.
- **Tertiary (#4CF2A0):** successful or efficient usage signals.
- **Warning (#FFD60A):** money values and metrics that need attention.
- **Error (#FF5F6D):** risk, saturation, and the hot end of heat ramps.
- **Cyan (#4DF3E8) and Magenta (#F05AF2):** secondary category accents for tools, models, and MCP-like surfaces.
- **Surface and Neutral:** layered dark blue-gray backgrounds; hierarchy comes from borders and color, not shadows.
- **Provider accents** (`colors.providers`): one muted brand-adjacent accent per model provider — Anthropic clay, OpenAI sage, Google blue, GitHub gray, Cursor violet, xAI silver. Tuned to sit quietly on the dark surface; they identify, never decorate. Desktop-only; the TUI keeps its five-color accent set.
- **Brand bars:** the icon may use a warm orange gradient within the primary family, but this gradient is limited to the app icon and bars mark.

### Data-derived names

Model names, provider names, and family names come from the model registry (`src/models/registry.json`) and are data, not copy. Panel titles, empty states, and every other shipped string live in `src/copy/copy.json`.

### Density values

Both tracks favor information density over whitespace: right-aligned numerics, left-aligned labels, tabular figures, one-pixel borders. The desktop relaxes density only in hero bands (KPIs, page headers, hero charts) — tables stay tight everywhere.

## TUI Track

The terminal experience is unchanged in philosophy: a compact operator console rendered with native TUI widgets.

- Strict monospace (JetBrains Mono stack), square or nearly square corners (2–4px max), flat depth, no animation, no shadows.
- Tab strip: **Overview, Deep Dive, Usage**. Config and Session are sub-pages reachable from any tab. Footer command hints stay visible at all times.
- Panels use one-pixel borders, a colored title, and dense table content. The summary panel uses the primary border and brighter numeric emphasis.
- Heat bars use stepped color ramps to imply magnitude; gradients are reserved for the brand asset.
- Bold is reserved for the brand title, panel titles, active navigation, and important numeric values. No display-scale type inside the dashboard.
- Layout is a high-density grid with thin gaps and fixed-height summary/nav/footer bands; panels preserve predictable column alignment at common terminal widths, truncating long labels before hiding key metrics.

## Desktop Track

The desktop track inherits the shared foundation and is otherwise its own application. It must read as a native analytics tool — Linear-calm chrome around chart-forward pages — not as a themed web page and not as a terminal emulator.

### Typography

Dual-font system. Inter (variable, 400/500/600/700) is the UI font; JetBrains Mono is for numbers, IDs, paths, and table data cells. Both are bundled via `@fontsource` — the app never reaches a font CDN at runtime.

- **`desktop.typography.ui`** — base 13px Inter for nav, buttons, dropdown labels, prose.
- **`desktop.typography.heading`** — 13px Inter 600 for panel titles and section headings.
- **`desktop.typography.label`** — 11px Inter 600 uppercase with letter-spacing for KPI labels, table headers, status bar text.
- **`desktop.typography.body`** — 13px Inter 400 for descriptions and modal copy.
- **`desktop.typography.numeric`** — JetBrains Mono with `tabular-nums slashed-zero` for costs, counts, durations, IDs, paths.
- **`desktop.typography.display-lg` (20px) and `display-xl` (28px)** — JetBrains Mono hero numerics. Reserved for the Overview KPI band and per-page hero numbers. Never inside tables, never for prose.

Use the `.mono` utility class (or `font-family: var(--font-mono)`) on any cell or span containing a number or identifier.

### Sidebar rail

The primary navigation is a fixed left rail, `desktop.sidebar.width` (200px) expanded and `width-collapsed` (64px) when collapsed; the collapse toggle lives at the rail's foot and the choice persists in local storage for that desktop webview.

- Brand block at top: bars mark + `Token Use` (mark only when collapsed).
- Primary items — Overview, Analytics, Coach, Tools, Models, Projects — each a 32px row: 16px icon, 13px Inter label, `rounded.sm` hover tint, primary-colored active indicator (2px inset bar on the left edge).
- Claude Code, Cursor, Codex, Copilot, and Gemini are direct peer rows below the primary views. They use 16px monochrome provider/tool marks, are not nested in an accordion subtree, and dynamically order from highest to lowest rolling 24-hour call activity with a stable fallback for ties.
- Config is pinned at the bottom above the collapse control.
- The rail is flat: surface background, hairline right border, no elevation, no rounded container.

### Page anatomy and scrolling

The shell is fixed (sidebar + status bar); each page scrolls vertically on its own. No page may scroll horizontally — wide tables scroll inside their panel.

Data-list panels stay content-sized for short results and cap at 480px for long results. Once capped, the panel body owns vertical scrolling and keeps its table header sticky; grid siblings never stretch merely to match a longer list.

- **Sticky page header** at the top of every page: page title (13px Inter 600), page-scoped filter chips (period, and where relevant tool/sort/project), and page actions (refresh, export). The header keeps the hairline bottom border while content scrolls under it.
- Grid zones below the header use `desktop.spacing.xl` between sibling panels and `2xl` between unrelated sections; `3xl` is page padding only.
- Panels are `desktop-panel` (8px corners, hairline border, neutral background, flat). Never stack a card inside a card.

### Screen inventory

Six screens. Data pages fetch page-scoped queries from the core (memoized per filter set); the 3-second snapshot poll carries only the shared dashboard, limits, and filter state.

- **Overview** — the daily read. Top: KPI hero band (cost in `display-xl`, calls/sessions/cache-hit/in-out in `display-lg`) with count-up on load. Second: **utilisation strip** — active primary limits grouped into compact tool modules. Each module has a horizontal identity header with the provider-accented tool mark inside a threshold ring for its most constrained window, followed by a two-column 5h/weekly/credits detail matrix; a single limit spans both columns. Claude Extra Usage and Codex Spark model-specific windows remain on their dedicated tool pages instead of crowding this summary. Modules stay side by side until the narrow layout breakpoint, avoiding both tall limit stacks and full-width form rows. Third: the hero activity chart (spend bars + calls line, full width, hover crosshair). Bottom: top projects and top models tables side by side.
- **Analytics** — the time explorer (evolves Deep Dive). Hero area+bar combo with period framing, stacked per-tool daily bars, hour×weekday heatmap of activity, cache-efficiency panel, and the ranked tables (projects, sessions, models, commands, MCP servers).
- **Tools** — the parent route shows all rolling 24-hour consoles; direct sidebar tool rows open period-aware pages with hero numbers, limit gauges (UsageConsole lineage), top models, projects, and sessions.
- **Models** — the unified catalog. Rows grouped by provider (icon + provider label as group headers), each canonical model showing cost/calls for all five periods plus active-period cache-hit and an expandable per-tool split. This is the one place the same model's use across Claude Code, Copilot, and Cursor reads as one row.
- **Projects** — master list of projects with per-project spend and tool mix; selecting a project reveals its sessions; selecting a session opens the call-level detail pane (timestamp, model, tokens, cache rates, prompt) without leaving the page.
- **Config** — currency, data downloads, limit sidecars and subscription sync, desktop behavior toggles, updates, and clear-data. Quiet page; no charts.

### Chart vocabulary

All charts are hand-rolled SVG driven by d3 scales — no chart library. Every fill, stroke, and ramp comes from chart tokens (`--chart-*`) or provider tokens; **no hex literals inside components**.

- **Hero area+bar combo** — spend bars (primary) with calls line+area (cyan), monotone curve, rAF-throttled hover crosshair and tooltip. The Overview and Analytics hero.
- **Stacked bars** — per-tool or per-provider composition over time; segment colors from the series ramp or provider accents, hairline gaps between segments.
- **Donut** — share-of-total (spend by provider or tool); 3px-radius corner caps, center label in `display-lg`, legend as a right-hand table, never floating labels.
- **Heatmap** — hour × weekday activity; stepped ramp from `bar-empty` through secondary to primary to error at saturation. Cells square, 1px gaps.
- **Gauges** — horizontal utilisation bars with threshold tones: tertiary below 60%, warning 60–88%, error above. Track is `bar-empty`; label left, percentage right in mono. Overview adds one circular threshold gauge per tool around its mark, representing that tool's most constrained active window; detailed limit values remain horizontal beside it.
- **Sparklines** — tick bars + trend line for compact per-tool cadence in tool cards and usage consoles.
- **Rank bars** — 12-segment discrete meters in tables, stepped blue→yellow→red ramp.

Axis and framing rules: gridlines `desktop.charts.grid` hairlines, horizontal only where they aid reading; axis labels 10px muted Inter, no axis titles when the panel title says it; tooltips on `neutral` surface with `popover` elevation, values in mono; legends are text rows, never overlaid on the plot. Series colors are assigned in ramp order and stay stable within a page.

### Provider identity

Vendored SVG marks (from the MIT-licensed lobehub icon set, attribution in `desktop/LICENSES-THIRD-PARTY.md`) identify providers: Anthropic, OpenAI, Google, GitHub, Cursor, xAI, plus a neutral fallback glyph.

- In tables, the sidebar, and any dense row: **16px, monochrome `currentColor`**, inheriting the row's text color. Never brand-colored inside tables.
- In page headers, Models group headers, and Tools hero bands: **20px+, provider accent color** (`colors.providers.*`).
- Icons always pair with a text label at first use; never icon-only identification in data tables.

### Elevation, shapes, depth

Outer panels and KPI tiles use `desktop.rounded.md` (8px) — this is the cap. Inner chips, badges, segmented controls use 3px. Status pills are fully rounded. In-flow surfaces are flat; only transient surfaces (dropdowns, popovers, modals, tray popover) use `desktop.elevation.popover`. No drop-shadow halos, no stacked cards, no decorative gradients — the wow lives in the charts and hero numerics, not the chrome.

### Motion

Built on the `motion` library in `desktop/src/motion.ts`; every animation respects `prefers-reduced-motion`.

- **Route change** — 120ms cross-fade between pages.
- **Sidebar collapse/expand** — 180ms standard width tween; labels fade at 120ms.
- **Panel reveal** — staggered (25ms stagger, 220ms each) on page mount.
- **Hero numerics** — `countUp` on Overview KPI band load and data-generation change.
- **Charts** — route/panel reveal handles entrance; hover and data updates transition opacity or geometry without replaying a separate chart animation.
- **Gauges and rank bars** — fill tween 280ms slow.
- **Status toast** — bottom-right event feedback enters translateY 6→0 + fade 180ms, then auto-dismisses with a softer 360ms fade and 6px downward settle; routine refresh state never occupies permanent header space.
- **Hover lift** — background tint via CSS custom property, 120ms; no JS layout reads.

### Empty states

One pattern everywhere: muted 16px icon, one-line explanation from a copy key (`empty.*`), optional single action. Idle tools keep their console frame with the empty pattern inside, so the user still sees the tool was checked. No illustrations, no oversized art.

### Status bar

A slim 24px bottom status bar replaces the TUI-style footer: left — data-source pill (live/sample) and currency; right — contextual keyboard hints for the active page (muted, 11px labels). Desktop shortcuts stay discoverable on-screen while the TUI retains its full `h`/`?` reference.

## Iconography

Use `desktop/tokenusebars.svg` as the source asset for generated app icons. The full icon keeps its dark square background; in app chrome, use only the four orange bars next to `Token Use`. Interface icons are lucide at 16px; provider marks follow the Provider identity rules above.

## Do's and Don'ts

- Do keep the first screen a working dashboard — never a splash, marketing, or onboarding page.
- Do keep metric values right-aligned, labels left-aligned, and numerics in mono with `tabular-nums`.
- Do use color to group panels and encode magnitude, keeping the dark surface dominant.
- Do use `Token Use` for product-facing desktop labels and `tokenuse` only for literal technical identifiers.
- Do generate desktop app icons from `desktop/tokenusebars.svg` and use the bars-only mark in app chrome.
- Do prefer native TUI widgets and layout primitives over custom terminal drawing.
- Do bundle Inter and JetBrains Mono via `@fontsource` so the app never reaches a font CDN at runtime.
- Do route every chart color through chart or provider tokens; a hex literal in a component is a defect.
- Don't add decorative backgrounds, oversized type outside the hero bands, or large empty hero areas.
- Don't add drop-shadow halos, stacked cards, or rounded-card styling that makes the desktop feel like a generic web mockup. The 8px corners on outer desktop panels are the cap, paired with hairline borders and flat depth.
- Don't show version numbers or live/source badges in the sidebar brand block; provenance lives in the status bar and Config.
- Don't hide keyboard commands behind help text; the status bar keeps them on-screen.
- Don't introduce a third font family, and don't use brand-colored provider icons inside dense tables.
- Don't ship a screen that renders raw model identifiers; every model name flows through the registry.
