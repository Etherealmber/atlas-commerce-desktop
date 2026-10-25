//! Expenses and income: a searchable ledger with period totals and a
//! create editor. Wire contract: the `/api/expenses` block of the worker;
//! the validation mirrors `validateExpenseWrite` (amount >= 0.01,
//! description <= 300 characters). Deletes stay password-gated and are
//! handled in a later phase alongside the other destructive endpoints.

use atlas_api::ApiError;
use atlas_api::models::{Expense, ExpenseWrite};
use atlas_core::format::money;
use atlas_core::jobs::Job;
use egui::{RichText, Sense, vec2};

use crate::icons::Icon;
use crate::state::AtlasApp;
use crate::theme::Palette;
use crate::widgets::{
    card, empty_state, error_banner, icon, labelled_field, page_title, primary_button,
    secondary_button,
};
use crate::{t, widgets};

/// Worker payment-method keys and their interface labels.
const PAYMENT_METHODS: [(&str, &str); 5] = [
    ("cash", "Cash"),
    ("card", "Card"),
    ("bank_company", "Company transfer"),
    ("check_company", "Company check"),
    ("other", "Other"),
];

/// Create form for an expense or an income entry.
#[derive(Default)]
pub struct ExpenseEditor {
    /// `"expense"` or `"income"`; owner movements are created server-side.
    pub entry_type: String,
    pub description: String,
    pub category: String,
    pub amount: String,
    pub payment_method: String,
    pub expense_date: String,
    pub vendor_name: String,
    pub reference: String,
    pub notes: String,
    pub saving: bool,
    pub error: Option<String>,
    pub job: Option<Job<Result<Expense, ApiError>>>,
}

impl ExpenseEditor {
    #[must_use]
    pub fn new() -> Self {
        Self {
            entry_type: "expense".to_owned(),
            payment_method: "cash".to_owned(),
            expense_date: chrono::Local::now().format("%Y-%m-%d").to_string(),
            ..Self::default()
        }
    }

    fn validate(&self) -> Option<&'static str> {
        if self.description.trim().is_empty() {
            return Some("The description is required.");
        }
        if self.description.chars().count() > 300 {
            return Some("The description must be 300 characters or fewer.");
        }
        match self.amount.trim().replace(',', ".").parse::<f64>() {
            Ok(amount) if amount >= 0.01 => None,
            _ => Some("The amount must be a positive number."),
        }
    }

    fn to_write(&self) -> ExpenseWrite {
        ExpenseWrite {
            category: if self.category.trim().is_empty() {
                "General".to_owned()
            } else {
                self.category.trim().to_owned()
            },
            description: self.description.trim().to_owned(),
            amount: self.amount.trim().replace(',', ".").parse().unwrap_or(0.0),
            payment_method: self.payment_method.clone(),
            expense_date: self.expense_date.trim().to_owned(),
            vendor_name: self.vendor_name.trim().to_owned(),
            reference: self.reference.trim().to_owned(),
            notes: self.notes.trim().to_owned(),
            entry_type: self.entry_type.clone(),
        }
    }
}

pub fn draw(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    poll_editor(app);

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        page_title(ui, t!(app.locale, "Expenses"), palette);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if primary_button(ui, t!(app.locale, "New entry"), palette, true) {
                app.expense_editor = Some(ExpenseEditor::new());
            }
            if secondary_button(ui, t!(app.locale, "Refresh"), palette) {
                app.load_expenses();
            }
        });
    });
    ui.add_space(12.0);

    if let Some(error) = app.expenses.error.clone() {
        error_banner(ui, &error, palette);
        ui.add_space(10.0);
    }

    draw_totals(app, ui, palette);
    ui.add_space(12.0);

    card(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            icon(ui, Icon::Search, palette.secondary, 16.0);
            ui.add(
                egui::TextEdit::singleline(&mut app.expense_search)
                    .hint_text(t!(
                        app.locale,
                        "Search by description, category, vendor or reference"
                    ))
                    .margin(egui::Margin::symmetric(10, 8))
                    .desired_width(ui.available_width() - 24.0),
            );
        });
        ui.add_space(10.0);

        if app.expenses.loading && app.expenses.data.is_none() {
            super::centered_spinner(ui, palette);
            return;
        }
        let expenses = app.expenses.data.clone().unwrap_or_default();
        if expenses.is_empty() {
            empty_state(
                ui,
                Icon::Wallet,
                t!(app.locale, "No expense yet"),
                t!(app.locale, "Recorded expenses and income appear here."),
                palette,
            );
            return;
        }

        let query = app.expense_search.to_lowercase();
        let currency = app
            .dashboard
            .data
            .as_ref()
            .map(|data| data.currency())
            .unwrap_or("MAD");
        let mut matches = 0;
        for expense in &expenses {
            let q = query.as_str();
            if !query.is_empty()
                && !expense.description.to_lowercase().contains(q)
                && !expense.category.to_lowercase().contains(q)
                && !expense.vendor_name.to_lowercase().contains(q)
                && !expense.reference.to_lowercase().contains(q)
            {
                continue;
            }
            matches += 1;
            expense_row(ui, palette, app.locale, expense, currency);
            ui.separator();
        }
        if matches == 0 {
            empty_state(
                ui,
                Icon::Search,
                t!(app.locale, "No matching entry"),
                t!(app.locale, "Try another search term."),
                palette,
            );
        }
    });

    if app.expense_editor.is_some() {
        draw_editor(app, ui, palette);
    }
}

