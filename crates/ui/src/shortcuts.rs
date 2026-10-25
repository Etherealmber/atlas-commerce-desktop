//! Keyboard-first control: global shortcuts and the Ctrl+K command palette.
//!
//! The web app can only react to keys inside a focused page; a native app
//! owns them globally. This module wires:
//!
//! * `Ctrl/⌘+1…9` — switch between the nine rail sections,
//! * `Ctrl/⌘+K` — open the command palette (jump to any screen or action),
//! * `Ctrl/⌘+R` and `F5` — reload the current screen's data,
//! * `Ctrl/⌘+F` — focus the title-bar product search,
//! * `Escape` — dismiss the topmost overlay, then release text focus.
//!
//! While a text field owns the focus, every shortcut except `Ctrl+K` and
//! `Escape` belongs to the field, so Ctrl+A/C/V never get hijacked. All
//! filtering logic lives in pure functions so it is unit-tested without a
//! window (`cargo test -p atlas-ui shortcuts`).

use egui::{Color32, RichText, Sense, TextWrapMode};

use crate::i18n::Locale;
use crate::icons::Icon;
use crate::screens::shell::route_meta;
use crate::state::{AtlasApp, Route, Screen};
use crate::t;
use crate::theme::Palette;
use crate::widgets;

/// The fixed egui id of the palette's filter field, used to hand it focus.
fn input_id() -> egui::Id {
    egui::Id::new("command-palette-input")
}

/// Releases whatever widget currently holds keyboard focus. Requesting the
/// NULL id would leave a focused id in memory and keep the "typing" guard
/// active on the next frame.
fn clear_focus(ctx: &egui::Context) {
    ctx.memory_mut(|memory| {
        if let Some(id) = memory.focused() {
            memory.surrender_focus(id);
        }
    });
}

/// Every command the palette can run. Routes are kept as data so the entry
/// list and the shortcut digits share one source of truth.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaletteTarget {
    Route(Route),
    Refresh,
    NewProduct,
    NewClient,
    NewVendor,
    NewExpense,
    NewDelivery,
    FocusSearch,
    SettingsProfile,
    SettingsGeneral,
    SignOut,
}

/// One row of the palette: an action, its French label and an optional
/// right-aligned key hint ("Ctrl+R").
#[derive(Clone, Debug)]
pub struct PaletteEntry {
    pub target: PaletteTarget,
    pub label: String,
    pub hint: Option<&'static str>,
}

/// The nine rail sections in display order — exactly what Ctrl+1…9 reaches.
/// Notifications and Réglages live in the title bar, not the rail.
#[must_use]
pub fn rail_route_at(index: usize) -> Option<Route> {
    Route::ALL
        .into_iter()
        .filter(|route| !matches!(route, Route::Notifications | Route::Settings))
        .nth(index)
}

/// Builds the full entry list for a locale. Routes first (rail order, then
/// the title-bar sections), then global actions.
#[must_use]
pub fn entries(locale: Locale) -> Vec<PaletteEntry> {
    let mut list = Vec::with_capacity(24);
    for (index, route) in Route::ALL.into_iter().enumerate() {
        let (_, label) = route_meta(locale, route);
        let hint = rail_route_at(index).and_then(|rail| {
            (rail == route).then_some(match index {
                0 => "Ctrl+1",
                1 => "Ctrl+2",
                2 => "Ctrl+3",
                3 => "Ctrl+4",
                4 => "Ctrl+5",
                5 => "Ctrl+6",
                6 => "Ctrl+7",
                7 => "Ctrl+8",
                _ => "Ctrl+9",
            })
        });
        list.push(PaletteEntry {
            target: PaletteTarget::Route(route),
            label,
            hint,
        });
    }

    let actions = [
        (
            PaletteTarget::Refresh,
            t!(locale, "Refresh the current page").into_owned(),
            Some("Ctrl+R"),
        ),
        (
            PaletteTarget::FocusSearch,
            t!(locale, "Search a product…").into_owned(),
            Some("Ctrl+F"),
        ),
        (
            PaletteTarget::NewProduct,
            t!(locale, "New product").into_owned(),
            None,
        ),
        (
            PaletteTarget::NewClient,
            t!(locale, "New client").into_owned(),
            None,
        ),
        (
            PaletteTarget::NewVendor,
            t!(locale, "New vendor").into_owned(),
            None,
        ),
        (
            PaletteTarget::NewExpense,
            t!(locale, "New expense").into_owned(),
            None,
        ),
        (
            PaletteTarget::NewDelivery,
            t!(locale, "New delivery note").into_owned(),
            None,
        ),
        (
            PaletteTarget::SettingsProfile,
            t!(locale, "My profile").into_owned(),
            None,
        ),
        (
            PaletteTarget::SettingsGeneral,
            t!(locale, "Settings").into_owned(),
            None,
        ),
        (
            PaletteTarget::SignOut,
            t!(locale, "Sign out").into_owned(),
            None,
        ),
    ];
    for (target, label, hint) in actions {
        list.push(PaletteEntry {
            target,
            label,
            hint,
        });
    }
    list
}

