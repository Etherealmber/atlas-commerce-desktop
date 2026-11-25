//! The authenticated shell: custom title bar, left icon rail, top bar and
//! the routed page content.

use egui::{Align, Layout, RichText, vec2};

use crate::dialogs::ConfirmAction;
use crate::i18n::Locale;
use crate::icons::Icon;
use crate::state::{AtlasApp, Route, Screen};
use crate::theme::Palette;
use crate::widgets::{danger_button, secondary_button};
use crate::{t, widgets};

mod sidebar;
mod top_page_actions;
mod topbar;

#[cfg(test)]
use topbar::search_destination;

/// Height of the custom caption row (web topbar: 56 px).
const TITLEBAR_HEIGHT: f32 = 56.0;
/// Compact labelled rail and icon-only rail at narrower window widths.
const RAIL_WIDTH_EXPANDED: f32 = 280.0;
const RAIL_WIDTH_COLLAPSED: f32 = 64.0;

/// Localized label + icon of every rail destination. Shared with the Ctrl+K
/// command palette so navigation labels never diverge and the English locale
/// gets English route names instead of hard-coded French.
pub(crate) fn route_meta(locale: Locale, route: Route) -> (Icon, String) {
    match route {
        Route::Dashboard => (Icon::Dashboard, t!(locale, "Dashboard").into_owned()),
        Route::Products => (Icon::Package, t!(locale, "Products").into_owned()),
        Route::Catalog => (Icon::Boxes, t!(locale, "Catalog").into_owned()),
        Route::Clients => (Icon::Users, t!(locale, "Clients").into_owned()),
        Route::Vendors => (Icon::Store, t!(locale, "Vendors").into_owned()),
        Route::Expenses => (Icon::Wallet, t!(locale, "Balance and income").into_owned()),
        Route::DeliveryNotes => (Icon::Truck, t!(locale, "Delivery notes").into_owned()),
        Route::Sales => (Icon::Receipt, t!(locale, "Sales").into_owned()),
        Route::Employees => (Icon::Users, t!(locale, "Employees").into_owned()),
        Route::Attendance => (Icon::Clock, t!(locale, "Attendance").into_owned()),
        Route::Payroll => (Icon::Wallet, t!(locale, "Payroll CNSS / AMO").into_owned()),
        Route::InventoryOps => (Icon::Boxes, t!(locale, "Stock operations").into_owned()),
        Route::CashShifts => (Icon::Wallet, t!(locale, "Cash shifts").into_owned()),
        Route::Actions => (Icon::CircleCheck, t!(locale, "Mobile actions").into_owned()),
        Route::Agent => (Icon::Bot, t!(locale, "AI Agent").into_owned()),
        Route::Notifications => (Icon::Bell, t!(locale, "Notifications").into_owned()),
    }
}

