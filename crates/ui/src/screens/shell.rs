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
mod topbar;

#[cfg(test)]
use topbar::search_destination;

/// Height of the custom caption row (web topbar: 56 px).
const TITLEBAR_HEIGHT: f32 = 56.0;
/// Compact labelled rail and icon-only rail at narrower window widths.
const RAIL_WIDTH_EXPANDED: f32 = 224.0;
const RAIL_WIDTH_COLLAPSED: f32 = 72.0;
const RAIL_COLLAPSE_BELOW: f32 = 1180.0;

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
        Route::Expenses => (Icon::Wallet, t!(locale, "Expenses").into_owned()),
        Route::DeliveryNotes => (Icon::Truck, t!(locale, "Delivery notes").into_owned()),
        Route::Sales => (Icon::Receipt, t!(locale, "Sales").into_owned()),
        Route::Employees => (Icon::Users, t!(locale, "Employees").into_owned()),
        Route::Agent => (Icon::Bot, t!(locale, "AI Agent").into_owned()),
        Route::Notifications => (Icon::Bell, t!(locale, "Notifications").into_owned()),
        Route::Settings => (Icon::Settings, t!(locale, "Settings").into_owned()),
    }
}

pub fn draw_shell(app: &mut AtlasApp, ui: &mut egui::Ui, ctx: &egui::Context, palette: &Palette) {
    // Global keyboard layer first: Ctrl+K palette, Ctrl+1…9, Ctrl+R/F5,
    // Ctrl+F and Escape, all suppressed while a text field owns the keys.
    crate::shortcuts::handle_shortcuts(app, ctx);

    // Dedicated panels, not a hand-rolled horizontal layout: a `Panel::left`
    // with an exact size reserves its width up front, so the rail can never
    // expand to the full window and starve the central column (the blank
    // shell bug came from Frame + set_min_size, whose minimum width did not
    // stop rows calling available_width() from growing the frame).
    egui::Panel::top(egui::Id::new("shell-titlebar"))
        .exact_size(TITLEBAR_HEIGHT)
        .resizable(false)
        .show_separator_line(false)
        .frame(
            // Same lifted chrome as the rail, with a single hairline on the
            // bottom edge so the title bar reads as one piece with the rail
            // instead of blending into the content background.
            egui::Frame::NONE
                .fill(palette.panel)
                .inner_margin(egui::Margin::same(0)),
        )
        .show(ui, |ui| {
            topbar::draw_titlebar(app, ui, ctx, palette);
            let bottom = ui.min_rect().bottom();
            let left = egui::pos2(ui.min_rect().left(), bottom);
            let right = egui::pos2(ui.max_rect().right(), bottom);
            ui.painter()
                .line_segment([left, right], egui::Stroke::new(1.0, palette.outline));
        });

    // Below the 1180 px breakpoint the rail drops its labels and becomes
    // the 72 px icon strip, mirroring the web sidebar's collapsed state.
    let viewport_width = ctx.input(|input| input.viewport_rect().width());
    let rail_collapsed = ctx
        .data(|data| data.get_temp::<bool>(egui::Id::new("rail-collapsed")))
        .unwrap_or(viewport_width < RAIL_COLLAPSE_BELOW);
    let rail_width = if rail_collapsed {
        RAIL_WIDTH_COLLAPSED
    } else {
        RAIL_WIDTH_EXPANDED
    };
    let rail_frame = egui::Frame::NONE
        .fill(palette.panel)
        .inner_margin(if rail_collapsed {
            // Twelve-pixel gutters keep a full-width accessible row.
            egui::Margin::symmetric(12, 14)
        } else {
            egui::Margin::symmetric(10, 14)
        });
    egui::Panel::left(egui::Id::new("shell-rail"))
        .exact_size(rail_width)
        .resizable(false)
        .show_separator_line(false)
        .frame(rail_frame)
        .show(ui, |ui| {
            sidebar::draw_rail(app, ui, palette, rail_collapsed);
        });

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
                | Route::Catalog
        )
    );
    let drawer_visible = app.agent_drawer_open && agent_available;
    let drawer_width = (viewport_width * 0.42)
        .clamp(280.0, 440.0)
        .min((viewport_width - rail_width - 32.0).max(1.0));
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
                draw_content(app, ui, ctx, palette);
            });
        });

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
    if matches!(app.screen, Screen::Shell(Route::Employees)) {
        super::employees::draw_dialogs(app, ctx, palette);
    }
    crate::shortcuts::draw_palette(app, ui, ctx, palette);

    if app.confirm_dialog.is_some() {
        draw_confirm_dialog(app, ui, palette);
    }
}

// ── Custom title bar ───────────────────────────────────────────────────────

// ── Routed content ─────────────────────────────────────────────────────────

fn draw_content(app: &mut AtlasApp, ui: &mut egui::Ui, ctx: &egui::Context, palette: &Palette) {
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
    // Vertical-only scrolling: every screen wraps inside the viewport, so a
    // horizontal scrollbar must never appear (content overflow would paint
    // under the rail instead of reflowing).
    egui::ScrollArea::vertical()
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
                Screen::Shell(Route::Agent) => {
                    super::agent::draw(app, ui, palette);
                }
                Screen::Shell(Route::Notifications) => {
                    super::notifications::draw(app, ui, palette);
                }
                Screen::Shell(Route::Settings) => {
                    super::settings::draw(app, ui, ctx, palette);
                }
                _ => {}
            }
        });
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
                        Route::Sales if hit.receipt => { app.receipts_open = true; app.receipt_search = hit.label.clone(); app.invoice_detail_id = None; app.invoice_detail = Default::default(); app.go(Route::Sales); app.load_receipts(); }
                        Route::Sales => { app.invoice_search.clear(); app.go(Route::Sales); app.open_invoice_detail(hit.id); }
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
        assert_eq!(search_destination("Réglages"), Some(Route::Settings));
        assert_eq!(search_destination("Situation"), Some(Route::Clients));
        assert_eq!(search_destination("café 1kg"), None);
    }
}
