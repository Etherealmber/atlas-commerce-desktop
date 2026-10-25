//! Delivery notes (`bons de livraison`): the 100 most recent notes with a
//! standalone-note creator. Wire contract: the `/api/delivery-notes` block
//! of the worker. The worker assigns the `BL-YYYY-NNNN` number, the
//! `confirmed` status and, when an invoice is linked, copies its items —
//! this screen creates standalone notes (`invoice_id = 0`).

use atlas_api::ApiError;
use atlas_api::models::{DeliveryNote, DeliveryNoteWrite};
use atlas_core::jobs::Job;
use egui::{RichText, Sense, vec2};

use crate::icons::Icon;
use crate::state::AtlasApp;
use crate::theme::Palette;
use crate::widgets::{
    card, empty_state, error_banner, icon, labelled_field, page_title, pill, primary_button,
    secondary_button,
};
use crate::{t, widgets};

/// Standalone-note create form.
#[derive(Default)]
pub struct DeliveryNoteEditor {
    pub client_name: String,
    pub issue_date: String,
    pub notes: String,
    pub saving: bool,
    pub error: Option<String>,
    pub job: Option<Job<Result<DeliveryNote, ApiError>>>,
}

impl DeliveryNoteEditor {
    #[must_use]
    pub fn new() -> Self {
        Self {
            issue_date: chrono::Local::now().format("%Y-%m-%d").to_string(),
            ..Self::default()
        }
    }

    fn validate(&self) -> Option<&'static str> {
        if self.client_name.trim().is_empty() {
            return Some("The client name is required.");
        }
        if self.client_name.chars().count() > 200 {
            return Some("The client name must be 200 characters or fewer.");
        }
        None
    }

    fn to_write(&self) -> DeliveryNoteWrite {
        DeliveryNoteWrite {
            invoice_id: 0,
            client_name: self.client_name.trim().to_owned(),
            issue_date: self.issue_date.trim().to_owned(),
            notes: self.notes.trim().to_owned(),
        }
    }
}

pub fn draw(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    poll_editor(app);

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        page_title(ui, t!(app.locale, "Delivery notes"), palette);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if primary_button(ui, t!(app.locale, "New delivery note"), palette, true) {
                app.delivery_note_editor = Some(DeliveryNoteEditor::new());
            }
            if secondary_button(ui, t!(app.locale, "Refresh"), palette) {
                app.load_delivery_notes();
            }
        });
    });
    ui.add_space(12.0);

    if let Some(error) = app.delivery_notes.error.clone() {
        error_banner(ui, &error, palette);
        ui.add_space(10.0);
    }

    card(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());
        if app.delivery_notes.loading && app.delivery_notes.data.is_none() {
            super::centered_spinner(ui, palette);
            return;
        }
        let notes = app.delivery_notes.data.clone().unwrap_or_default();
        if notes.is_empty() {
            empty_state(
                ui,
                Icon::Truck,
                t!(app.locale, "No delivery note yet"),
                t!(app.locale, "Created delivery notes appear here."),
                palette,
            );
            return;
        }
        for note in &notes {
            note_row(ui, palette, app.locale, note);
            ui.separator();
        }
    });

    if app.delivery_note_editor.is_some() {
        draw_editor(app, ui, palette);
    }
}

fn note_row(
    ui: &mut egui::Ui,
    palette: &Palette,
    locale: crate::i18n::Locale,
    note: &DeliveryNote,
) {
    let height = 56.0;
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    if response.hovered() {
        ui.painter().rect_filled(rect, 10.0, palette.surface_hover);
    }
    widgets::overlay_at(
        ui,
        egui::Rect::from_min_size(rect.min + vec2(8.0, 0.0), rect.size() - vec2(16.0, 0.0)),
        |ui| {
            ui.horizontal_centered(|ui| {
                icon(ui, Icon::Truck, palette.secondary, 18.0);
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(note.number.clone())
                            .color(palette.text)
                            .strong()
                            .size(13.5),
                    );
                    let subline = [
                        if note.client_name.is_empty() {
                            String::new()
                        } else {
                            note.client_name.clone()
                        },
                        note.issue_date.clone(),
                    ]
                    .into_iter()
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
                    .join(" · ");
                    ui.label(
                        RichText::new(if subline.is_empty() {
                            "—".to_owned()
                        } else {
                            subline
                        })
                        .color(palette.dim)
                        .size(11.5),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    pill(
                        ui,
                        crate::i18n::gettext(locale, status_label(note.status.as_str())),
                        palette.surface_active,
                        palette.secondary,
                    );
                });
            });
        },
    );
}