/// Lowercases and folds the common French accents so "reglages" matches
/// "Réglages" and "client" matches a typed "clïent".
#[must_use]
pub fn normalize(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| match c {
            'à' | 'á' | 'â' | 'ä' | 'ã' | 'å' => 'a',
            'ç' => 'c',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ñ' => 'n',
            'ó' | 'ò' | 'ô' | 'ö' | 'õ' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ý' | 'ÿ' => 'y',
            other => other,
        })
        .collect()
}

/// Returns the indices of entries whose label contains every query word
/// (order-independent). An empty query keeps the whole list.
#[must_use]
pub fn filter_entries(query: &str, entries: &[PaletteEntry]) -> Vec<usize> {
    let words: Vec<String> = query
        .split_whitespace()
        .map(normalize)
        .filter(|word| !word.is_empty())
        .collect();
    if words.is_empty() {
        return (0..entries.len()).collect();
    }
    entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| {
            let label = normalize(&entry.label);
            words.iter().all(|word| label.contains(word))
        })
        .map(|(index, _)| index)
        .collect()
}

/// Global shortcut handling, called once per frame before the shell draws.
pub fn handle_shortcuts(app: &mut AtlasApp, ctx: &egui::Context) {
    if !matches!(app.screen, Screen::Shell(_)) {
        return;
    }

    // Escape always wins: dialog → palette → popover → editor → text focus.
    if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
        app.dismiss_top_overlay();
        // Whatever was open no longer owns the keyboard: surrender focus so
        // the next keystroke cannot land in a now-hidden text field.
        clear_focus(ctx);
        return;
    }

    // A modal editor or confirmation owns every other key: navigating,
    // refreshing or opening the palette underneath could drop unsaved input
    // or replace an editor (e.g. the palette's "New product" action).
    if app.modal_open() {
        return;
    }

    // F5 refreshes without a modifier, even while a field is focused.
    if ctx.input(|input| input.key_pressed(egui::Key::F5)) {
        let palette_was_open = app.palette_open;
        app.close_palette();
        if palette_was_open {
            clear_focus(ctx);
        }
        app.refresh_current_route();
        return;
    }

    // Ctrl on Windows/Linux, ⌘ (mac_cmd) on macOS. This fork exposes the
    // modifier fields directly rather than the upstream `command()` helper.
    let ctrl = ctx.input(|input| input.modifiers.ctrl || input.modifiers.mac_cmd);
    if !ctrl {
        return;
    }

    // Ctrl+K toggles the palette, stealing focus from any field when opening.
    if ctx.input(|input| input.key_pressed(egui::Key::K)) {
        if app.palette_open {
            app.close_palette();
            clear_focus(ctx);
        } else {
            app.open_palette();
            ctx.memory_mut(|memory| memory.request_focus(input_id()));
        }
        return;
    }

    // Any other Ctrl+combination belongs to the focused text field.
    let typing = ctx.memory(|memory| memory.focused().is_some());
    if typing && !app.palette_open {
        return;
    }

    // A navigation/action shortcut always leaves the palette first.
    if app.palette_open {
        app.close_palette();
        clear_focus(ctx);
    }

    if ctx.input(|input| input.key_pressed(egui::Key::R)) {
        app.refresh_current_route();
        return;
    }

    if ctx.input(|input| input.key_pressed(egui::Key::F)) {
        ctx.memory_mut(|memory| memory.request_focus(egui::Id::new("titlebar-search")));
        return;
    }

    // Digit variants are resolved by name so no fragile Key enum path is used.
    for digit in 1..=9u32 {
        let Some(key) = egui::Key::from_name(&digit.to_string()) else {
            continue;
        };
        if ctx.input(|input| input.key_pressed(key))
            && let Some(route) = rail_route_at(digit as usize - 1)
        {
            app.go(route);
            return;
        }
    }
}

