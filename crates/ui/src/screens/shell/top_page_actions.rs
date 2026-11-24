//! Shared, non-scrolling page action row. It uses the existing page actions
//! and retains their role checks; it never invokes a mutation by itself.
use crate::dialogs::{Confirm, ConfirmAction};
use crate::state::{AtlasApp, Route, Screen, SettingsFocus};
use crate::t;
use crate::theme::Palette;
use crate::widgets::{primary_button, secondary_button};

/// Measure the currently visible localized controls, including padding and
/// spacing. If they cannot fit beside the dock, use the wrapping row instead.
pub(super) fn required_top_width(app: &AtlasApp, ui: &egui::Ui, palette: &Palette) -> Option<f32> {
    let Screen::Shell(route) = app.screen else {
        return None;
    };
    if !app.role_policy().allows_route(route) {
        return None;
    }
    let mut labels: Vec<&'static str> = Vec::new();
    match route {
        Route::Dashboard
        | Route::InventoryOps
        | Route::CashShifts
        | Route::Actions
        | Route::Payroll => {}
        Route::Products
            if !app
                .stock_history
                .as_ref()
                .is_some_and(|history| history.full) =>
        {
            if app.role_policy().can_edit_prices() {
                labels.push("New product");
            }
        }
        Route::Clients if app.client_report_id.is_none() => labels.push("New client"),
        Route::Vendors => labels.push("New vendor"),
        Route::DeliveryNotes => labels.push("New delivery note"),
        Route::Employees if app.employee_situation_id.is_none() => {
            labels.extend(["Add employee", "Print hiring form"])
        }
        Route::Attendance => {
            if app
                .attendance
                .last_error
                .as_ref()
                .is_some_and(|error| error.code.as_deref() == Some("ATTENDANCE_DISABLED"))
            {
                labels.extend(["Enable now", "Open attendance settings"]);
            }
        }
        Route::Expenses if !app.finance_report_open => {
            if app.role_policy().can_write_ledger() {
                labels.extend(["Income", "Owner contribution", "Owner draw", "Expense"]);
            }
        }
        _ => return None,
    }
    labels.push("Refresh");
    let width = labels
        .into_iter()
        .map(|label| {
            let text = crate::i18n::gettext(app.locale, label).into_owned();
            ui.painter()
                .layout_no_wrap(text, crate::fonts::semibold(16.0), palette.text)
                .size()
                .x
                + 48.0
        })
        .sum::<f32>();
    Some(width + 8.0)
}

/// Available top-band space to the right of the largest possible dock,
/// excluding the native window controls. Never overlap either surface.
pub(super) fn top_slot(
    rect: egui::Rect,
    caption_left: f32,
    caption_right: f32,
    required: f32,
) -> Option<egui::Rect> {
    let left = rect.left() + caption_left.max(8.0);
    let right = (rect.right() - caption_right.max(120.0) - 8.0).max(left);
    let center = (left + right) * 0.5;
    // The dock has at most six 48px controls, a 180px label and 32px padding.
    let slot_left = center + 250.0 + 12.0;
    if right - slot_left < required || rect.height() < 48.0 {
        return None;
    }
    Some(egui::Rect::from_min_max(
        egui::pos2(slot_left, rect.top()),
        egui::pos2(right, rect.bottom()),
    ))
}

