# Deployments

Tagged releases run through `.github/workflows/ci.yml`.

## Release checks

Pull requests, pushes to `main`, and release tags run RustSec audits of both `Cargo.lock` and `desktop/src-tauri/Cargo.lock`, plus `pnpm audit --audit-level low` for the desktop lockfile. The audits include development dependencies and use current advisory data. Rust vulnerabilities and frontend advisories at any severity fail CI; advisory service failures also fail the check. RustSec maintenance and unsoundness warnings remain visible for review, including upstream GTK and font dependencies, and are not silently ignored.

Every release must pass the root formatting, compilation, Clippy, and test checks and the desktop frontend/shell checks on Linux, macOS, and Windows. Run the local equivalents in `AGENTS.md`, including all three audits, then confirm the final commit's CI results before tagging. Every release build depends on both audit jobs, so a failed audit prevents publication.

The tag version must match all four package/config versions and have a non-empty `docs/releases/<version>.md`. GitHub Releases use that file's first heading as the release title and its contents as the release notes. After publication, check the assets and updater manifest along with the Homebrew, WinGet, and website dispatch jobs.

## Release Assets

The release workflow builds and uploads these TUI binaries:

| Platform | Asset |
| --- | --- |
| Linux AMD64 | `tokenuse-linux-amd64` |
| Linux ARM64 | `tokenuse-linux-arm64` |
| macOS Intel | `tokenuse-darwin-amd64` |
| macOS Apple Silicon | `tokenuse-darwin-arm64` |
| Windows AMD64 | `tokenuse-windows-amd64.exe` |

Each asset has a matching `.sha256` checksum file.

## Desktop Apps

Tagged releases also build desktop app bundles:

| Platform | Assets |
| --- | --- |
| macOS ARM64 | `tokenuse-desktop-macos-arm64.dmg` |
| Windows AMD64 | `tokenuse-desktop-windows-amd64-setup.exe`, `tokenuse-desktop-windows-amd64-setup.exe.sig`, `tokenuse-desktop-windows-amd64.msi` |
| Linux AMD64 | `tokenuse-desktop-linux-amd64.AppImage`, `tokenuse-desktop-linux-amd64.AppImage.sig`, `tokenuse-desktop-linux-amd64.deb`, `tokenuse-desktop-linux-amd64.rpm` |
| Linux ARM64 | `tokenuse-desktop-linux-arm64.AppImage`, `tokenuse-desktop-linux-arm64.AppImage.sig`, `tokenuse-desktop-linux-arm64.deb`, `tokenuse-desktop-linux-arm64.rpm` |

Each asset has a matching `.sha256` checksum file.

The release also uploads `latest.json`, the static manifest consumed by the Windows/Linux Tauri updater. The manifest points Windows to the normalized NSIS setup installer and Linux to the normalized AppImage assets. `.deb` and `.rpm` packages remain manual GitHub Release installs.

The macOS desktop release job builds the Apple Silicon DMG, signs it with a Developer ID Application certificate, notarizes through App Store Connect, verifies the mounted DMG, and uploads the normalized artifact to the GitHub Release. Windows and Linux desktop assets are not OS code-signed for now and should be verified with their checksum files before installing. Their `.sig` files are Tauri updater signatures, not Authenticode or Linux package signatures.

## Required Secrets

The macOS desktop release job requires:

| Secret | Purpose |
| --- | --- |
| `APPLE_CERTIFICATE` | Base64-encoded Developer ID Application `.p12` certificate |
| `APPLE_CERTIFICATE_PASSWORD` | Password for the exported certificate |
| `KEYCHAIN_PASSWORD` | Temporary CI keychain password |
| `APPLE_API_ISSUER` | App Store Connect issuer ID |
| `APPLE_API_KEY` | App Store Connect key ID |
| `APPLE_API_PRIVATE_KEY` | App Store Connect `.p8` private key contents |
| `TAURI_SIGNING_PRIVATE_KEY` | Tauri updater private key content for Windows/Linux update artifacts |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Optional Tauri updater private key password |
| `HOMEBREW_TAP_TOKEN` | Token with push access to `russmckendrick/homebrew-tap` |
| `WINGET_CREATE_GITHUB_TOKEN` | Classic PAT with `public_repo` and `workflow` scopes and push access to the `russmckendrick/winget-pkgs` fork |

Use a Developer ID Application certificate for direct-download DMGs. Apple Distribution is for App Store distribution, and Developer ID Installer is for `.pkg` installers.

## Homebrew Tap

After the GitHub Release is created, `.github/workflows/update-tap.yml` updates:

- `Formula/tokenuse.rb` for the TUI on macOS and Linux.
- `Casks/tokenuse-desktop.rb` for the Apple Silicon macOS desktop DMG.

The tap downloads checksums from the newly published release before writing the formula and cask. Linux desktop assets are published only to GitHub Releases for now.

## WinGet

After the GitHub Release is created, `.github/workflows/update-winget.yml` submits the Windows desktop app to WinGet as `RussMckendrick.TokenUse`. It uses `gh repo sync` to fast-forward the `russmckendrick/winget-pkgs` fork from `microsoft/winget-pkgs`, then runs `vedantmgoyal9/winget-releaser` to open a manifest pull request against upstream pointing at `tokenuse-desktop-windows-amd64.msi`. The classic PAT needs both `public_repo` and `workflow` because upstream changes can include files under `.github/workflows/`. The installer regex is pinned to the MSI for two reasons: the raw TUI binary `tokenuse-windows-amd64.exe` in the same release must never be picked up as an installer, and komac (which `winget-releaser` runs under the hood) fails to emulate the Tauri NSIS setup installer (it aborts in the WebView2 branch of the installer script), while the MSI yields a clean `wix` manifest with ProductCode and UpgradeCode metadata.

The action only updates packages that already exist in `winget-pkgs`; the initial `RussMckendrick.TokenUse` version was bootstrapped manually with `komac new`. The workflow can also be re-run for a given tag via `workflow_dispatch`. WinGet installs the MSI silently and each manifest pins the installer SHA256, so no Authenticode signature is required, though users may see a SmartScreen prompt. Note the in-app Config-page updater ships the NSIS installer; WinGet users should update via `winget upgrade` to keep a single Apps & Features entry.

If the automatic fork sync fails, verify the PAT scopes and run `gh repo sync russmckendrick/winget-pkgs --source microsoft/winget-pkgs` with that token before re-running the workflow. Do not force the sync unless the fork intentionally contains commits that should be discarded.
