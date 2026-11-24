//! Cash-shift list and basic opening, movement and closure workflows.
//! The Worker owns the calculations; writes are not retried automatically.
use crate::dialogs::{Confirm, ConfirmAction};
use crate::state::{AtlasApp, Route};
use crate::theme::Palette;
use crate::widgets::{
    card, error_banner, labelled_field, page_title, primary_button, secondary_button,
    section_header,
};
use crate::{t, widgets};
use egui::RichText;

pub(crate) struct CashShiftForm {
    pub selected_id: Option<i64>,
    pub till: String,
    pub opening: String,
    pub amount: String,
    pub counted: String,
    pub label: String,
    pub reference: String,
    pub notes: String,
    pub direction: bool,
    pub method: usize,
}
impl Default for CashShiftForm {
    fn default() -> Self {
        Self {
            selected_id: None,
            till: "CAISSE-1".into(),
            opening: "0".into(),
            amount: String::new(),
            counted: String::new(),
            label: String::new(),
            reference: String::new(),
            notes: String::new(),
            direction: true,
            method: 0,
        }
    }
}
pub(crate) enum CashShiftAction {
    Opened(i64),
    Movement(i64),
    Closed(i64),
}
const METHODS: [&str; 5] = ["cash", "card", "transfer", "check", "other"];

impl AtlasApp {
    pub(crate) fn load_cash_shifts(&mut self) {
        if !self.role_policy().allows_route(Route::CashShifts)
            || self.cash_shifts.loading
            || self.cash_shift_action.loading
        {
            return;
        }
        let Some(api) = self.client.clone() else {
            return;
        };
        self.cash_shifts
            .start(self.runner.spawn(async move { api.cash_shifts().await }));
    }
    pub(crate) fn load_cash_shift_detail(&mut self, id: i64) {
        if !self.role_policy().allows_route(Route::CashShifts)
            || id <= 0
            || self.cash_shift_action.loading
        {
            return;
        }
        let Some(api) = self.client.clone() else {
            return;
        };
        self.cash_shift_form.selected_id = Some(id);
        self.cash_shift_detail = Default::default();
        self.cash_shift_detail
            .start(self.runner.spawn(async move { api.cash_shift(id).await }));
    }
    pub(crate) fn refresh_cash_shifts(&mut self) {
        self.load_cash_shifts();
        if let Some(id) = self.cash_shift_form.selected_id {
            self.load_cash_shift_detail(id);
        }
    }
    fn cash_write_ready(&self) -> bool {
        self.role_policy().allows_route(Route::CashShifts)
            && !self.cash_shifts.loading
            && self.cash_shifts.error.is_none()
            && !self.cash_shift_action.loading
            && self.cash_shift_action.error.is_none()
    }
    pub(crate) fn open_cash_shift(&mut self, till: String, opening: f64, notes: String) {
        if !self.cash_write_ready() {
            return;
        }
        let Some(api) = self.client.clone() else {
            return;
        };
        self.cash_shift_action.start(self.runner.spawn(async move {
            api.open_cash_shift(&till, opening, &notes)
                .await
                .map(|shift| CashShiftAction::Opened(shift.id))
        }));
    }
    pub(crate) fn cash_movement(
        &mut self,
        id: i64,
        direction: &'static str,
        method: &'static str,
        amount: f64,
        label: String,
        reference: String,
    ) {
        if !self.cash_write_ready()
            || self.cash_shift_detail.loading
            || self.cash_shift_detail.error.is_some()
            || !self
                .cash_shift_detail
                .data
                .as_ref()
                .is_some_and(|detail| detail.shift.id == id && detail.shift.status == "open")
        {
            return;
        }
        let Some(api) = self.client.clone() else {
            return;
        };
        self.cash_shift_action.start(self.runner.spawn(async move {
            api.cash_shift_movement(id, direction, method, amount, &label, &reference)
                .await
                .map(|_| CashShiftAction::Movement(id))
        }));
    }
    pub(crate) fn close_cash_shift(&mut self, id: i64, amount: f64, notes: String) {
        if !self.role_policy().can_close_cash_shift()
            || !self.cash_write_ready()
            || self.cash_shift_detail.loading
            || self.cash_shift_detail.error.is_some()
            || !self
                .cash_shift_detail
                .data
                .as_ref()
                .is_some_and(|detail| detail.shift.id == id && detail.shift.status == "open")
        {
            return;
        }
        let Some(api) = self.client.clone() else {
            return;
        };
        self.cash_shift_action.start(self.runner.spawn(async move {
            api.close_cash_shift(id, amount, &notes)
                .await
                .map(|_| CashShiftAction::Closed(id))
        }));
    }
}

