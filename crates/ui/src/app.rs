//! Application lifecycle: the eframe wrapper, the fastframe shell `Resident`
//! implementation, session bootstrap, background polling and self-update.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use atlas_core::update_feed::UpdateDecision;
use atlas_core::{SessionRecord, UpdateManifest};
use egui::{Context, Ui};
use fastframe_shell::{Headless, Held, Lease, Resident};

use crate::screens;
use crate::state::{AtlasApp, Route, Screen};
use crate::t;
use crate::theme::Palette;
use crate::tray::TrayCommand;

/// Choose the screen to resume after a mandatory update is withdrawn.
fn update_gate_destination(
    restoring: bool,
    signed_in: bool,
    has_server: bool,
    route: Route,
) -> Screen {
    if restoring {
        Screen::Splash
    } else if signed_in {
        Screen::Shell(route)
    } else if has_server {
        Screen::Login
    } else {
        Screen::ServerSetup
    }
}

/// A staged download may only be advertised for its own manifest version.
fn staged_version_matches(staged: Option<&str>, manifest: &str) -> bool {
    staged == Some(manifest)
}

impl AtlasApp {
    /// Opens a session: persist it, remember the account, rebuild the
    /// authenticated client and load the first shell data.
    pub fn enter_session(
        &mut self,
        token: String,
        username: String,
        role: String,
        display_name: String,
        avatar_url: String,
    ) {
        // A legacy login must supersede any previous v1 vault authority.
        // Delete the old active legacy record before making it authoritative,
        // so an interrupted replacement cannot revive the prior account.
        if let Some(server) = self.server_store.selected_url().map(str::to_owned)
            && server.starts_with("https://")
        {
            let deleted = match std::fs::remove_file(self.paths.session_file()) {
                Ok(()) => true,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
                Err(_) => false,
            };
            if !deleted || self.vault_worker.legacy(&server).is_err() {
                self.notice(
                    "La restauration de la session ne peut pas être garantie sur cet appareil."
                        .to_owned(),
                );
            }
        }
        self.enter_session_with_persistence(token, username, role, display_name, avatar_url, true);
    }

    /// Enters a v1 browser session and queues its OS-vault persistence.
    pub(crate) fn enter_browser_session(&mut self, token: String, username: String, role: String) {
        self.enter_session_with_persistence(
            token,
            username,
            role,
            String::new(),
            String::new(),
            false,
        );
    }

    fn enter_session_with_persistence(
        &mut self,
        token: String,
        username: String,
        role: String,
        display_name: String,
        avatar_url: String,
        persist: bool,
    ) {
        self.transient_session = !persist;
        self.requires_password = false;
        self.login.password.clear();
        let server_url = self
            .server_store
            .selected_url()
            .unwrap_or_default()
            .to_owned();
        let record = SessionRecord {
            server_url,
            username: username.to_lowercase(),
            role: if role.is_empty() {
                "user".to_owned()
            } else {
                role
            },
            display_name: if display_name.is_empty() {
                username
            } else {
                display_name
            },
            email: String::new(),
            avatar_url,
            token,
        };
        // A password login and a QR approval can race. Once either succeeds,
        // discard all other pending authentication jobs and their secrets so
        // a late completion cannot replace the active account.
        self.login = Default::default();
        self.qr = Default::default();
        self.session = Some(record.clone());
        if !persist {
            match self.vault_worker.save(record.clone()) {
                Ok(result) => self.vault_saves.push(result),
                Err(_) => self.notice(
                    "Coffre indisponible : session conservée uniquement en mémoire.".to_owned(),
                ),
            }
        }
        if persist {
            self.account_store.upsert(&record);
            self.save_session();
            self.save_accounts();
        }
        self.rebuild_client();
        self.bootstrap_shell();
    }

    /// Begins a bounded, asynchronous restoration after startup selected a v1 authority.
    pub fn begin_vault_restore(&mut self, server: String) {
        match self.vault_worker.load(server.clone()) {
            Ok(receiver) => {
                self.screen = Screen::Splash;
                self.vault_restore = Some((server, Instant::now(), receiver));
            }
            Err(_) => self.notice("Coffre indisponible : reconnectez-vous.".to_owned()),
        }
    }

    fn poll_vault(&mut self) {
        use std::sync::mpsc::TryRecvError;
        if let Some((server, started, receiver)) = self.vault_restore.take() {
            match receiver.try_recv() {
                Ok(Ok(Some(session)))
                    if matches!(self.screen, Screen::Splash)
                        && self.session.is_none()
                        && self.server_store.selected_url() == Some(server.as_str()) =>
                {
                    self.session = Some(session);
                    self.transient_session = true;
                    self.rebuild_client();
                }
                Ok(Ok(_)) => {}
                Ok(Err(_)) | Err(TryRecvError::Disconnected) => {
                    self.notice("Coffre indisponible : reconnectez-vous.".to_owned());
                }
                Err(TryRecvError::Empty) if started.elapsed() < Duration::from_secs(10) => {
                    self.vault_restore = Some((server, started, receiver));
                }
                Err(TryRecvError::Empty) => {
                    self.notice("Le coffre ne répond pas ; reconnectez-vous.".to_owned());
                }
            }
        }
        let mut save_error = false;
        self.vault_saves
            .retain(|receiver| match receiver.try_recv() {
                Ok(Ok(_)) => false,
                Ok(Err(_)) | Err(TryRecvError::Disconnected) => {
                    save_error = true;
                    false
                }
                Err(TryRecvError::Empty) => true,
            });
        if save_error {
            self.notice("Coffre indisponible : la session reste en mémoire seulement.".to_owned());
        }
        let mut removal_error = false;
        self.vault_removals
            .retain(|receiver| match receiver.try_recv() {
                Ok(Ok(())) => false,
                Ok(Err(_)) | Err(TryRecvError::Disconnected) => {
                    removal_error = true;
                    false
                }
                Err(TryRecvError::Empty) => true,
            });
        if removal_error {
            self.notice("La suppression du secret dans le coffre a échoué ; la restauration locale est bloquée.".to_owned());
        }
    }

    /// First loads after entering the shell or restoring a session.
    ///
    /// On-demand only: the landing dashboard is the single automatic call,
    /// so startup does not fan out one request per screen. Every other cache
    /// is fetched the first time its route opens (`ensure_route_data`) or
    /// when a chrome surface needs it (bell badge, profile popover). The
    /// update check stays, gated by its own setting.
    pub fn bootstrap_shell(&mut self) {
        self.load_profile();
        self.load_avatar();
        let route = self.role_policy().initial_route();
        self.screen = Screen::Shell(route);
        if route == Route::Dashboard {
            self.load_dashboard();
        } else {
            self.ensure_route_data(route);
        }
        // Native stock toasts need alert data even if the user never opens
        // the Notifications screen; only spend the request when the
        // persisted setting is enabled.
        if self.settings.notifications_enabled
            && self.role_policy().allows_route(Route::Notifications)
        {
            self.load_alerts();
        }
        self.update.cache = crate::update::UpdateCache::load(&self.paths.update_state_file());
        let cached_manifest = match self.settings.update_channel {
            atlas_core::settings::UpdateChannel::Stable => {
                self.update.cache.manifest_stable.clone()
            }
            atlas_core::settings::UpdateChannel::Prerelease => {
                self.update.cache.manifest_prerelease.clone()
            }
        };
        if let Some(manifest) = cached_manifest {
            self.apply_manifest(manifest);
        }
        self.schedule_update_checks();
        self.maybe_auto_check_updates();
    }

    // ── Data loaders ─────────────────────────────────────────────────────

