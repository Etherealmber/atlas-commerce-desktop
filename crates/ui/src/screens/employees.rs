//! Employees page: list, filters, detail, pay/advance/resign/reactivate and
//! the printable employee form. Contracts were checked against
//! `worker/routes/employees.ts`; no endpoint or field was invented.

use atlas_api::ApiError;
use atlas_core::format::money;
use egui::{RichText, vec2};

use crate::employees::Employee;
use crate::icons::Icon;
use crate::state::AtlasApp;
use crate::theme::Palette;
use crate::widgets::{
    card, dialog_frame, empty_state, error_banner, icon, labelled_field, page_title,
    primary_button, search_field, secondary_button,
};
use crate::{t, widgets};

#[derive(Default)]
pub struct NewEmployeeForm {
    pub first_name: String,
    pub last_name: String,
    pub phone: String,
    pub email: String,
    pub role_title: String,
    pub department: String,
    pub category: String,
    pub national_id: String,
    pub city: String,
    pub salary: String,
    pub days_cycle: bool,
    pub cycle_days: String,
    pub notes: String,
    pub saving: bool,
    pub error: Option<String>,
}

#[derive(Default)]
pub struct EmployeePayForm {
    pub employee_id: i64,
    pub name: String,
    pub amount: String,
    pub date: String,
    pub note: String,
    pub saving: bool,
    pub error: Option<String>,
}

#[derive(Default)]
pub struct EmployeeResignForm {
    pub employee_id: i64,
    pub name: String,
    pub reason: String,
    pub saving: bool,
    pub error: Option<String>,
}

const STATUSES: [(&str, &str); 3] = [
    ("active", "Actifs"),
    ("resigned", "Démissionnaires"),
    ("all", "Tous"),
];

pub fn draw(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        page_title(ui, t!(app.locale, "Employees"), palette);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if primary_button(ui, t!(app.locale, "Add employee"), palette, true) {
                app.employee_new = Some(NewEmployeeForm::default());
            }
            if secondary_button(ui, t!(app.locale, "Print hiring form"), palette) {
                app.open_printable("/api/employees/form/print", &[]);
            }
            if secondary_button(ui, t!(app.locale, "Refresh"), palette) {
                app.load_employees();
            }
        });
    });
    ui.add_space(12.0);

    if let Some(error) = app.employees.error.clone() {
        error_banner(ui, &error, palette);
        ui.add_space(10.0);
    }

    toolbar(app, ui, palette);
    ui.add_space(12.0);
    metrics(app, ui, palette);
    ui.add_space(12.0);

    if app.employees.loading && app.employees.data.is_none() {
        super::centered_spinner(ui, palette);
    } else if app
        .employees
        .data
        .as_ref()
        .is_none_or(|list| list.rows.is_empty())
    {
        empty_state(
            ui,
            Icon::Users,
            t!(app.locale, "No employee"),
            t!(
                app.locale,
                "Add a collaborator with identity, role and salary."
            ),
            palette,
        );
    } else {
        card(palette).show(ui, |ui| {
            ui.set_width(ui.available_width());
            list(app, ui, palette);
        });
    }

    if app.employee_detail_id.is_some() {
        ui.add_space(12.0);
        card(palette).show(ui, |ui| {
            ui.set_width(ui.available_width());
            detail(app, ui, palette);
        });
    }

    if app.employee_new.is_some() {
        new_dialog(app, ui, palette);
    }
    if app.employee_pay.is_some() {
        pay_dialog(app, ui, palette);
    }
    if app.employee_resign.is_some() {
        resign_dialog(app, ui, palette);
    }
}

