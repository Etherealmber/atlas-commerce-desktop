# Packaging and releases

All native packages are produced by GitHub Actions from a `v*` tag; no
package is ever built on a developer machine and attached manually. The
shared tool is [native-packages](https://github.com/crmne/native-packages)
0.10.0, driven by [`native-packages.yaml`](native-packages.yaml).

## Inputs

| Path | Purpose |
|---|---|
| `packaging/applications/atlas-commerce-desktop.desktop` | Freedesktop entry, registers the `atlascommerce://` MIME type and `StartupWMClass`. |
| `packaging/icons/atlas-commerce-desktop.svg` | hicolor scalable icon, the monochrome brand mark. |
| `packaging/icons` + `packaging/macos/icon-1024.png` | Sources for the Windows `.ico` and the macOS `.icns`. |
| `scripts/generate-icons.py` | Regenerates the SVG, 1024 px PNG and multi-size `.ico` from the shared monochrome design (Pillow). |
| `packaging/windows/atlas-commerce-desktop.iss` | Universal Inno Setup per-user installer: one `setup.exe` carrying the x64 and ARM64 binaries, installs the matching one via `IsX64`/`IsARM64`, registers the URL protocol under `HKCU`. |
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
  packagers, the desktop entry and the universal Inno Setup script without
  cutting a release. Its Windows build reuses the `Swatinem/rust-cache`
  family; release binaries themselves are never built from a restored
  cache.
- **`dependabot.yml`** — weekly Cargo and GitHub Actions updates. The
  fastframe/egui git pins move deliberately, one tag at a time, and are not
  touched by Dependabot.

## Release flow (`release.yml`)

One pipeline, production only — no beta/rc channel. It never executes
`git push`: a manual run creates its tag through the GitHub releases API
and every release is promoted to *latest*.

- **Manual run** — Actions → *Release* → *Run workflow*. Inputs:
  - `repository`: source repo as `owner/name` or a full GitHub URL
    (defaults to `Etherealmber/atlas-commerce-desktop`);
  - `version`: stable `x.y.z` only (e.g. `0.2.0`); it must already match
    the workspace `Cargo.toml` version, since the shipped binary reports
    that version;
  - `release_notes`: optional French notes; when left empty
    `scripts/release-notes.py` summarises the commits since the previous
    tag (same generator as atlas-commerce-android);
  - `mandatory`: marks the release as a blocking update in `update.json`;
  - `publish`: uncheck to build every installer and keep them as run
    artifacts without creating a GitHub Release — the way to produce and
    download a universal setup `.exe` on demand.
- **Tag push** — pushing `vX.Y.Z` triggers the identical pipeline with
  publishing on.

1. The `prepare` job validates the strict `x.y.z` version against
   `Cargo.toml`, normalises the tag, and records every choice in its job
   summary.
2. The matrix builds `x86_64`/`aarch64` Linux archives plus AppImages with
   `.zsync`, and Windows `x86_64`/`arm64` portable zips. A dedicated
   `windows-installer` job downloads both Windows zips and compiles **one
   universal Inno Setup installer (`*-setup.exe`)** that places the correct
   binary on Intel/AMD or ARM machines — users never pick an architecture.
   Another separate job builds the universal macOS DMG from a `lipo` binary.
   There is no MSI: the Inno installer covers per-user installs and updates.
3. The `publish` job (environment `release-signing`, skipped when
   `publish` is unchecked) generates the notes, creates the release via
   the API, adds the source archive, writes `update.json` (including the
   mandatory flag), and generates `checksums.txt`.
4. When `UPDATE_SIGNING_ENABLED` is set, the checksums are signed with
   `ATLAS_UPDATE_SIGNING_KEY` and verified against
   `assets/update-public-key.hex`.
5. The GitHub CLI uploads all assets with `--clobber` and keeps the
   release on *latest*.

## Build summaries and artifacts

No workflow commits or pushes back to a branch. Each job appends a
Markdown report to the GitHub Actions *job summary* (toolchain, test
results, produced files, SHA-256 fingerprints), emits a one-line
`::notice::`/`::error::` annotation mirroring that result (readable
through the check-run API), and uploads its captured logs and packages
as run artifacts: `quality-logs`, `test-logs-<os>`,
`vulnerability-logs` under CI, per-target archives, `windows-universal`
and `macos-universal` under releases, and the packaging validation logs.
Release summaries also list every step outcome and, on failure, point at
the first error block; the CI failure bot files an issue with the
diagnosis and the fix guidance.

## Application identity

- Bundle / App ID: `com.etherealmber.AtlasCommerceDesktop`
- Inno `AppId`: `{E74305A7-4960-45B6-93C9-CEA8D83B1517}` (never change)
- URL scheme: `atlascommerce://` (verify links and QR handoff)