pub fn draw_shell(app: &mut AtlasApp, ui: &mut egui::Ui, ctx: &egui::Context, palette: &Palette) {
    // Global keyboard layer first: Ctrl+K palette, Ctrl+1…9, Ctrl+R/F5,
    // Ctrl+F and Escape, all suppressed while a text field owns the keys.
    crate::shortcuts::handle_shortcuts(app, ctx);

    let viewport_width = ctx.input(|input| input.viewport_rect().width());
    let rail_collapsed = app.settings.sidebar_collapsed;
    let rail_width = if rail_collapsed {
        RAIL_WIDTH_COLLAPSED
    } else {
        RAIL_WIDTH_EXPANDED
    };
    egui::Panel::left(egui::Id::new("shell-rail"))
        .exact_size(rail_width)
        .resizable(false)
        .show_separator_line(false)
        .frame(egui::Frame::NONE.fill(if palette.dark {
            egui::Color32::from_rgb(17, 17, 19)
        } else {
            egui::Color32::WHITE
        }))
        .show(ui, |ui| {
            sidebar::draw_rail(app, ui, palette, rail_collapsed)
        });

    // Transparent reserved space keeps content from sliding under controls;
    // the command surface itself is painted by the topbar, not a full slab.
    let mut topbar_rect = egui::Rect::NOTHING;
    egui::Panel::top(egui::Id::new("shell-top-controls"))
        .exact_size(TITLEBAR_HEIGHT)
        .resizable(false)
        .show_separator_line(false)
        .frame(egui::Frame::NONE)
        .show(ui, |ui| topbar_rect = ui.max_rect());

    let agent_available = matches!(
        app.screen,
        Screen::Shell(
            Route::Dashboard
                | Route::Products
                | Route::Clients
                | Route::Vendors
                | Route::Expenses
                | Route::DeliveryNotes
                | Route::Sales
                | Route::Employees
                | Route::Attendance
                | Route::Payroll
                | Route::InventoryOps
                | Route::CashShifts
                | Route::Actions
                | Route::Catalog
        )
    );
    let drawer_visible = app.agent_drawer_open && agent_available;
    let drawer_width = (viewport_width * 0.42)
        .clamp(280.0, 440.0)
        .min((viewport_width - rail_width - 32.0).max(1.0));
    let top_actions_slot =
        if viewport_width >= 1360.0 && !drawer_visible && topbar_rect.is_positive() {
            let footprint = fastframe_titlebar::footprint(ctx, TITLEBAR_HEIGHT);
            top_page_actions::required_top_width(app, ui, palette).and_then(|width| {
                top_page_actions::top_slot(topbar_rect, footprint.left, footprint.right, width)
            })
        } else {
            None
        };
    if drawer_visible {
        egui::Panel::right(egui::Id::new("agent-drawer"))
            .exact_size(drawer_width)
            .resizable(false)
            .frame(
                egui::Frame::NONE
                    .fill(palette.panel)
                    .stroke(egui::Stroke::new(1.0, palette.outline))
                    .inner_margin(egui::Margin::same(12)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new("Atlas Agent")
                                .color(palette.text)
                                .strong()
                                .size(17.0),
                        );
                        ui.label(
                            RichText::new(t!(app.locale, "Here to help with your shop."))
                                .color(palette.secondary)
                                .size(12.0),
                        );
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if secondary_button(ui, t!(app.locale, "Close"), palette) {
                            app.agent_drawer_open = false;
                        }
                    });
                });
                ui.separator();
                super::agent::draw(app, ui, palette);
            });
    }

    egui::CentralPanel::no_frame()
        .frame(
            // The framebuffer already uses palette.window. An opaque panel
            // frame can expand beyond its assigned bounds and cover the
            // neighbouring drawer before the child clip is applied.
            egui::Frame::NONE.inner_margin(egui::Margin::same(0)),
        )
        .show(ui, |ui| {
            // Child widgets can request more than the central panel's width.
            // Clip their painting to the space left after the right drawer is
            // reserved, so they cannot paint across its controls.
            let mut bounds = ui.max_rect();
            if drawer_visible {
                bounds.max.x = bounds.max.x.min(viewport_width - drawer_width);
                ui.set_max_width(bounds.width().max(0.0));
            }
            ui.set_clip_rect(ui.clip_rect().intersect(bounds));
            widgets::overlay_at(ui, bounds, |ui| {
                ui.set_clip_rect(ui.clip_rect().intersect(bounds));
                draw_content(app, ui, ctx, palette, top_actions_slot.is_some());
            });
        });

    if topbar_rect.is_positive() {
        egui::Area::new(egui::Id::new("shell-floating-topbar"))
            .order(egui::Order::Middle)
            .fixed_pos(topbar_rect.min)
            .constrain(false)
            .show(ctx, |ui| {
                ui.set_min_size(topbar_rect.size());
                ui.set_max_size(topbar_rect.size());
                topbar::draw_titlebar(app, ui, ctx, palette);
            });
    }

    if let Some(rect) = top_actions_slot {
        egui::Area::new(egui::Id::new("shell-floating-page-actions"))
            .order(egui::Order::Middle)
            .fixed_pos(rect.min)
            .constrain(false)
            .show(ctx, |ui| {
                ui.set_min_size(rect.size());
                ui.set_max_size(rect.size());
                ui.set_clip_rect(ui.clip_rect().intersect(rect));
                top_page_actions::draw(app, ui, palette);
            });
    }

    // Global agent entry point, mirroring the web's floating Atlas Agent:
    // one FAB on every shell screen except the agent itself (which carries
    // its own new-conversation FAB). Hidden behind modal overlays.
    if agent_available
        && !app.agent_drawer_open
        && !app.modal_open()
        && !app.palette_open
        && !app.profile_menu_open
    {
        let fab = widgets::fab(
            ctx,
            palette,
            Icon::Bot,
            "shell-agent-fab",
            &t!(app.locale, "AI Agent"),
            app.settings.reduced_motion,
        );
        if fab.clicked() {
            app.agent_drawer_open = true;
        }
    }

    // The palette sits above the panels but below a blocking confirmation,
    // which always owns the topmost layer.
    let dialogs_open = match app.screen {
        Screen::Shell(Route::Products) => super::products::has_dialogs(app),
        Screen::Shell(Route::Clients) => super::clients::has_dialogs(app),
        Screen::Shell(Route::Employees) => super::employees::has_dialogs(app),
        Screen::Shell(Route::Vendors) => super::vendors::has_dialogs(app),
        Screen::Shell(Route::Expenses) => super::expenses::has_dialogs(app),
        Screen::Shell(Route::DeliveryNotes) => super::delivery_notes::has_dialogs(app),
        _ => false,
    };
    if dialogs_open {
        let screen = ctx.input(|input| input.viewport_rect());
        egui::Area::new(egui::Id::new("page-dialog-layer"))
            .order(egui::Order::Foreground)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                ui.set_min_size(screen.size());
                match app.screen {
                    Screen::Shell(Route::Products) => {
                        super::products::draw_dialogs(app, ui, palette)
                    }
                    Screen::Shell(Route::Clients) => super::clients::draw_dialogs(app, ui, palette),
                    Screen::Shell(Route::Employees) => {
                        super::employees::draw_dialogs(app, ui, palette)
                    }
                    Screen::Shell(Route::Vendors) => super::vendors::draw_dialogs(app, ui, palette),
                    Screen::Shell(Route::Expenses) => {
                        super::expenses::draw_dialogs(app, ui, palette)
                    }
                    Screen::Shell(Route::DeliveryNotes) => {
                        super::delivery_notes::draw_dialogs(app, ui, palette)
                    }
                    _ => {}
                }
            });
    }
    if app.palette_open {
        let screen = ctx.input(|input| input.viewport_rect());
        egui::Area::new(egui::Id::new("command-palette-layer"))
            .order(egui::Order::Foreground)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                ui.set_min_size(screen.size());
                crate::shortcuts::draw_palette(app, ui, ctx, palette);
            });
    }

    if app.settings_dialog_open {
        draw_settings_dialog(app, ctx, palette);
    } else {
        ctx.data_mut(|data| {
            data.insert_temp(egui::Id::new(super::settings::SETTINGS_OPEN_ID), false)
        });
    }
    if app.confirm_dialog.is_some() && !app.settings_dialog_open {
        let screen = ctx.input(|input| input.viewport_rect());
        egui::Area::new(egui::Id::new("confirmation-layer"))
            .order(egui::Order::Foreground)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                ui.set_min_size(screen.size());
                draw_confirm_dialog(app, ui, palette);
            });
    }
    if app.update.dialog_open {
        draw_update_dialog(app, ctx, palette);
    }
}

