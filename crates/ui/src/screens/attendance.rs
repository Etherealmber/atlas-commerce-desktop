//! Today’s attendance, backed by the Worker's attendance endpoints.
//! The server determines the Morocco-local date and time.
use egui::RichText;

use crate::state::AtlasApp;
use crate::t;
use crate::theme::Palette;
use crate::widgets::{card, error_banner, page_title, primary_button};

pub fn draw(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    let disabled = app
        .attendance
        .last_error
        .as_ref()
        .is_some_and(|error| error.code.as_deref() == Some("ATTENDANCE_DISABLED"));
    page_title(ui, t!(app.locale, "Attendance"), palette);
    ui.add_space(12.0);
    if disabled {
        card(palette).show(ui, |ui| {
            ui.label(
                RichText::new(t!(app.locale, "Attendance is disabled."))
                    .strong()
                    .color(palette.text),
            );
            ui.label(
                RichText::new(t!(
                    app.locale,
                    "Enable attendance for the company to use check-in and check-out."
                ))
                .color(palette.secondary),
            );
        });
    } else if let Some(error) = &app.attendance.error {
        error_banner(ui, error, palette);
    }
    if let Some(error) = &app.attendance_action.error {
        error_banner(ui, error, palette);
        ui.label(
            RichText::new(t!(
                app.locale,
                "Refresh before retrying: the server may have saved the change."
            ))
            .color(palette.secondary),
        );
    }
    if disabled {
        return;
    }
    if app.attendance.loading && app.attendance.data.is_none() {
        let time = ui.input(|input| input.time);
        crate::widgets::draw_page_skeleton(
            ui,
            palette,
            crate::widgets::PageSkeleton::Attendance,
            time,
            app.settings.reduced_motion,
        );
        return;
    }
    let Some(day) = app.attendance.data.clone() else {
        return;
    };
    ui.label(
        RichText::new(format!("{} · {}", t!(app.locale, "Date"), day.date))
            .color(palette.secondary),
    );
    ui.add_space(8.0);
    ui.horizontal_wrapped(|ui| {
        for (label, count) in [
            (t!(app.locale, "Present"), day.summary.present),
            (t!(app.locale, "Absent"), day.summary.absent),
            (t!(app.locale, "Late"), day.summary.late),
            (t!(app.locale, "Excused"), day.summary.excused),
            (t!(app.locale, "Total"), day.summary.total),
        ] {
            ui.label(
                RichText::new(format!("{label} : {count}"))
                    .strong()
                    .color(palette.text),
            );
            ui.add_space(12.0);
        }
    });
    ui.add_space(12.0);
    if day.rows.is_empty() {
        ui.label(RichText::new(t!(app.locale, "No active employee.")).color(palette.secondary));
        return;
    }
    let mut selected = None;
    card(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());
        for row in &day.rows {
            ui.horizontal_wrapped(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new(&row.name).strong().color(palette.text));
                    ui.label(
                        RichText::new(format!(
                            "{} · {}",
                            row.role_title,
                            attendance_status(app, &row.status)
                        ))
                        .color(palette.secondary),
                    );
                    ui.label(
                        RichText::new(format!(
                            "{} : {}  ·  {} : {}",
                            t!(app.locale, "Check-in"),
                            row.check_in.as_deref().unwrap_or("—"),
                            t!(app.locale, "Check-out"),
                            row.check_out.as_deref().unwrap_or("—")
                        ))
                        .color(palette.secondary),
                    );
                    if !row.note.is_empty() {
                        ui.label(RichText::new(&row.note).color(palette.dim));
                    }
                });
                let ready = !disabled
                    && !app.attendance.loading
                    && app.attendance.error.is_none()
                    && !app.attendance_action.loading
                    && app.attendance_action.error.is_none();
                if row.check_in.is_none() && row.status != "excused" {
                    if primary_button(ui, t!(app.locale, "Check-in"), palette, ready) {
                        selected = Some((row.employee_id, false));
                    }
                } else if row.check_in.is_some()
                    && row.check_out.is_none()
                    && primary_button(ui, t!(app.locale, "Check-out"), palette, ready)
                {
                    selected = Some((row.employee_id, true));
                }
            });
            ui.separator();
        }
    });
    if let Some((id, check_out)) = selected {
        app.mark_attendance(id, day.date, check_out);
    }
}

fn attendance_status(app: &AtlasApp, status: &str) -> String {
    match status {
        "present" => t!(app.locale, "Present").into_owned(),
        "absent" => t!(app.locale, "Absent").into_owned(),
        "late" => t!(app.locale, "Late").into_owned(),
        "excused" => t!(app.locale, "Excused").into_owned(),
        "half" => t!(app.locale, "Half day").into_owned(),
        _ => status.to_owned(),
    }
}
