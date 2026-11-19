//! Tableau de bord: key figures, unpaid sales, the monthly mini chart and
//! the most recent invoices.

use egui::{RichText, Sense, vec2};

use atlas_api::models::{Dashboard, MonthlyPoint};
use atlas_core::format::{money, number, short_date};

use crate::icons::Icon;
use crate::state::AtlasApp;
use crate::theme::Palette;
use crate::widgets::{card, icon, page_title, pill, primary_button};
use crate::{t, widgets};

pub fn draw(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    let width = ui.available_width().min(1040.0);
    let margin = ((ui.available_width() - width) / 2.0).max(0.0);
    ui.horizontal_top(|ui| {
        ui.add_space(margin);
        ui.vertical(|ui| {
            ui.set_width(width);
            draw_dashboard(app, ui, palette);
        });
    });
}

fn draw_dashboard(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
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
            }
        });
    });
    let name = app
        .session
        .as_ref()
        .map(|session| {
            if session.display_name.is_empty() {
                session.username.as_str()
            } else {
                session.display_name.as_str()
            }
        })
        .unwrap_or("");
    if !name.is_empty() {
        ui.label(
            RichText::new(format!("{}, {name}.", t!(app.locale, "Hello")))
                .color(palette.text)
                .size(20.0)
                .strong(),
        );
        ui.label(
            RichText::new(t!(app.locale, "Your workspace at a glance."))
                .color(palette.secondary)
                .size(13.0),
        );
    }
    ui.add_space(12.0);

    if let Some(error) = app.dashboard.error.clone() {
        widgets::error_banner(ui, &error, palette);
        ui.add_space(10.0);
    }

    if app.dashboard.loading && app.dashboard.data.is_none() {
        super::list_skeleton(ui, palette, app.locale, app.settings.reduced_motion);
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

    if data.operational_only {
        low_stock_card(app, ui, palette, &data);
        return;
    }

    // The server sends monthly buckets, not daily data. Filter the cached
    // response locally: changing the period does not issue another request.
    let period_id = egui::Id::new("dashboard-period-months");
    let mut months = ui
        .ctx()
        .data(|data| data.get_temp::<usize>(period_id))
        .unwrap_or(6);
    ui.horizontal(|ui| {
        ui.label(RichText::new(t!(app.locale, "Period")).color(palette.secondary));
        egui::ComboBox::from_id_salt("dashboard-period-picker")
            .selected_text(format!("{} {}", months, t!(app.locale, "months")))
            .show_ui(ui, |ui| {
                for count in [3, 6, 12, 24, 36] {
                    ui.selectable_value(
                        &mut months,
                        count,
                        format!("{} {}", count, t!(app.locale, "months")),
                    );
                }
            });
    });
    ui.ctx()
        .data_mut(|data| data.insert_temp(period_id, months));
    let points = recent_months(&data.monthly_series, months);
    let charts = [
        Chart {
            title: t!(app.locale, "Cash flow"),
            first_label: t!(app.locale, "Income"),
            second_label: t!(app.locale, "Expenses"),
            first: |p| Some(p.sales),
            second: |p| Some(p.expenses),
        },
        Chart {
            title: t!(app.locale, "Profit trend"),
            first_label: t!(app.locale, "Profit"),
            second_label: "".into(),
            first: |p| Some(p.profit),
            second: |_| None,
        },
        Chart {
            title: t!(app.locale, "Invoices and quick sales"),
            first_label: t!(app.locale, "Invoices"),
            second_label: t!(app.locale, "Quick sales"),
            first: |p| p.invoices_fa,
            second: |p| p.sales_vr,
        },
        Chart {
            title: t!(app.locale, "Supplier purchases and salaries"),
            first_label: t!(app.locale, "Supplier bills"),
            second_label: t!(app.locale, "Salaries"),
            first: |p| p.bills,
            second: |p| p.salaries,
        },
        Chart {
            title: t!(app.locale, "Payments received"),
            first_label: t!(app.locale, "Payments received"),
            second_label: "".into(),
            first: |p| p.receipts,
            second: |_| None,
        },
        Chart {
            title: t!(app.locale, "New clients"),
            first_label: t!(app.locale, "Clients"),
            second_label: "".into(),
            first: |p| p.clients_new,
            second: |_| None,
        },
    ];
    for chart in &charts {
        chart_card(ui, palette, points, chart);
    }
    card(palette).show(ui, |ui| {
        ui.set_width((ui.available_width() - 34.0).max(0.0));
        unpaid_card(app, ui, palette, &data);
    });

    ui.add_space(14.0);
    card(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());
        egui::CollapsingHeader::new(
            RichText::new(t!(app.locale, "Recent invoices"))
                .color(palette.text)
                .strong(),
        )
        .id_salt("dashboard-recent-invoices")
        .default_open(false)
        .show(ui, |ui| recent_invoices(app, ui, palette, &data));
    });
}

