//! Tableau de bord: key figures, unpaid sales, the monthly mini chart and
//! the most recent invoices.

use egui::{RichText, Sense, vec2};

use atlas_api::models::Dashboard;
use atlas_core::format::{money, number, short_date};

use crate::icons::Icon;
use crate::state::AtlasApp;
use crate::theme::Palette;
use crate::widgets::{card, icon, page_title, pill, primary_button};
use crate::{t, widgets};

pub fn draw(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        page_title(ui, t!(app.locale, "Dashboard"), palette);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if primary_button(
                ui,
                t!(app.locale, "Refresh"),
                palette,
                !app.dashboard.loading,
            ) {
                app.load_dashboard();
                app.load_alerts();
            }
        });
    });
    ui.add_space(12.0);

    if let Some(error) = app.dashboard.error.clone() {
        widgets::error_banner(ui, &error, palette);
        ui.add_space(10.0);
    }

    if app.dashboard.loading && app.dashboard.data.is_none() {
        super::spinner(ui, palette);
        return;
    }

    let Some(data) = app.dashboard.data.clone() else {
        widgets::empty_state(
            ui,
            Icon::Dashboard,
            t!(app.locale, "No data yet"),
            t!(app.locale, "Press refresh to load the dashboard."),
            palette,
        );
        return;
    };

    metric_cards(app, ui, palette, &data);
    ui.add_space(14.0);

    ui.horizontal_top(|ui| {
        ui.set_min_height(180.0);
        card(palette).show(ui, |ui| {
            ui.set_width((ui.available_width() * 0.58).max(280.0));
            unpaid_card(app, ui, palette, &data);
        });
        card(palette).show(ui, |ui| {
            ui.set_width(ui.available_width());
            monthly_card(app, ui, palette, &data);
        });
    });

    ui.add_space(14.0);
    card(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());
        recent_invoices(app, ui, palette, &data);
    });
}

fn metric_cards(app: &AtlasApp, ui: &mut egui::Ui, palette: &Palette, data: &Dashboard) {
    let currency = data.currency();
    let cards: [(Icon, String, String, bool); 4] = [
        (
            Icon::Wallet,
            t!(app.locale, "Current balance").into_owned(),
            money(data.current_balance, currency),
            false,
        ),
        (
            Icon::Boxes,
            t!(app.locale, "Stock value").into_owned(),
            money(data.stock_value, currency),
            false,
        ),
        (
            Icon::Package,
            t!(app.locale, "Products").into_owned(),
            number(data.product_count as f64),
            false,
        ),
        (
            Icon::TriangleAlert,
            t!(app.locale, "Low stock").into_owned(),
            number(data.low_stock_count as f64),
            data.low_stock_count > 0,
        ),
    ];
    egui::Grid::new("dashboard-metrics")
        .num_columns(4)
        .spacing([12.0, 12.0])
        .show(ui, |ui| {
            for (index, (which, label, value, warn)) in cards.iter().enumerate() {
                metric_card(ui, palette, *which, label, value, *warn);
                if (index + 1) % 4 == 0 {
                    ui.end_row();
                }
            }
        });
}

fn metric_card(
    ui: &mut egui::Ui,
    palette: &Palette,
    which: Icon,
    label: &str,
    value: &str,
    warn: bool,
) {
    card(palette).show(ui, |ui| {
        ui.set_min_size(vec2(168.0, 92.0));
        ui.horizontal(|ui| {
            icon(ui, which, if warn { palette.warning } else { palette.accent }, 20.0);
            ui.label(RichText::new(label).color(palette.secondary).size(12.5));
        });
        ui.add_space(8.0);
        ui.label(
            RichText::new(value)
                .color(if warn { palette.warning } else { palette.text })
                .strong()
                .size(21.0),
        );
    });
}

fn unpaid_card(app: &AtlasApp, ui: &mut egui::Ui, palette: &Palette, data: &Dashboard) {
    let currency = data.currency();
    let unpaid = &data.unpaid_sales;
    ui.label(
        RichText::new(t!(app.locale, "Unpaid sales"))
            .color(palette.text)
            .strong()
            .size(15.0),
    );
    ui.add_space(10.0);
    stat_row(
        ui,
        palette,
        t!(app.locale, "Open"),
        &money(unpaid.open, currency),
    );
    stat_row(
        ui,
        palette,
        t!(app.locale, "Overdue"),
        &money(unpaid.overdue, currency),
    );
    stat_row(
        ui,
        palette,
        t!(app.locale, "Total due"),
        &money(unpaid.total, currency),
    );
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        pill(
            ui,
            format!("{} {}", number(unpaid.count as f64), t!(app.locale, "invoices")),
            palette.surface_active,
            palette.secondary,
        );
    });
}