fn toolbar(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    card(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            let width = (ui.available_width() - 300.0).clamp(180.0, 460.0);
            icon(ui, Icon::Search, palette.secondary, 16.0);
            let response = search_field(
                ui,
                &mut app.employee_search,
                t!(app.locale, "Name, number, role, phone or ID"),
                width,
            );
            if response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
                app.load_employees();
            }
            ui.label(RichText::new(t!(app.locale, "Status")).color(palette.secondary));
            egui::ComboBox::from_id_salt("employee-status")
                .selected_text(status_label(&app.employee_status))
                .show_ui(ui, |ui| {
                    for (value, label) in STATUSES {
                        if ui
                            .selectable_label(app.employee_status == value, label)
                            .clicked()
                        {
                            app.employee_status = value.to_owned();
                            app.load_employees();
                        }
                    }
                });
            let categories = app
                .employees
                .data
                .as_ref()
                .map(|list| list.categories.clone())
                .unwrap_or_default();
            if !categories.is_empty() {
                egui::ComboBox::from_id_salt("employee-category")
                    .selected_text(if app.employee_category.is_empty() {
                        t!(app.locale, "All categories").into_owned()
                    } else {
                        app.employee_category.clone()
                    })
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(app.employee_category.is_empty(), "—")
                            .clicked()
                        {
                            app.employee_category.clear();
                            app.load_employees();
                        }
                        for category in &categories {
                            if ui
                                .selectable_label(app.employee_category == *category, category)
                                .clicked()
                            {
                                app.employee_category = category.clone();
                                app.load_employees();
                            }
                        }
                    });
            }
        });
    });
}

fn status_label(value: &str) -> String {
    STATUSES
        .iter()
        .find(|(status, _)| *status == value)
        .map(|(_, label)| (*label).to_owned())
        .unwrap_or_else(|| "Tous".to_owned())
}

fn metrics(app: &AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    let rows = app
        .employees
        .data
        .as_ref()
        .map(|list| list.rows.as_slice())
        .unwrap_or_default();
    let active = rows.iter().filter(|row| row.is_active()).count();
    let due = rows.iter().filter(|row| row.pay_overdue).count();
    let payroll: f64 = rows
        .iter()
        .filter(|row| row.is_active())
        .map(|row| row.pay_amount)
        .sum();
    let currency = app
        .dashboard
        .data
        .as_ref()
        .map(|data| data.currency().to_owned())
        .unwrap_or_else(|| "MAD".to_owned());
    let values = [
        (
            t!(app.locale, "Active").into_owned(),
            active.to_string(),
            false,
        ),
        (t!(app.locale, "Due").into_owned(), due.to_string(), due > 0),
        (
            t!(app.locale, "Payroll (active)").into_owned(),
            money(payroll, &currency),
            false,
        ),
    ];
    // Three equal columns, minus the card's own 16 px padding on each side:
    // without that reserve the right column paints past the window edge.
    const GAP: f32 = 12.0;
    const PADDING: f32 = 32.0;
    let columns = 3.0;
    let width = ((ui.available_width() - GAP * (columns - 1.0)) / columns - PADDING).max(96.0);
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = GAP;
        for (label, value, warn) in values {
            card(palette).show(ui, |ui| {
                ui.set_width(width);
                ui.set_min_height(64.0);
                ui.label(
                    RichText::new(label)
                        .color(palette.secondary)
                        .font(crate::fonts::medium(12.0)),
                );
                ui.add_space(6.0);
                // Long amounts drop a few points instead of clipping.
                let size = if value.chars().count() > 10 {
                    15.0
                } else {
                    20.0
                };
                ui.add(
                    egui::Label::new(
                        RichText::new(value)
                            .color(if warn {
                                palette.finance_warning
                            } else {
                                palette.text
                            })
                            .font(crate::fonts::mono_semibold(size)),
                    )
                    .truncate(),
                );
            });
        }
    });
}

