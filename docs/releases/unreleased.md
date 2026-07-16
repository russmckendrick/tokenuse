# Unreleased

Changes that should be included in the next release go here. Keep this file current during normal development; move the relevant notes into `docs/releases/<version>.md` only when preparing a release.

## Added

- Added current Cursor Agent `store.db` ingestion, interaction/token/timestamp provenance in archive v5, and provenance labels in TUI/desktop call details.

## Changed

- Rebuilt Cursor ingestion as one canonical user-turn reconstruction across state bubbles, AgentKv, request context, chat stores, transcripts, and AI tracking. This fixes modern unknown session ids, removes source overlap safely, enriches Cursor for Coach, and applies official Cursor first-party model pricing.
- Made Coach timing analysis require exact timestamps and made model diversity and premium-model checks canonical and tool-aware.
- Restored Codex Desktop usage ingestion for the new string credit-balance schema and backfilled sessions that were previously marked as processed without calls.

## Removed

No removals recorded yet.
