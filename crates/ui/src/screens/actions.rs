//! Mobile handoff inbox. List data may be cached by the Worker; direct detail
//! is re-read before a write. The Worker does not provide atomic state changes.
use crate::dialogs::{Confirm, ConfirmAction};
use crate::state::{AtlasApp, Route};
use crate::theme::Palette;
use crate::widgets::{
    card, empty_state, error_banner, labelled_field, page_title, primary_button, secondary_button,
};
use crate::{t, widgets};
use egui::RichText;

#[derive(Default)]
pub(crate) struct ActionsForm {
    pub filter: usize,
    pub selected_id: Option<i64>,
    pub reason: String,
    pub uncertain_id: Option<i64>,
}

impl AtlasApp {
    pub(crate) fn load_mobile_actions(&mut self) {
        if !self.role_policy().allows_route(Route::Actions)
            || self.mobile_actions.loading
            || self.mobile_action_write.loading
        {
            return;
        }
        let Some(api) = self.client.clone() else {
            return;
        };
        self.mobile_actions
            .start(self.runner.spawn(async move { api.mobile_actions().await }));
    }

    pub(crate) fn load_mobile_action_detail(&mut self, id: i64) {
        if !self.role_policy().allows_route(Route::Actions)
            || id <= 0
            || self.mobile_action_detail.loading
            || self.mobile_action_write.loading
        {
            return;
        }
        let Some(api) = self.client.clone() else {
            return;
        };
        self.actions_form.selected_id = Some(id);
        self.mobile_action_detail = Default::default();
        self.mobile_action_detail.start(
            self.runner
                .spawn(async move { api.mobile_action(id).await }),
        );
    }

    pub(crate) fn submit_mobile_action(&mut self, id: i64, reason: Option<String>) {
        if !self.role_policy().allows_route(Route::Actions)
            || self.mobile_actions.loading
            || self.mobile_action_write.loading
            || self.mobile_action_write.error.is_some()
            || self.actions_form.uncertain_id.is_some()
            || self.mobile_action_detail.loading
            || self.mobile_action_detail.error.is_some()
            || !self
                .mobile_action_detail
                .data
                .as_ref()
                .is_some_and(|item| item.id == id && item.status == "pending")
        {
            return;
        }
        if reason
            .as_ref()
            .is_some_and(|reason| reason.trim().is_empty() || reason.chars().count() > 400)
        {
            return;
        }
        let Some(api) = self.client.clone() else {
            return;
        };
        self.actions_form.uncertain_id = Some(id);
        // The prior detail predates the write and cannot verify its outcome.
        self.mobile_action_detail = Default::default();
        self.mobile_action_write
            .start(self.runner.spawn(async move {
                let current = api.mobile_action(id).await?;
                if current.status != "pending" {
                    return Err(atlas_api::ApiError::client(
                        "La demande n'est plus en attente.",
                    ));
                }
                if reason.is_none() && current.kind != "scan" {
                    return Err(atlas_api::ApiError::client(
                        "Ce type de demande nécessite son workflow dédié.",
                    ));
                }
                match reason {
                    Some(reason) => api.reject_mobile_action(id, &reason).await,
                    None => api.complete_mobile_action(id).await,
                }
            }));
    }
}

/// Display only explicitly supported metadata, never raw JSON or embedded images.
fn metadata_summary(
    raw: Option<&str>,
) -> (
    Option<String>,
    Option<String>,
    Option<usize>,
    Option<String>,
) {
    let Some(value) = raw.and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok()) else {
        return (None, None, None, None);
    };
    let number = |key: &str| {
        value
            .get(key)
            .and_then(|v| v.as_f64())
            .filter(|v| v.is_finite())
            .map(|v| v.to_string())
    };
    let batch = if value.get("cycle_batch").and_then(|v| v.as_bool()) == Some(true) {
        value.get("lines").and_then(|v| v.as_array()).map(Vec::len)
    } else {
        None
    };
    let reason = value
        .get("reject_reason")
        .and_then(|v| v.as_str())
        .map(|s| s.chars().take(400).collect());
    (number("proposed_stock"), number("stock"), batch, reason)
}