/// Draws the centered palette overlay when open. Backdrop clicks cancel;
/// arrows move the selection, Enter runs it, Escape is handled globally.
pub fn draw_palette(app: &mut AtlasApp, ui: &mut egui::Ui, ctx: &egui::Context, palette: &Palette) {
    if !app.palette_open {
        return;
    }
    // viewport_rect() (not Context::screen_rect, which this egui fork omits).
    let screen = ctx.input(|input| input.viewport_rect());
    let width = 580.0;
    let max_height = 460.0;
    let card_rect = egui::Rect::from_min_size(
        egui::pos2(screen.center().x - width / 2.0, screen.top() + 96.0),
        egui::vec2(width, max_height),
    );

    // Register the full-screen backdrop as an interaction target first: this
    // consumes the press so a rail row underneath cannot retain it and
    // navigate on release. The pointer check also dismisses on the initial
    // press, and palette rows (registered later) win inside the card.
    let backdrop = ui.interact(
        screen,
        egui::Id::new("command-palette-backdrop"),
        Sense::click(),
    );
    let pressed_outside = ctx.input(|input| {
        input.pointer.any_pressed()
            && input
                .pointer
                .interact_pos()
                .is_some_and(|pos| !card_rect.contains(pos))
    });
    if backdrop.clicked() || pressed_outside {
        app.close_palette();
        clear_focus(ctx);
        return;
    }
    ui.painter()
        .rect_filled(screen, 0.0, Color32::from_black_alpha(140));

    let entries = entries(app.locale);
    let matches = filter_entries(&app.palette_query, &entries);
    if app.palette_selected >= matches.len() {
        app.palette_selected = matches.len().saturating_sub(1);
    }

    // Keep the filter focused while the palette is open.
    if ctx.memory(|memory| memory.focused()) != Some(input_id()) {
        ctx.memory_mut(|memory| memory.request_focus(input_id()));
    }

    if ui.input(|input| input.key_pressed(egui::Key::ArrowDown)) && !matches.is_empty() {
        app.palette_selected = (app.palette_selected + 1) % matches.len();
    }
    if ui.input(|input| input.key_pressed(egui::Key::ArrowUp)) && !matches.is_empty() {
        app.palette_selected = match app.palette_selected {
            0 => matches.len() - 1,
            n => n - 1,
        };
    }
    let activate = ui.input(|input| input.key_pressed(egui::Key::Enter));

    widgets::overlay_at(ui, card_rect, |ui| {
        ui.set_max_width(width);
        ui.set_max_height(max_height);
        widgets::dialog_frame(palette).show(ui, |ui| {
            ui.set_width(width - 44.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                widgets::icon(ui, Icon::Search, palette.secondary, 17.0);
                ui.add(
                    egui::TextEdit::singleline(&mut app.palette_query)
                        .id(input_id())
                        .frame(egui::Frame::NONE)
                        .hint_text(t!(app.locale, "Search or jump to…"))
                        .desired_width(width - 110.0)
                        .clip_text(true),
                );
            });
            ui.add_space(10.0);
            egui::Frame::NONE
                .fill(palette.outline.gamma_multiply(0.6))
                .show(ui, |ui| ui.set_height(1.0));
            ui.add_space(8.0);

            if matches.is_empty() {
                ui.add_space(8.0);
                ui.label(
                    RichText::new(t!(app.locale, "No matching command."))
                        .color(palette.dim)
                        .size(13.0),
                );
            } else {
                let chosen = if activate {
                    Some(matches[app.palette_selected])
                } else {
                    None
                };
                egui::ScrollArea::vertical()
                    .max_height(max_height - 150.0)
                    .show(ui, |ui| {
                        for (position, entry_index) in matches.iter().enumerate() {
                            let entry = &entries[*entry_index];
                            let selected = position == app.palette_selected;
                            let row = palette_row(ui, palette, entry, selected);
                            if row.hovered() {
                                app.palette_selected = position;
                            }
                            if row.clicked() {
                                app.palette_selected = position;
                                run_target(app, ctx, entry.target);
                                return;
                            }
                        }
                    });
                if let Some(index) = chosen {
                    let target = entries[index].target;
                    run_target(app, ctx, target);
                }
            }

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(t!(app.locale, "Escape to close"))
                        .color(palette.dim)
                        .size(11.5),
                );
            });
        });
    });
}