/// Settings is an overlay, not navigation: the active page and its draft
/// remain intact. Clicking the scrim does not discard settings in progress.
fn draw_settings_dialog(app: &mut AtlasApp, ctx: &egui::Context, palette: &Palette) {
    let screen = ctx.input(|input| input.viewport_rect());
    let width = (screen.width() - 32.0).clamp(1.0, 1080.0);
    let height = (screen.height() - 32.0).clamp(1.0, 740.0);
    let dialog = egui::Rect::from_center_size(screen.center(), vec2(width, height));
    egui::Area::new(egui::Id::new("settings-dialog-layer"))
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            ui.set_min_size(screen.size());
            let _ = ui.interact(
                screen,
                egui::Id::new("settings-dialog-scrim"),
                egui::Sense::click(),
            );
            ui.painter()
                .rect_filled(screen, 0.0, egui::Color32::from_black_alpha(133));
            widgets::overlay_at(ui, dialog, |ui| {
                ui.set_clip_rect(ui.clip_rect().intersect(dialog));
                egui::Frame::NONE
                    .fill(palette.overlay)
                    .stroke(egui::Stroke::new(1.0, palette.outline))
                    .corner_radius(20.0)
                    .shadow(crate::theme::elevation_shadow(palette))
                    .inner_margin(egui::Margin::same(16))
                    .show(ui, |ui| {
                        ui.set_width((width - 32.0).max(1.0));
                        ui.set_height((height - 32.0).max(1.0));
                        super::settings::draw(app, ui, ctx, palette);
                    });
            });
            if app.created_key.is_some() {
                super::settings::draw_created_key_modal(app, ui, palette);
            }
            if app.confirm_dialog.is_some() {
                draw_confirm_dialog(app, ui, palette);
            }
        });
}

