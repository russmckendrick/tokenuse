# Source Control

## Branches

Keep feature branches focused on one behavior change. Parser changes should include parser tests and docs updates for the affected tool page.

## Generated Files

Do not hand-edit generated snapshots:

- `src/pricing/snapshot.json`
- `currency/rates.json`
- desktop icon outputs under `desktop/src-tauri/icons/`

Use the maintainer commands or Tauri icon tooling that produced the file originally.

## Docs

Website docs are copied from this `docs/` directory, excluding `docs/releases/**`. Keep product docs under:

- `docs/guides/`
- `docs/development/`

Release notes in `docs/releases/` are maintainer source material only. The website release pages are sourced from GitHub Releases.

## Release Prep

Release tags must match all version fields:

- `Cargo.toml`
- `desktop/src-tauri/Cargo.toml`
- `desktop/package.json`
- `desktop/src-tauri/tauri.conf.json`

When preparing a release, bump all four version fields together before tagging.

Move completed notes from `docs/releases/unreleased.md` into `docs/releases/<version>.md` only as part of release preparation.
