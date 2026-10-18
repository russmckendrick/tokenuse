# Unreleased

Changes that should be included in the next release go here. Keep this file current during normal development; move the relevant notes into `docs/releases/<version>.md` only when preparing a release.

## Added

No additions recorded yet.

## Changed

- Money reads the same everywhere in the TUI and desktop app: two decimals with thousands separators (`£1,088.42`, not `£1088`), `£0` for exactly zero, and `<£0.01` for amounts below a penny instead of `£0.0000` or `£0.0070`. Desktop call rows and the call detail show the exact cost on hover, and the TUI call detail shows it inline. Zero-decimal currencies such as JPY show whole units.
- Claude Haiku 5.5 and Sonnet 5.5 have their own model names and prices. Haiku 5.5 no longer shows as "Haiku 5 5" with a fallback-pricing warning, and Sonnet 5.5 no longer shows as Sonnet 5 at Sonnet 5's higher cache-read rate. New calls price correctly, and calls already in the archive are repaired on upgrade (see Fixed). Haiku 5.5 prompts over 100,000 tokens still use the standard-tier estimate. Cursor's Gemini 3.7/3.8 Flash output rate is updated to $3.75/MTok.
- Coach colours its practice score bars and practice sparklines on the same curve as the grade letters (green from 90, yellow from 70), so an 80 no longer shows a green bar beside a yellow B. The TUI's grade colours now follow the same three tones.
- Ranked-table rows show magnitude as a quieter wash with a thin underline instead of a full-height edge that looked like a text cursor. The TUI drops its brighter terminus cell the same way.
- The desktop status bar only lists period, tool, sort, and project shortcuts on pages that show those controls, and those shortcuts only act there. The tool shortcut now also works on Coach. The TUI's Coach footer no longer advertises `g sort`.
- Windows releases are now Authenticode-signed with a Certum code-signing certificate: the TUI's `tokenuse-windows-amd64.exe`, the desktop NSIS and MSI installers, and the app executable inside them. SmartScreen warnings should fade as the signed releases build reputation.

## Fixed

- The desktop page title no longer stays on the previous tool when you move between tool pages, for example showing "Claude Code" on the Codex page.
- Claude Haiku 5.5 and Sonnet 5.5 calls already in the archive are repriced once when you upgrade. Haiku 5.5 calls were charged at the Sonnet 4.6 fallback, about 30 times too much, and now use Haiku 5.5's standard rate; prompts over 100,000 tokens still use that standard-rate estimate. Claude Code Sonnet 5.5 calls were charged Sonnet 5's cache-read rate, twice the real one. Only rows whose stored cost proves the old price are changed, so correct rows are left alone and the repair never applies twice. Cursor and Copilot Sonnet 5.5 calls are not repriced, because those tools still price new Sonnet 5.5 calls at the Sonnet 5 rate.

## Removed

No removals recorded yet.
