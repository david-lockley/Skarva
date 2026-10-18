# Releasing Skarva

The version in `Cargo.toml` is the source of truth. The app UI, CLI and Windows executable metadata read it during compilation. Package names and generated macOS bundle metadata read it during packaging. macOS requires numeric OS version fields, so a prerelease such as `0.2.0-beta.1` uses `0.2.0` there and keeps the complete version in `SkarvaVersion` and the app UI.

## Choose a version

Current version: read `Cargo.toml` or run `python3 scripts/release.py version` (Python 3.11+).

- `0.1.1`: fixes to the current release.
- `0.2.0`: a new feature release while the app is still evolving.
- `0.2.0-beta.1`: a preview to test before `0.2.0`.
- `1.0.0`: a future milestone with a stable product and compatibility expectations.

The helper updates only Skarva's version in Cargo.toml and Cargo.lock:

```sh
python3 scripts/release.py set 0.1.0-beta.1
python3 scripts/release.py check
```

Update CHANGELOG.md and the release checklist as appropriate. Regenerate dependency notices if dependencies changed.

## Publish a release

Review the [release checklist](RELEASE_CHECKLIST.md), commit your changes, and push the reviewed commit to main. Then tag that exact commit:

```sh
git tag -a v0.1.0-beta.1 -m "Skarva 0.1.0-beta.1"
git push origin v0.1.0-beta.1
```

**Pushing a `v...` tag publishes a GitHub Release automatically after every job passes.** Do not push a version tag until you intend to publish. The tag must exactly match Cargo.toml and Cargo.lock; a mismatch fails before building. Tags containing a prerelease suffix are marked as GitHub prereleases and are not made Latest.

The workflow runs the Rust tests/lints, release-helper tests, and a dependency audit. It builds these native packages, uploads them, computes SHA-256 checksums, and attaches everything to the tagged release:

| Platform | Architecture | Formats |
| --- | --- | --- |
| macOS | Apple Silicon / arm64 | ZIP, DMG |
| macOS | Intel / x64 | ZIP, DMG |
| Windows | x64 | Portable ZIP, per-user NSIS installer EXE |
| Linux | x64 | tar.gz, Debian/Ubuntu DEB |

Main-branch pushes and pull requests run the checks and upload build artifacts without publishing. **Run workflow** on a branch also checks/builds without creating a release. A failed build or audit prevents publication. The workflow uses GitHub's built-in token; no personal GitHub token is needed. Release publishing needs Actions workflow write permissions to be allowed by repository/organization policy.

If a published release needs fixing, issue a new version instead of moving the old tag or replacing public assets. Do not use the version tag again for a different commit.

## Local packaging

- macOS: `sh scripts/package-macos-release.sh` builds ZIP and DMG for the current Mac's architecture.
- Windows: install NSIS (`choco install nsis`), then run `./scripts/package-windows.ps1`.
- Linux: run `sh scripts/package-linux.sh`; install `dpkg-deb` to also generate a DEB. The CI Ubuntu runner includes it.

The CI images build Windows x64, Linux x64, and separate macOS ARM/Intel apps. Linux ARM packages can be built natively with the Linux script, but are not part of the automated matrix. AppImage, RPM, MSI, automatic application updates, Windows signing, and macOS Developer ID/notarization are not configured.

macOS bundles are ad-hoc signed and Windows installers are unsigned. Adding installers does not remove OS trust warnings. Test real installation, upgrade, and uninstall behavior on each platform before advertising broad compatibility. User project data and credentials are kept outside the installation folder; the Windows uninstaller preserves them.
