# Unreleased

Changes that should be included in the next release go here. Keep this file current during normal development; move the relevant notes into `docs/releases/<version>.md` only when preparing a release.

## Added

- Desktop per-model overview page: clicking a row on the Models catalog now opens a full model page — hero KPIs, Activity Pulse, the model's sessions, a token-composition split (input / output / cache read / cache write), a per-tool donut, an effective-pricing panel (per-Mtok rates, cache rates, average cost per call, with a warning when fallback pricing is in use), plus which projects and activity categories the model's spend comes from.
- TUI model filter: `m` opens a typeable model picker (mirroring the `p` project picker) that scopes Overview and Deep Dive to one canonical model; the active model shows in the title bar next to the project filter.
- Sample/demo mode supports the model scope end to end, including the model page and picker, with totals that still reconcile.
- Drill-in navigation pattern: project and model pages open with an origin-aware back chip (`←` plus the page you came from) that restores your scroll position, and `Esc` steps back through chained drill-ins. Model rows are now links everywhere the shared model table appears (Overview, Analytics, tool pages, project pages), and the pattern is documented in `DESIGN.md`.

## Changed

- Models catalog rows navigate to the model page instead of expanding inline; sample-mode catalog rows now fold by canonical model id across tools (matching live behaviour), so a model used from several tools shows one row with a real per-tool split.

## Removed

- The inline per-tool expansion on the desktop Models page (superseded by the model page).
