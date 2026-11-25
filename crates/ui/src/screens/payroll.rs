//! Read-only payroll runs. The Worker is authoritative for statutory amounts.
//! Creating, editing, closing and exporting runs are not implemented here yet.
use crate::state::{AtlasApp, Route};
use crate::theme::Palette;
use crate::widgets::{card, empty_state, error_banner, page_title, secondary_button};
use crate::{t, widgets};
use egui::RichText;

impl AtlasApp {
    pub(crate) fn load_payroll(&mut self) {
        if !self.role_policy().allows_route(Route::Payroll) {
            return;
        }
        let Some(api) = self.client.clone() else {
            return;
        };
        if !self.payroll_runs.loading {
            let api = api.clone();
            self.payroll_runs
                .start(self.runner.spawn(async move { api.payroll_runs().await }));
        }
        if !self.payroll_settings.loading {
            self.payroll_settings.start(
                self.runner
                    .spawn(async move { api.payroll_settings().await }),
            );
        }
    }

    pub(crate) fn load_payroll_detail(&mut self, id: i64) {
        if !self.role_policy().allows_route(Route::Payroll)
            || id <= 0
            || self.payroll_detail.loading
        {
            return;
        }
        let Some(api) = self.client.clone() else {
            return;
        };
        self.payroll_selected_id = Some(id);
        self.payroll_detail = Default::default();
        self.payroll_detail
            .start(self.runner.spawn(async move { api.payroll_run(id).await }));
    }

    pub(crate) fn refresh_payroll(&mut self) {
        self.load_payroll();
        if let Some(id) = self.payroll_selected_id {
            self.load_payroll_detail(id);
        }
    }
}

fn amount(value: f64, currency: &str) -> String {
    format!("{value:.2} {currency}")
}