    pub fn load_dashboard(&mut self) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let job = self.runner.spawn(async move { client.dashboard().await });
        self.dashboard.start(job);
    }

    pub fn load_products(&mut self) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let query = self.product_search.clone();
        let job = self
            .runner
            .spawn(async move { client.products(&query, 300).await });
        self.products.start(job);
    }

    pub fn load_clients(&mut self) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let job = self.runner.spawn(async move { client.clients().await });
        self.clients.start(job);
    }

    pub fn load_vendors(&mut self) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let job = self.runner.spawn(async move { client.vendors().await });
        self.vendors.start(job);
    }

    pub fn load_employees(&mut self) {
        if !self.role_policy().allows_route(Route::Employees) {
            return;
        }
        let Some(api) = self.client.clone() else {
            return;
        };
        let status = self.employee_status.clone();
        let query = self.employee_search.clone();
        let category = self.employee_category.clone();
        self.employees.start(
            self.runner.spawn(async move {
                crate::employees::fetch(&api, &status, &query, &category).await
            }),
        );
    }

    pub(crate) fn load_employee_detail(&mut self, id: i64) {
        if id <= 0 || !self.role_policy().allows_route(Route::Employees) {
            return;
        }
        let Some(api) = self.client.clone() else {
            return;
        };
        self.employee_detail_id = Some(id);
        self.employee_detail.start(
            self.runner
                .spawn(async move { crate::employees::detail(&api, id).await }),
        );
    }

    /// Mints a short-lived ticket and opens one allowlisted printable
    /// document in the system browser. The ticket is never logged.
    pub(crate) fn open_printable(&mut self, path: &str, params: &[(&str, String)]) {
        if path == "/api/reports/stock" && !self.role_policy().can_view_reports() {
            return;
        }
        if !crate::employees::printable_path(path) {
            self.notice("Document imprimable non autorisé.".to_owned());
            return;
        }
        let Some(api) = self.client.clone() else {
            return;
        };
        if self.report_job.is_some() {
            return;
        }
        let path = path.to_owned();
        let params: Vec<(String, String)> = params
            .iter()
            .map(|(key, value)| ((*key).to_owned(), value.clone()))
            .collect();
        let locale = self.locale;
        let job = self.runner.spawn(async move {
            if !crate::employees::printable_path(&path) {
                return Err(atlas_api::ApiError::client(
                    t!(locale, "Invalid report link.").into_owned(),
                ));
            }
            let ticket = api.report_link_ticket().await?;
            let mut url =
                url::Url::parse(&format!("{}{path}", api.base_url())).map_err(|error| {
                    log::warn!("invalid printable link: {error}");
                    atlas_api::ApiError::client(t!(locale, "Invalid report link.").into_owned())
                })?;
            {
                let mut query = url.query_pairs_mut();
                query.append_pair("rt", &ticket.rt);
                for (key, value) in &params {
                    if !value.is_empty() {
                        query.append_pair(key, value);
                    }
                }
            }
            Ok(url.to_string())
        });
        self.report_job = Some(job);
    }

    pub(crate) fn pay_employee(&mut self) {
        let parsed = self.employee_pay.as_mut().and_then(|form| {
            if form.saving {
                return None;
            }
            let amount = form
                .amount
                .trim()
                .replace(',', ".")
                .parse::<f64>()
                .ok()
                .filter(|value| value.is_finite() && *value > 0.0);
            match amount {
                Some(amount) => {
                    form.saving = true;
                    form.error = None;
                    Some((
                        form.employee_id,
                        amount,
                        form.date.trim().to_owned(),
                        form.note.trim().to_owned(),
                    ))
                }
                None => {
                    form.error = Some("Montant de paiement invalide.".to_owned());
                    None
                }
            }
        });
        let Some((id, amount, date, note)) = parsed else {
            return;
        };
        let Some(api) = self.client.clone() else {
            if let Some(form) = self.employee_pay.as_mut() {
                form.saving = false;
            }
            return;
        };
        self.employee_action = Some(
            self.runner
                .spawn(async move { crate::employees::pay(&api, id, amount, &date, &note).await }),
        );
    }

    pub(crate) fn advance_employee(&mut self, id: i64, amount: f64, note: &str) {
        if !self.role_policy().allows_route(Route::Employees) {
            return;
        }
        let Some(api) = self.client.clone() else {
            return;
        };
        let note = note.to_owned();
        self.employee_action = Some(
            self.runner
                .spawn(async move { crate::employees::advance(&api, id, amount, &note).await }),
        );
    }

    pub(crate) fn resign_employee(&mut self) {
        let parsed = self.employee_resign.as_mut().and_then(|form| {
            if form.saving {
                return None;
            }
            let reason = form.reason.trim().to_owned();
            if reason.is_empty() || reason.chars().count() > 200 {
                form.error = Some("Motif de démission invalide.".to_owned());
                return None;
            }
            form.saving = true;
            form.error = None;
            Some((form.employee_id, reason))
        });
        let Some((id, reason)) = parsed else {
            return;
        };
        let Some(api) = self.client.clone() else {
            if let Some(form) = self.employee_resign.as_mut() {
                form.saving = false;
            }
            return;
        };
        self.employee_action = Some(
            self.runner
                .spawn(async move { crate::employees::resign(&api, id, &reason).await }),
        );
    }

    pub(crate) fn reactivate_employee(&mut self, id: i64) {
        if !self.role_policy().allows_route(Route::Employees) {
            return;
        }
        let Some(api) = self.client.clone() else {
            return;
        };
        self.employee_action = Some(
            self.runner
                .spawn(async move { crate::employees::reactivate(&api, id).await }),
        );
    }

    pub(crate) fn create_employee(&mut self) {
        let Some(body) = self.employee_new.as_mut().and_then(|form| {
            if form.saving {
                return None;
            }
            let salary = form
                .salary
                .trim()
                .replace(',', ".")
                .parse::<f64>()
                .unwrap_or(0.0);
            let days = form.cycle_days.trim().parse::<i64>().unwrap_or(7);
            form.saving = true;
            form.error = None;
            Some(crate::employees::new_employee(
                form.first_name.trim(),
                form.last_name.trim(),
                form.phone.trim(),
                form.email.trim(),
                form.role_title.trim(),
                form.department.trim(),
                form.category.trim(),
                form.national_id.trim(),
                form.city.trim(),
                salary,
                if form.days_cycle { "days" } else { "monthly" },
                days,
                form.notes.trim(),
            ))
        }) else {
            return;
        };
        let Some(api) = self.client.clone() else {
            if let Some(form) = self.employee_new.as_mut() {
                form.saving = false;
            }
            return;
        };
        self.employee_action = Some(
            self.runner
                .spawn(async move { crate::employees::create(&api, &body).await }),
        );
    }

    fn poll_employee_jobs(&mut self) {
        let Some(result) = self.employee_action.as_ref().and_then(|job| job.poll()) else {
            return;
        };
        self.employee_action = None;
        match result {
            Ok(action) => {
                if !action.message.is_empty() {
                    self.notice(action.message);
                }
                self.employee_new = None;
                self.employee_pay = None;
                self.employee_resign = None;
                self.load_employees();
                if let Some(id) = self.employee_detail_id {
                    self.load_employee_detail(id);
                }
            }
            Err(error) => screens::employees::action_failed(self, &error),
        }
    }

    pub(crate) fn reconcile_bank_csv(&mut self) {
        if !self.role_policy().allows_route(Route::Expenses) || self.bank_reconciliation.loading {
            return;
        }
        let Some(client) = self.client.clone() else {
            return;
        };
        let csv = self.bank_csv.clone();
        self.bank_reconciliation.start(
            self.runner.spawn(async move {
                crate::client_invoices::reconcile_bank_csv(&client, csv).await
            }),
        );
    }

    pub(crate) fn load_ledger_reconciliation(&mut self) {
        if !self.role_policy().allows_route(Route::Expenses) {
            return;
        }
        let Some(client) = self.client.clone() else {
            return;
        };
        self.ledger_reconciliation
            .start(self.runner.spawn(async move {
                crate::client_invoices::fetch_ledger_reconciliation(&client).await
            }));
    }

    pub(crate) fn load_bills(&mut self) {
        if !self.role_policy().allows_route(Route::Expenses) {
            return;
        }
        let Some(client) = self.client.clone() else {
            return;
        };
        self.bills.start(
            self.runner
                .spawn(async move { crate::client_invoices::fetch_bills(&client).await }),
        );
    }

    pub fn load_expenses(&mut self) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let job = self.runner.spawn(async move { client.expenses().await });
        self.expenses.start(job);
    }

    pub fn load_delivery_notes(&mut self) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let job = self
            .runner
            .spawn(async move { client.delivery_notes().await });
        self.delivery_notes.start(job);
    }

    pub fn load_categories(&mut self) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let job = self.runner.spawn(async move { client.categories().await });
        self.categories.start(job);
    }

    pub fn load_units(&mut self) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let job = self.runner.spawn(async move { client.units().await });
        self.units.start(job);
    }

    pub fn load_invoices(&mut self) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let job = self
            .runner
            .spawn(async move { client.invoices("all").await });
        self.invoices.start(job);
    }

    pub(crate) fn load_receipts(&mut self) {
        if !self.role_policy().allows_route(Route::Sales) {
            return;
        }
        let Some(client) = self.client.clone() else {
            return;
        };
        self.receipts.start(
            self.runner
                .spawn(async move { crate::client_invoices::fetch_receipts(&client).await }),
        );
    }

    pub fn load_alerts(&mut self) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let job = self.runner.spawn(async move { client.alerts().await });
        self.alerts.start(job);
    }

    pub fn load_nav_badges(&mut self) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let job = self.runner.spawn(async move { client.nav_badges().await });
        self.nav_badges.start(job);
    }

    /// Lazy fetch of the title-bar badge count. The first call starts a
    /// request immediately; after a failure (`data` still `None`) retries are
    /// rate-limited by [`crate::state::NAV_BADGES_RETRY_BACKOFF`] so a
    /// persistently failing endpoint cannot trigger one request per frame.
    pub fn ensure_nav_badges(&mut self) {
        if !self.role_policy().allows_route(Route::Notifications) {
            return;
        }
        let backoff_open = self
            .nav_badges_retry_at
            .is_none_or(|at| Instant::now() >= at);
        if self.session.is_some()
            && self.nav_badges.data.is_none()
            && !self.nav_badges.loading
            && backoff_open
        {
            self.load_nav_badges();
            self.nav_badges_retry_at =
                Some(Instant::now() + crate::state::NAV_BADGES_RETRY_BACKOFF);
        }
    }

    pub fn load_devices(&mut self) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let job = self.runner.spawn(async move { client.devices().await });
        self.devices.start(job);
    }

    // ── Profile / integrations ───────────────────────────────────────────

    /// Loads the current account's picture from the documented same-server
    /// proxy. No third-party avatar URL receives the bearer token.
    fn load_avatar(&mut self) {
        let Some(client) = self.client.as_ref() else {
            return;
        };
        let Some(token) = client.token() else {
            return;
        };
        let base = client.base_url().to_owned();
        let fingerprint = client.fingerprint().to_owned();
        self.avatar_job = Some(self.runner.spawn(async move {
            let http = reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(15))
                .build()
                .ok()?;
            let mut response = http
                .get(format!("{base}/api/me/avatar"))
                .bearer_auth(token)
                .header("X-Atlas-Client", "native")
                .header("X-Device-Fingerprint", fingerprint)
                .send()
                .await
                .ok()?
                .error_for_status()
                .ok()?;
            let mime = response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)?
                .to_str()
                .ok()?
                .split(';')
                .next()?
                .trim();
            let extension = match mime {
                "image/png" => "png",
                "image/jpeg" => "jpg",
                "image/webp" => "webp",
                "image/gif" => "gif",
                "image/svg+xml" => "svg",
                _ => return None,
            };
            let uri = format!("bytes://atlas-profile-avatar.{extension}");
            const LIMIT: usize = 4 * 1024 * 1024;
            if response
                .content_length()
                .is_some_and(|size| size > LIMIT as u64)
            {
                return None;
            }
            let mut bytes = Vec::new();
            while let Some(chunk) = response.chunk().await.ok()? {
                if bytes.len().saturating_add(chunk.len()) > LIMIT {
                    return None;
                }
                bytes.extend_from_slice(&chunk);
            }
            Some((uri, bytes))
        }));
    }

    pub fn load_profile(&mut self) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let job = self.runner.spawn(async move { client.me().await });
        self.profile.start(job);
    }

    /// Pushes the form through `PATCH /api/auth/me`.
    pub fn save_profile(&mut self) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let patch = atlas_api::models::MeUpdate {
            display_name: Some(self.profile_form.display_name.trim().to_owned()),
            email: Some(self.profile_form.email.trim().to_owned()),
            phone: Some(self.profile_form.phone.trim().to_owned()),
            notes: Some(self.profile_form.notes.trim().to_owned()),
            avatar_url: Some(self.profile_form.avatar_url.trim().to_owned()),
        };
        let job = self
            .runner
            .spawn(async move { client.update_me(&patch).await });
        self.profile_save_job = Some(job);
    }

    pub fn load_api_keys(&mut self) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let job = self
            .runner
            .spawn(async move { client.list_api_keys().await });
        self.api_keys.start(job);
    }

    /// Creates a key from the open editor; the raw secret comes back once and
    /// is kept for the reveal modal.
    pub fn create_api_key(&mut self) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let Some(editor) = self.key_editor.clone() else {
            return;
        };
        let Some(scopes) = editor.scopes else {
            return;
        };
        let name = editor.name.trim().to_owned();
        if name.is_empty() {
            return;
        }
        let job = self
            .runner
            .spawn(async move { client.create_api_key(&name, scopes.as_str()).await });
        self.key_create_job = Some(job);
    }

    pub(crate) fn search_business(&mut self, query: String) {
        let Some(api) = self.client.clone() else {
            return;
        };
        let policy = self.role_policy();
        let access = crate::business_search::SearchAccess {
            products: policy.allows_route(Route::Products),
            clients: policy.allows_route(Route::Clients),
            sales: policy.allows_route(Route::Sales),
            vendors: policy.allows_route(Route::Vendors),
            delivery: policy.allows_route(Route::DeliveryNotes),
            bills: policy.allows_route(Route::Expenses),
        };
        self.business_search_open = true;
        self.business_search_query = query.clone();
        self.business_search = Default::default();
        self.business_search.start(
            self.runner
                .spawn(async move { crate::business_search::search(&api, query, access).await }),
        );
    }

    /// Read an individual invoice with its lines without opening a browser.
    pub(crate) fn open_invoice_detail(&mut self, id: i64) {
        if id <= 0
            || !(self.role_policy().allows_route(Route::Sales)
                || self.role_policy().allows_route(Route::Clients))
        {
            return;
        }
        let Some(api) = self.client.clone() else {
            return;
        };
        self.invoice_detail_id = Some(id);
        self.invoice_detail = Default::default();
        self.invoice_detail.start(
            self.runner
                .spawn(async move { crate::client_invoices::fetch_detail(&api, id).await }),
        );
    }

    /// Load the server-filtered list into the native client detail panel.
    pub fn open_client_invoices(&mut self, client: &atlas_api::models::Client) {
        let Some(api) = self.client.clone() else {
            return;
        };
        let id = client.id;
        if self.client_invoice_id != Some(id) {
            self.invoice_detail_id = None;
            self.invoice_detail = Default::default();
        }
        self.client_invoice_id = Some(id);
        self.client_invoices = Default::default();
        self.client_invoices.start(
            self.runner
                .spawn(async move { crate::client_invoices::fetch(&api, id).await }),
        );
    }

    /// Optional system-browser link to the web client's filtered list.
    pub fn open_client_invoices_in_browser(&mut self, client: &atlas_api::models::Client) {
        let Some(api) = self.client.clone() else {
            return;
        };
        // Only the stable client ID goes in the query: a `q` name search
        // would hide older invoices after the client is renamed (the web
        // page treats `q` as an additional text filter, not part of the key).
        let url = match url::Url::parse_with_params(
            &format!("{}/invoices", api.base_url()),
            &[("client", client.id.to_string())],
        ) {
            Ok(url) => url,
            Err(error) => {
                log::warn!("invalid invoices link: {error}");
                self.notice(t!(self.locale, "Invalid report link.").into_owned());
                return;
            }
        };
        if let Err(error) = open::that(url.as_str()) {
            log::warn!("open browser invoices: {error}");
            self.notice(t!(self.locale, "The system could not open the browser.").into_owned());
        }
    }

    /// Show an offline, fictional receivables report in the visual fixture only.
    #[cfg(feature = "visual-fixtures")]
    #[doc(hidden)]
    pub fn show_fictional_receivables(&mut self) {
        self.receivables_open = true;
        self.receivables_report.data = Some(crate::client_report::fictional_receivables());
    }

    /// Show an offline, fictional payables report in the visual fixture only.
    #[cfg(feature = "visual-fixtures")]
    #[doc(hidden)]
    pub fn show_fictional_payables(&mut self) {
        self.payables_open = true;
        self.payables_report.data = Some(crate::client_report::fictional_payables());
    }

    /// Display the pre-authorization waiting state without making a request.
    #[cfg(feature = "visual-fixtures")]
    #[doc(hidden)]
    pub fn show_fictional_browser_wait(&mut self) {
        self.browser_login.active = true;
        self.login.busy = true;
        self.login.browser_auto_started = true;
    }

    /// Show an offline, fictional stock report in the visual fixture only.
    #[cfg(feature = "visual-fixtures")]
    #[doc(hidden)]
    pub fn show_fictional_stock_report(&mut self) {
        self.stock_report_open = true;
        self.stock_report.data = Some(crate::client_report::fictional_stock());
        self.stock_locations.data = Some(vec![crate::client_report::StockLocation {
            id: 42,
            label: "Entrepôt Exemple (défaut)".into(),
        }]);
    }

    /// Load the official stock valuation for the selected location.
    pub(crate) fn load_stock_report(&mut self) {
        if !self.role_policy().can_view_reports() {
            return;
        }
        let Some(api) = self.client.clone() else {
            return;
        };
        if !self.stock_report_open {
            self.load_stock_locations();
        }
        self.stock_report_open = true;
        self.stock_report = Default::default();
        let location = self.stock_report_location;
        self.stock_report.start(
            self.runner
                .spawn(async move { crate::client_report::fetch_stock(&api, location).await }),
        );
    }

    /// Fetch the locations offered by the worker's official report chooser.
    pub(crate) fn load_stock_locations(&mut self) {
        if !self.role_policy().can_view_reports() || self.stock_locations.loading {
            return;
        }
        let Some(api) = self.client.clone() else {
            return;
        };
        self.stock_locations = Default::default();
        self.stock_locations.start(
            self.runner
                .spawn(async move { crate::client_report::fetch_stock_locations(&api).await }),
        );
    }

    /// Load the worker's authoritative supplier payables report.
    pub(crate) fn load_payables_report(&mut self) {
        if !self.role_policy().can_view_reports() {
            return;
        }
        let Some(api) = self.client.clone() else {
            return;
        };
        self.payables_open = true;
        self.payables_report = Default::default();
        self.payables_report.start(
            self.runner
                .spawn(async move { crate::client_report::fetch_payables(&api).await }),
        );
    }

    /// Load the worker's authoritative receivables report for administrators.
    pub(crate) fn load_receivables_report(&mut self) {
        if !self.role_policy().can_view_reports() {
            return;
        }
        let Some(api) = self.client.clone() else {
            return;
        };
        self.receivables_open = true;
        self.receivables_report = Default::default();
        self.receivables_report.start(
            self.runner
                .spawn(async move { crate::client_report::fetch_receivables(&api).await }),
        );
    }

    /// Load the server's report as inert native text and tables.
    pub(crate) fn load_client_report(&mut self, client: &atlas_api::models::Client, full: bool) {
        if !self.role_policy().allows_route(Route::Clients) {
            return;
        }
        let Some(api) = self.client.clone() else {
            return;
        };
        self.client_report_id = Some(client.id);
        self.client_report_full = full;
        self.client_report = Default::default();
        let client = client.clone();
        self.client_report.start(
            self.runner
                .spawn(async move { crate::client_report::fetch(&api, &client, full).await }),
        );
    }

    /// Mint a 30-minute report ticket and open the printable client
    /// situation report (`Synthèse` or `Détail complet`) in the browser.
    pub fn open_client_situation(&mut self, client: &atlas_api::models::Client, full: bool) {
        let mut params = vec![
            ("client_id", client.id.to_string()),
            ("name", client.name.clone()),
            ("ice", client.ice.clone()),
        ];
        if full {
            params.push(("detail", "full".to_owned()));
        }
        self.open_printable("/api/reports/client-situation", &params);
    }

    pub fn revoke_api_key(&mut self, id: i64) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let job = self.runner.spawn(async move {
            client.delete_api_key(id).await?;
            client.list_api_keys().await
        });
        self.api_keys.start(job);
        // The confirmation notice waits for the job to succeed (see
        // poll_background); a failed deletion stays on the cache error path.
        self.api_keys_revoke_pending = true;
    }

    pub fn load_assist(&mut self) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let job = self
            .runner
            .spawn(async move { client.assist_provider().await });
        self.assist.start(job);
    }

    /// Copies a freshly loaded profile into the editable form once.
    fn fill_profile_form(&mut self, me: &atlas_api::models::Me) {
        if self.profile_form.loaded {
            return;
        }
        self.profile_form = crate::state::ProfileForm {
            display_name: me.display_name.clone().unwrap_or_default(),
            email: me.email.clone().unwrap_or_default(),
            phone: me.phone.clone().unwrap_or_default(),
            notes: me.notes.clone().unwrap_or_default(),
            avatar_url: me.avatar_url.clone().unwrap_or_default(),
            loaded: true,
        };
    }

    fn poll_profile_jobs(&mut self) {
        if let Some(job) = &self.profile_save_job
            && let Some(result) = job.poll()
        {
            self.profile_save_job = None;
            match result {
                Ok(me) => {
                    // The PATCH response only carries the editable profile
                    // fields; preserve identity fields from the loaded Me
                    // instead of replacing them with serde defaults.
                    let merged = if let Some(existing) = self.profile.data.as_mut() {
                        existing.display_name = me.display_name.clone();
                        existing.email = me.email.clone();
                        existing.phone = me.phone.clone();
                        existing.notes = me.notes.clone();
                        existing.avatar_url = me.avatar_url.clone();
                        existing.clone()
                    } else {
                        me.clone()
                    };
                    self.profile.data = Some(merged.clone());
                    self.profile_form.loaded = true;
                    self.fill_profile_form(&merged);
                    if let Some(session) = self.session.as_mut() {
                        // Drop the previous picture from the texture cache
                        // when the avatar URL actually changes.
                        let new_url = merged.avatar_url.clone().unwrap_or_default();
                        if !session.avatar_url.is_empty() && session.avatar_url != new_url {
                            self.avatar_evict = Some(session.avatar_url.clone());
                        }
                        session.display_name = merged.display_name.clone().unwrap_or_default();
                        session.avatar_url = new_url;
                        session.email = merged.email.clone().unwrap_or_default();
                    }
                    self.save_session();
                    self.load_avatar();
                    self.notice(t!(self.locale, "Profile updated.").into_owned());
                }
                Err(error) => self.notice(error.message),
            }
        }
        if let Some(job) = &self.key_create_job
            && let Some(result) = job.poll()
        {
            self.key_create_job = None;
            match result {
                Ok(created) => {
                    self.created_key = Some(created);
                    self.key_editor = None;
                    self.load_api_keys();
                }
                Err(error) => self.notice(error.message),
            }
        }
        if let Some(job) = &self.report_job
            && let Some(result) = job.poll()
        {
            self.report_job = None;
            match result {
                Ok(url) => {
                    if let Err(error) = open::that(url.as_str()) {
                        log::warn!("could not open browser report: {:?}", error.kind());
                        self.notice(
                            t!(self.locale, "The system could not open the browser.").into_owned(),
                        );
                    }
                }
                Err(error) => self.notice(error.message),
            }
        }
    }

    // ── Updates ──────────────────────────────────────────────────────────

    pub fn check_for_updates(&mut self, prerelease_override: bool) {
        if self.update.checking {
            return;
        }
        let prerelease = prerelease_override
            || matches!(
                self.settings.update_channel,
                atlas_core::settings::UpdateChannel::Prerelease
            );
        self.update.checking = true;
        self.update.error = None;
        let cache = self.update.cache.clone();
        let cache_path = self.paths.update_state_file();
        let job = self.runner.spawn(async move {
            crate::update::fetch_manifest(prerelease, cache, cache_path).await
        });
        self.update.job = Some(job);
    }

    /// Recomputes when the next automatic check may run. Checks honour the
    /// stored ETag and GitHub's rate-limit window, so several launches in an
    /// hour never exhaust the unauthenticated quota.
    pub fn schedule_update_checks(&mut self) {
        if !self.settings.update_check_on_start {
            self.update.next_check_at = None;
            return;
        }
        let interval = self.settings.update_check_interval_hours.clamp(1, 168) * 3_600;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs());
        let elapsed = now.saturating_sub(self.update.cache.last_checked_unix);
        let delay = interval.saturating_sub(elapsed);
        self.update.next_check_at = Some(Instant::now() + Duration::from_secs(delay.min(interval)));
    }

    /// Runs an automatic check once its delay elapsed, never more often
    /// than the configured interval.
    pub fn maybe_auto_check_updates(&mut self) {
        if !self.settings.update_check_on_start || self.update.checking {
            return;
        }
        if self.update.next_check_at.is_none() {
            self.schedule_update_checks();
        }
        let Some(next) = self.update.next_check_at else {
            return;
        };
        if Instant::now() < next {
            return;
        }
        self.check_for_updates(false);
        self.schedule_update_checks();
    }

    fn save_update_cache(&self) {
        self.update.cache.save(&self.paths.update_state_file());
    }

    /// User-triggered install: reuses an already downloaded copy when the
    /// auto-download setting fetched one, otherwise downloads it now.
    pub fn start_update_download(&mut self) {
        self.begin_update_download(false);
    }

    fn begin_update_download(&mut self, auto: bool) {
        if self.update.progress.is_some() {
            return;
        }
        self.update.auto_download = auto;
        let progress = Arc::new(Mutex::new((0_u64, 0_u64)));
        self.update_progress = progress.clone();
        self.update.progress = Some((0, 0));
        self.update.stage = t!(self.locale, "Downloading…").into_owned();
        let locale = self.locale;
        let expected = self
            .update
            .last_manifest
            .as_ref()
            .map(|manifest| manifest.version.clone());
        self.update.download_version = expected.clone();
        let job: atlas_core::Job<Result<(), String>> = self.runner.spawn(async move {
            // Pin the blocking closure's error type: without this, inference
            // starts from the &str guard below and resolves it to unsized str.
            tokio::task::spawn_blocking(move || -> Result<(), String> {
                let updater = crate::update::build_updater().map_err(|error| error.to_string())?;
                // Refuse early with a clear message when this copy has no
                // portable/installer marker (e.g. a dev build): the helper
                // cannot replace an executable it cannot identify.
                updater.installation().map_err(|_| {
                    t!(
                        locale,
                        "This build cannot update itself. Download the latest installer from the releases page."
                    )
                    .into_owned()
                })?;
                // A previous auto-download left its verified copy staged
                // beside the app; reuse it instead of spending bandwidth.
                let kept = updater.kept().ok().flatten().filter(|prepared| {
                    expected.as_deref().is_none_or(|version| prepared.version().trim_start_matches('v') == version.trim_start_matches('v'))
                });
                let prepared = match kept {
                    Some(prepared) => prepared,
                    None => {
                        let release = updater
                            .check()
                            .map_err(|error| error.to_string())?
                            .ok_or_else(|| {
                                t!(locale, "No published release found.").into_owned()
                            })?;
                        if expected.as_deref().is_some_and(|version| release.version.trim_start_matches('v') != version.trim_start_matches('v')) {
                            return Err("La version proposée par le programme de mise à jour ne correspond pas au manifeste ; réessayez plus tard.".to_owned());
                        }
                        updater
                            .download(&release, |received, total| {
                                if let Ok(mut guard) = progress.lock() {
                                    *guard = (received, total);
                                }
                            })
                            .map_err(|error| error.to_string())?
                    }
                };
                if auto {
                    // Keep the staged files for a later install; never
                    // restart the app while someone is working.
                    drop(prepared);
                    return Ok(());
                }
                updater
                    .handoff(prepared, Vec::new())
                    .map_err(|error| error.to_string())?;
                Ok(())
            })
            .await
            .map_err(|error| error.to_string())
            .and_then(|job_outcome: Result<(), String>| {
                // Classify after the blocking work unwinds: a 404 / missing
                // asset in the release's publish window is a "try again
                // shortly", not a permanent fault.
                match job_outcome {
                    Err(message)
                        if crate::update::is_release_not_ready(message.as_str()) =>
                    {
                        Err(t!(
                            locale,
                            "The new release is still being published on GitHub. Try again in a few minutes."
                        )
                        .into_owned())
                    }
                    other => other,
                }
            })
        });
        self.update_download_job = Some(job);
    }

    fn poll_update_jobs(&mut self, ctx: &Context) {
        if let Some(job) = &self.update.job
            && let Some(result) = job.poll()
        {
            self.update.job = None;
            self.update.checking = false;
            match result {
                Ok(checked) => {
                    let not_modified = checked.not_modified;
                    self.update.cache = checked.cache;
                    self.save_update_cache();
                    if let Some(manifest) = checked.manifest {
                        self.apply_manifest(manifest);
                    } else if !not_modified {
                        // No manifest (e.g. an asset still being uploaded)
                        // does not establish that the installation is current.
                        self.update.up_to_date_at = None;
                        self.update.error = Some(
                            t!(
                                self.locale,
                                "Update information is not available yet. Try again later."
                            )
                            .into_owned(),
                        );
                    }
                }
                Err(error) => {
                    // Record failure as a check too, to avoid retrying every
                    // frame offline; preserve any quota reset learned by HTTP.
                    self.update.cache =
                        crate::update::UpdateCache::load(&self.paths.update_state_file());
                    self.update.cache.last_checked_unix = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_or(0, |elapsed| elapsed.as_secs());
                    self.update.error = Some(error);
                    self.save_update_cache();
                }
            }
            self.schedule_update_checks();
        }
        if let Some(job) = &self.update_download_job {
            if let Ok(guard) = self.update_progress.lock() {
                self.update.progress = Some(*guard);
            }
            if let Some(result) = job.poll() {
                self.update_download_job = None;
                match result {
                    Ok(()) if self.update.auto_download => {
                        // Downloaded in the background and kept staged: the
                        // app never restarts under someone because a byte
                        // arrived. Installing waits for the user or the next
                        // launch, and reuses this copy instead of downloading.
                        self.update.auto_download = false;
                        self.update.progress = None;
                        self.update.stage =
                            t!(self.locale, "Update downloaded. Restart to install.").into_owned();
                        // A check may have completed while the download was
                        // running. Never label the older staged file as the
                        // newer manifest or the other channel.
                        let downloaded = self.update.download_version.take();
                        self.update.ready_version = downloaded.filter(|version| {
                            self.update
                                .last_manifest
                                .as_ref()
                                .is_some_and(|manifest| &manifest.version == version)
                        });
                        if let Some(version) = self.update.ready_version.clone() {
                            self.notice(format!(
                                "{} v{version}",
                                t!(self.locale, "Update downloaded. Restart to install.")
                            ));
                        }
                    }
                    Ok(()) => {
                        self.update.download_version = None;
                        self.update.stage = t!(self.locale, "Installing…").into_owned();
                        self.wants_quit = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    Err(error) => {
                        self.update.download_version = None;
                        self.update.auto_download = false;
                        self.update.progress = None;
                        self.update.error = Some(error);
                    }
                }
            }
        }
    }

    fn apply_manifest(&mut self, manifest: UpdateManifest) {
        self.update.release_notes = Some(manifest.clone());
        // A staged installer is only identified by the version that was
        // actually downloaded, never by a later manifest.
        if !staged_version_matches(self.update.ready_version.as_deref(), &manifest.version) {
            self.update.ready_version = None;
        }
        let decision = manifest.decision_for(env!("CARGO_PKG_VERSION"));
        match decision {
            UpdateDecision::UpToDate => {
                // Surface an explicit status in the About card instead of
                // silently swallowing the answer.
                self.update.up_to_date_at = Some(Instant::now());
                self.update.last_manifest = None;
                self.update.ready_version = None;
                self.update.mandatory = None;
                self.update.error = None;
                self.leave_update_gate();
            }
            UpdateDecision::Mandatory { .. } => {
                self.update.up_to_date_at = None;
                self.update.mandatory = Some(manifest.clone());
                self.update.last_manifest = Some(manifest);
                self.screen = Screen::UpdateGate;
            }
            UpdateDecision::Optional { .. } => {
                self.update.mandatory = None;
                self.leave_update_gate();
                self.update.up_to_date_at = None;
                let auto = self.settings.update_auto_download;
                self.notice(format!("Mise à jour disponible : v{}", manifest.version));
                self.update.last_manifest = Some(manifest);
                if auto && self.update.progress.is_none() && self.update_download_job.is_none() {
                    // Background download; installation stays explicit.
                    self.begin_update_download(true);
                }
            }
        }
    }

    /// Resume the existing session when a previously mandatory update is withdrawn.
    fn leave_update_gate(&mut self) {
        if !matches!(self.screen, Screen::UpdateGate) {
            return;
        }
        let destination = update_gate_destination(
            self.vault_restore.is_some(),
            self.session.is_some() && self.client.is_some(),
            self.server_store.selected_url().is_some() && !self.server_store.requires_verification,
            self.role_policy().initial_route(),
        );
        if let Screen::Shell(route) = destination {
            self.load_profile();
            self.load_avatar();
            self.go(route);
        } else {
            self.screen = destination;
        }
    }

    // ── Cross-thread events ──────────────────────────────────────────────

    fn drain_tray(&mut self, ctx: &Context) {
        // The fastframe tray reports on its own channel; drain it here, on
        // the UI thread, both for windowed frames and headless ticks.
        let commands: Vec<TrayCommand> = self
            .tray
            .as_mut()
            .map_or_else(Vec::new, crate::tray::TrayHandle::drain);
        for command in commands {
            match command {
                TrayCommand::Show => {
                    self.show_requested = true;
                }
                TrayCommand::QuickStats => {
                    if self.session.is_some() {
                        self.go(Route::Dashboard);
                        self.show_requested = true;
                    }
                }
                TrayCommand::Quit => {
                    self.wants_quit = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }

    fn consume_deep_links(&mut self) {
        if let Ok(mut queue) = self.incoming_links.lock() {
            self.pending_deep_links.append(&mut queue);
        }
        let links = std::mem::take(&mut self.pending_deep_links);
        for raw in links {
            if raw == crate::SHOW_REQUEST {
                // A second launch asked this instance to come forward. The
                // headless loop turns this flag into Headless::Show; when a
                // window already exists, poll_background makes it visible.
                self.show_requested = true;
                continue;
            }
            if self.server_store.requires_verification
                || self.server_store.selected_url().is_none()
                || matches!(self.screen, Screen::ServerSetup)
            {
                // Preserve links until the server choice and verification are complete.
                self.pending_deep_links.push(raw);
                continue;
            }
            if let Some(link) = atlas_core::VerifyLink::parse(&raw)
                && self.session.is_none()
                && !self.requires_password
                && self.login.job_link.is_none()
            {
                // A deep link supersedes an in-flight hosted login.
                crate::browser_auth::cancel(self);
                self.qr = Default::default();
                self.login.pending_link = Some(link);
                self.screen = Screen::Login;
            } else if let Some(token) = atlas_core::QrTokenParser::extract(&raw)
                && self.session.is_none()
                && !self.requires_password
                && self.login.job_link.is_none()
            {
                crate::browser_auth::cancel(self);
                self.login.pending_link = None;
                self.qr = Default::default();
                self.qr.active = true;
                self.qr.token = token;
                self.qr.status = "pending".to_owned();
                self.qr.next_poll = None;
                self.screen = Screen::Login;
            }
        }
    }

    /// One frame's worth of background work. Called from both the windowed
    /// eframe loop and the headless shell loop.
    pub fn poll_background(&mut self, ctx: &Context) {
        self.drain_power_events();
        self.drain_tray(ctx);
        self.poll_vault();
        self.consume_deep_links();
        // Release HTTP-loaded avatar textures queued by state changes.
        if let Some(uri) = self.avatar_evict.take() {
            ctx.forget_image(&uri);
        }
        screens::auth::poll_login_jobs(self);
        if let Some(result) = self.avatar_job.as_ref().and_then(|job| job.poll()) {
            self.avatar_job = None;
            if let Some(old) = self.avatar_texture_uri.take() {
                ctx.forget_image(&old);
            }
            if let Some((uri, bytes)) = result {
                ctx.forget_image(&uri);
                ctx.include_bytes(uri.clone(), bytes);
                self.avatar_texture_uri = Some(uri);
            }
            ctx.request_repaint();
        }

        // Caches.
        if self.dashboard.poll() {
            ctx.request_repaint();
        }
        if self.products.poll() {
            ctx.request_repaint();
        }
        if self.clients.poll() {
            ctx.request_repaint();
        }
        if self.stock_locations.poll() {
            ctx.request_repaint();
        }
        if self.stock_report.poll() {
            ctx.request_repaint();
        }
        if self.payables_report.poll() {
            ctx.request_repaint();
        }
        if self.receivables_report.poll() {
            ctx.request_repaint();
        }
        if self.client_report.poll() {
            ctx.request_repaint();
        }
        if self.client_invoices.poll() {
            ctx.request_repaint();
        }
        if self.business_search.poll() {
            ctx.request_repaint();
        }
        if self.invoice_detail.poll() {
            ctx.request_repaint();
        }
        if self.vendors.poll() {
            ctx.request_repaint();
        }
        if self.employees.poll() {
            ctx.request_repaint();
        }
        if self.employee_detail.poll() {
            ctx.request_repaint();
        }
        if self.expenses.poll() {
            ctx.request_repaint();
        }
        if self.bills.poll() {
            ctx.request_repaint();
        }
        if self.ledger_reconciliation.poll() {
            ctx.request_repaint();
        }
        if self.bank_reconciliation.poll() {
            ctx.request_repaint();
        }
        if self.delivery_notes.poll() {
            ctx.request_repaint();
        }
        if self.categories.poll() {
            ctx.request_repaint();
        }
        if self.units.poll() {
            ctx.request_repaint();
        }
        if self.receipts.poll() {
            ctx.request_repaint();
        }
        if self.invoices.poll() {
            ctx.request_repaint();
        }
        if self.alerts.poll() {
            self.maybe_notify_alerts();
            ctx.request_repaint();
        }
        if self.devices.poll() {
            ctx.request_repaint();
        }
        if self.nav_badges.poll() {
            ctx.request_repaint();
        }
        if self.profile.poll() {
            if let Some(me) = self.profile.data.clone() {
                self.fill_profile_form(&me);
            }
            ctx.request_repaint();
        }
        if self.assist.poll() {
            ctx.request_repaint();
        }
        if self.api_keys.poll() {
            if self.api_keys_revoke_pending {
                self.api_keys_revoke_pending = false;
                if self.api_keys.error.is_none() {
                    self.notice(t!(self.locale, "API key revoked.").into_owned());
                }
            }
            ctx.request_repaint();
        }
        self.poll_employee_jobs();
        self.poll_profile_jobs();
        self.poll_agent_jobs();
        self.poll_update_jobs(ctx);
        self.maybe_auto_check_updates();
        self.enforce_session_binding();

        // Periodic refresh while a session is live. A cache only enters the
        // rotation once its screen has actually been opened (last_updated is
        // Some): an unvisited route never generates background traffic, so
        // the app no longer fans out one request per screen on a timer.
        if self.session.is_some() && !self.power_paused {
            let interval = Duration::from_secs(self.settings.poll_interval_secs.max(15));
            if self.dashboard.last_updated.is_some() && self.dashboard.stale_for(interval) {
                self.load_dashboard();
            }
            if self.products.last_updated.is_some()
                && self.products.stale_for(interval)
                && self.product_search.is_empty()
            {
                self.load_products();
            }
            if self.alerts.last_updated.is_some() && self.alerts.stale_for(interval) {
                self.load_alerts();
            }
            if self.nav_badges.last_updated.is_some() && self.nav_badges.stale_for(interval) {
                self.load_nav_badges();
            }
        }
    }

    /// A 401 on an authenticated call whose code is not a user mistake means
    /// the server-side (IP, fingerprint) binding changed: the token is dead
    /// and the worker demands a fresh login.
    fn enforce_session_binding(&mut self) {
        if self.session.is_none() {
            return;
        }
        let expired = [
            &self.dashboard.last_error,
            &self.products.last_error,
            &self.invoices.last_error,
            &self.alerts.last_error,
            &self.devices.last_error,
            &self.nav_badges.last_error,
            &self.profile.last_error,
            &self.api_keys.last_error,
        ]
        .iter()
        .any(|error| {
            error
                .as_ref()
                .is_some_and(|error| error.is_session_expired(true))
        });
        if expired {
            self.sign_out();
            self.notice("Votre session a expiré. Veuillez vous reconnecter.".to_owned());
        }
    }

    fn maybe_notify_alerts(&mut self) {
        if !self.settings.notifications_enabled {
            return;
        }
        let Some(alerts) = &self.alerts.data else {
            return;
        };
        let high = alerts
            .iter()
            .filter(|alert| alert.severity.as_deref() == Some("high"))
            .count();
        if high > self.notification_state.last_high_severity_count
            && let Some(client) = &self.notifications
        {
            let newest = alerts
                .iter()
                .find(|alert| alert.severity.as_deref() == Some("high"));
            if let Some(alert) = newest {
                let notification =
                    fastframe_notifications::Notification::new("Alerte stock Atlas Commerce")
                        .body(alert.label.clone())
                        .category("stock");
                if let Err(error) = client.show(notification) {
                    log::warn!("notification failed: {error:?}");
                }
            }
        }
        self.notification_state.last_high_severity_count = high;
    }

    fn poll_agent_jobs(&mut self) {
        if let Some(job) = &self.agent.job_policy
            && let Some(result) = job.poll()
        {
            self.agent.job_policy = None;
            if let Ok(policy) = result {
                self.agent.policy = Some(policy);
                self.agent.policy_retry_at = None;
            } else {
                // Back off: without this gate the next frame would spawn a
                // new request while the previous error is still on screen.
                self.agent.policy_retry_at =
                    Some(Instant::now() + crate::state::POLICY_RETRY_BACKOFF);
            }
        }
        if let Some(job) = &self.agent.job
            && let Some(result) = job.poll()
        {
            self.agent.job = None;
            self.agent.busy = false;
            match result {
                Ok(reply) => {
                    let text = if reply.reply.is_empty() {
                        "Aucune réponse de l'agent.".to_owned()
                    } else {
                        reply.reply
                    };
                    self.agent.push_message(false, text);
                }
                Err(error) => {
                    self.agent
                        .push_message(false, format!("Erreur : {}", error.message));
                }
            }
        }
    }

    fn drain_power_events(&mut self) {
        for event in self.power.events() {
            match event {
                fastframe_power::Event::Suspending(ack) => {
                    self.handle_suspending();
                    self.save_settings();
                    self.save_servers();
                    self.save_session();
                    ack.done();
                }
                fastframe_power::Event::Resumed => self.handle_resume(),
                fastframe_power::Event::EndingSession(ack) => {
                    self.handle_suspending();
                    self.save_settings();
                    self.save_servers();
                    self.save_session();
                    ack.done();
                }
            }
        }
    }

    /// Called by the binary when the system resumes from sleep.
    pub fn handle_resume(&mut self) {
        self.power_paused = false;
        // Reload every cache the user has actually opened. Clearing the
        // timestamps instead would make the periodic rotation skip them,
        // since it only refreshes caches whose last_updated is Some.
        if self.session.is_some() {
            if self.dashboard.last_updated.is_some() {
                self.load_dashboard();
            }
            if self.products.last_updated.is_some() && self.product_search.is_empty() {
                self.load_products();
            }
            if self.alerts.last_updated.is_some() {
                self.load_alerts();
            }
            if self.nav_badges.last_updated.is_some() {
                self.load_nav_badges();
            }
        }
    }

    /// Called by the binary when the system is about to sleep.
    pub fn handle_suspending(&mut self) {
        self.power_paused = true;
    }

    // ── Frame helpers ────────────────────────────────────────────────────

    /// Whether any background work is in flight and the frame should keep
    /// repainting on its own.
    #[must_use]
    pub fn busy(&self) -> bool {
        self.login.busy
            || self.vault_restore.is_some()
            || !self.vault_saves.is_empty()
            || !self.vault_removals.is_empty()
            || self.qr.job_status.is_some()
            || self.qr.job_create.is_some()
            || self.qr.job_complete.is_some()
            || self.server_form.probe_job.is_some()
            || self.dashboard.loading
            || self.products.loading
            || self.client_invoices.loading
            || self.client_report.loading
            || self.receivables_report.loading
            || self.payables_report.loading
            || self.stock_report.loading
            || self.stock_locations.loading
            || self.invoice_detail.loading
            || self.business_search.loading
            || self.bills.loading
            || self.ledger_reconciliation.loading
            || self.bank_reconciliation.loading
            || self.invoices.loading
            || self.receipts.loading
            || self.alerts.loading
            || self.devices.loading
            || self.profile.loading
            || self.avatar_job.is_some()
            || self.api_keys.loading
            || self.assist.loading
            || self.profile_save_job.is_some()
            || self.key_create_job.is_some()
            || self.agent.busy
            || self.agent.job_policy.is_some()
            || self.update.checking
            || self.update.progress.is_some()
    }

    /// Draws the whole window for the current screen.
    pub fn draw(&mut self, ui: &mut Ui, ctx: &Context, palette: &Palette) {
        match self.screen {
            Screen::Splash => screens::auth::draw_splash(self, ui, palette),
            Screen::Onboarding => screens::auth::draw_onboarding(self, ui, palette),
            Screen::ServerSetup => {
                screens::auth::draw_server_setup(self, ui, palette, false);
            }
            Screen::Login => screens::auth::draw_login(self, ui, palette),
            Screen::UpdateGate => screens::auth::draw_update_gate(self, ui, palette),
            Screen::Shell(_) => screens::shell::draw_shell(self, ui, ctx, palette),
        }
        screens::notices(self, ui, palette);
    }
}

// ── fastframe shell Resident ────────────────────────────────────────────────

impl Resident for AtlasApp {
    fn closed(&self) -> fastframe_shell::Closed {
        if self.wants_quit || !self.tray_available || !self.settings.close_to_tray {
            fastframe_shell::Closed::Quit
        } else {
            fastframe_shell::Closed::Hide
        }
    }

    fn window_gone(&mut self) {
        self.show_requested = false;
        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
        if self.browser_login.embedded {
            crate::browser_auth::cancel(self);
        }
    }

    fn headless_frame(&mut self, ctx: &Context) -> Headless {
        self.poll_background(ctx);
        if self.wants_quit {
            return Headless::Quit;
        }
        if self.show_requested {
            self.show_requested = false;
            return Headless::Show;
        }
        // Stay asleep between polls; the waker cuts the wait for tray events.
        Headless::WaitFor(Duration::from_secs(
            self.settings.poll_interval_secs.max(15),
        ))
    }

    fn start_hidden(&mut self) -> bool {
        if self.tray_available && self.settings.close_to_tray {
            // macOS builds its menu-bar item on demand while no window
            // exists; the other platforms no-op here.
            if let Some(tray) = self.tray.as_mut() {
                tray.create_item();
            }
            true
        } else {
            false
        }
    }

    fn shutdown(&mut self) {
        self.save_settings();
        self.save_servers();
        self.save_session();
        self.save_accounts();
    }
}

// ── eframe wrapper ──────────────────────────────────────────────────────────

/// The eframe app owning the shell-held [`AtlasApp`].
pub struct EframeShell {
    pub held: Held<AtlasApp>,
    first_frame: bool,
    #[cfg(target_os = "windows")]
    login_view: Option<crate::webview_auth::LoginView>,
    #[cfg(target_os = "linux")]
    login_view: Option<crate::webview_auth_linux::LoginView>,
    #[cfg(target_os = "macos")]
    login_view: Option<atlas_webview_macos::LoginView>,
}

impl EframeShell {
    #[must_use]
    pub fn new(lease: Lease<AtlasApp>, ctx: &Context) -> Self {
        // `take` attaches the shell waker to this context itself.
        let mut held = lease.take(ctx);
        // Hiding to the tray destroys the eframe window while the app state
        // survives; showing it again builds a brand-new egui Context with
        // stock styles. The retained cache would otherwise match the
        // palette and make the first frame skip theme::apply on that fresh
        // context, so every new context starts with an empty cache.
        held.applied_palette = None;
        held.applied_reduced_motion = None;
        crate::theme::install_fonts_and_icons(ctx);
        log::info!("Startup: fonts and image loaders ready");
        // Make egui's own preference follow the app setting before the first
        // palette read, so a forced Light/Dark mode resolves correctly even
        // when the OS reports the opposite appearance.
        held.sync_egui_theme(ctx);
        // eframe asks for the clear colour before the first `logic` run, so
        // seed it at construction (the CreationContext already knows the
        // system theme) to avoid the dark fallback on even the first frame.
        held.last_clear_color = Some(held.palette(ctx).window.to_normalized_gamma_f32());
        Self {
            held,
            first_frame: true,
            #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
            login_view: None,
        }
    }
}

impl eframe::App for EframeShell {
    fn persist_egui_memory(&self) -> bool {
        false
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        // Match the active theme from the first frame on, so a light theme
        // never flashes the dark background before the first UI frame.
        // The dark token (#111113) is only the pre-first-frame fallback.
        self.held
            .last_clear_color
            .unwrap_or_else(|| egui::Color32::from_rgb(0x11, 0x11, 0x13).to_normalized_gamma_f32())
    }

    fn logic(&mut self, ctx: &Context, frame: &mut eframe::Frame) {
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        let _ = &frame;
        if self.first_frame {
            log::info!("Startup: first window logic frame");
            self.first_frame = false;
            // The viewport was born hidden (see native_options): reveal it
            // only now that the first frame carries the real palette, fonts
            // and screen, so no unstyled white window ever reaches the
            // screen. The command applies after this frame presents.
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            // macOS moves the status item onto the main window's AppKit loop.
            if let Some(tray) = self.held.tray.as_mut() {
                tray.attach();
            }
            if let Some(receipt) = self.held.update_receipt.take() {
                // The helper stands ready to roll back until the new build
                // proves it can draw; answer off the UI thread.
                std::thread::spawn(move || {
                    if let Err(error) = receipt.acknowledge() {
                        log::warn!("update receipt not acknowledged: {error}");
                    }
                });
            }
        }
        // Keep egui's preference and the native window chrome aligned with
        // the app setting before resolving the palette for this frame.
        self.held.sync_egui_theme(ctx);
        self.held.scrolling.apply(ctx);
        let palette = self.held.palette(ctx);
        // Keep the framebuffer clear colour in sync, and rebuild the global
        // styles only when the palette actually changes instead of every
        // frame. Comparing the whole palette also catches token edits, not
        // just dark/light flips.
        self.held.last_clear_color = Some(palette.window.to_normalized_gamma_f32());
        let reduced_motion = self.held.settings.reduced_motion;
        if self.held.applied_palette != Some(palette)
            || self.held.applied_reduced_motion != Some(reduced_motion)
        {
            crate::theme::apply(ctx, &palette, reduced_motion);
            self.held.applied_palette = Some(palette);
            self.held.applied_reduced_motion = Some(reduced_motion);
        }

        // Window title contract: "Atlas Commerce — <serveur>" once a server
        // is selected, plain "Atlas Commerce" on the auth screens. Pushed
        // only when it changes, so no per-frame viewport commands.
        let window_title = match self.held.server_store.selected_url() {
            Some(base) => format!("Atlas Commerce — {}", atlas_core::ServerUrl::host(base)),
            None => "Atlas Commerce".to_owned(),
        };
        if self.held.applied_title.as_deref() != Some(window_title.as_str()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(window_title.clone()));
            self.held.applied_title = Some(window_title);
        }

        self.held.poll_background(ctx);
        // Wake the app exactly when the next automatic update check is due,
        // even when no input, job or animation requests a repaint.
        if let Some(at) = self.held.update.next_check_at {
            ctx.request_repaint_after(at.saturating_duration_since(Instant::now()));
        }
        #[cfg(target_os = "windows")]
        {
            if !crate::browser_auth::embedded_active(&self.held) {
                self.login_view = None;
            } else if let Some(window) = frame.winit_window() {
                let size = window.inner_size();
                let geometry = (size.width, size.height, window.scale_factor());
                if let Some(url) = self.held.browser_login.pending_url.take() {
                    let server = self
                        .held
                        .server_store
                        .selected_url()
                        .unwrap_or_default()
                        .to_owned();
                    match crate::webview_auth::LoginView::open(
                        window.as_ref(),
                        geometry,
                        &url,
                        &server,
                        &self.held.paths.dirs.cache,
                    ) {
                        Ok(view) => self.login_view = Some(view),
                        Err(message) if crate::browser_auth::fallback_allowed(message) => {
                            crate::browser_auth::fallback_to_browser(&mut self.held, url, message);
                        }
                        Err(message) => {
                            crate::browser_auth::cancel(&mut self.held);
                            self.held.login.error = Some(message.to_owned());
                        }
                    }
                }
                if let Some(view) = &self.login_view {
                    view.resize(geometry.0, geometry.1, geometry.2);
                }
            }
        }
        #[cfg(target_os = "linux")]
        {
            if !crate::browser_auth::embedded_active(&self.held) {
                self.login_view = None;
            } else {
                if let Some(url) = self.held.browser_login.pending_url.take() {
                    let server = self
                        .held
                        .server_store
                        .selected_url()
                        .unwrap_or_default()
                        .to_owned();
                    let code = self
                        .held
                        .browser_login
                        .pairing_code
                        .as_deref()
                        .unwrap_or_default();
                    match crate::webview_auth_linux::LoginView::open(&url, &server, code) {
                        Ok(view) => self.login_view = Some(view),
                        Err(crate::webview_auth_linux::UNAVAILABLE) => {
                            crate::browser_auth::fallback_to_browser(
                                &mut self.held,
                                url,
                                crate::webview_auth_linux::UNAVAILABLE,
                            );
                        }
                        Err(message) => {
                            crate::browser_auth::cancel(&mut self.held);
                            self.held.login.error = Some(message.to_owned());
                        }
                    }
                }
                if let Some(view) = self.login_view.as_mut() {
                    match view.pump() {
                        Ok(true) => {}
                        Ok(false) => {
                            self.login_view = None;
                            crate::browser_auth::cancel(&mut self.held);
                        }
                        Err(()) => {
                            self.login_view = None;
                            crate::browser_auth::cancel(&mut self.held);
                            self.held.login.error = Some(
                                "Connexion intégrée indisponible. Utilisez le navigateur système."
                                    .to_owned(),
                            );
                        }
                    }
                }
                if self.login_view.is_some() {
                    ctx.request_repaint_after(Duration::from_millis(16));
                }
            }
        }
        #[cfg(target_os = "macos")]
        {
            if !crate::browser_auth::embedded_active(&self.held) {
                self.login_view = None;
            } else if let Some(window) = frame.winit_window() {
                let size = window.inner_size();
                let geometry = (size.width, size.height, window.scale_factor());
                if let Some(url) = self.held.browser_login.pending_url.take() {
                    let server = self
                        .held
                        .server_store
                        .selected_url()
                        .unwrap_or_default()
                        .to_owned();
                    match atlas_webview_macos::LoginView::open(
                        window.as_ref(),
                        geometry,
                        &url,
                        &server,
                    ) {
                        Ok(view) => self.login_view = Some(view),
                        Err(message) if crate::browser_auth::fallback_allowed(message) => {
                            crate::browser_auth::fallback_to_browser(&mut self.held, url, message);
                        }
                        Err(message) => {
                            crate::browser_auth::cancel(&mut self.held);
                            self.held.login.error = Some(message.to_owned());
                        }
                    }
                }
                if let Some(view) = &self.login_view {
                    view.resize(geometry.0, geometry.1, geometry.2);
                }
            }
        }
        // Unlike viewport.visible(), this also handles minimized, occluded,
        // and hidden windows. Consume the request only in the windowed loop:
        // the headless loop needs it to recreate the window instead.
        if std::mem::take(&mut self.held.show_requested) {
            log::info!("Restoring the existing window");
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
        // Keep repainting while any job runs, so spinners and poll results
        // appear without user input.
        if self.held.busy() {
            // React promptly to the verified authorization URL, without
            // continuously redrawing at high frequency during browser polling.
            let delay = if self.held.browser_login.active
                && self.held.browser_login.pairing_code.is_none()
            {
                30
            } else {
                250
            };
            ctx.request_repaint_after(Duration::from_millis(delay));
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let mut palette = self.held.palette(&ctx);
        if palette.dark
            && matches!(
                self.held.screen,
                Screen::Login | Screen::ServerSetup | Screen::Onboarding
            )
        {
            // SOURCED-FROM: supplied authentication layout specification.
            palette.window = egui::Color32::from_rgb(0x12, 0x13, 0x16);
            palette.surface = egui::Color32::from_rgb(0x1e, 0x20, 0x25);
        }
        if !matches!(self.held.screen, Screen::Shell(_)) && fastframe_titlebar::enabled() {
            egui::Panel::top(egui::Id::new("auth-titlebar"))
                .exact_size(48.0)
                .resizable(false)
                .show_separator_line(false)
                .frame(egui::Frame::NONE.fill(palette.window))
                .show(ui, |ui| {
                    let rect = ui.available_rect_before_wrap();
                    ui.allocate_exact_size(rect.size(), egui::Sense::hover());
                    fastframe_titlebar::drag(ui, rect);
                    let footprint = fastframe_titlebar::footprint(&ctx, 48.0);
                    let style = fastframe_titlebar::Style {
                        icon: palette.secondary,
                        button: palette.surface_hover,
                        header_height: 48.0,
                    };
                    let minimize = t!(self.held.locale, "Minimize").into_owned();
                    let maximize = t!(self.held.locale, "Maximize").into_owned();
                    let restore = t!(self.held.locale, "Restore").into_owned();
                    let close = t!(self.held.locale, "Close").into_owned();
                    let labels = fastframe_titlebar::Labels {
                        minimize: &minimize,
                        maximize: &maximize,
                        restore: &restore,
                        close: &close,
                    };
                    crate::widgets::overlay_at(
                        ui,
                        egui::Rect::from_min_size(
                            egui::pos2(rect.right() - footprint.right - 4.0, rect.top()),
                            egui::vec2(footprint.right.max(120.0), 48.0),
                        ),
                        |ui| {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    fastframe_titlebar::controls(ui, &style, &labels);
                                },
                            );
                        },
                    );
                });
        }
        #[cfg(any(target_os = "windows", target_os = "macos"))]
        if self.login_view.is_some() && crate::browser_auth::embedded_active(&self.held) {
            egui::CentralPanel::default()
                .frame(
                    egui::Frame::NONE
                        .fill(palette.window)
                        .inner_margin(egui::Margin::symmetric(16, 0)),
                )
                .show(ui, |ui| {
                    screens::auth::draw_embedded_header(&mut self.held, ui, &palette)
                });
            return;
        }
        if matches!(self.held.screen, Screen::Shell(_)) {
            // The shell creates its own top/left/central panels; wrapping it
            // in another CentralPanel would nest panels and fight for the
            // available rect.
            self.held.draw(ui, &ctx, &palette);
        } else {
            egui::CentralPanel::default()
                .frame(
                    egui::Frame::NONE
                        .fill(palette.window)
                        .inner_margin(egui::Margin::same(0)),
                )
                .show(ui, |ui| {
                    let transition = self.held.onboarding_transition_at.filter(|at| {
                        !self.held.settings.reduced_motion
                            && matches!(self.held.screen, Screen::Login | Screen::ServerSetup)
                            && at.elapsed().as_secs_f64() < crate::motion::BASE
                    });
                    if let Some(started) = transition {
                        let progress = crate::motion::standard(
                            (started.elapsed().as_secs_f64() / crate::motion::BASE) as f32,
                        );
                        let rect = ui.available_rect_before_wrap();
                        ui.scope(|ui| {
                            ui.disable();
                            ui.set_opacity(1.0 - progress);
                            screens::auth::draw_onboarding(&mut self.held, ui, &palette);
                        });
                        crate::widgets::overlay_at(ui, rect, |ui| {
                            ui.set_opacity(progress);
                            self.held.draw(ui, &ctx, &palette);
                        });
                        ctx.request_repaint_after(Duration::from_millis(16));
                    } else {
                        self.held.onboarding_transition_at = None;
                        self.held.draw(ui, &ctx, &palette);
                    }
                });
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.held.save_settings();
    }
}

#[cfg(test)]
mod update_gate_recovery_tests {
    use super::{Route, Screen, staged_version_matches, update_gate_destination};

    #[test]
    fn staged_update_is_not_mislabelled_as_a_newer_release() {
        assert!(staged_version_matches(Some("1.0.0"), "1.0.0"));
        assert!(!staged_version_matches(Some("1.0.0"), "1.1.0"));
        assert!(!staged_version_matches(None, "1.1.0"));
    }

    #[test]
    fn restores_session_or_the_appropriate_startup_screen() {
        assert_eq!(
            update_gate_destination(false, true, true, Route::Products),
            Screen::Shell(Route::Products)
        );
        assert_eq!(
            update_gate_destination(true, false, true, Route::Dashboard),
            Screen::Splash
        );
        assert_eq!(
            update_gate_destination(false, false, true, Route::Dashboard),
            Screen::Login
        );
        assert_eq!(
            update_gate_destination(false, false, false, Route::Dashboard),
            Screen::ServerSetup
        );
    }
}
