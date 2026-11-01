# Unreleased

Changes that should be included in the next release go here. Keep this file current during normal development; move the relevant notes into `docs/releases/<version>.md` only when preparing a release.

## Added

- Claude subscription sync now shows model-scoped weekly caps as their own gauges, starting with the separate Fable weekly limit on Max and Ultra plans.
- Copilot Fusion turns that only record the router id now show as Copilot Fusion, Fusion Max or Fusion Fast instead of a raw model id.

## Changed

- Claude Ultra (formerly Max 20x) accounts without a 5-hour limit now show only the weekly gauges, and the weekly gauge falls back to the new `weekly_all` limit when the legacy `seven_day` field is missing. The demo data now shows an Ultra plan with weekly and Fable gauges.

## Fixed

No fixes recorded yet.

## Removed

No removals recorded yet.