pub(super) fn draw(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    let Screen::Shell(route) = app.screen else {
        return;
    };
    let product_history = app
        .stock_history
        .as_ref()
        .is_some_and(|history| history.full);
    if !matches!(
        route,
        Route::Dashboard
            | Route::Products
            | Route::Attendance
            | Route::Payroll
            | Route::Expenses
            | Route::InventoryOps
            | Route::CashShifts
            | Route::Actions
            | Route::Clients
            | Route::Vendors
            | Route::DeliveryNotes
            | Route::Employees
    ) || (route == Route::Products && product_history)
        || (route == Route::Clients && app.client_report_id.is_some())
        || (route == Route::Employees && app.employee_situation_id.is_some())
        || (route == Route::Expenses && app.finance_report_open)
        || !app.role_policy().allows_route(route)
    {
        return;
    }
    let enabled = !app.modal_open() && !app.palette_open;
    ui.add_enabled_ui(enabled, |ui| {
        // This row remains above the route scroll and wraps on narrow windows.
        // Right-to-left layout keeps the available actions against the outer edge.
        ui.with_layout(
            egui::Layout::right_to_left(egui::Align::Center).with_main_wrap(true),
            |ui| match route {
                Route::Dashboard => {
                    if crate::buttons::primary_refresh_button(
                        ui,
                        t!(app.locale, "Refresh"),
                        palette,
                        !app.dashboard.loading,
                    ) {
                        app.load_dashboard();
                    }
                }
                Route::Products => {
                    if app.role_policy().can_edit_prices()
                        && primary_button(ui, t!(app.locale, "New product"), palette, true)
                    {
                        app.product_editor = Some(crate::screens::products::ProductEditor::new());
                    }
                    if crate::buttons::refresh_button(ui, t!(app.locale, "Refresh"), palette) {
                        app.load_products();
                    }
                }
                Route::Expenses => {
                    if app.role_policy().can_write_ledger() {
                        for (kind, label) in [
                            ("income", "Income"),
                            ("owner_contrib", "Owner contribution"),
                            ("owner_draw", "Owner draw"),
                            ("expense", "Expense"),
                        ] {
                            if primary_button(
                                ui,
                                crate::i18n::gettext(app.locale, label),
                                palette,
                                !app.ledger_uncertain,
                            ) {
                                let mut editor = crate::screens::expenses::ExpenseEditor::new();
                                editor.entry_type = kind.to_owned();
                                app.expense_editor = Some(editor);
                            }
                        }
                    }
                    if crate::buttons::refresh_button(ui, t!(app.locale, "Refresh"), palette) {
                        if !app.expenses.loading {
                            app.load_expenses();
                        }
                        if !app.dashboard.loading {
                            app.load_dashboard();
                        }
                    }
                }
                Route::Clients => {
                    if primary_button(ui, t!(app.locale, "New client"), palette, true) {
                        app.client_editor = Some(crate::screens::clients::ClientEditor::new());
                    }
                    if crate::buttons::refresh_button(ui, t!(app.locale, "Refresh"), palette) {
                        app.load_clients();
                    }
                }
                Route::Vendors => {
                    if primary_button(ui, t!(app.locale, "New vendor"), palette, true) {
                        app.vendor_editor = Some(crate::screens::vendors::VendorEditor::new());
                    }
                    if crate::buttons::refresh_button(ui, t!(app.locale, "Refresh"), palette) {
                        app.load_vendors();
                    }
                }
                Route::DeliveryNotes => {
                    if primary_button(ui, t!(app.locale, "New delivery note"), palette, true) {
                        app.delivery_note_editor =
                            Some(crate::screens::delivery_notes::DeliveryNoteEditor::new());
                    }
                    if crate::buttons::refresh_button(ui, t!(app.locale, "Refresh"), palette) {
                        app.load_delivery_notes();
                    }
                }
                Route::Employees => {
                    if primary_button(ui, t!(app.locale, "Add employee"), palette, true) {
                        app.employee_new =
                            Some(crate::screens::employees::NewEmployeeForm::default());
                    }
                    if crate::buttons::refresh_button(ui, t!(app.locale, "Refresh"), palette) {
                        app.load_employees();
                    }
                    if secondary_button(ui, t!(app.locale, "Print hiring form"), palette) {
                        app.open_printable("/api/employees/form/print", &[]);
                    }
                }
                Route::Payroll => {
                    if crate::buttons::refresh_button(ui, t!(app.locale, "Refresh"), palette) {
                        app.refresh_payroll();
                    }
                }
                Route::Actions => {
                    if crate::buttons::refresh_button(ui, t!(app.locale, "Refresh"), palette) {
                        app.load_mobile_actions();
                    }
                }
                Route::InventoryOps => {
                    if crate::buttons::refresh_button(ui, t!(app.locale, "Refresh"), palette) {
                        app.load_inventory_ops();
                    }
                }
                Route::CashShifts => {
                    if crate::buttons::refresh_button(ui, t!(app.locale, "Refresh"), palette) {
                        app.refresh_cash_shifts();
                    }
                }
                Route::Attendance => {
                    let disabled =
                        app.attendance.last_error.as_ref().is_some_and(|error| {
                            error.code.as_deref() == Some("ATTENDANCE_DISABLED")
                        });
                    if disabled {
                        if primary_button(
                            ui,
                            t!(app.locale, "Enable now"),
                            palette,
                            !app.attendance.loading
                                && !app.attendance_action.loading
                                && app.attendance_action.error.is_none(),
                        ) {
                            app.confirm_dialog = Some(Confirm {
                                title: t!(app.locale, "Enable attendance?").into_owned(),
                                body: t!(
                                    app.locale,
                                    "This enables attendance for the entire company."
                                )
                                .into_owned(),
                                confirm_label: t!(app.locale, "Enable now").into_owned(),
                                danger: false,
                                action: ConfirmAction::EnableAttendance,
                            });
                        }
                        if secondary_button(ui, t!(app.locale, "Open attendance settings"), palette)
                        {
                            app.settings_focus = SettingsFocus::Attendance;
                            app.go(Route::Settings);
                        }
                    }
                    if secondary_button(ui, t!(app.locale, "Refresh"), palette) {
                        app.load_attendance();
                    }
                }
                _ => {}
            },
        );
    });
    ui.add_space(4.0);
}

#[cfg(test)]
mod tests {
    use super::top_slot;
    use egui::{Rect, pos2};

    #[test]
    fn slot_avoids_maximum_dock_and_caption_controls() {
        let rect = Rect::from_min_max(pos2(252.0, 0.0), pos2(1920.0, 56.0));
        let slot = top_slot(rect, 0.0, 120.0, 300.0).expect("wide window");
        let usable_left = rect.left() + 8.0;
        let usable_right = rect.right() - 128.0;
        let center = (usable_left + usable_right) / 2.0;
        assert!(slot.left() >= center + 250.0 + 12.0);
        assert!(slot.right() <= usable_right);
        assert!(slot.width() >= 300.0);
    }

    #[test]
    fn insufficient_room_falls_back_to_wrapping_row() {
        let rect = Rect::from_min_max(pos2(252.0, 0.0), pos2(1360.0, 56.0));
        assert!(top_slot(rect, 0.0, 120.0, 470.0).is_none());
        assert!(top_slot(rect, 0.0, 120.0, 165.0).is_some());
    }
}