fn list(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    let rows = match app.employees.data.as_ref() {
        Some(list) => list.rows.clone(),
        None => return,
    };
    let currency = app
        .dashboard
        .data
        .as_ref()
        .map(|data| data.currency().to_owned())
        .unwrap_or_else(|| "MAD".to_owned());
    for employee in rows {
        ui.add_space(6.0);
        ui.vertical(|ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                let selected = app.employee_detail_id == Some(employee.id);
                let name = RichText::new(employee.label())
                    .color(if selected {
                        palette.accent
                    } else {
                        palette.text
                    })
                    .strong()
                    .size(13.5);
                if ui
                    .add(egui::Label::new(name).sense(egui::Sense::click()))
                    .clicked()
                {
                    app.employee_detail_id = Some(employee.id);
                    app.load_employee_detail(employee.id);
                }
                ui.label(
                    RichText::new(format!(
                        "{} · {}",
                        if employee.emp_number.is_empty() {
                            "—"
                        } else {
                            &employee.emp_number
                        },
                        if employee.role_title.is_empty() {
                            "—"
                        } else {
                            &employee.role_title
                        }
                    ))
                    .color(palette.dim)
                    .size(12.0),
                );
                if !employee.is_active() {
                    ui.label(
                        RichText::new(t!(app.locale, "Resigned"))
                            .color(palette.finance_warning)
                            .size(12.0),
                    );
                }
                ui.label(
                    RichText::new(format!(
                        "{} · {}",
                        money(employee.pay_amount, &currency),
                        employee.cycle_label()
                    ))
                    .color(palette.secondary)
                    .size(12.0),
                );
                if employee.pay_overdue {
                    ui.label(
                        RichText::new(format!(
                            "{} · {}",
                            employee.next_due,
                            t!(app.locale, "to pay")
                        ))
                        .color(palette.finance_warning)
                        .size(12.0),
                    );
                } else if !employee.next_due.is_empty() {
                    ui.label(
                        RichText::new(format!("→ {}", employee.next_due))
                            .color(palette.dim)
                            .size(12.0),
                    );
                }
            });
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                if employee.is_active() {
                    if primary_button(ui, t!(app.locale, "Mark paid"), palette, true) {
                        open_pay(&employee, app);
                    }
                } else if secondary_button(ui, t!(app.locale, "Reactivate"), palette) {
                    app.reactivate_employee(employee.id);
                }
                if secondary_button(ui, t!(app.locale, "File"), palette) {
                    app.open_printable(&format!("/api/employees/{}/form/print", employee.id), &[]);
                }
                if secondary_button(ui, t!(app.locale, "Situation"), palette) {
                    app.open_printable(
                        "/api/reports/employee-situation",
                        &[("employee_id", employee.id.to_string())],
                    );
                }
            });
        });
        ui.separator();
    }
}

fn open_pay(employee: &Employee, app: &mut AtlasApp) {
    let suggested = if employee.suggested_pay > 0.0 {
        employee.suggested_pay
    } else {
        employee.pay_amount
    };
    let note = employee
        .period_from
        .as_deref()
        .map_or_else(String::new, |from| {
            format!(
                "Période {} → aujourd’hui ({} j · cycle {} j)",
                from.chars().take(10).collect::<String>(),
                employee.accrued_days.max(0),
                employee.cycle_days.max(1)
            )
        });
    app.employee_pay = Some(EmployeePayForm {
        employee_id: employee.id,
        name: employee.label().to_owned(),
        amount: format!("{suggested:.2}"),
        date: today(),
        note,
        saving: false,
        error: None,
    });
}