pub fn draw(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    let currency = app
        .dashboard
        .data
        .as_ref()
        .map(|data| data.currency().to_owned())
        .unwrap_or_else(|| "MAD".to_owned());
    page_title(ui, t!(app.locale, "Payroll CNSS / AMO"), palette);
    ui.label(RichText::new(t!(app.locale, "Read-only payroll overview. Create, edit, close and export payroll runs in the web application.")).color(palette.secondary));
    ui.add_space(10.0);
    if let Some(error) = &app.payroll_runs.error {
        error_banner(ui, error, palette);
    }
    if app.payroll_runs.loading && app.payroll_runs.data.is_none() {
        let time = ui.input(|i| i.time);
        widgets::draw_page_skeleton(
            ui,
            palette,
            widgets::PageSkeleton::Payroll,
            time,
            app.settings.reduced_motion,
        );
        return;
    }
    if app.payroll_runs.error.is_some() && app.payroll_runs.data.is_none() {
        return;
    }
    let rows = app.payroll_runs.data.clone().unwrap_or_default();
    let closed = rows.iter().filter(|r| r.status == "closed").count();
    // Selected-run amounts are shown only after its detail has loaded; a dash
    // means “not selected”, never an invented zero balance.
    let selected = app
        .payroll_detail
        .data
        .as_ref()
        .filter(|run| Some(run.id) == app.payroll_selected_id);
    let metrics = [
        (
            t!(app.locale, "Payroll runs"),
            format!("{} · {} {}", rows.len(), closed, t!(app.locale, "closed")),
        ),
        (
            t!(app.locale, "Gross payroll"),
            selected
                .map(|run| amount(run.total_gross, &currency))
                .unwrap_or_else(|| "—".to_owned()),
        ),
        (
            t!(app.locale, "Net pay"),
            selected
                .map(|run| amount(run.total_net, &currency))
                .unwrap_or_else(|| "—".to_owned()),
        ),
        (
            t!(app.locale, "Employer cost"),
            selected
                .map(|run| amount(run.total_employer_cost, &currency))
                .unwrap_or_else(|| "—".to_owned()),
        ),
    ];
    let width = ui.available_width();
    let columns = if width >= 850.0 {
        4
    } else if width >= 400.0 {
        2
    } else {
        1
    };
    for row in metrics.chunks(columns) {
        ui.columns(columns, |items| {
            for (column, (label, value)) in items.iter_mut().zip(row.iter()) {
                let width = (column.available_width() - 32.0).max(1.0);
                card(palette).show(column, |ui| {
                    ui.set_width(width);
                    ui.label(
                        RichText::new(label.to_string())
                            .color(palette.secondary)
                            .size(12.0),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(value)
                            .color(palette.text)
                            .font(crate::fonts::mono_semibold(17.0)),
                    );
                });
            }
        });
        ui.add_space(8.0);
    }
    if let Some(error) = &app.payroll_settings.error {
        error_banner(ui, error, palette);
    }
    if let Some(rates) = &app.payroll_settings.data {
        card(palette).show(ui, |ui| {
            ui.label(RichText::new(t!(app.locale, "Current indicative rates")).strong());
            ui.label(format!(
                "CNSS {:.2}% · AMO {:.2}% · {} {}",
                rates.cnss_emp_rate * 100.0,
                rates.amo_emp_rate * 100.0,
                t!(app.locale, "CNSS ceiling"),
                amount(rates.cnss_ceiling, &currency)
            ));
            ui.label(RichText::new(&rates.disclaimer).color(palette.secondary));
        });
    }
    ui.add_space(8.0);
    if rows.is_empty() {
        empty_state(
            ui,
            crate::icons::Icon::Users,
            t!(app.locale, "No payroll runs"),
            t!(app.locale, "Payroll runs created on the web appear here."),
            palette,
        );
    }
    for run in rows {
        card(palette).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new(format!("{} · #{}", run.period_month, run.id)).strong());
                    let kind = if run.run_kind == "corrective" {
                        t!(app.locale, "Corrective")
                    } else {
                        t!(app.locale, "Regular")
                    };
                    let status = if run.status == "closed" {
                        t!(app.locale, "Closed")
                    } else {
                        t!(app.locale, "Draft")
                    };
                    ui.label(
                        RichText::new(format!(
                            "{kind} · {status} · {}",
                            amount(run.total_net, &currency)
                        ))
                        .color(palette.secondary),
                    );
                });
                if secondary_button(ui, t!(app.locale, "View details"), palette) {
                    app.load_payroll_detail(run.id);
                }
            });
        });
        ui.add_space(5.0);
    }
    if app.payroll_detail.loading {
        let time = ui.input(|i| i.time);
        widgets::draw_page_skeleton(
            ui,
            palette,
            widgets::PageSkeleton::TableOnly,
            time,
            app.settings.reduced_motion,
        );
    } else if let Some(error) = &app.payroll_detail.error {
        error_banner(ui, error, palette);
        if let Some(id) = app.payroll_selected_id
            && secondary_button(ui, t!(app.locale, "Retry"), palette)
        {
            app.load_payroll_detail(id);
        }
    } else if let Some(run) = &app.payroll_detail.data {
        ui.add_space(12.0);
        card(palette).show(ui, |ui| {
            ui.label(
                RichText::new(format!(
                    "{} #{} · {}",
                    t!(app.locale, "Payroll run"),
                    run.id,
                    run.period_month
                ))
                .strong(),
            );
            if let Some(id) = run.corrects_run_id {
                ui.label(format!("{} #{id}", t!(app.locale, "Corrects run")));
            }
            for (label, value) in [
                ("Gross payroll", run.total_gross),
                ("Employee CNSS", run.total_cnss_emp),
                ("Employee AMO", run.total_amo_emp),
                ("Income tax", run.total_igr),
                ("Net pay", run.total_net),
                ("Employer cost", run.total_employer_cost),
            ] {
                ui.horizontal_wrapped(|ui| {
                    ui.label(crate::i18n::gettext(app.locale, label));
                    ui.label(RichText::new(amount(value, &currency)).monospace().strong());
                });
            }
        });
        for line in &run.lines {
            card(palette).show(ui, |ui| {
                ui.label(RichText::new(&line.employee_name).strong());
                ui.label(
                    RichText::new(format!(
                        "{} · {} {} · {} {}",
                        line.emp_number.as_deref().unwrap_or("—"),
                        line.days_worked,
                        t!(app.locale, "days"),
                        line.overtime_hours,
                        t!(app.locale, "overtime hours")
                    ))
                    .color(palette.secondary),
                );
                for (label, value) in [
                    ("Base salary", line.base_salary),
                    ("Attendance adjustment", line.attendance_adjustment),
                    ("Bonuses", line.bonuses),
                    ("Overtime", line.overtime_amount),
                    ("Gross payroll", line.gross_salary),
                    ("Employee CNSS", line.cnss_emp),
                    ("Employee AMO", line.amo_emp),
                    ("Income tax", line.igr),
                    ("Advances deducted", line.advances_deducted),
                    ("Net pay", line.net_salary),
                    ("Employer cost", line.employer_cost),
                ] {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(crate::i18n::gettext(app.locale, label));
                        ui.label(RichText::new(amount(value, &currency)).monospace());
                    });
                }
            });
            ui.add_space(5.0);
        }
    }
}