/// The feed describes metadata only; the existing updater performs the actual
/// verified GitHub handoff after the user explicitly starts installation.
fn draw_update_dialog(app: &mut AtlasApp, ctx: &egui::Context, palette: &Palette) {
    let Some(manifest) = app.update.last_manifest.clone() else {
        app.update.dialog_open = false;
        return;
    };
    let screen = ctx.input(|input| input.viewport_rect());
    let width = (screen.width() - 32.0).clamp(1.0, 520.0);
    let height = (screen.height() - 32.0).clamp(1.0, 600.0);
    let dialog = egui::Rect::from_center_size(screen.center(), vec2(width, height));
    egui::Area::new(egui::Id::new("update-dialog-layer"))
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            ui.set_min_size(screen.size());
            let _ = ui.interact(screen, egui::Id::new("update-scrim"), egui::Sense::click());
            ui.painter()
                .rect_filled(screen, 0.0, egui::Color32::from_black_alpha(133));
            widgets::overlay_at(ui, dialog, |ui| {
                egui::Frame::NONE
                    .fill(palette.overlay)
                    .stroke(egui::Stroke::new(1.0, palette.outline))
                    .corner_radius(20.0)
                    .shadow(crate::theme::elevation_shadow(palette))
                    .inner_margin(egui::Margin::same(20))
                    .show(ui, |ui| {
                        ui.set_width((width - 40.0).max(1.0));
                        ui.set_height((height - 40.0).max(1.0));
                        ui.label(
                            egui::RichText::new(t!(app.locale, "Update available"))
                                .size(20.0)
                                .strong()
                                .color(palette.text),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new(format!(
                                "v{} → v{}",
                                env!("CARGO_PKG_VERSION"),
                                manifest.version
                            ))
                            .strong()
                            .color(palette.text),
                        );
                        let channel = match app.settings.update_channel {
                            atlas_core::settings::UpdateChannel::Stable => t!(app.locale, "Stable"),
                            atlas_core::settings::UpdateChannel::Prerelease => {
                                t!(app.locale, "Pre-release")
                            }
                        };
                        ui.label(
                            egui::RichText::new(format!(
                                "{} · {}",
                                channel,
                                if manifest.published_at.is_empty() {
                                    "—"
                                } else {
                                    &manifest.published_at
                                }
                            ))
                            .color(palette.secondary),
                        );
                        ui.add_space(8.0);
                        widgets::update_metadata(
                            ui,
                            app.update.desktop_release.as_ref(),
                            palette,
                            app.locale,
                        );
                        ui.add_space(8.0);
                        if !manifest.notes.trim().is_empty() {
                            widgets::release_notes(
                                ui,
                                &manifest.notes,
                                palette,
                                (height - 260.0).clamp(40.0, 260.0),
                            );
                        }
                        if let Some((received, total)) = app.update.progress {
                            ui.label(
                                egui::RichText::new(&app.update.stage).color(palette.secondary),
                            );
                            let value = if total > 0 {
                                (received as f32 / total as f32).clamp(0.0, 1.0)
                            } else {
                                0.0
                            };
                            ui.add(egui::ProgressBar::new(value).show_percentage());
                        }
                        if let Some(error) = &app.update.error {
                            widgets::error_banner(ui, error, palette);
                        }
                        ui.add_space(12.0);
                        ui.horizontal_wrapped(|ui| {
                            if widgets::primary_button(
                                ui,
                                t!(app.locale, "Download and install update"),
                                palette,
                                app.update.progress.is_none(),
                            ) {
                                app.start_update_download();
                            }
                            ui.add_enabled_ui(app.update.mandatory.is_none(), |ui| {
                                if widgets::secondary_button(ui, t!(app.locale, "Later"), palette) {
                                    app.update.dialog_open = false;
                                }
                            });
                        });
                    });
            });
        });
}