pub fn draw(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    page_title(ui, t!(app.locale, "Mobile actions"), palette);
    ui.add_space(8.0);
    ui.label(RichText::new(t!(app.locale, "The list contains up to 100 recent requests and may be cached. Open a request to read its current state.")).color(palette.secondary));
    if let Some(error) = &app.mobile_actions.error {
        error_banner(ui, error, palette);
    }
    if app.mobile_actions.loading && app.mobile_actions.data.is_none() {
        let time = ui.input(|i| i.time);
        widgets::draw_page_skeleton(
            ui,
            palette,
            widgets::PageSkeleton::Actions,
            time,
            app.settings.reduced_motion,
        );
        return;
    }
    if app.mobile_actions.error.is_some() && app.mobile_actions.data.is_none() {
        return;
    }
    ui.add_space(8.0);
    ui.horizontal_wrapped(|ui| {
        for (index, label) in [(0, "Pending"), (1, "Completed"), (2, "All")] {
            if ui
                .selectable_label(
                    app.actions_form.filter == index,
                    crate::i18n::gettext(app.locale, label),
                )
                .clicked()
            {
                app.actions_form.filter = index;
            }
        }
    });
    let rows = app.mobile_actions.data.clone().unwrap_or_default();
    let filtered: Vec<_> = rows
        .iter()
        .filter(|row| match app.actions_form.filter {
            0 => row.status == "pending",
            1 => row.status == "done",
            _ => true,
        })
        .collect();
    if filtered.is_empty() {
        empty_state(
            ui,
            crate::icons::Icon::CircleCheck,
            t!(app.locale, "No mobile actions"),
            t!(app.locale, "Mobile requests will appear here."),
            palette,
        );
    } else {
        for row in filtered {
            card(palette).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal_wrapped(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&row.title).strong().color(palette.text));
                        ui.label(
                            RichText::new(format!(
                                "{} · {} · {}",
                                row.kind,
                                row.status,
                                row.created_at.as_deref().unwrap_or("—")
                            ))
                            .color(palette.secondary)
                            .size(11.5),
                        );
                        if let Some(detail) = row.detail.as_deref().filter(|s| !s.is_empty()) {
                            ui.label(RichText::new(detail).color(palette.secondary));
                        }
                    });
                    if secondary_button(ui, t!(app.locale, "Open request"), palette) {
                        app.load_mobile_action_detail(row.id);
                    }
                });
            });
            ui.add_space(6.0);
        }
    }
    if let Some(error) = &app.mobile_action_write.error {
        error_banner(ui, error, palette);
        ui.label(RichText::new(t!(app.locale, "The server may have processed the request. Read its current state before continuing; the list may be cached.")).color(palette.secondary));
    }
    if let Some(id) = app.actions_form.uncertain_id
        && !app.mobile_action_write.loading
    {
        if secondary_button(ui, t!(app.locale, "Verify current request state"), palette) {
            app.load_mobile_action_detail(id);
        }
        let verified = app.actions_form.selected_id == Some(id)
            && !app.mobile_action_detail.loading
            && app.mobile_action_detail.error.is_none()
            && app
                .mobile_action_detail
                .data
                .as_ref()
                .is_some_and(|row| row.id == id);
        if verified
            && secondary_button(
                ui,
                t!(app.locale, "I have reviewed the current state"),
                palette,
            )
        {
            app.mobile_action_write.error = None;
            app.mobile_action_write.last_error = None;
            app.actions_form.uncertain_id = None;
        }
    }
    if app.mobile_action_detail.loading {
        let time = ui.input(|i| i.time);
        widgets::draw_page_skeleton(
            ui,
            palette,
            widgets::PageSkeleton::TableOnly,
            time,
            app.settings.reduced_motion,
        );
    } else if let Some(error) = &app.mobile_action_detail.error {
        error_banner(ui, error, palette);
    } else if let Some(detail) = app.mobile_action_detail.data.clone() {
        ui.add_space(12.0);
        card(palette).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(format!("{} · #{}", detail.title, detail.id)).strong().color(palette.text));
            ui.label(RichText::new(format!("{} · {}", detail.kind, detail.status)).color(palette.secondary));
            if let Some(text) = detail.detail.as_deref().filter(|s| !s.is_empty()) { ui.label(text); }
            if let Some(barcode) = detail.barcode.as_deref().filter(|s| !s.is_empty()) { ui.label(format!("{}: {barcode}", t!(app.locale, "Barcode"))); }
            if let Some(actor) = detail.created_by.as_deref().filter(|s| !s.is_empty()) { ui.label(format!("{}: {actor}", t!(app.locale, "Requested by"))); }
            if let Some(actor) = detail.completed_by.as_deref().filter(|s| !s.is_empty()) { ui.label(format!("{}: {actor}", t!(app.locale, "Handled by"))); }
            if let Some(id) = detail.product_id { ui.label(format!("{}: #{id}", t!(app.locale, "Product"))); }
            let (proposed, stock, batch, reason) = metadata_summary(detail.meta_json.as_deref());
            if let Some(value) = proposed { ui.label(format!("{}: {value}", t!(app.locale, "Proposed stock"))); }
            if let Some(value) = stock { ui.label(format!("{}: {value}", t!(app.locale, "System stock"))); }
            if let Some(count) = batch { ui.label(format!("{}: {count}", t!(app.locale, "Inventory lines"))); }
            if let Some(reason) = reason.filter(|s| !s.is_empty()) { ui.label(format!("{}: {reason}", t!(app.locale, "Reason for rejection"))); }
            if detail.status == "pending" {
                ui.label(RichText::new(t!(app.locale, "Marking handled does not create a product or invoice and does not apply stock. Review the request before continuing.")).color(palette.secondary));
                let ready = !app.mobile_actions.loading && !app.mobile_action_write.loading && app.actions_form.uncertain_id.is_none() && app.mobile_action_write.error.is_none();
                if detail.kind != "scan" {
                    ui.label(RichText::new(t!(app.locale, "Product, invoice and stock approvals are not available here yet. Use the web workflow to process them.")).color(palette.secondary));
                }
                if detail.kind == "scan" && primary_button(ui, t!(app.locale, "Mark handled"), palette, ready) {
                    app.confirm_dialog = Some(Confirm { title: t!(app.locale, "Mark request handled?").into_owned(), body: t!(app.locale, "No product, invoice or stock change will be created. Another operator may modify this request concurrently.").into_owned(), confirm_label: t!(app.locale, "Mark handled").into_owned(), danger: false, action: ConfirmAction::CompleteMobileAction(detail.id) });
                }
                labelled_field(ui, t!(app.locale, "Reason for rejection"), &mut app.actions_form.reason, palette, false);
                let valid = !app.actions_form.reason.trim().is_empty() && app.actions_form.reason.chars().count() <= 400;
                if primary_button(ui, t!(app.locale, "Reject request"), palette, ready && valid) {
                    app.confirm_dialog = Some(Confirm { title: t!(app.locale, "Reject this request?").into_owned(), body: t!(app.locale, "The reason is visible to the requester. Another operator may modify this request concurrently.").into_owned(), confirm_label: t!(app.locale, "Reject request").into_owned(), danger: true, action: ConfirmAction::RejectMobileAction { id: detail.id, reason: app.actions_form.reason.trim().to_owned() } });
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::metadata_summary;
    #[test]
    fn extracts_only_review_fields_not_images() {
        let raw = r#"{"proposed_stock":12,"stock":4,"cycle_batch":true,"lines":[{},{}],"reject_reason":"test","image_data_url":"data:image/private"}"#;
        let (proposed, stock, count, reason) = metadata_summary(Some(raw));
        assert_eq!(proposed.as_deref(), Some("12"));
        assert_eq!(stock.as_deref(), Some("4"));
        assert_eq!(count, Some(2));
        assert_eq!(reason.as_deref(), Some("test"));
        assert_eq!(metadata_summary(Some("bad json")), (None, None, None, None));
    }
}