fn expense_row(
    ui: &mut egui::Ui,
    palette: &Palette,
    locale: crate::i18n::Locale,
    expense: &Expense,
    currency: &str,
) {
    let is_income = expense.entry_type == "income";
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
                icon(
                    ui,
                    if is_income {
                        Icon::TrendingUp
                    } else {
                        Icon::Wallet
                    },
                    palette.secondary,
                    18.0,
                );
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(expense.description.clone())
                            .color(palette.text)
                            .strong()
                            .size(13.5),
                    );
                    let method = crate::i18n::gettext(
                        locale,
                        payment_source(expense.payment_method.as_str()),
                    )
                    .to_string();
                    let subline = [
                        expense.expense_date.clone(),
                        expense.category.clone(),
                        method,
                        expense.vendor_name.clone(),
                    ]
                    .into_iter()
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
                    .join(" · ");
                    ui.label(RichText::new(subline).color(palette.dim).size(11.5));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let sign = if is_income { "+" } else { "−" };
                    ui.label(
                        RichText::new(format!("{sign}{}", money(expense.amount, currency)))
                            .color(palette.text)
                            .strong()
                            .size(13.0),
                    );
                });
            });
        },
    );
}

fn draw_totals(app: &AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    let Some(expenses) = app.expenses.data.clone() else {
        return;
    };
    let currency = app
        .dashboard
        .data
        .as_ref()
        .map(|data| data.currency())
        .unwrap_or("MAD");
    let mut spent = 0.0;
    let mut earned = 0.0;
    for expense in &expenses {
        match expense.entry_type.as_str() {
            "income" => earned += expense.amount,
            _ => spent += expense.amount,
        }
    }
    let net = earned - spent;
    let width = ((ui.available_width() - 24.0) / 3.0).max(160.0);
    ui.horizontal(|ui| {
        stat_card(
            ui,
            palette,
            width,
            &t!(app.locale, "Expenses"),
            &money(spent, currency),
        );
        stat_card(
            ui,
            palette,
            width,
            &t!(app.locale, "Income"),
            &money(earned, currency),
        );
        stat_card(
            ui,
            palette,
            width,
            &t!(app.locale, "Net balance"),
            &money(net, currency),
        );
    });
}

fn stat_card(ui: &mut egui::Ui, palette: &Palette, width: f32, label: &str, value: &str) {
    card(palette).show(ui, |ui| {
        ui.set_width(width);
        ui.label(RichText::new(label).color(palette.secondary).size(12.0));
        ui.add_space(4.0);
        ui.label(RichText::new(value).color(palette.text).strong().size(17.0));
    });
}

/// Maps a worker payment key to the static English source the i18n
/// catalog translates. `gettext` only accepts `&'static str`, so unknown
/// keys collapse to the generic label (the worker itself defaults
/// unrecognised methods to `cash`).
fn payment_source(key: &str) -> &'static str {
    PAYMENT_METHODS
        .iter()
        .find(|(candidate, _)| *candidate == key)
        .map_or("Other", |(_, label)| label)
}