fn status_label(status: &str) -> &'static str {
    match status {
        "confirmed" => "Confirmed",
        "draft" => "Draft",
        "cancelled" => "Cancelled",
        _ => "Confirmed",
    }
}

fn draw_editor(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    let screen = ui.ctx().input(|input| input.viewport_rect());
    let rect = egui::Rect::from_center_size(screen.center(), vec2(480.0, 420.0));
    ui.painter()
        .rect_filled(screen, 0.0, egui::Color32::from_black_alpha(140));

    let locale = app.locale;
    widgets::overlay_at(ui, rect, |ui| {
        widgets::dialog_frame(palette).show(ui, |ui| {
            ui.set_min_size(vec2(436.0, 0.0));
            ui.label(
                RichText::new(t!(locale, "New delivery note"))
                    .color(palette.text)
                    .strong()
                    .size(17.0),
            );
            ui.add_space(10.0);

            if let Some(editor) = app.delivery_note_editor.as_mut() {
                labelled_field(
                    ui,
                    t!(locale, "Client name"),
                    &mut editor.client_name,
                    palette,
                    false,
                );
                ui.add_space(8.0);
                labelled_field(
                    ui,
                    t!(locale, "Issue date"),
                    &mut editor.issue_date,
                    palette,
                    false,
                );
                ui.add_space(8.0);
                labelled_field(
                    ui,
                    t!(locale, "Notes (optional)"),
                    &mut editor.notes,
                    palette,
                    false,
                );
            }

            if let Some(error) = app
                .delivery_note_editor
                .as_ref()
                .and_then(|editor| editor.error.clone())
            {
                ui.add_space(8.0);
                error_banner(ui, &error, palette);
            }

            ui.add_space(12.0);
            widgets::dialog_actions(ui, |ui| {
                let saving = app
                    .delivery_note_editor
                    .as_ref()
                    .is_some_and(|editor| editor.saving);
                let save = primary_button(ui, t!(locale, "Save"), palette, !saving);
                let cancel = secondary_button(ui, t!(locale, "Cancel"), palette);
                if cancel {
                    app.delivery_note_editor = None;
                }
                if save {
                    submit_editor(app);
                }
            });
        });
    });
}

fn poll_editor(app: &mut AtlasApp) {
    if let Some(editor) = app.delivery_note_editor.as_mut()
        && let Some(job) = &editor.job
        && let Some(result) = job.poll()
    {
        editor.job = None;
        editor.saving = false;
        match result {
            Ok(note) => {
                app.delivery_note_editor = None;
                app.notice(format!(
                    "{} {}",
                    t!(app.locale, "Delivery note created."),
                    note.number
                ));
                app.load_delivery_notes();
            }
            Err(error) => editor.error = Some(error.message),
        }
    }
}

fn submit_editor(app: &mut AtlasApp) {
    let Some(client) = app.client.clone() else {
        return;
    };
    let Some(editor) = app.delivery_note_editor.as_mut() else {
        return;
    };
    if let Some(validation) = editor.validate() {
        editor.error = Some(crate::i18n::gettext(app.locale, validation).into_owned());
        return;
    }
    let body = editor.to_write();
    editor.saving = true;
    editor.error = None;
    let job = app
        .runner
        .spawn(async move { client.create_delivery_note(&body).await });
    editor.job = Some(job);
}