// ── Routed content ─────────────────────────────────────────────────────────

fn draw_content(
    app: &mut AtlasApp,
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    palette: &Palette,
    actions_in_top_band: bool,
) {
    // Web-style entrance: each navigation replays the standard 320 ms
    // cubic-bezier(0.2, 0, 0, 1) fade while the content rises ~18 px into
    // place. The id carries the animation token, so a fresh navigation
    // starts from zero. The very first shell entry (token 0) stays static:
    // a fade from opacity zero on a cold window reads as launch flicker,
    // not as navigation.
    let entrance = if app.anim_token == 0 {
        1.0
    } else {
        ctx.animate_bool_with_time_and_easing(
            egui::Id::new(("shell-route-enter", app.anim_token)),
            true,
            crate::motion::duration(crate::motion::BASE, app.settings.reduced_motion),
            crate::motion::standard,
        )
    };
    if !actions_in_top_band {
        top_page_actions::draw(app, ui, palette);
    }
    // Vertical-only scrolling: every screen wraps inside the viewport, so a
    // horizontal scrollbar must never appear (content overflow would paint
    // under the rail instead of reflowing).
    let route_id = format!("{:?}", app.screen);
    let scroll = egui::ScrollArea::vertical()
        .id_salt(("shell-route-content", route_id))
        .max_width(ui.available_width())
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.set_opacity(entrance);
            ui.add_space(4.0 + (1.0 - entrance) * 18.0);
            if app.business_search_open {
                draw_business_search(app, ui, palette);
                ui.add_space(12.0);
            }
            match app.screen {
                Screen::Shell(Route::Dashboard) => {
                    super::dashboard::draw(app, ui, palette);
                }
                Screen::Shell(Route::Products) => {
                    super::products::draw(app, ui, palette);
                }
                Screen::Shell(Route::Clients) => {
                    super::clients::draw(app, ui, palette);
                }
                Screen::Shell(Route::Catalog) => {
                    super::catalog::draw(app, ui, palette);
                }
                Screen::Shell(Route::Vendors) => {
                    super::vendors::draw(app, ui, palette);
                }
                Screen::Shell(Route::Expenses) => {
                    super::expenses::draw(app, ui, palette);
                }
                Screen::Shell(Route::DeliveryNotes) => {
                    super::delivery_notes::draw(app, ui, palette);
                }
                Screen::Shell(Route::Sales) => {
                    super::sales::draw(app, ui, palette);
                }
                Screen::Shell(Route::Employees) => {
                    super::employees::draw(app, ui, palette);
                }
                Screen::Shell(Route::Attendance) => {
                    super::attendance::draw(app, ui, palette);
                }
                Screen::Shell(Route::Payroll) => {
                    super::payroll::draw(app, ui, palette);
                }
                Screen::Shell(Route::InventoryOps) => {
                    super::inventory_ops::draw(app, ui, palette);
                }
                Screen::Shell(Route::CashShifts) => {
                    super::cash_shifts::draw(app, ui, palette);
                }
                Screen::Shell(Route::Actions) => {
                    super::actions::draw(app, ui, palette);
                }
                Screen::Shell(Route::Agent) => {
                    super::agent::draw(app, ui, palette);
                }
                Screen::Shell(Route::Notifications) => {
                    super::notifications::draw(app, ui, palette);
                }
                _ => {}
            }
        });
    topbar::record_scroll(app, ctx, scroll.state.offset.y);
}