fn draw_editor(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    let screen = ui.ctx().input(|input| input.viewport_rect());
    let rect = egui::Rect::from_center_size(screen.center(), vec2(520.0, 620.0));
    ui.painter()
        .rect_filled(screen, 0.0, egui::Color32::from_black_alpha(140));

    let locale = app.locale;
    widgets::overlay_at(ui, rect, |ui| {
        widgets::dialog_frame(palette).show(ui, |ui| {
            ui.set_min_size(vec2(476.0, 0.0));
            ui.label(
                RichText::new(t!(locale, "New entry"))
                    .color(palette.text)
                    .strong()
                    .size(17.0),
            );
            ui.add_space(10.0);

            if let Some(editor) = app.expense_editor.as_mut() {
                segmented_type(ui, editor, palette, locale);
                ui.add_space(10.0);
                labelled_field(
                    ui,
                    t!(locale, "Description"),
                    &mut editor.description,
                    palette,
                    false,
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        labelled_field(
                            ui,
                            t!(locale, "Amount"),
                            &mut editor.amount,
                            palette,
                            false,
                        );
                    });
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(t!(locale, "Payment method"))
                                .color(palette.secondary)
                                .size(12.5),
                        );
                        egui::ComboBox::from_id_salt("expense-payment-method")
                            .selected_text(crate::i18n::gettext(
                                locale,
                                payment_source(editor.payment_method.as_str()),
                            ))
                            .show_ui(ui, |ui| {
                                for (key, label) in PAYMENT_METHODS {
                                    ui.selectable_value(
                                        &mut editor.payment_method,
                                        key.to_owned(),
                                        crate::i18n::gettext(locale, label),
                                    );
                                }
                            });
                    });
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        labelled_field(
                            ui,
                            t!(locale, "Category"),
                            &mut editor.category,
                            palette,
                            false,
                        );
                    });
                    ui.vertical(|ui| {
                        labelled_field(
                            ui,
                            t!(locale, "Date"),
                            &mut editor.expense_date,
                            palette,
                            false,
                        );
                    });
                });
                ui.add_space(8.0);
                labelled_field(
                    ui,
                    t!(locale, "Vendor (optional)"),
                    &mut editor.vendor_name,
                    palette,
                    false,
                );
                ui.add_space(8.0);
                labelled_field(
                    ui,
                    t!(locale, "Reference (optional)"),
                    &mut editor.reference,
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
                .expense_editor
                .as_ref()
                .and_then(|editor| editor.error.clone())
            {
                ui.add_space(8.0);
                error_banner(ui, &error, palette);
            }

            ui.add_space(12.0);
            widgets::dialog_actions(ui, |ui| {
                let saving = app
                    .expense_editor
                    .as_ref()
                    .is_some_and(|editor| editor.saving);
                let save = primary_button(ui, t!(locale, "Save"), palette, !saving);
                let cancel = secondary_button(ui, t!(locale, "Cancel"), palette);
                if cancel {
                    app.expense_editor = None;
                }
                if save {
                    submit_editor(app);
                }
            });
        });
    });
}

fn segmented_type(
    ui: &mut egui::Ui,
    editor: &mut ExpenseEditor,
    palette: &Palette,
    locale: crate::i18n::Locale,
) {
    ui.horizontal(|ui| {
        for (key, label) in [("expense", "Expense"), ("income", "Income")] {
            let selected = editor.entry_type == key;
            let button = egui::Button::new(
                RichText::new(crate::i18n::gettext(locale, label))
                    .color(if selected {
                        palette.on_accent
                    } else {
                        palette.secondary
                    })
                    .strong(),
            )
            .wrap_mode(egui::TextWrapMode::Extend)
            .fill(if selected {
                palette.accent
            } else {
                palette.surface
            })
            .stroke(if selected {
                egui::Stroke::NONE
            } else {
                egui::Stroke::new(1.0, palette.outline)
            })
            .corner_radius(8.0);
            if ui.add(button).clicked() {
                editor.entry_type = key.to_owned();
            }
        }
    });
}

fn poll_editor(app: &mut AtlasApp) {
    if let Some(editor) = app.expense_editor.as_mut()
        && let Some(job) = &editor.job
        && let Some(result) = job.poll()
    {
        editor.job = None;
        editor.saving = false;
        match result {
            Ok(_) => {
                app.expense_editor = None;
                app.notice(t!(app.locale, "Entry saved.").into_owned());
                app.load_expenses();
                app.load_dashboard();
            }
            Err(error) => editor.error = Some(error.message),
        }
    }
}

fn submit_editor(app: &mut AtlasApp) {
    let Some(client) = app.client.clone() else {
        return;
    };
    let Some(editor) = app.expense_editor.as_mut() else {
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
        .spawn(async move { client.create_expense(&body).await });
    editor.job = Some(job);
}
