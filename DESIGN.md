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
- Primary items — Overview, Analytics, Coach, Models, Projects — each a 32px row: 16px icon, 13px Inter label, `rounded.sm` hover tint, primary-colored active indicator (2px inset bar on the left edge).
- Tools sits below the primary views as the tool group's summary entry, directly above the tool rows it summarizes. Claude Code, Cursor, Codex, Copilot, and Gemini are direct peer rows beneath it. They use 16px monochrome provider/tool marks, are not nested in an accordion subtree, and dynamically order from highest to lowest rolling 24-hour call activity with a stable fallback for ties.
- Config is pinned at the bottom above the collapse control.
- The rail is flat: surface background, hairline right border, no elevation, no rounded container.

### Page anatomy and scrolling

The shell is fixed (sidebar + status bar); each page scrolls vertically on its own. No page may scroll horizontally — wide tables scroll inside their panel.

Sibling cards in the same grid row are always equal height: the tallest card sets the row and every other card stretches to match, so each band ends on one flush bottom edge — no ragged card bottoms, ever. Stretched cards keep their content top-anchored and let calm surface show below; they never pad with filler to fake fullness. Data-list panels cap at 480px; once capped, the panel body owns vertical scrolling and keeps its table header sticky. A capped list may stretch up to its cap to match the row but never drives the row taller than 480px — do not compose a row that pairs a taller-than-cap panel with a capped list, since the cap would break the flush edge.

- **Sticky page header** at the top of every page: page title (13px Inter 600), period selector, page-scoped tool/project/sort controls, then refresh and export in one primary toolbar. At compact desktop widths the contextual controls collapse from text labels to icon + current value; wrapping is reserved for the narrowest supported window sizes. The header keeps the hairline bottom border while content scrolls under it.
- Grid zones below the header use `desktop.spacing.xl` between sibling panels and `2xl` between unrelated sections; `3xl` is page padding only.
- Panels are `desktop-panel` (8px corners, hairline border, neutral background, flat). Never stack a card inside a card.

### Expanding an analytical page

More available data should produce better information architecture, not a taller pile of cards. The Coach workspace is the reference: it keeps one shared filter context, gives distinct analytical jobs their own views, leads each view with orientation, and keeps evidence visible beside the selected item. These rules apply whenever an existing desktop page gains substantial new detail.

#### Choose the smallest structure that fits

Use this escalation order. Move to the next level only when the current level would mix different user questions or make the primary view cramped.

1. **Extend the existing panel** when the new value answers the same question, uses the same filters and grain, and can be read without another heading.
2. **Add a full-width section** when the material is a distinct comparison or trend but still belongs to the same task. A section gets one heading, an optional one-line hint, and one dominant visual or explorer.
3. **Add a local subview** when one analysis has mutually exclusive representations of the same measure, such as language/model/project or hours/calendar/projects. Use a compact segmented control inside the owning section.
4. **Add a page-level tab** when the content is a stable workflow with its own summary, analysis, and detail state. Tabs receive the full content width; they do not squeeze independent workspaces into columns merely to avoid navigation.
5. **Add a route** only when the content has a different primary entity, navigation identity, or filter model and should be directly reachable from the sidebar.

Page-level tabs are for durable jobs, not two small display toggles. Prefer four or fewer; five is the practical maximum before the information architecture needs another pass. Local subviews never masquerade as page tabs. Do not use accordions or expanding cards for primary analytics, evidence, timelines, or ranked data; expansion changes surrounding geometry, hides comparison context, and makes scanning unpredictable. Accordions are reserved for optional explanatory or configuration help.

#### Layer detailed views consistently

An expanded page follows this reading order:

1. **Shared toolbar** — page title and authoritative period/tool/project/sort controls, followed by refresh and export.
2. **View navigation** — full-width page tabs when the page owns multiple workflows.
3. **Orientation** — four to six comparable KPIs or one clear hero summary. Do not repeat a large page-wide hero on every analytical tab.
4. **Primary analysis** — the trend, comparison, or explorer that answers the tab's main question. Give it the full width when labels, time, or selection matter.
5. **Detail and evidence** — a persistent inspector, master/detail workspace, or selected-interval summary that explains the primary analysis without opening an overlay.
6. **Secondary breakdowns** — ranked lists and alternate dimensions below the primary analysis, not competing beside it for hero space.