/// Results are explicitly scoped: products are server-filtered (max 200),
/// clients and documents are filtered from the current list endpoints.
fn draw_business_search(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    widgets::card(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(format!("{} : {}", t!(app.locale, "Business search"), app.business_search_query)).color(palette.text).strong());
            if secondary_button(ui, t!(app.locale, "Close"), palette) {
                app.business_search_open = false;
                app.business_search = Default::default();
            }
        });
        ui.label(RichText::new(t!(app.locale, "Products: first 200 server matches; other categories: first 40 matches. Delivery notes: latest 100 only.")).color(palette.secondary));
        if app.business_search.loading { super::centered_spinner(ui, palette); return; }
        if let Some(error) = &app.business_search.error { widgets::error_banner(ui, error, palette); return; }
        let Some(result) = app.business_search.data.clone() else { return; };
        if !result.failed.is_empty() {
            let labels = result.failed.iter().map(|name| match *name { "Products" => t!(app.locale, "Products").into_owned(), "Clients" => t!(app.locale, "Clients").into_owned(), "Vendors" => t!(app.locale, "Vendors").into_owned(), "Receipts" => t!(app.locale, "Receipts").into_owned(), "Delivery notes" => t!(app.locale, "Delivery notes").into_owned(), "Supplier bills" => t!(app.locale, "Supplier bills").into_owned(), _ => t!(app.locale, "Sales").into_owned() }).collect::<Vec<_>>().join(", ");
            ui.label(RichText::new(format!("{} : {labels}", t!(app.locale, "Search unavailable for"))).color(palette.warning));
        }
        if result.hits.is_empty() && result.failed.is_empty() { ui.label(t!(app.locale, "No results.")); }
        for hit in result.hits {
            let (_, route_category) = route_meta(app.locale, hit.route);
            let category = if hit.receipt { t!(app.locale, "Receipts").into_owned() } else if hit.route == Route::Expenses { t!(app.locale, "Supplier bills").into_owned() } else { route_category };
            ui.horizontal_wrapped(|ui| {
                if secondary_button(ui, t!(app.locale, "View"), palette) {
                    app.business_search_open = false;
                    app.business_search = Default::default();
                    match hit.route {
                        Route::Products => { app.product_search = hit.label.clone(); app.go(Route::Products); app.load_products(); }
                        Route::Clients => { app.client_search.clear(); app.selected_client = Some(hit.id); app.client_invoice_id = None; app.client_invoices = Default::default(); app.invoice_detail_id = None; app.invoice_detail = Default::default(); app.go(Route::Clients); app.load_clients(); }
                        Route::Vendors => { app.vendor_search.clear(); app.selected_vendor = Some(hit.id); app.go(Route::Vendors); app.load_vendors(); }
                        Route::DeliveryNotes => { app.delivery_search = hit.label.clone(); app.go(Route::DeliveryNotes); app.load_delivery_notes(); }
                        Route::Expenses => { app.bills_open = true; app.bill_search = hit.label.clone(); app.go(Route::Expenses); app.load_bills(); }
                        Route::Sales if hit.receipt => { app.quotes_open = false; app.refunds_open = false; app.quick_sale_open = false; app.receipts_open = true; app.receipt_search = hit.label.clone(); app.invoice_detail_id = None; app.invoice_detail = Default::default(); app.go(Route::Sales); app.load_receipts(); }
                        Route::Sales => { app.quotes_open = false; app.refunds_open = false; app.quick_sale_open = false; app.receipts_open = false; app.invoice_search.clear(); app.go(Route::Sales); app.open_invoice_detail(hit.id); }
                        _ => {}
                    }
                }
                ui.label(RichText::new(format!("{category} · {} · {}", hit.label, hit.subtitle)).color(palette.text));
            });
        }
    });
}

// ── Confirm dialog ─────────────────────────────────────────────────────────

