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

## Desktop DMG

Tagged releases also build a universal macOS desktop DMG:

```text
tokenuse-desktop-macos-universal.dmg
tokenuse-desktop-macos-universal.dmg.sha256
```

The desktop release job signs with a Developer ID Application certificate, notarizes through App Store Connect, verifies the mounted DMG, and uploads the normalized artifact to the GitHub Release.

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
- `Casks/tokenuse.rb` for the macOS desktop DMG.

The tap downloads checksums from the newly published release before writing the formula and cask.