Each section must have one clear question. If its title, hint, KPIs, and chart describe different grains or entities, split it. A page may be long when the sequence is coherent; it may not be cramped. Vertical scrolling is preferable to shrinking charts, truncating evidence, or fitting three unrelated panels across one row.

#### Summary and KPI guardrails

- Reuse the shared KPI band/tile patterns. Do not invent a new card treatment for each page.
- Keep a KPI row to four to six measures with comparable visual weight. A metric needs a short label, a prominent value, and at most one qualifying line.
- KPIs orient the analysis below them: total, active population, average, peak, and dominant dimensions are a useful sequence. Avoid vanity metrics that have no corresponding detail.
- A KPI must be computed from the complete filtered dataset. Never derive totals, active counts, averages, or peaks from a display-capped `Top N` array.
- If a list is capped for rendering, label it as ranked/Top N and keep its cap separate from the summary and chart data contracts.
- Sibling panels in a shared row are always equal height — the row stretches every card to its tallest member (see Page anatomy and scrolling). Choose row partners so the symmetry reads as intentional: pair panels of comparable content volume, and let a shorter list end with calm empty surface below its rows rather than filler content.

#### Filters, time, and data integrity

- The sticky page toolbar is the single source of truth for period, tool, project, and page sort. Contextual controls sit before refresh/export and collapse to icon + current value at compact widths. Do not repeat global filters inside a tab.
- Local controls may change representation or selection, but never silently override the shared filter context. Their scope must be obvious from placement inside the owning section.
- Every page-scoped query key includes every global input that changes its data, including data generation and currency where relevant. A filter change must produce a new payload and a visibly appropriate chart domain.
- Backend payloads carry the complete data required by summaries and trends. Frontend display limits are for ranked lists only; they must never truncate a time range or become an accidental analytics boundary.
- Time charts preserve the selected range, including zero-activity buckets, so sparse data does not collapse into uniformly spaced active points. The first and last buckets must frame the actual filter period.
- Resolution should produce a readable timeline rather than a fixed number of recent active rows. Use finer buckets for short periods and aggregate long history to a coarser grain. Coach AI Output is the canonical mapping: 30-minute/24 Hours, hourly/7 Days, daily/30 Days and This Month, monthly/All Time.
- Titles, legends, rolling windows, axis labels, selected details, and accessibility labels all change with the grain. A monthly chart cannot retain “By day” or “3-day average” copy.
- All Time begins with the oldest available signal for that measure, not the oldest row from an unrelated dataset. If a parser or tool cannot provide the signal historically, state that limitation rather than implying zero.
- On payload change, preserve a selected item only when the same identity still exists. Otherwise select the newest interval or first ranked item. Never leave a detail pane describing data outside the active filter.

#### Drill-down and exploration

- Summaries that name an actionable item should navigate to its exact detail, not merely open the destination tab. Priority Findings demonstrates the pattern: clear incompatible local filters, select the rule, then activate Findings.
- Master/detail is the default for evidence-rich collections. Keep the stable list visible, select rows without expanding them, and use the adjacent inspector for rationale, metrics, actions, and samples.
- Use real buttons for selectable cards, chart marks, rows, and tabs. Provide hover, active, focus-visible, keyboard, and `aria-selected`/`aria-pressed` states as appropriate.
- A click should reveal more precision, not merely repeat the visible label in a modal. Overlays are for transient actions; analytical detail stays in the page whenever space permits.
- Selection is a view state, not a layout mode. Selecting an item must not resize neighboring rows, move controls, or collapse the comparison set.

#### Entity drill-in routes

Projects and Models are the reference implementations. When an entity (a project, a model, a session) earns its own detail page, the whole app follows one pattern:

- **Every row that names the entity is a link.** A ranked table naming projects navigates to the project page; a table naming models navigates to the model page — on every page that renders the shared table component, not just the entity's own index. Cross-links are expected: the model page's By Project rows open project pages and vice versa.
- **Detail pages open with an origin-aware back chip.** The first element of the page is the session-view back affordance (`←` + the originating page's title). Opening a drill-in records the current route and scroll position; back returns there and restores scroll — it does not dump the user on the entity's index when they arrived from Overview. Chained drill-ins (project → model → project) unwind step by step.
- **`Esc` pops a drill-in** after call detail, modals, and session, in that order. Plain navigation (sidebar, tabs, nav keys) ends the trail; the session sub-route preserves it across its round trip.
- **The index stays the canonical home.** The sidebar item always leads to the index; the drill-in is a sub-route of it (`Route.project`, `Route.model`), never a separate sidebar entry.

#### Responsive behavior for expanded pages

- Preserve the information hierarchy as width decreases. Collapse toolbar labels before removing context; reduce columns before hiding metrics.
- KPI bands step down predictably (for example six→three→two columns). Values and labels remain aligned and readable; they do not become horizontally scrolling cards.
- Full-width page tabs remain one coherent tab rail. If labels no longer fit, shorten copy or use the established icon + label treatment; do not wrap the rail into a button cluster.
- Master/detail explorers become a narrower split or a list followed by detail at the narrow breakpoint. They do not turn every list row into an accordion.
- Charts keep their plot width and reduce tick frequency responsively. Dense labels are sampled; the underlying buckets and selectable data remain complete.
- The page itself never scrolls horizontally. Wide tables, code, or timelines own their local overflow.

#### Expanded-page acceptance checklist

Before an expanded section is complete, verify:

- every period produces the correct range, grain, title, legend, KPIs, and selected detail;
- tool and project filters affect summaries, charts, rankings, and evidence consistently;
- complete-range totals reconcile with chart buckets, allowing only documented no-signal rows;
- empty, one-point, sparse, dense, and long-history datasets remain legible;
- switching filters cannot leave a stale selection or reuse a capped previous series;
- the primary workflow works without expanding cards or opening a modal;
- tabs, rows, cards, and chart marks are keyboard reachable with visible focus;
- compact-width layouts preserve context without page-level horizontal scrolling;
- all wording comes from `src/copy/copy.json`, all colors from tokens, and all shipped behavior is covered in the unreleased notes.

### Screen inventory

Seven screens. Data pages fetch page-scoped queries from the core (memoized per filter set); the 3-second snapshot poll carries only the shared dashboard, limits, and filter state.

- **Overview** — the daily read. Top: KPI hero band (cost in `display-xl`, calls/sessions/cache-hit/in-out in `display-lg`) with count-up on load. Second: **utilisation strip** — active primary limits grouped into compact tool modules. Each module has a horizontal identity header with the provider-accented tool mark inside a threshold ring for its most constrained window, followed by a two-column 5h/weekly/credits detail matrix; a single limit spans both columns. Claude Extra Usage and Codex Spark model-specific windows remain on their dedicated tool pages instead of crowding this summary. Modules stay side by side until the narrow layout breakpoint, avoiding both tall limit stacks and full-width form rows. Third: the hero activity chart (spend bars + calls line, full width, hover crosshair). Bottom: top projects and top models tables side by side.
- **Analytics** — the time explorer (evolves Deep Dive). Hero area+bar combo with period framing, stacked per-tool daily bars, hour×weekday heatmap of activity, cache-efficiency panel, and the ranked tables (projects, sessions, models, commands, MCP servers).
- **Coach** — the report-card workspace. A full-width row of document-style tabs separates **Report**, **Findings**, **AI Output**, and **Activity**. Each tab pairs a small icon with its label; the active tab lifts onto the neutral report surface with rounded top corners and a connected bottom edge while inactive tabs remain recessed in the tab rail. The large composite-grade band and four practice cards belong to Report only, leaving the analytical tabs a full workspace. Report surfaces the three priority findings as direct links to their matching Findings evidence detail, plus equally weighted Flow (score, KPIs, recent sparkline) and Pace panels. Findings opens with the shared four-tile KPI pattern, then uses a severity-filtered master list with a fixed left-aligned badge/title/count grid and a persistent detail workspace with rule rationale, signal metrics, next action, and every captured evidence sample. AI Output leads with six outcome KPIs, then gives the period-aware output trend full width with bars, a matching rolling average, selection details, and a selectable language/model/project ranking. Activity follows a three-view pattern: Work Hours (hour×weekday intensity, weekday/weekend profile, period trend), Calendar (period-scoped daily-activity bars with trailing context, session table with timeline track, fixed call inspector), and Projects (ranked project rows with spend, calls, sessions, AI LoC, and tool mix).
- **Tools** — the parent route is a rolling 24-hour fleet overview: one compact KPI card per tool (accent tool mark, 24h cost in `display-lg`, activity sparkline, calls/tokens/seen stats, primary limit gauges with resets, plan-value line when priced), each card a button into that tool's page. Cards order busiest-first by rolling 24-hour calls — the same dynamic order as the sidebar tool rows — and in the two-column layout the busiest tool's card spans both columns as a spotlight, adding limit plans and its top-model rows; the sparkline and stat tiles sit side by side there. Idle tools keep their card with the empty pattern inside. The full consoles live only on the direct tool pages — period-aware, with hero numbers, limit gauges (UsageConsole lineage), top models, projects, and sessions.
- **Models** — the unified catalog. Rows grouped by provider (icon + provider label as group headers), each canonical model showing cost/calls for all five periods plus active-period cache-hit and an expandable per-tool split. This is the one place the same model's use across Claude Code, Copilot, and Cursor reads as one row.
- **Projects** — master list of projects with per-project spend and tool mix; selecting a project reveals its sessions; selecting a session opens the call-level detail pane (timestamp, model, tokens, cache rates, prompt) without leaving the page.
- **Config** — currency, data downloads, limit sidecars and subscription sync, desktop behavior toggles, updates, and clear-data. Quiet page; no charts.

### Chart vocabulary

All charts are hand-rolled SVG driven by d3 scales — no chart library. Every fill, stroke, and ramp comes from chart tokens (`--chart-*`) or provider tokens; **no hex literals inside components**.

- **Hero area+bar combo** — spend bars (primary) with calls line+area (cyan), monotone curve, rAF-throttled hover crosshair and tooltip. The Overview and Analytics hero.
- **Stacked bars** — per-tool or per-provider composition over time; segment colors from the series ramp or provider accents, hairline gaps between segments.
- **Donut** — share-of-total (spend by provider or tool); 3px-radius corner caps, center label in `display-lg`, legend as a right-hand table, never floating labels.
- **Heatmap** — hour × weekday activity; stepped ramp from `bar-empty` through secondary to primary to error at saturation. Cells square, 1px gaps. The Coach Work Hours grid instead reuses the activity calendar's single-hue primary ramp with a Less→More legend and keeps exact counts in cell tooltips only — no in-cell figures.
- **Activity day picker** — the Coach Calendar's daily-bars strip, sharing the Output-trend bar language: one primary-toned bar per calendar day (turns), continuous from the first shipped day through today so quiet stretches read as gaps, windowed by the selected period (a trailing ~9-week context for scoped periods, the trailing year for All Time). Horizontal hairline gridlines with small turn-count labels and a warning-toned 7-day moving-average line (with an Output-trend-style text legend) frame the bars. In-period bars sit at 0.72 opacity rising to full on hover/selection; out-of-period context bars dim to 0.28 but stay clickable; the selected bar carries an `on-surface` outline and a tick marker under the baseline; tooltips lead with the weekday. MM-DD axis labels in 10px muted Inter step to at most ~12 across the strip. Clicking a bar picks the day for the Coach session table; a flat selected-day summary (day in `display-lg` mono with its weekday, session/turn/spend facts) sits beside the strip, and selecting a session row drives the request inspector.
- **Session table** — the Coach Calendar's per-day session view: one 30px single-line row per session with fixed metadata columns (start–end time in mono, short project name, tool dot+label, turns, cost) and a flexible timeline track under a shared whole-hour axis; hairline hour gridlines run through the rows, session blocks are tool-colored 12px bars at 2px radius. The header row is sticky and the list caps at ten rows before scrolling; the selected row carries the 2px primary left border and tint.
- **Gauges** — horizontal utilisation bars with threshold tones: tertiary below 60%, warning 60–88%, error above. Track is `bar-empty`; label left, percentage right in mono. Overview adds one circular threshold gauge per tool around its mark, representing that tool's most constrained active window; detailed limit values remain horizontal beside it. Coach inverts the tones for its radial grade gauge and grade badges, and grades on a curve — green is earned by A-tier only (tertiary ≥90, warning ≥70, error below) — with the letter grade centered in `display-xl` mono.
- **Sparklines** — tick bars + trend line for compact per-tool cadence in tool cards and usage consoles. Coach uses a plain line+area sparkline (fixed 0–100 domain, stroke toned by the latest value) for weekly practice-score trends; a one-point period renders as a horizontal score state with a centred latest-point marker.
- **Output trend** — Coach's code-output chart gets a full plot with horizontal gridlines, exact bucket selection, bars, and a same-scale three-bucket moving-average line. Resolution follows the selected period: 30-minute buckets with a 90-minute average for 24 Hours, hourly buckets with a three-hour average for 7 Days, complete daily timelines for 30 Days and This Month, and monthly buckets across the full history for All Time. The selected interval remains summarized under the chart. Flow uses the more compact score sparkline.
- **Rank fills** — ranked table rows paint their relative magnitude directly across the row background: a muted secondary-blue wash (`--color-rank-fill`, ~12% alpha) from the row's left edge to the value percentage, closed by a 1px brighter terminus edge (`--color-rank-edge`). The fill is a hard stop, never a fade — gradients stay reserved for the brand asset — and it must stay quiet enough that a 100% row never reads as hover or selection (selection keeps its own border/tint treatment on top). The leading table column belongs to the entity name; there is no meter column. The TUI mirrors the pattern by tinting the entity cell's background for the same proportion of a fixed character budget, with a one-cell brighter terminus. Exact percentages live in row tooltips and screen-reader text, not extra columns. The old 12-segment stepped meter (`RankBar`) survives only inline in console and tool-card model rows, where the bar sits mid-row and no name column is competing for space.

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
- **Gauges, rank fills, and inline rank bars** — fill tween 280ms slow (rank fills animate `--rank-fill` via a registered `@property`).
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
- Do give a substantial new analytical workflow its own full-width section or page tab instead of compressing it into the existing grid.
- Do connect actionable summaries to the exact matching detail and preserve the surrounding comparison context.
- Do keep summary calculations and time-series payloads independent from ranked-list display caps.
- Do keep sibling cards in a grid row equal height on the desktop — the tallest card sets the row and the others stretch to its flush bottom edge.
- Don't add decorative backgrounds, oversized type outside the hero bands, or large empty hero areas.
- Don't add drop-shadow halos, stacked cards, or rounded-card styling that makes the desktop feel like a generic web mockup. The 8px corners on outer desktop panels are the cap, paired with hairline borders and flat depth.
- Don't use accordions or expanding cards for primary analytics, timelines, findings, or evidence.
- Don't reuse a fixed recent slice for multiple periods; the chart domain and aggregation grain must reflect the active filter.
- Don't show version numbers or live/source badges in the sidebar brand block; provenance lives in the status bar and Config.
- Don't hide keyboard commands behind help text; the status bar keeps them on-screen.
- Don't introduce a third font family, and don't use brand-colored provider icons inside dense tables.
- Don't ship a screen that renders raw model identifiers; every model name flows through the registry.
