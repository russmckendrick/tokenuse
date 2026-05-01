# Deployments

Tagged releases run through `.github/workflows/ci.yml`.

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
| macOS Universal | `tokenuse-desktop-macos-universal.dmg` |
| Windows AMD64 | `tokenuse-desktop-windows-amd64-setup.exe`, `tokenuse-desktop-windows-amd64.msi` |
| Linux AMD64 | `tokenuse-desktop-linux-amd64.AppImage`, `tokenuse-desktop-linux-amd64.deb`, `tokenuse-desktop-linux-amd64.rpm` |
| Linux ARM64 | `tokenuse-desktop-linux-arm64.AppImage`, `tokenuse-desktop-linux-arm64.deb`, `tokenuse-desktop-linux-arm64.rpm` |

Each asset has a matching `.sha256` checksum file.

The macOS desktop release job signs with a Developer ID Application certificate, notarizes through App Store Connect, verifies the mounted DMG, and uploads the normalized artifact to the GitHub Release. Windows and Linux desktop assets are unsigned for now and should be verified with their checksum files before installing.

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
| `HOMEBREW_TAP_TOKEN` | Token with push access to `russmckendrick/homebrew-tap` |

Use a Developer ID Application certificate for direct-download DMGs. Apple Distribution is for App Store distribution, and Developer ID Installer is for `.pkg` installers.

## Homebrew Tap

After the GitHub Release is created, `.github/workflows/update-tap.yml` updates:

- `Formula/tokenuse.rb` for the TUI on macOS and Linux.
- `Casks/tokenuse-desktop.rb` for the macOS desktop DMG.
- `cask_renames.json` so existing installs of the old `tokenuse` cask migrate to `tokenuse-desktop`.

The tap downloads checksums from the newly published release before writing the formula and cask. Windows and Linux desktop assets are published only to GitHub Releases for now.