/// The restricted dashboard response already contains up to 50 inventory
/// records; use them directly without an additional products request.
fn low_stock_card(app: &AtlasApp, ui: &mut egui::Ui, palette: &Palette, data: &Dashboard) {
    card(palette).show(ui, |ui| {
        ui.set_width((ui.available_width() - 34.0).max(0.0));
        ui.label(
            RichText::new(t!(app.locale, "Low stock"))
                .color(palette.text)
                .strong()
                .size(15.0),
        );
        ui.add_space(8.0);
        if data.low_stock.is_empty() {
            ui.label(
                RichText::new(t!(app.locale, "No low-stock products.")).color(palette.secondary),
            );
            return;
        }
        if data.low_stock.len() >= 50 {
            ui.label(
                RichText::new(t!(app.locale, "Showing up to 50 products."))
                    .color(palette.secondary),
            );
        }
        for product in &data.low_stock {
            ui.vertical(|ui| {
                ui.label(RichText::new(&product.name).color(palette.text).strong());
                ui.horizontal_wrapped(|ui| {
                    if let Some(sku) = product.sku.as_deref().filter(|sku| !sku.is_empty()) {
                        ui.label(RichText::new(format!("SKU: {sku}")).color(palette.secondary));
                    }
                    let unit = product.unit.as_deref().unwrap_or("");
                    ui.label(
                        RichText::new(format!(
                            "{}: {} {}",
                            t!(app.locale, "Stock"),
                            number(product.stock),
                            unit
                        ))
                        .color(palette.finance_warning),
                    );
                    if let Some(threshold) = product.low_stock_threshold {
                        ui.label(
                            RichText::new(format!(
                                "{}: {}",
                                t!(app.locale, "Low stock threshold"),
                                number(threshold)
                            ))
                            .color(palette.secondary),
                        );
                    }
                });
            });
            ui.separator();
        }
    });
}

fn metric_cards(app: &AtlasApp, ui: &mut egui::Ui, palette: &Palette, data: &Dashboard) {
    let currency = data.currency();
    if data.operational_only {
        let cards = [
            (
                Icon::Package,
                t!(app.locale, "Products").into_owned(),
                number(data.products as f64),
                false,
            ),
            (
                Icon::TriangleAlert,
                t!(app.locale, "Low stock").into_owned(),
                if data.low_stock.len() >= 50 {
                    "≥ 50".to_owned()
                } else {
                    number(data.low_stock.len() as f64)
                },
                !data.low_stock.is_empty(),
            ),
        ];
        render_metric_cards(ui, palette, &cards);
        ui.label(RichText::new(t!(app.locale, "Operational overview")).color(palette.secondary));
        return;
    }
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
    render_metric_cards(ui, palette, &cards);
}

fn render_metric_cards(
    ui: &mut egui::Ui,
    palette: &Palette,
    cards: &[(Icon, String, String, bool)],
) {
    // A fixed 4-column grid starved narrow windows (each card demanded at
    // least 168px, so the row overflowed its card). Pick a column count from
    // the real available width and let the row wrap instead. The card frame
    // adds 34px around the content (2×16 margin + 2×1 stroke), so the column
    // math runs on outer widths.
    const FRAME_OVERHEAD: f32 = 34.0;
    let gap = 12.0_f32;
    let available = ui.available_width();
    let min_outer = 180.0 + FRAME_OVERHEAD;
    let cols = ((available + gap) / min_outer).floor() as usize;
    let cols = cols.clamp(1, cards.len());
    let outer_width =
        ((available - gap * (cols.saturating_sub(1)) as f32) / cols as f32).max(140.0);
    let card_width = (outer_width - FRAME_OVERHEAD).max(120.0);
    // A wrapped horizontal layout can grow past its intended width when
    // frames request space before wrapping. Build explicit rows instead.
    for row in cards.chunks(cols) {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for (which, label, value, warn) in row {
                metric_card(ui, palette, *which, label, value, *warn, card_width);
            }
        });
    }
}

