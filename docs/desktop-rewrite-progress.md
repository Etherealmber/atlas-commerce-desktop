# Atlas Desktop rewrite — work-in-progress report

This is a rough implementation estimate, not a measured completion or a release claim. The requested 70% milestone is **not reached**.

| Area | Approximate implementation progress | Current limitation |
| --- | ---: | --- |
| Shell, dock and theme | 60% | Floating surfaces, sidebar, responsive action row and partial dock behavior exist; focus routes, visual parity and all interactions need completion and verification. |
| Settings and updates | 45% | Modal, partial search, update check and observed-version history exist; sections and exact layout remain incomplete. CDN1 integration is now requested but not implemented; its publishing contract still needs confirmation. |
| ERP screens and workflows | 30% | Existing commerce screens plus partial attendance, inventory, cash shifts, mobile actions and read-only payroll; several workflows and route parity are missing. |
| Native v3 authentication | 20% | Auth core and isolated transport/account methods exist, but are not connected to login, OS key storage, session lifecycle or protected ERP requests. |
| Verification | 5% | CI Rust tests passed on Linux, macOS and Windows at 7839f55; Clippy and visual fixtures failed. Interactive Windows/provider testing remains outstanding. |
| Overall | roughly 35–40% | These estimates overlap and are subjective; they do not establish production readiness. |

## Follow-up — CI, screenshots and mandatory-update safety

Phase 1 remains approximately 40–50% written, not end-to-end verified; overall rewrite remains approximately 35–40%. These estimates do not imply working CDN installation.