fn run_target(app: &mut AtlasApp, ctx: &egui::Context, target: PaletteTarget) {
    if app.run_palette_target(target) {
        // The product search explicitly takes the focus over.
        ctx.memory_mut(|memory| memory.request_focus(egui::Id::new("titlebar-search")));
    } else {
        // Every other action leaves the palette: its field must not stay
        // focused and suppress the next global shortcut.
        clear_focus(ctx);
    }
}

/// One palette row: highlight fill when selected, label left, key hint right.
fn palette_row(
    ui: &mut egui::Ui,
    palette: &Palette,
    entry: &PaletteEntry,
    selected: bool,
) -> egui::Response {
    let height = 38.0;
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), height), Sense::click());
    if selected || response.hovered() {
        ui.painter().rect_filled(rect, 8.0, palette.surface_hover);
    }
    widgets::overlay_at(
        ui,
        egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), rect.height())),
        |ui| {
            ui.horizontal(|ui| {
                ui.set_height(height);
                ui.add_space(10.0);
                // One line even for long translated labels.
                ui.style_mut().wrap_mode = Some(TextWrapMode::Extend);
                ui.label(
                    RichText::new(entry.label.clone())
                        .color(palette.text)
                        .size(13.5),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(hint) = entry.hint {
                        ui.label(RichText::new(hint).color(palette.dim).size(11.0));
                    }
                });
            });
        },
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Locale;

    #[test]
    fn rail_has_exactly_nine_keyboard_sections() {
        let routes: Vec<Route> = (0..9).flat_map(rail_route_at).collect();
        assert_eq!(routes.len(), 9);
        // The title-bar sections are not in the rail's digit range.
        assert!(!routes.contains(&Route::Notifications));
        assert!(!routes.contains(&Route::Settings));
        assert_eq!(rail_route_at(0), Some(Route::Dashboard));
        assert_eq!(rail_route_at(9), None);
    }

    #[test]
    fn empty_query_lists_every_entry() {
        let entries = entries(Locale::English);
        assert_eq!(filter_entries("", &entries).len(), entries.len());
        assert_eq!(filter_entries("   ", &entries).len(), entries.len());
    }

    #[test]
    fn filtering_matches_case_insensitively_and_accent_folded() {
        let entries = entries(Locale::French);
        // "ventes" matches the Ventes route; "reglages" folds into Réglages.
        assert!(
            filter_entries("ventes", &entries)
                .iter()
                .any(|index| entries[*index].target == PaletteTarget::Route(Route::Sales))
        );
        assert!(
            filter_entries("reglages", &entries)
                .iter()
                .any(|index| entries[*index].target == PaletteTarget::Route(Route::Settings))
        );
    }

    #[test]
    fn multi_word_queries_intersect() {
        let entries = entries(Locale::English);
        // Every word must hit: no label contains both "product" and "vendor".
        assert!(filter_entries("product vendor", &entries).is_empty());
        // Two words from the same label still match.
        assert!(
            filter_entries("new product", &entries)
                .iter()
                .any(|index| entries[*index].target == PaletteTarget::NewProduct)
        );
    }

    #[test]
    fn accent_folding_is_deterministic() {
        assert_eq!(normalize("RéGlages…"), "reglages…");
        assert_eq!(normalize("  CAFÉ "), normalize("  café "));
    }
}