fn metric_card(
    ui: &mut egui::Ui,
    palette: &Palette,
    which: Icon,
    label: &str,
    value: &str,
    warn: bool,
    width: f32,
) {
    card(palette).show(ui, |ui| {
        ui.set_width(width);
        ui.set_min_height(92.0);
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                icon(
                    ui,
                    which,
                    if warn {
                        palette.finance_warning
                    } else {
                        palette.accent
                    },
                    20.0,
                );
                ui.label(
                    RichText::new(label)
                        .color(palette.secondary)
                        .font(crate::fonts::medium(12.0)),
                );
            });
            ui.add_space(8.0);
            ui.label(
                RichText::new(value)
                    .color(if warn {
                        palette.finance_warning
                    } else {
                        palette.text
                    })
                    .font(crate::fonts::mono_semibold(22.0)),
            );
        });
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
        money(unpaid.open, currency),
        palette.text,
    );
    // Overdue money is a finance danger status, using the Android token.
    stat_row(
        ui,
        palette,
        t!(app.locale, "Overdue"),
        money(unpaid.overdue, currency),
        palette.finance_danger,
    );
    stat_row(
        ui,
        palette,
        t!(app.locale, "Total due"),
        money(unpaid.total, currency),
        palette.text,
    );
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        pill(
            ui,
            format!(
                "{} {}",
                number(unpaid.count as f64),
                t!(app.locale, "invoices")
            ),
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
    value_color: egui::Color32,
) {
    ui.horizontal(|ui| {
        // Pinning the label to the full available width starved the value
        // column until both texts painted on top of each other.
        ui.label(
            RichText::new(label.as_ref())
                .color(palette.secondary)
                .size(13.0),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                RichText::new(value.as_ref())
                    .color(value_color)
                    .font(crate::fonts::mono_semibold(13.5)),
            );
        });
    });
    ui.add_space(6.0);
}

/// Return the newest monthly buckets without another API call.
fn recent_months(points: &[MonthlyPoint], count: usize) -> &[MonthlyPoint] {
    &points[points.len().saturating_sub(count)..]
}

struct Chart<'a> {
    title: std::borrow::Cow<'a, str>,
    first_label: std::borrow::Cow<'a, str>,
    second_label: std::borrow::Cow<'a, str>,
    first: fn(&MonthlyPoint) -> Option<f64>,
    second: fn(&MonthlyPoint) -> Option<f64>,
}

fn chart_card(ui: &mut egui::Ui, palette: &Palette, points: &[MonthlyPoint], chart: &Chart<'_>) {
    let first = chart.first;
    let second = chart.second;
    card(palette).show(ui, |ui| {
        ui.set_width((ui.available_width() - 34.0).max(0.0));
        ui.label(
            RichText::new(chart.title.as_ref())
                .color(palette.text)
                .strong()
                .size(15.0),
        );
        ui.add_space(8.0);
        if points.is_empty()
            || !points
                .iter()
                .any(|p| first(p).is_some() || second(p).is_some())
        {
            ui.label(RichText::new("—").color(palette.secondary));
            return;
        }
        let max = points
            .iter()
            .flat_map(|p| [first(p), second(p)])
            .flatten()
            .filter(|v| v.is_finite())
            .map(f64::abs)
            .fold(1.0, f64::max);
        let negative = points
            .iter()
            .flat_map(|p| [first(p), second(p)])
            .flatten()
            .any(|v| v.is_finite() && v < 0.0);
        let width = ui.available_width().max(1.0);
        let height = 200.0;
        let (rect, response) = ui.allocate_exact_size(vec2(width, height + 22.0), Sense::hover());
        let bottom = rect.top() + height;
        let baseline = if negative {
            rect.top() + height / 2.0
        } else {
            bottom
        };
        let scale = if negative { height / 2.0 } else { height };
        for index in 0..=4 {
            let y = rect.top() + height * index as f32 / 4.0;
            ui.painter().line_segment(
                [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
                egui::Stroke::new(1.0, palette.outline),
            );
        }
        let slot = width / points.len() as f32;
        let bar_width = (slot * 0.28).clamp(1.0, 18.0);
        let label_step = ((52.0 / slot).ceil() as usize).max(1);
        for (index, point) in points.iter().enumerate() {
            let center = rect.left() + slot * (index as f32 + 0.5);
            for (value, offset, color) in [
                (first(point), -bar_width, palette.finance_info),
                (second(point), 0.0, palette.finance_warning),
            ] {
                if let Some(value) = value.filter(|v| v.is_finite()) {
                    let h = (value.abs() / max * f64::from(scale)) as f32;
                    let top = if value < 0.0 { baseline } else { baseline - h };
                    ui.painter().rect_filled(
                        egui::Rect::from_min_size(
                            egui::pos2(center + offset, top),
                            vec2(bar_width - 1.0, h),
                        ),
                        2.0,
                        color,
                    );
                }
            }
            if index % label_step == 0 {
                ui.painter().text(
                    egui::pos2(center, bottom + 4.0),
                    egui::Align2::CENTER_TOP,
                    month_label(point),
                    egui::FontId::proportional(10.5),
                    palette.secondary,
                );
            }
        }
        if let Some(position) = response.hover_pos() {
            let index =
                (((position.x - rect.left()) / slot).floor() as usize).min(points.len() - 1);
            let point = &points[index];
            let mut detail = month_label(point);
            if let Some(value) = first(point).filter(|v| v.is_finite()) {
                detail.push_str(&format!("\n{}: {}", chart.first_label, number(value)));
            }
            if let Some(value) = second(point).filter(|v| v.is_finite()) {
                detail.push_str(&format!("\n{}: {}", chart.second_label, number(value)));
            }
            response.on_hover_text(detail);
        }
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("●").color(palette.finance_info));
            ui.label(
                RichText::new(chart.first_label.as_ref())
                    .color(palette.secondary)
                    .size(12.0),
            );
            if !chart.second_label.as_ref().is_empty() {
                ui.label(RichText::new("●").color(palette.finance_warning));
                ui.label(
                    RichText::new(chart.second_label.as_ref())
                        .color(palette.secondary)
                        .size(12.0),
                );
            }
        });
    });
    ui.add_space(14.0);
}

