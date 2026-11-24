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

## Current pass (web palette)

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