pub fn draw(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    page_title(ui, t!(app.locale, "Cash shifts"), palette);
    if let Some(error) = &app.cash_shifts.error {
        error_banner(ui, error, palette);
    }
    if let Some(error) = &app.cash_shift_action.error {
        error_banner(ui, error, palette);
        ui.label(RichText::new(t!(app.locale, "The server may have saved the change. Verify the shift and movements before continuing.")).color(palette.secondary));
        if secondary_button(ui, t!(app.locale, "I have verified the result"), palette) {
            app.confirm_dialog = Some(Confirm { title: t!(app.locale, "Continue after verification?").into_owned(), body: t!(app.locale, "Only continue after checking the actual shift and its movements. Repeating an operation that already succeeded can duplicate it.").into_owned(), confirm_label: t!(app.locale, "I have verified the result").into_owned(), danger: true, action: ConfirmAction::AcknowledgeCashShiftUncertainty });
        }
    }
    if app.cash_shifts.loading && app.cash_shifts.data.is_none() {
        let time = ui.input(|i| i.time);
        widgets::draw_page_skeleton(
            ui,
            palette,
            widgets::PageSkeleton::CashShifts,
            time,
            app.settings.reduced_motion,
        );
        return;
    }
    let Some(shifts) = app.cash_shifts.data.clone() else {
        return;
    };
    let ready = app.cash_write_ready();
    ui.add_space(12.0);
    section_header(ui, palette, t!(app.locale, "Open a cash shift"));
    card(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());
        labelled_field(
            ui,
            t!(app.locale, "Till"),
            &mut app.cash_shift_form.till,
            palette,
            false,
        );
        labelled_field(
            ui,
            t!(app.locale, "Opening float (MAD)"),
            &mut app.cash_shift_form.opening,
            palette,
            false,
        );
        labelled_field(
            ui,
            t!(app.locale, "Notes"),
            &mut app.cash_shift_form.notes,
            palette,
            false,
        );
        let value = parse_amount(&app.cash_shift_form.opening, true);
        let valid = value.is_some()
            && !app.cash_shift_form.till.trim().is_empty()
            && app.cash_shift_form.till.chars().count() <= 64
            && app.cash_shift_form.notes.chars().count() <= 500;
        if primary_button(ui, t!(app.locale, "Open shift"), palette, ready && valid) {
            if let Some(opening) = value {
                app.confirm_dialog = Some(Confirm {
                    title: t!(app.locale, "Open this cash shift?").into_owned(),
                    body: format!("{} · {opening:.2} MAD", app.cash_shift_form.till.trim()),
                    confirm_label: t!(app.locale, "Open shift").into_owned(),
                    danger: false,
                    action: ConfirmAction::OpenCashShift {
                        till: app.cash_shift_form.till.clone(),
                        opening,
                        notes: app.cash_shift_form.notes.clone(),
                    },
                });
            }
        }
    });
    ui.add_space(12.0);
    section_header(ui, palette, t!(app.locale, "Recent cash shifts"));
    ui.label(
        RichText::new(t!(app.locale, "Showing up to 50 recent shifts.")).color(palette.secondary),
    );
    if shifts.is_empty() {
        ui.label(t!(app.locale, "No cash shifts."));
    }
    let mut selection = None;
    card(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());
        for shift in &shifts {
            ui.horizontal_wrapped(|ui| {
                if secondary_button(ui, t!(app.locale, "View"), palette)
                    && !app.cash_shift_action.loading
                {
                    selection = Some(shift.id);
                }
                ui.label(
                    RichText::new(format!(
                        "#{} · {} · {} · {} · {:.2} MAD",
                        shift.id,
                        shift.till_id,
                        shift.operator,
                        shift.opened_at,
                        shift.opening_float
                    ))
                    .color(palette.text),
                );
                ui.label(
                    RichText::new(if shift.status == "open" {
                        t!(app.locale, "Open")
                    } else {
                        t!(app.locale, "Closed")
                    })
                    .color(palette.secondary),
                );
            });
            ui.separator();
        }
    });
    if let Some(id) = selection {
        app.load_cash_shift_detail(id);
    }
    if let Some(error) = &app.cash_shift_detail.error {
        error_banner(ui, error, palette);
    }
    if app.cash_shift_detail.loading {
        super::list_skeleton(ui, palette, app.locale, app.settings.reduced_motion);
        return;
    }
    let Some(detail) = app.cash_shift_detail.data.clone() else {
        return;
    };
    ui.add_space(12.0);
    section_header(
        ui,
        palette,
        format!("{} #{}", t!(app.locale, "Cash shift"), detail.shift.id),
    );
    card(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(
            RichText::new(format!(
                "{} : {:.2} MAD · {} : {:.2} MAD",
                t!(app.locale, "Expected cash"),
                detail.summary.expected_cash,
                t!(app.locale, "Expected all methods"),
                detail.summary.total_expected_all
            ))
            .strong()
            .color(palette.text),
        );
        for method in detail.summary.methods.values() {
            ui.label(format!(
                "{} · +{:.2} / −{:.2} · {} {:.2} MAD",
                method.label,
                method.in_total,
                method.out_total,
                t!(app.locale, "Expected"),
                method.expected
            ));
        }
        if let Some(counted) = detail.shift.counted_amount {
            ui.label(format!(
                "{} : {counted:.2} MAD · {} : {}",
                t!(app.locale, "Counted cash"),
                t!(app.locale, "Variance"),
                detail
                    .shift
                    .variance
                    .map_or_else(|| "—".to_owned(), |value| format!("{value:.2} MAD"))
            ));
        }
        ui.add_space(8.0);
        if detail.movements.is_empty() {
            ui.label(t!(app.locale, "No movements."));
        }
        for movement in &detail.movements {
            ui.label(format!(
                "{} · {} · {} · {} · {:.2} MAD · {}",
                movement.occurred_at,
                movement.direction,
                movement.payment_method,
                movement.label,
                movement.amount,
                movement.reference
            ));
        }
    });
    if detail.shift.status != "open" {
        return;
    }
    let detail_ready =
        ready && !app.cash_shift_detail.loading && app.cash_shift_detail.error.is_none();
    ui.add_space(12.0);
    section_header(ui, palette, t!(app.locale, "Manual movement"));
    card(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.selectable_value(
                &mut app.cash_shift_form.direction,
                true,
                t!(app.locale, "Cash in"),
            );
            ui.selectable_value(
                &mut app.cash_shift_form.direction,
                false,
                t!(app.locale, "Cash out"),
            );
        });
        app.cash_shift_form.method = app.cash_shift_form.method.min(METHODS.len() - 1);
        let names = [
            t!(app.locale, "Cash"),
            t!(app.locale, "Card"),
            t!(app.locale, "Bank transfer"),
            t!(app.locale, "Cheque"),
            t!(app.locale, "Other"),
        ];
        egui::ComboBox::from_id_salt("cash-shift-method")
            .selected_text(names[app.cash_shift_form.method].clone())
            .show_ui(ui, |ui| {
                for (index, label) in names.iter().enumerate() {
                    ui.selectable_value(&mut app.cash_shift_form.method, index, label.as_ref());
                }
            });
        labelled_field(
            ui,
            t!(app.locale, "Amount (MAD)"),
            &mut app.cash_shift_form.amount,
            palette,
            false,
        );
        labelled_field(
            ui,
            t!(app.locale, "Label"),
            &mut app.cash_shift_form.label,
            palette,
            false,
        );
        labelled_field(
            ui,
            t!(app.locale, "Reference"),
            &mut app.cash_shift_form.reference,
            palette,
            false,
        );
        let amount = parse_amount(&app.cash_shift_form.amount, false);
        let valid = amount.is_some()
            && app.cash_shift_form.label.chars().count() <= 200
            && app.cash_shift_form.reference.chars().count() <= 120;
        if primary_button(
            ui,
            t!(app.locale, "Record movement"),
            palette,
            detail_ready && valid,
        ) {
            if let Some(amount) = amount {
                let direction = if app.cash_shift_form.direction {
                    "in"
                } else {
                    "out"
                };
                let method = METHODS[app.cash_shift_form.method];
                app.confirm_dialog = Some(Confirm {
                    title: t!(app.locale, "Record this movement?").into_owned(),
                    body: format!(
                        "{} · {} · {amount:.2} MAD",
                        if direction == "in" {
                            t!(app.locale, "Cash in")
                        } else {
                            t!(app.locale, "Cash out")
                        },
                        names[app.cash_shift_form.method]
                    ),
                    confirm_label: t!(app.locale, "Record movement").into_owned(),
                    danger: direction == "out",
                    action: ConfirmAction::CashShiftMovement {
                        id: detail.shift.id,
                        direction,
                        method,
                        amount,
                        label: app.cash_shift_form.label.clone(),
                        reference: app.cash_shift_form.reference.clone(),
                    },
                });
            }
        }
    });
    if app.role_policy().can_close_cash_shift() {
        ui.add_space(12.0);
        section_header(ui, palette, t!(app.locale, "Close cash shift"));
        card(palette).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(t!(app.locale, "This records counted cash only. Other payment methods are not counted in this form.")).color(palette.secondary));
            labelled_field(ui, t!(app.locale, "Counted cash (MAD)"), &mut app.cash_shift_form.counted, palette, false);
            labelled_field(ui, t!(app.locale, "Closing notes"), &mut app.cash_shift_form.notes, palette, false);
            let amount = parse_amount(&app.cash_shift_form.counted, true);
            if primary_button(ui, t!(app.locale, "Close shift"), palette, detail_ready && amount.is_some() && app.cash_shift_form.notes.chars().count() <= 1000) {
                if let Some(amount) = amount {
                    app.confirm_dialog = Some(Confirm { title: t!(app.locale, "Close this cash shift?").into_owned(), body: format!("{} : {:.2} MAD · {} : {amount:.2} MAD. {}", t!(app.locale, "Expected cash"), detail.summary.expected_cash, t!(app.locale, "Counted cash"), t!(app.locale, "The server recalculates the expected amount at closing. Stop other movements before closing; concurrent changes may not be included.")), confirm_label: t!(app.locale, "Close shift").into_owned(), danger: true, action: ConfirmAction::CloseCashShift { id: detail.shift.id, amount, notes: app.cash_shift_form.notes.clone() } });
                }
            }
        });
    }
}
fn parse_amount(input: &str, zero: bool) -> Option<f64> {
    let n = input.trim().replace(',', ".").parse::<f64>().ok()?;
    let cents = n * 100.0;
    (n.is_finite()
        && cents.is_finite()
        && (cents - cents.round()).abs() < 0.000_001
        && if zero { n >= 0.0 } else { n > 0.0 })
    .then_some(n)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn amounts_are_finite_and_non_negative() {
        assert_eq!(parse_amount("12,50", false), Some(12.5));
        assert_eq!(parse_amount("0", false), None);
        assert_eq!(parse_amount("0", true), Some(0.0));
        assert_eq!(parse_amount("NaN", true), None);
        assert_eq!(parse_amount("-2", true), None);
        assert_eq!(parse_amount("0.001", false), None);
    }
}