fn draw_confirm_dialog(app: &mut AtlasApp, ui: &mut egui::Ui, palette: &Palette) {
    let Some(dialog) = app.confirm_dialog.clone() else {
        return;
    };
    let screen = ui.ctx().input(|i| i.viewport_rect());
    let width = 420.0;
    let height = 210.0;
    let rect = egui::Rect::from_center_size(screen.center(), vec2(width, height));

    // Outside click cancels, mirroring the web wizard behaviour.
    if widgets::scrim_clicked(ui, screen, rect, egui::Id::new("confirm-scrim")) {
        app.confirm_dialog = None;
        return;
    }
    widgets::overlay_at(ui, rect, |ui| {
        widgets::dialog_frame(palette).show(ui, |ui| {
            egui::ScrollArea::vertical()
                .max_height((rect.height() - 46.0).max(1.0))
                .show(ui, |ui| {
                    ui.set_min_size(vec2(width - 44.0, 0.0));
                    ui.label(
                        RichText::new(dialog.title.clone())
                            .color(palette.text)
                            .font(crate::fonts::semibold(17.0)),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(dialog.body.clone())
                            .color(palette.secondary)
                            .size(13.5),
                    );
                    ui.add_space(16.0);
                    widgets::dialog_actions(ui, |ui| {
                        let confirmed = if dialog.danger {
                            danger_button(ui, &dialog.confirm_label, palette, true)
                        } else {
                            crate::widgets::primary_button(ui, &dialog.confirm_label, palette, true)
                        };
                        let dismissed = secondary_button(ui, t!(app.locale, "Cancel"), palette);
                        if dismissed {
                            app.confirm_dialog = None;
                        }
                        if confirmed {
                            run_confirm_action(app, dialog.action);
                            app.confirm_dialog = None;
                        }
                    });
                });
        });
    });
}

fn run_confirm_action(app: &mut AtlasApp, action: ConfirmAction) {
    match action {
        ConfirmAction::EnableAttendance => app.enable_attendance(),
        ConfirmAction::CompleteMobileAction(id) => app.submit_mobile_action(id, None),
        ConfirmAction::RejectMobileAction { id, reason } => {
            app.submit_mobile_action(id, Some(reason))
        }
        ConfirmAction::AcknowledgeInventoryUncertainty => {
            app.inventory_action.error = None;
            app.inventory_action.last_error = None;
        }
        ConfirmAction::AcknowledgeCashShiftUncertainty => {
            app.cash_shift_action.error = None;
            app.cash_shift_action.last_error = None;
        }
        ConfirmAction::AcknowledgeLedgerUncertainty => app.ledger_uncertain = false,
        ConfirmAction::OpenCashShift {
            till,
            opening,
            notes,
        } => app.open_cash_shift(till, opening, notes),
        ConfirmAction::CashShiftMovement {
            id,
            direction,
            method,
            amount,
            label,
            reference,
        } => app.cash_movement(id, direction, method, amount, label, reference),
        ConfirmAction::CloseCashShift { id, amount, notes } => {
            app.close_cash_shift(id, amount, notes)
        }
        ConfirmAction::CountInventory {
            product_id,
            quantity,
            note,
        } => app.count_inventory(product_id, quantity, note),
        ConfirmAction::SignOut => app.sign_out(),
        ConfirmAction::DiscardEditor => {
            app.product_editor = None;
        }
        ConfirmAction::ForgetServer(url) => {
            app.server_store.forget(&url);
            app.account_store.clear_server(&url);
            app.save_servers();
            app.save_accounts();
            app.notice(t!(app.locale, "Server removed.").into_owned());
            if app.server_store.selected_url().is_none() {
                app.sign_out();
                app.screen = Screen::ServerSetup;
            }
        }
        ConfirmAction::RevokeDevice(fingerprint) => {
            let Some(client) = app.client.clone() else {
                return;
            };
            // Revoke, then reload the list in the same job slot so the
            // removed device disappears on the next poll.
            let job = app.runner.spawn(async move {
                client.revoke_device(&fingerprint).await?;
                client.devices().await
            });
            app.devices.start(job);
            app.notice(t!(app.locale, "Device revoked.").into_owned());
        }
        ConfirmAction::RevokeApiKey(id) => app.revoke_api_key(id),
    }
}

#[cfg(test)]
mod search_tests {
    use super::{Route, search_destination};

    #[test]
    fn recognizes_navigation_but_leaves_product_queries_intact() {
        assert_eq!(search_destination(" Factures "), Some(Route::Sales));
        assert_eq!(search_destination("Réglages"), None);
        assert_eq!(search_destination("Situation"), Some(Route::Clients));
        assert_eq!(search_destination("café 1kg"), None);
    }
}