fn stat_row(
    ui: &mut egui::Ui,
    palette: &Palette,
    label: impl AsRef<str>,
    value: impl AsRef<str>,
) {
    ui.horizontal(|ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new(label.as_ref()).color(palette.secondary).size(13.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(value.as_ref()).color(palette.text).strong().size(13.5));
        });
    });
    ui.add_space(6.0);
}

fn monthly_card(app: &AtlasApp, ui: &mut egui::Ui, palette: &Palette, data: &Dashboard) {
    ui.label(
        RichText::new(t!(app.locale, "Monthly activity"))
            .color(palette.text)
            .strong()
            .size(15.0),
    );
    ui.add_space(10.0);
    if data.monthly_series.is_empty() {
        widgets::empty_state(
            ui,
            Icon::TrendingUp,
            t!(app.locale, "No history yet"),
            t!(app.locale, "Sales appear here month after month."),
            palette,
        );
        return;
    }
    let max = data
        .monthly_series
        .iter()
        .map(|point| point.sales.max(point.expenses))
        .fold(1.0, f64::max);
    let height = 110.0;
    let width = ui.available_width();
    let slot = (width / data.monthly_series.len() as f32).max(28.0);
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    for (index, point) in data.monthly_series.iter().enumerate() {
        let x = rect.left() + slot * index as f32 + slot * 0.25;
        let bar_width = slot * 0.32;
        let sales_h = (point.sales / max * height as f64) as f32;
        let expenses_h = (point.expenses / max * height as f64) as f32;
        ui.painter().rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(x, rect.bottom() - sales_h),
                vec2(bar_width, sales_h),
            ),
            3.0,
            palette.accent,
        );
        ui.painter().rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(x + bar_width + 2.0, rect.bottom() - expenses_h),
                vec2(bar_width, expenses_h),
            ),
            3.0,
            palette.surface_active,
        );
        ui.painter().text(
            egui::pos2(x, rect.bottom() + 4.0),
            egui::Align2::LEFT_TOP,
            point.label.clone(),
            egui::FontId::proportional(10.5),
            palette.dim,
        );
    }
}

fn recent_invoices(app: &AtlasApp, ui: &mut egui::Ui, palette: &Palette, data: &Dashboard) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(t!(app.locale, "Recent invoices"))
                .color(palette.text)
                .strong()
                .size(15.0),
        );
    });
    ui.add_space(8.0);
    if data.recent_invoices.is_empty() {
        widgets::empty_state(
            ui,
            Icon::Receipt,
            t!(app.locale, "No invoice yet"),
            t!(app.locale, "Quick sales will show up here."),
            palette,
        );
        return;
    }
    let currency = data.currency();
    for invoice in &data.recent_invoices {
        ui.horizontal(|ui| {
            ui.set_width(ui.available_width());
            icon(ui, Icon::Receipt, palette.secondary, 16.0);
            ui.label(
                RichText::new(
                    invoice
                        .number
                        .clone()
                        .unwrap_or_else(|| format!("#{}", invoice.id)),
                )
                .color(palette.text)
                .size(13.5),
            );
            if let Some(date) = &invoice.issue_date {
                ui.label(
                    RichText::new(short_date(date))
                        .color(palette.dim)
                        .size(12.0),
                );
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(money(invoice.total, currency))
                        .color(palette.text)
                        .strong()
                        .size(13.5),
                );
                if let Some(status) = &invoice.status {
                    status_pill(ui, palette, status);
                }
            });
        });
        ui.separator();
    }
}

pub fn status_pill(ui: &mut egui::Ui, palette: &Palette, status: &str) {
    let (fill, text) = match status {
        "paid" | "payée" => (palette.accent.gamma_multiply(0.18), palette.accent),
        "overdue" | "en_retard" => (palette.danger.gamma_multiply(0.18), palette.danger),
        _ => (palette.surface_active, palette.secondary),
    };
    pill(ui, status_label(status), fill, text);
}

fn status_label(status: &str) -> String {
    match status {
        "paid" => "Payée".to_owned(),
        "partial" => "Partielle".to_owned(),
        "unpaid" => "Non payée".to_owned(),
        "overdue" => "En retard".to_owned(),
        other => other.to_owned(),
    }
}