fn month_label(point: &MonthlyPoint) -> String {
    let bytes = point.key.as_bytes();
    if bytes.len() == 7
        && bytes[4] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(i, c)| i == 4 || c.is_ascii_digit())
    {
        format!("{}/{}", &point.key[5..7], &point.key[2..4])
    } else {
        point.label.clone()
    }
}

fn recent_invoices(app: &AtlasApp, ui: &mut egui::Ui, palette: &Palette, data: &Dashboard) {
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
                        .font(crate::fonts::mono_semibold(13.5)),
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
    // Financial invoice statuses use the Android finance semantics: green
    // for paid, red for overdue; other states stay neutral chrome.
    let (fill, text) = match status {
        "paid" | "payée" => (palette.finance_positive_soft, palette.finance_positive),
        "overdue" | "en_retard" => (palette.finance_danger_soft, palette.finance_danger),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metric_rows_do_not_expand_a_narrow_dashboard() {
        let ctx = egui::Context::default();
        ctx.set_fonts(crate::fonts::definitions());
        let mut measured_width = 0.0;
        let mut output = ctx.run_ui(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.allocate_ui_with_layout(
                    vec2(560.0, 700.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        let cards = [
                            (
                                Icon::Wallet,
                                "Solde actuel".into(),
                                "42 000 MAD".into(),
                                false,
                            ),
                            (
                                Icon::Boxes,
                                "Valeur du stock".into(),
                                "17 500 MAD".into(),
                                false,
                            ),
                            (Icon::Package, "Produits".into(), "2".into(), false),
                            (Icon::TriangleAlert, "Stock bas".into(), "0".into(), false),
                        ];
                        render_metric_cards(ui, &Palette::dark(), &cards);
                        measured_width = ui.max_rect().width();
                    },
                );
            });
        });
        output.textures_delta.set.clear();
        output.textures_delta.free.clear();
        assert!(
            measured_width <= 561.0,
            "dashboard expanded to {measured_width}"
        );
    }
}

#[cfg(test)]
mod period_tests {
    use super::{month_label, recent_months};
    use atlas_api::models::MonthlyPoint;

    #[test]
    fn recent_period_is_bounded_by_available_buckets() {
        let points: Vec<_> = (0..12).map(|_| MonthlyPoint::default()).collect();
        assert_eq!(recent_months(&points, 6).len(), 6);
        assert_eq!(recent_months(&points, 36).len(), 12);
        assert!(recent_months(&points, 0).is_empty());
    }

    #[test]
    fn uses_full_year_month_key_and_safe_fallback() {
        let point = MonthlyPoint {
            key: "2026-01".into(),
            ..Default::default()
        };
        assert_eq!(month_label(&point), "01/26");
        let point = MonthlyPoint {
            key: "☀".into(),
            label: "Exemple".into(),
            ..Default::default()
        };
        assert_eq!(month_label(&point), "Exemple");
    }
}