fn detail(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    let Some(id) = app.employee_detail_id else {
        return;
    };
    let currency = app
        .dashboard
        .data
        .as_ref()
        .map(|data| data.currency().to_owned())
        .unwrap_or_else(|| "MAD".to_owned());
    if app.employee_detail.loading && app.employee_detail.data.is_none() {
        super::centered_spinner(ui, palette);
        return;
    }
    if let Some(error) = app.employee_detail.error.clone() {
        error_banner(ui, &error, palette);
        return;
    }
    let Some(detail) = app.employee_detail.data.clone() else {
        return;
    };
    let employee = &detail.employee;
    ui.label(
        RichText::new(employee.label())
            .color(palette.text)
            .font(crate::fonts::semibold(16.0)),
    );
    ui.label(
        RichText::new(format!(
            "{} · {} · {}",
            employee.emp_number, employee.role_title, employee.department
        ))
        .color(palette.secondary)
        .size(12.5),
    );
    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        if secondary_button(ui, t!(app.locale, "Close"), palette) {
            app.employee_detail_id = None;
            app.employee_detail = Default::default();
        }
        if secondary_button(ui, t!(app.locale, "File"), palette) {
            app.open_printable(&format!("/api/employees/{}/form/print", employee.id), &[]);
        }
        if secondary_button(ui, t!(app.locale, "Situation"), palette) {
            app.open_printable(
                "/api/reports/employee-situation",
                &[("employee_id", employee.id.to_string())],
            );
        }
    });
    ui.add_space(8.0);
    let lines = [
        (t!(app.locale, "Phone").into_owned(), employee.phone.clone()),
        (
            t!(app.locale, "E-mail").into_owned(),
            employee.email.clone(),
        ),
        (
            t!(app.locale, "National ID").into_owned(),
            employee.national_id.clone(),
        ),
        (
            t!(app.locale, "Joined").into_owned(),
            employee.joined_date.clone(),
        ),
        (
            t!(app.locale, "Salary").into_owned(),
            format!(
                "{} · {}",
                money(employee.pay_amount, &currency),
                employee.cycle_label()
            ),
        ),
        (
            t!(app.locale, "Next due").into_owned(),
            employee.next_due.clone(),
        ),
        (
            t!(app.locale, "Accrued").into_owned(),
            format!(
                "{} ({} j · avances {})",
                money(employee.gross_accrued, &currency),
                employee.accrued_days.max(0),
                money(employee.advances_in_period, &currency)
            ),
        ),
        (
            t!(app.locale, "Last payment").into_owned(),
            employee
                .last_paid_at
                .as_deref()
                .map(|value| value[..value.len().min(10)].to_owned())
                .unwrap_or_else(|| "—".to_owned()),
        ),
    ];
    egui::Grid::new("employee-detail-grid")
        .num_columns(2)
        .spacing([18.0, 6.0])
        .show(ui, |ui| {
            for (label, value) in &lines {
                ui.label(RichText::new(label).color(palette.secondary).size(12.0));
                ui.label(
                    RichText::new(if value.is_empty() { "—" } else { value })
                        .color(palette.text)
                        .size(12.5),
                );
                ui.end_row();
            }
        });

    ui.add_space(8.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        if employee.is_active() {
            if primary_button(ui, t!(app.locale, "Mark paid"), palette, true) {
                open_pay(employee, app);
            }
            if secondary_button(ui, t!(app.locale, "Advance"), palette) {
                let amount = (employee.pay_amount * 0.05).max(0.0);
                app.advance_employee(employee.id, amount, "");
            }
            if secondary_button(ui, t!(app.locale, "Resign"), palette) {
                app.employee_resign = Some(EmployeeResignForm {
                    employee_id: employee.id,
                    name: employee.label().to_owned(),
                    reason: if employee.employment_status.is_empty() {
                        "Démission".to_owned()
                    } else {
                        employee.employment_status.clone()
                    },
                    saving: false,
                    error: None,
                });
            }
        } else if secondary_button(ui, t!(app.locale, "Reactivate"), palette) {
            app.reactivate_employee(employee.id);
        }
        ui.label(
            RichText::new(t!(
                app.locale,
                "Advances and settlements follow the server rules; nothing is rounded locally."
            ))
            .color(palette.dim)
            .size(11.5),
        );
    });

    ui.add_space(12.0);
    ui.label(
        RichText::new(t!(app.locale, "Payment history"))
            .color(palette.text)
            .strong(),
    );
    if detail.payments.is_empty() {
        ui.label(
            RichText::new(t!(app.locale, "No payment recorded."))
                .color(palette.dim)
                .size(12.0),
        );
    } else {
        egui::ScrollArea::vertical()
            .id_salt(("employee-payments", id))
            .max_height(260.0)
            .show(ui, |ui| {
                for payment in &detail.payments {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(
                            RichText::new(&payment.paid_at)
                                .color(palette.secondary)
                                .size(12.0),
                        );
                        ui.label(
                            RichText::new(money(payment.amount, &currency))
                                .color(palette.text)
                                .size(12.5),
                        );
                        if !payment.period_label.is_empty() {
                            ui.label(
                                RichText::new(&payment.period_label)
                                    .color(palette.dim)
                                    .size(11.5),
                            );
                        }
                        if !payment.note.is_empty() {
                            ui.label(RichText::new(&payment.note).color(palette.dim).size(11.5));
                        }
                    });
                }
            });
    }
}