- Merged current `main` into the PR branch, and enabled the reqwest 0.13 `query` feature (the fastframe updater alias is unchanged). [CI run for 14aad6c](https://github.com/Etherealmber/atlas-commerce-desktop/actions/runs/45463464534), [visual run and artifact page](https://github.com/Etherealmber/atlas-commerce-desktop/actions/runs/45463464618): in progress at time of writing; these runs predate the follow-up changes. Artifact availability is not yet confirmed.
- Screenshots are uploaded even after prior failure, review notices are best-effort, and the six required screenshots are checked after upload. Existing image assertions remain unchanged.
- An unavailable mandatory CDN install now shows a non-blocking error instead of trapping the user. 204/404 and empty Worker responses try direct CDN; added mock tests for that transition and invalid metadata. Full Worker→CDN→GitHub integration, secure staging and verified OS-specific handoff, progress and cancellation remain unfinished; CDN installation stays disabled. Interactive Windows installation has not been tested.
- CDN release upload retries and checks the last Worker response for all six matching filenames, hashes and sizes. The Worker publishes and purges sequentially, so a failure can expose an incomplete feed; retries after an ambiguous timeout can overlap a still-running request. A truly atomic publish requires a Worker-side staged/commit protocol. This workflow change was tested with a mock only; no release or real upload was made.
- Dark palette updates cover canvas, sidebar, cards, hairlines, inputs, hover and dashboard blue/amber series/grid. Table headers, all charts and screenshot parity still need review against PR #221; no parity claim is made.
- Follow-up commit `3b629a9`: [CI run](https://github.com/Etherealmber/atlas-commerce-desktop/actions/runs/45463819770) and [visual run / artifact page](https://github.com/Etherealmber/atlas-commerce-desktop/actions/runs/45463819751); both pending/in progress at the time recorded. The screenshot artifact has not yet been confirmed. The earlier `14aad6c` quality job passed; other builds were still running.
- Local Python unittest: 10 passed. Rust compiler/cargo are unavailable locally; fresh CI and cross-platform runtime checks remain required.

## Current pass — CDN discovery in progress

- Added Worker-first and direct-CDN discovery on supported Desktop OS/CPU pairs, bounded metadata reads, no redirect following, strict parsing and explicit error handling. GitHub remains the fallback for absent/unreachable Desktop sources. No GitHub ETag or credentials are sent to CDN1/Worker.
- Threaded CDN metadata into update state, the update dialog and Settings, with size, checksum tooltip and source. CDN installation is deliberately disabled with an explicit error; the existing GitHub installer cannot safely install the CDN artifact. Private staging, verified handoff, progress/cancellation and OS-specific replacement still need implementation and tests. A mandatory CDN manifest can block the app while this remains incomplete.
- Added mock local HTTP tests for request isolation, 503 and malformed metadata; full fallback and staging/handoff integration tests remain outstanding. New Rust changes were formatted but have not yet been compiled locally. Local Python unittest: 10 passed; pytest: 10 passed and 2 subtests.
- Previous `d0847da` CI: all checks passed except macOS test, whose runner could not be acquired due to GitHub hosted-runner capacity. Fresh CI is required for this pass; the PR remains unready to merge.

## CI snapshot at `6619f94`

GitHub Actions passed quality (fmt, Clippy, docs), tests on Ubuntu, macOS and Windows, offline desktop screenshots, Linux packaging inputs and WebView package checks, Windows universal installer, and vulnerability scan. The screenshot failure was fixed without removing the assertions. Python: 10 local tests passed, including the mocked upload. The release workflow was not run and no CDN upload or real installer test was performed. These green checks do **not** establish Phase 1 completion: discovery, UI metadata wiring and verified installation are still absent.

## Phase 1 — multi-OS pass, still incomplete

- Worker PR #218 merged as `6082c6d`; this establishes the repository contract, not proof of a particular live deployment. Added multi-OS API/latest.json parsing with strict platform and architecture checks, plus a bounded streaming SHA-256/size verifier. These components are not yet connected to network discovery or installation.
- Updated the optional post-release upload to require all six artifacts before any request and send them sequentially with the platform/arch/kind headers. An offline mocked-curl test verifies the six requests and token isolation from arguments. No release/upload has been performed; the Worker replaces previous-version files on the first new-version upload, so a later failure can leave an incomplete feed.
- Screenshot failure root cause: ImageMagick emitted `1.02398e+06` for the valid pixel difference, while the workflow only accepted integer syntax. The assertions now accept scientific notation without lowering thresholds. Fresh CI is required.
- Phase 1 rough implementation estimate: **35–45% written, not verified end-to-end**. CDN network selection/fallback, metadata display, secure staging/handoff and all-green CI are still missing.

## Previous Phase 1 pass — in progress, not yet verified

- Corrected the dark primary color and unused import. The previous screenshot failure is still under investigation; the workflow now preserves failure artifacts and reports the page rather than exiting silently if a wizard marker is missing. No visual assertion has been removed.
- Inspected Worker PR #217 head `060d3f8`: GitHub reports the PR **open**, so deployment status is not established by repository state. Implemented a standalone bounded metadata parser and tests for explicit absence, platform, SHA-256, URL, size and minimum-version handling. The parser is **not yet wired** to the updater; CDN discovery, fallback, display and verified installation are unfinished.
- Added an optional CDN1 upload after successful GitHub publication, restricted to the current repository; no release or upload was performed. The Worker deletes old Desktop objects when ingesting. Endpoint deployment and upload behavior are not verified here.
- The currently pinned fastframe updater expects its own signed feed format, not Worker `latest.json`, and does not expose the prepared installer path. Do not claim SHA-256 verification of the CDN artifact or launch it until the actual bytes can be verified before handoff.
- GitHub Actions for the current changes is still needed. Rust toolchain is not available in this sandbox. Python tests pass; Windows interactive tests remain outstanding.

## Prior pass (update dialog and notifications)

- Added a centered shell update dialog for available GitHub manifest metadata, with explicit install, progress, optional dismissal and an in-session mandatory lock. Pre-shell mandatory updates still use the existing gate. The manifest does not provide size or checksum details, which are explicitly marked unavailable; this does not implement CDN1 or claim a verified status before the updater verifies anything.
- The notifications popover now uses a count of loaded alerts, known Worker alert-kind icon tiles, dividers and a footer link. Navigation continues to use the existing route allowlist and role checks. Mobile-action rows still need parity.
- Added fictional light/dark update-dialog screenshot targets and Escape checks, pending CI execution. No real Windows interaction or update install was performed.

## Prior pass (CI and update version validation)

- At `fe98565`, compilation failed on all CI platforms because the skeleton's `div_ceil` input was ambiguous. Added an explicit `usize` type. Addressed previously reported Clippy findings without suppressing lints; fresh CI is needed to discover any remaining findings.
- Update decisions now use strict SemVer precedence, including prerelease ordering and ignoring build metadata; invalid versions and inconsistent minimum versions produce an explicit error instead of an up-to-date result. Added tests, not yet run under Cargo.
- The earlier visual fixture failed during the Xvfb rendering step; the available check annotations do not identify the failing assertion. Screenshots and interactive Windows tests remain unverified.

## Prior pass (web palette)

- Updated central dark and light palette tokens to the requested zinc/slate values and removed the Settings modal's heavy dark shadow. Settings already uses a centered scrim, close control and Escape handling; full web parity remains incomplete.
- CI at `7839f55`: platform tests, vulnerability scan and Windows installer passed; quality (Clippy) and offline screenshots failed. Some Clippy findings have been addressed locally, but multiple collapsible-if findings remain. No visual or interactive Windows verification has been performed for this pass.
- CDN1 and release upload are now in scope, but not implemented: the admin endpoint payload, authorization, atomic publication and rollback contract are not established. No installer execution or v3 lifecycle wiring has been claimed.

## Latest pass

- Audited Worker main at `3b0e8c6` and wrote `docs/prompt-backend-auth-v3-followup.md`: existing v3 routes, deployment prerequisites, ambiguities to resolve, tests, and Desktop-only remaining work.
- Balance now uses the actual Worker dashboard fields for four treasury metrics instead of presenting partial ledger totals as a balance. Missing or restricted values render as unavailable; the UI notes the Worker dashboard may be cached. Refresh loads both dashboard and ledger.
- Added a Rust test for aggregate completeness and privacy, French strings and a four-metric visual skeleton. These Rust/visual tests remain unrun.

## Prior pass

- Added an admin-only read-only payroll overview backed by Worker runs, detail and settings endpoints. Values are supplied by the server; no local statutory calculation is implied. Creating, editing, closing and exporting payroll runs remain web-only.
- Added a scanner entry to the sidebar using the existing on-demand Products scanner; it is not a separate scan route.
- Expanded mobile-action details with selected stock metadata and rejection context, without rendering raw metadata or image data. Specialized approvals remain web-only; uncertain write outcomes must be checked against the direct detail endpoint.
- Added fictional payroll/mobile-actions fixture targets, translations, policy/contract tests and responsive payroll metrics. The new Rust tests and visual fixture targets are not yet executed.

## Checks performed

- WASM Rust formatter on files changed in this pass (formatting is not compilation).
- `python3 -m unittest discover -s tests -p 'test_*.py'`: 9 passed.
- Cargo manifests parsed with Python `tomllib`.
- `git diff --check`: passed.
- At the time of this report's checks, no push, deployment, production-provider test or Windows test had been performed.

## Modified tracked files

- `.github/workflows/release.yml`
- `.github/workflows/visual-fixtures.yml`
- `Cargo.toml`
- `crates/api/Cargo.toml`
- `crates/api/src/agent.rs`
- `crates/api/src/lib.rs`
- `crates/api/src/models.rs`
- `crates/app/src/main.rs`
- `crates/core/Cargo.toml`
- `crates/core/src/settings.rs`
- `crates/core/src/vault.rs`
- `crates/core/src/vault_authority.rs`
- `crates/core/src/webview_policy.rs`
- `crates/ui/Cargo.toml`
- `crates/ui/assets/i18n/fr.po`
- `crates/ui/examples/visual_fixture.rs`
- `crates/ui/src/app.rs`
- `crates/ui/src/branding.rs`
- `crates/ui/src/buttons.rs`
- `crates/ui/src/client_report.rs`
- `crates/ui/src/dialogs.rs`
- `crates/ui/src/employees.rs`
- `crates/ui/src/native_auth_v2.rs`
- `crates/ui/src/product_images.rs`
- `crates/ui/src/screens/agent.rs`
- `crates/ui/src/screens/auth.rs`
- `crates/ui/src/screens/catalog.rs`
- `crates/ui/src/screens/clients.rs`
- `crates/ui/src/screens/dashboard.rs`
- `crates/ui/src/screens/delivery_notes.rs`
- `crates/ui/src/screens/employees.rs`
- `crates/ui/src/screens/expenses.rs`
- `crates/ui/src/screens/mod.rs`
- `crates/ui/src/screens/notifications.rs`
- `crates/ui/src/screens/products.rs`
- `crates/ui/src/screens/sales.rs`
- `crates/ui/src/screens/settings.rs`
- `crates/ui/src/screens/shell.rs`
- `crates/ui/src/screens/shell/sidebar.rs`
- `crates/ui/src/screens/shell/topbar.rs`
- `crates/ui/src/screens/vendors.rs`
- `crates/ui/src/shortcuts.rs`
- `crates/ui/src/state.rs`
- `crates/ui/src/theme.rs`
- `crates/ui/src/tray.rs`
- `crates/ui/src/update.rs`
- `crates/ui/src/widgets.rs`
- `packaging/icons/atlas-commerce-desktop.svg`
- `packaging/windows/atlas-commerce-desktop.ico`
- `scripts/release-notes.py`

## New untracked files

- `crates/api/src/attendance.rs`
- `crates/api/src/auth_v3.rs`
- `crates/api/src/auth_v3_account.rs`
- `crates/api/src/cash_shifts.rs`
- `crates/api/src/inventory_ops.rs`
- `crates/api/src/mobile_actions.rs`
- `crates/api/src/payroll.rs`
- `crates/auth-core/Cargo.toml`
- `crates/auth-core/UPSTREAM.md`
- `crates/auth-core/examples/contract_driver.rs`
- `crates/auth-core/src/dpop.rs`
- `crates/auth-core/src/error.rs`
- `crates/auth-core/src/lib.rs`
- `crates/auth-core/src/origin.rs`
- `crates/auth-core/src/pkce.rs`
- `crates/auth-core/src/schemas.rs`
- `crates/auth-core/src/state.rs`
- `crates/auth-core/tests/contract_vectors.rs`
- `crates/ui/src/screens/actions.rs`
- `crates/ui/src/screens/attendance.rs`
- `crates/ui/src/screens/cash_shifts.rs`
- `crates/ui/src/screens/inventory_ops.rs`
- `crates/ui/src/screens/payroll.rs`
- `crates/ui/src/screens/shell/top_page_actions.rs`
- `docs/desktop-rewrite-progress.md`
- `docs/prompt-backend-auth-v3-followup.md`
- `docs/prompt-backend-cash-shifts.md`
- `docs/prompt-backend-desktop-updates.md`
- `docs/prompt-backend-inventory.md`
- `docs/prompt-backend-login-natif.md`
- `tests/test_release_notes.py`
