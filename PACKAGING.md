# Packaging and releases

All native packages are produced by GitHub Actions from a `v*` tag; no
package is ever built on a developer machine and attached manually. The
shared tool is [native-packages](https://github.com/crmne/native-packages)
0.10.0, driven by [`native-packages.yaml`](native-packages.yaml).

## Inputs

| Path | Purpose |
|---|---|
| `packaging/applications/atlas-commerce-desktop.desktop` | Freedesktop entry, registers the `atlascommerce://` MIME type and `StartupWMClass`. |
| `packaging/icons/atlas-commerce-desktop.svg` | hicolor scalable icon. |
| `packaging/icons` + `packaging/macos/icon-1024.png` | Sources for the Windows `.ico` and the macOS `.icns`. |
| `packaging/windows/atlas-commerce-desktop.iss` | Inno Setup per-user installer, registers the URL protocol under `HKCU`. |
| `packaging/windows/build-msi.py` | WiX 5 per-user MSI generator (no C custom actions). |
| `packaging/macos/Info.plist` | Bundle template with `__IDENTIFIER__`/`__EXECUTABLE__`/`__VERSION__`/`__BUILD__` placeholders and the `atlascommerce` URL scheme. |
| `packaging/macos/autostart.plist` | Login-item agent passing `--start-hidden`. |
| `packaging/macos/bundle.sh`, `packaging/macos/dmg.sh` | `.app` bundling (ad-hoc or Developer ID) and DMG creation. |
| `packaging/atlas-commerce-desktop-portable.txt` | Marker: next to the executable, data stays in a local `atlas-commerce-desktop-data` folder. |

## CI

- **`ci.yml`** — `cargo fmt --check`, clippy with `-D warnings` (default and
  all features), rustdoc with `-D warnings`, the test suite on Ubuntu,
  macOS and Windows, a release build, and an OSV vulnerability scan of
  `Cargo.lock`.
- **`packaging.yml`** — validates `native-packages.yaml`, the shell
  packagers, the desktop entry and the Inno Setup script without cutting a
  release.
- **`dependabot.yml`** — weekly Cargo and GitHub Actions updates. The
  fastframe/egui git pins move deliberately, one tag at a time, and are not
  touched by Dependabot.

## Release flow (`release.yml`)

Two entry points run the same pipeline, which never executes `git push`:
a manual run creates its tag through the GitHub releases API.

- **Manual run** — Actions → *Release* → *Run workflow*. Inputs:
  - `repository`: source repo as `owner/name` or a full GitHub URL
    (defaults to `Etherealmber/atlas-commerce-desktop`);
  - `version`: e.g. `0.2.0` or `v0.2.0` (a suffix such as `-rc.1` marks a
    pre-release, optionally forced with the `prerelease` toggle);
  - `notes`: optional release notes; when left empty GitHub generates
    them from commits and merged pull requests.
- **Tag push** — pushing `vX.Y.Z` triggers the identical pipeline.

1. The `prepare` job validates the version, normalises the tag, resolves
   pre-release status and notes mode, and records everything in its job
   summary.
2. The matrix builds `x86_64`/`aarch64` Linux archives, Windows zip +
   Inno Setup installer + WiX MSI, and a universal macOS DMG from a
   `lipo` binary.
3. Stable versions additionally build AppImages with `.zsync` update
   information pointing at the newest stable GitHub release.
4. The `publish` job (environment `release-signing`) collects every
   artifact, adds the source archive, writes `update.json`, and generates
   `checksums.txt`.
5. When `UPDATE_SIGNING_ENABLED` is set, the checksums are signed with
   `ATLAS_UPDATE_SIGNING_KEY` and verified against
   `assets/update-public-key.hex`.
6. The GitHub CLI creates the release (with `--generate-notes` or the
   supplied notes) and uploads all assets; pre-releases never replace
   `latest`, so the app's stable update channel never offers them.

## Build summaries and artifacts

No workflow commits or pushes back to a branch. Each job appends a
Markdown report to the GitHub Actions *job summary* (toolchain, test
results, produced files, SHA-256 fingerprints) and uploads its captured
logs and packages as run artifacts: `quality-logs`, `test-logs-<os>`,
`vulnerability-logs` under CI, per-target archives and `macos-universal`
under releases, and the packaging validation logs.

## Application identity

- Bundle / App ID: `com.etherealmber.AtlasCommerceDesktop`
- Inno `AppId`: `{E74305A7-4960-45B6-93C9-CEA8D83B1517}` (never change)
- WiX `UpgradeCode`: `{07D6D2FA-2A3F-42BE-8938-B2AFC2A17C7B}` (never change)
- URL scheme: `atlascommerce://` (verify links and QR handoff)