fn new_dialog(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    let screen = ui.ctx().input(|input| input.viewport_rect());
    let rect = egui::Rect::from_center_size(
        screen.center(),
        vec2(560.0, 620.0_f32.min((screen.height() - 32.0).max(1.0))),
    );
    if widgets::scrim_clicked(ui, screen, rect, egui::Id::new("employee-new-scrim")) {
        app.employee_new = None;
        return;
    }
    let locale = app.locale;
    let (mut save, mut cancel) = (false, false);
    widgets::overlay_at(ui, rect, |ui| {
        dialog_frame(palette).show(ui, |ui| {
            egui::ScrollArea::vertical()
                .max_height((rect.height() - 46.0).max(1.0))
                .show(ui, |ui| {
                    ui.set_min_size(vec2(512.0, 0.0));
                    ui.label(
                        RichText::new(t!(locale, "New employee"))
                            .color(palette.text)
                            .font(crate::fonts::semibold(17.0)),
                    );
                    ui.add_space(10.0);
                    if let Some(form) = app.employee_new.as_mut() {
                        ui.columns(2, |columns| {
                            labelled_field(
                                &mut columns[0],
                                t!(locale, "First name"),
                                &mut form.first_name,
                                palette,
                                false,
                            );
                            labelled_field(
                                &mut columns[1],
                                t!(locale, "Last name"),
                                &mut form.last_name,
                                palette,
                                false,
                            );
                        });
                        ui.add_space(8.0);
                        ui.columns(2, |columns| {
                            labelled_field(
                                &mut columns[0],
                                t!(locale, "Phone"),
                                &mut form.phone,
                                palette,
                                false,
                            );
                            labelled_field(
                                &mut columns[1],
                                t!(locale, "E-mail"),
                                &mut form.email,
                                palette,
                                false,
                            );
                        });
                        ui.add_space(8.0);
                        ui.columns(2, |columns| {
                            labelled_field(
                                &mut columns[0],
                                t!(locale, "Role"),
                                &mut form.role_title,
                                palette,
                                false,
                            );
                            labelled_field(
                                &mut columns[1],
                                t!(locale, "Department"),
                                &mut form.department,
                                palette,
                                false,
                            );
                        });
                        ui.add_space(8.0);
                        ui.columns(2, |columns| {
                            labelled_field(
                                &mut columns[0],
                                t!(locale, "National ID"),
                                &mut form.national_id,
                                palette,
                                false,
                            );
                            labelled_field(
                                &mut columns[1],
                                t!(locale, "City"),
                                &mut form.city,
                                palette,
                                false,
                            );
                        });
                        ui.add_space(8.0);
                        ui.columns(2, |columns| {
                            labelled_field(
                                &mut columns[0],
                                t!(locale, "Salary"),
                                &mut form.salary,
                                palette,
                                false,
                            );
                            columns[1].vertical(|ui| {
                                ui.label(
                                    RichText::new(t!(locale, "Pay cycle"))
                                        .color(palette.secondary)
                                        .font(crate::fonts::medium(12.0)),
                                );
                                ui.horizontal(|ui| {
                                    ui.selectable_value(
                                        &mut form.days_cycle,
                                        false,
                                        t!(locale, "Monthly"),
                                    );
                                    ui.selectable_value(
                                        &mut form.days_cycle,
                                        true,
                                        t!(locale, "Every N days"),
                                    );
                                });
                                if form.days_cycle {
                                    labelled_field(
                                        ui,
                                        t!(locale, "Days"),
                                        &mut form.cycle_days,
                                        palette,
                                        false,
                                    );
                                }
                            });
                        });
                        ui.add_space(8.0);
                        labelled_field(ui, t!(locale, "Notes"), &mut form.notes, palette, false);
                        if let Some(error) = form.error.clone() {
                            ui.add_space(8.0);
                            error_banner(ui, &error, palette);
                        }
                        ui.add_space(12.0);
                        let saving = form.saving;
                        widgets::dialog_actions(ui, |ui| {
                            save = primary_button(ui, t!(locale, "Save"), palette, !saving);
                            cancel = secondary_button(ui, t!(locale, "Cancel"), palette);
                        });
                    }
                });
        });
    });
    if cancel {
        app.employee_new = None;
    } else if save {
        let valid = app
            .employee_new
            .as_ref()
            .is_some_and(|form| validate_new(form).is_none());
        if valid {
            app.create_employee();
        }
    }
}

fn validate_new(form: &NewEmployeeForm) -> Option<&'static str> {
    if form.first_name.trim().is_empty() && form.last_name.trim().is_empty() {
        return Some("Indiquez le prénom ou le nom de famille.");
    }
    let salary = form.salary.trim().replace(',', ".");
    if !salary.is_empty() && salary.parse::<f64>().is_err() {
        return Some("Chiffres uniquement pour le salaire.");
    }
    None
}

fn pay_dialog(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    let screen = ui.ctx().input(|input| input.viewport_rect());
    let rect = egui::Rect::from_center_size(
        screen.center(),
        vec2(480.0, 360.0_f32.min((screen.height() - 32.0).max(1.0))),
    );
    if widgets::scrim_clicked(ui, screen, rect, egui::Id::new("employee-pay-scrim")) {
        app.employee_pay = None;
        return;
    }
    let locale = app.locale;
    let (mut save, mut cancel) = (false, false);
    widgets::overlay_at(ui, rect, |ui| {
        dialog_frame(palette).show(ui, |ui| {
            ui.set_min_size(vec2(436.0, 0.0));
            if let Some(form) = app.employee_pay.as_mut() {
                ui.label(
                    RichText::new(format!("{} — {}", t!(locale, "Mark paid"), form.name))
                        .color(palette.text)
                        .font(crate::fonts::semibold(16.0)),
                );
                ui.add_space(10.0);
                labelled_field(ui, t!(locale, "Amount"), &mut form.amount, palette, false);
                ui.add_space(8.0);
                labelled_field(ui, t!(locale, "Date"), &mut form.date, palette, false);
                ui.add_space(8.0);
                labelled_field(ui, t!(locale, "Note"), &mut form.note, palette, false);
                if let Some(error) = form.error.clone() {
                    ui.add_space(8.0);
                    error_banner(ui, &error, palette);
                }
                ui.add_space(12.0);
                let saving = form.saving;
                widgets::dialog_actions(ui, |ui| {
                    save = primary_button(ui, t!(locale, "Confirm"), palette, !saving);
                    cancel = secondary_button(ui, t!(locale, "Cancel"), palette);
                });
            }
        });
    });
    if cancel {
        app.employee_pay = None;
    } else if save {
        app.pay_employee();
    }
}

fn resign_dialog(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    let screen = ui.ctx().input(|input| input.viewport_rect());
    let rect = egui::Rect::from_center_size(
        screen.center(),
        vec2(480.0, 320.0_f32.min((screen.height() - 32.0).max(1.0))),
    );
    if widgets::scrim_clicked(ui, screen, rect, egui::Id::new("employee-resign-scrim")) {
        app.employee_resign = None;
        return;
    }
    let locale = app.locale;
    let (mut save, mut cancel) = (false, false);
    widgets::overlay_at(ui, rect, |ui| {
        dialog_frame(palette).show(ui, |ui| {
            ui.set_min_size(vec2(436.0, 0.0));
            if let Some(form) = app.employee_resign.as_mut() {
                ui.label(
                    RichText::new(format!("{} — {}", t!(locale, "Resign"), form.name))
                        .color(palette.text)
                        .font(crate::fonts::semibold(16.0)),
                );
                ui.add_space(6.0);
                ui.label(
                    RichText::new(t!(
                        locale,
                        "The server records the resignation and keeps the payment history."
                    ))
                    .color(palette.secondary)
                    .size(12.0),
                );
                ui.add_space(10.0);
                labelled_field(ui, t!(locale, "Reason"), &mut form.reason, palette, false);
                if let Some(error) = form.error.clone() {
                    ui.add_space(8.0);
                    error_banner(ui, &error, palette);
                }
                ui.add_space(12.0);
                let saving = form.saving;
                widgets::dialog_actions(ui, |ui| {
                    save = primary_button(ui, t!(locale, "Confirm"), palette, !saving);
                    cancel = secondary_button(ui, t!(locale, "Cancel"), palette);
                });
            }
        });
    });
    if cancel {
        app.employee_resign = None;
    } else if save {
        app.resign_employee();
    }
}

fn today() -> String {
    chrono::Utc::now().format("%Y-%m-%d").to_string()
}

/// Errors from employee writes surface inside the open dialog when there is
/// one, so the message stays next to the field that caused it.
pub(crate) fn action_failed(app: &mut AtlasApp, error: &ApiError) {
    if let Some(form) = app.employee_new.as_mut() {
        form.saving = false;
        form.error = Some(error.message.clone());
    } else if let Some(form) = app.employee_pay.as_mut() {
        form.saving = false;
        form.error = Some(error.message.clone());
    } else if let Some(form) = app.employee_resign.as_mut() {
        form.saving = false;
        form.error = Some(error.message.clone());
    } else {
        app.notice(error.message.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::{NewEmployeeForm, validate_new};

    #[test]
    fn creation_requires_a_name_and_a_numeric_salary() {
        let mut form = NewEmployeeForm::default();
        assert!(validate_new(&form).is_some());
        form.first_name = "Amina".to_owned();
        assert!(validate_new(&form).is_none());
        form.salary = "abc".to_owned();
        assert!(validate_new(&form).is_some());
        form.salary = "4 500,00".to_owned();
        assert!(
            validate_new(&form).is_some(),
            "spaces are rejected, not guessed"
        );
        form.salary = "4500,50".to_owned();
        assert!(validate_new(&form).is_none());
    }
}
