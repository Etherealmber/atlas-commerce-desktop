//! Native login layout, based on the measured web reference.
use crate::icons::Icon;
use crate::state::AtlasApp;
use crate::theme::Palette;
use egui::{
    Align2, Color32, FontId, Id, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2,
};

const WHITE: Color32 = Color32::from_rgb(250, 250, 250);
const MUTED: Color32 = Color32::from_rgb(161, 161, 170);
const BORDER: Color32 = Color32::from_rgb(63, 63, 70);
const LOGO: &[u8] = include_bytes!("../../assets/login/atlas-logo-dark.png");
const QR_ICON: &[u8] = include_bytes!("../../assets/login/atlas-icon-light.png");

fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect::from_min_size(pos2(x, y), vec2(w.max(0.0), h))
}

fn image(ui: &Ui, bytes: &'static [u8], name: &'static str, bounds: Rect) {
    let id = Id::new(name);
    let cached = ui
        .ctx()
        .data(|data| data.get_temp::<egui::TextureHandle>(id));
    let texture = cached.or_else(|| {
        let rgba = image::load_from_memory(bytes).ok()?.into_rgba8();
        let size = [rgba.width() as usize, rgba.height() as usize];
        let img = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
        let texture = ui
            .ctx()
            .load_texture(name, img, egui::TextureOptions::LINEAR);
        ui.ctx()
            .data_mut(|data| data.insert_temp(id, texture.clone()));
        Some(texture)
    });
    if let Some(texture) = texture {
        ui.painter().image(
            texture.id(),
            bounds,
            Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    }
}

fn text(ui: &Ui, at: Pos2, align: Align2, value: &str, font: FontId, color: Color32) {
    ui.painter().text(at, align, value, font, color);
}

fn border(ui: &Ui, bounds: Rect, radius: u8, fill: Color32, stroke: Color32) {
    ui.painter().rect_filled(bounds, radius, fill);
    ui.painter()
        .rect_stroke(bounds, radius, Stroke::new(1.0, stroke), StrokeKind::Inside);
}

fn button(
    ui: &mut Ui,
    id: &'static str,
    bounds: Rect,
    caption: &str,
    fill: Color32,
    ink: Color32,
    radius: u8,
) -> bool {
    let response = ui.interact(bounds, Id::new(id), Sense::click());
    let fill = if response.hovered() {
        fill.gamma_multiply(1.1)
    } else {
        fill
    };
    let stroke = if radius == 24 || fill == Color32::TRANSPARENT {
        fill
    } else {
        BORDER
    };
    border(ui, bounds, radius, fill, stroke);
    text(
        ui,
        bounds.center(),
        Align2::CENTER_CENTER,
        caption,
        crate::fonts::semibold(13.6),
        ink,
    );
    response.clicked()
}

fn input(
    ui: &mut Ui,
    bounds: Rect,
    id: Id,
    value: &mut String,
    hint: &str,
    secret: bool,
    dark: bool,
) -> egui::Response {
    let focused = ui.memory(|memory| memory.has_focus(id));
    let bg = if focused && dark {
        Color32::from_rgb(32, 32, 36)
    } else if dark {
        Color32::from_rgb(24, 24, 27)
    } else {
        Color32::from_rgb(10, 10, 10)
    };
    if focused {
        let ring = if dark {
            Color32::from_rgba_unmultiplied(250, 250, 250, 31)
        } else {
            WHITE
        };
        ui.painter().rect_stroke(
            bounds.expand(if dark { 3.0 } else { 1.0 }),
            11,
            Stroke::new(if dark { 3.0 } else { 1.0 }, ring),
            StrokeKind::Inside,
        );
    }
    border(ui, bounds, 10, bg, if focused { WHITE } else { BORDER });
    let edit_rect = rect(
        bounds.left() + 20.0,
        bounds.top() + 10.0,
        bounds.width() - if secret { 70.0 } else { 40.0 },
        24.0,
    );
    ui.put(
        edit_rect,
        egui::TextEdit::singleline(value)
            .id(id)
            .frame(false)
            .font(crate::fonts::regular(14.0))
            .text_color(WHITE)
            .hint_text(hint)
            .password(secret),
    )
}

/// Draw the browser-free login page; the v3 transport is not yet wired to UI.
pub(super) fn draw(app: &mut AtlasApp, ui: &mut Ui, palette: &Palette) {
    let dark = palette.dark;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let available = ui.available_size();
            let height = available.y.max(760.0);
            let (page, _) = ui.allocate_exact_size(vec2(available.x, height), Sense::hover());
            // A three-stop vertical gradient is the native equivalent of the web backdrop.
            let mut mesh = egui::Mesh::default();
            let colors = if dark {
                [
                    Color32::from_rgb(23, 23, 25),
                    Color32::from_rgb(17, 17, 19),
                    Color32::from_rgb(13, 13, 15),
                ]
            } else {
                [
                    Color32::from_rgb(25, 25, 27),
                    Color32::from_rgb(12, 12, 14),
                    Color32::from_rgb(8, 8, 9),
                ]
            };
            for (index, color) in colors.into_iter().enumerate() {
                let y = page.top() + page.height() * index as f32 / 2.0;
                mesh.colored_vertex(pos2(page.left(), y), color);
                mesh.colored_vertex(pos2(page.right(), y), color);
            }
            mesh.indices
                .extend_from_slice(&[0, 1, 2, 1, 3, 2, 2, 3, 4, 3, 5, 4]);
            ui.painter().add(egui::Shape::mesh(mesh));
            let card_width = (page.width() - 32.0).clamp(0.0, 960.0);
            let error = app.login.error.is_some();
            let card_height = if error { 613.7 } else { 556.2 };
            let top = (page.top() + (available.y - 800.0) / 2.0 + if error { 56.0 } else { 79.8 })
                .max(page.top() + 56.0);
            let card = rect(
                page.center().x - card_width / 2.0,
                top,
                card_width,
                card_height,
            );
            let split = card.right() - 281.0;
            border(
                ui,
                card,
                24,
                Color32::from_rgb(20, 20, 20),
                if dark {
                    Color32::from_rgb(47, 47, 49)
                } else {
                    Color32::from_rgb(46, 46, 51)
                },
            );
            let right = rect(split, card.top() + 1.0, 280.0, card.height() - 2.0);
            ui.painter().rect_filled(
                right.shrink2(vec2(0.0, 1.0)),
                0.0,
                if dark {
                    Color32::from_rgb(22, 22, 24)
                } else {
                    Color32::from_rgb(20, 20, 20)
                },
            );
            ui.painter().line_segment(
                [right.left_top(), right.left_bottom()],
                Stroke::new(1.0, Color32::from_rgb(39, 39, 42)),
            );
            let center = (card.left() + 1.0 + split) / 2.0;
            let form_width = 400.0_f32.min((split - card.left() - 96.0).max(0.0));
            let fx = center - form_width / 2.0;
            let base = card.top();
            image(
                ui,
                LOGO,
                "login-reference-logo",
                rect(center - 98.8, base + 49.0, 197.6, 64.0),
            );
            text(
                ui,
                pos2(center, base + 144.0),
                Align2::CENTER_CENTER,
                "Bon retour",
                crate::fonts::bold(24.0),
                WHITE,
            );
            text(
                ui,
                pos2(center, base + 179.5),
                Align2::CENTER_CENTER,
                "Connectez-vous pour gérer stock, caisse et facturation",
                crate::fonts::regular(14.4),
                MUTED,
            );
            let shift = if error { 57.5 } else { 0.0 };
            if let Some(message) = &app.login.error {
                let alert = rect(fx, base + 210.0, form_width, 41.5);
                border(
                    ui,
                    alert,
                    10,
                    Color32::from_rgb(48, 24, 24),
                    Color32::from_rgb(101, 40, 40),
                );
                text(
                    ui,
                    pos2(fx + 12.0, alert.center().y),
                    Align2::LEFT_CENTER,
                    message,
                    crate::fonts::regular(13.0),
                    Color32::from_rgb(248, 113, 113),
                );
            }
            let user_id = Id::new("login-reference-username");
            let pass_id = Id::new("login-reference-password");
            let otp_id = Id::new("login-reference-otp");
            let focus_once = Id::new("login-reference-focused");
            if !ui
                .ctx()
                .data(|data| data.get_temp::<bool>(focus_once).unwrap_or(false))
            {
                ui.memory_mut(|memory| {
                    memory.request_focus(if app.login.needs_otp {
                        otp_id
                    } else if app.login.username.is_empty() {
                        user_id
                    } else {
                        pass_id
                    })
                });
                ui.ctx().data_mut(|data| data.insert_temp(focus_once, true));
            }
            if app.login.needs_otp {
                let info = rect(fx, base + 209.7, form_width, 41.5);
                border(
                    ui,
                    info,
                    10,
                    if dark {
                        Color32::from_rgb(39, 39, 42)
                    } else {
                        Color32::from_rgb(244, 244, 245)
                    },
                    BORDER,
                );
                text(
                    ui,
                    pos2(fx + 12.0, info.center().y),
                    Align2::LEFT_CENTER,
                    app.login
                        .notice
                        .as_deref()
                        .unwrap_or("Un code de vérification a été envoyé."),
                    crate::fonts::regular(13.0),
                    if dark { WHITE } else { BORDER },
                );
                text(
                    ui,
                    pos2(fx, base + 267.0),
                    Align2::LEFT_TOP,
                    "CODE DE VÉRIFICATION (OTP)",
                    crate::fonts::bold(11.52),
                    MUTED,
                );
                let response = input(
                    ui,
                    rect(fx, base + 296.8, form_width, 44.0),
                    otp_id,
                    &mut app.login.otp,
                    "Code reçu par e-mail",
                    false,
                    dark,
                );
                if response.changed() {
                    app.login.otp.retain(|c| c.is_ascii_digit());
                    app.login.otp.truncate(8);
                }
                if button(
                    ui,
                    "login-otp-submit",
                    rect(fx, base + 360.8, form_width, 48.0),
                    "Entrer dans Atlas",
                    WHITE,
                    Color32::BLACK,
                    24,
                ) {
                    app.login.error = Some("Connexion native non intégrée.".to_owned());
                }
                if button(
                    ui,
                    "login-otp-back",
                    rect(fx, base + 432.8, form_width, 44.0),
                    "← Retour",
                    Color32::TRANSPARENT,
                    MUTED,
                    12,
                ) {
                    app.login.needs_otp = false;
                    app.login.error = None;
                }
                text(
                    ui,
                    pos2(center, base + 503.0),
                    Align2::CENTER_CENTER,
                    "Code valable 10 min.",
                    crate::fonts::regular(12.8),
                    MUTED,
                );
            } else {
                text(
                    ui,
                    pos2(fx, base + 209.9 + shift),
                    Align2::LEFT_TOP,
                    "IDENTIFIANT",
                    crate::fonts::bold(11.52),
                    MUTED,
                );
                input(
                    ui,
                    rect(fx, base + 239.5 + shift, form_width, 44.0),
                    user_id,
                    &mut app.login.username,
                    "Votre identifiant",
                    false,
                    dark,
                );
                text(
                    ui,
                    pos2(fx, base + 297.5 + shift),
                    Align2::LEFT_TOP,
                    "MOT DE PASSE",
                    crate::fonts::bold(11.52),
                    MUTED,
                );
                let password_rect = rect(fx, base + 327.2 + shift, form_width, 44.0);
                input(
                    ui,
                    password_rect,
                    pass_id,
                    &mut app.login.password,
                    "••••••••",
                    !app.login.show_password,
                    dark,
                );
                let icon = if app.login.show_password {
                    Icon::EyeOff
                } else {
                    Icon::Eye
                };
                if ui
                    .put(
                        rect(
                            password_rect.right() - 38.0,
                            password_rect.top() + 8.0,
                            28.0,
                            28.0,
                        ),
                        egui::Button::image(icon.image(MUTED, 20.0)).frame(egui::Frame::NONE),
                    )
                    .on_hover_text(if app.login.show_password {
                        "Masquer"
                    } else {
                        "Afficher"
                    })
                    .clicked()
                {
                    app.login.show_password = !app.login.show_password;
                }
                let check = rect(fx, base + 389.2 + shift, 20.0, 20.0);
                let check_id = Id::new("login-remember-30");
                let mut checked = ui
                    .ctx()
                    .data(|data| data.get_temp::<bool>(check_id).unwrap_or(false));
                if ui.interact(check, check_id, Sense::click()).clicked() {
                    checked = !checked;
                    ui.ctx()
                        .data_mut(|data| data.insert_temp(check_id, checked));
                }
                border(
                    ui,
                    check,
                    6,
                    if checked {
                        Color32::from_rgb(10, 10, 10)
                    } else if dark {
                        Color32::from_rgb(28, 28, 31)
                    } else {
                        Color32::WHITE
                    },
                    if checked {
                        Color32::from_rgb(10, 10, 10)
                    } else {
                        BORDER
                    },
                );
                if checked {
                    text(
                        ui,
                        check.center(),
                        Align2::CENTER_CENTER,
                        "✓",
                        crate::fonts::bold(16.0),
                        WHITE,
                    );
                }
                text(
                    ui,
                    pos2(fx + 28.0, check.center().y),
                    Align2::LEFT_CENTER,
                    "SE SOUVENIR 30 JOURS",
                    crate::fonts::bold(11.52),
                    MUTED,
                );
                let clicked = button(
                    ui,
                    "login-submit",
                    rect(fx, base + 439.2 + shift, form_width, 48.0),
                    if app.login.busy {
                        "Un instant…"
                    } else {
                        "Continuer  →"
                    },
                    WHITE,
                    Color32::from_rgb(10, 10, 10),
                    24,
                );
                let enter = ui.input(|input| input.key_pressed(egui::Key::Enter));
                if (clicked || enter) && !app.login.busy {
                    app.login.error = Some(
                        if app.login.username.trim().is_empty() {
                            "Identifiants manquants."
                        } else if app.login.password.is_empty() {
                            "Veuillez saisir votre mot de passe."
                        } else {
                            "Connexion native non intégrée."
                        }
                        .to_owned(),
                    );
                }
            }
            let rc = right.center().x;
            if app.qr.status == "claimed" && !app.qr.account.is_empty() {
                let avatar = pos2(rc, base + 188.5);
                ui.painter()
                    .circle_filled(avatar, 48.0, Color32::from_rgb(39, 39, 42));
                ui.painter().circle_stroke(
                    avatar,
                    46.5,
                    Stroke::new(3.0, Color32::from_rgb(39, 39, 42)),
                );
                let initial = app
                    .qr
                    .account
                    .chars()
                    .next()
                    .unwrap_or('?')
                    .to_uppercase()
                    .to_string();
                text(
                    ui,
                    avatar,
                    Align2::CENTER_CENTER,
                    &initial,
                    crate::fonts::bold(32.0),
                    WHITE,
                );
                text(
                    ui,
                    pos2(rc, base + 269.0),
                    Align2::CENTER_CENTER,
                    "Vérifiez votre téléphone !",
                    crate::fonts::bold(16.8),
                    WHITE,
                );
                text(
                    ui,
                    pos2(rc, base + 297.7),
                    Align2::CENTER_TOP,
                    &format!("Acceptez sur le téléphone pour\n{}", app.qr.account),
                    crate::fonts::regular(12.8),
                    MUTED,
                );
                if button(
                    ui,
                    "login-qr-restart",
                    rect(rc - 111.5, base + 352.8, 223.0, 62.8),
                    "Ce n’est pas moi —\nrecommencer",
                    Color32::from_rgb(63, 63, 70),
                    WHITE,
                    12,
                ) {
                    app.qr = Default::default();
                    app.login.error = Some("Connexion QR native non intégrée.".to_owned());
                }
            } else {
                let qr_bounds = rect(rc - 88.0, base + 69.3, 176.0, 176.0);
                let qr = app.qr.qr_url.as_str();
                if !qr.is_empty() {
                    ui.scope_builder(egui::UiBuilder::new().max_rect(qr_bounds), |ui| {
                        super::auth::qr_image(ui, qr, palette)
                    });
                } else if let Some(url) = app.native_login.qr_url.as_deref() {
                    ui.scope_builder(egui::UiBuilder::new().max_rect(qr_bounds), |ui| {
                        super::auth::qr_image(ui, url, palette)
                    });
                } else {
                    ui.painter().rect_filled(qr_bounds, 12, WHITE);
                }
                if !qr.is_empty() || app.native_login.qr_url.is_some() {
                    image(
                        ui,
                        QR_ICON,
                        "login-reference-qr-icon",
                        Rect::from_center_size(qr_bounds.center(), Vec2::splat(26.0)),
                    );
                }
                text(
                    ui,
                    pos2(rc, base + 279.0),
                    Align2::CENTER_CENTER,
                    "Connexion avec un QR",
                    crate::fonts::bold(16.8),
                    WHITE,
                );
                text(
                    ui,
                    pos2(rc, base + 320.0),
                    Align2::CENTER_TOP,
                    "Scannez avec l’app Atlas pour\nvous connecter instantanément.",
                    crate::fonts::regular(12.8),
                    MUTED,
                );
                let fill = if dark {
                    Color32::from_rgb(63, 63, 70)
                } else {
                    Color32::from_rgb(35, 35, 37)
                };
                if button(
                    ui,
                    "login-passkey",
                    rect(rc - 111.5, base + 365.6, 223.0, 62.8),
                    "Ou se connecter avec une\npasskey",
                    fill,
                    WHITE,
                    12,
                ) {
                    app.login.error = Some("Passkey native non intégrée.".to_owned());
                }
                if button(
                    ui,
                    "login-android",
                    rect(rc - 111.5, base + 444.4, 223.0, 42.4),
                    "Télécharger l’app Android",
                    Color32::from_rgb(32, 32, 34),
                    WHITE,
                    12,
                ) {
                    ui.ctx()
                        .open_url(egui::OpenUrl::new_tab("https://stock.afmarbre.com/android"));
                }
            }
            let footer_y = card.bottom() + 44.0;
            ui.painter().line_segment(
                [
                    pos2(page.center().x - 233.85, footer_y),
                    pos2(page.center().x + 233.85, footer_y),
                ],
                Stroke::new(1.0, Color32::from_rgb(39, 39, 42)),
            );
            let links = [
                ("Confidentialité", "/privacy"),
                ("Conditions", "/terms"),
                ("Licences", "/licenses"),
                ("À propos", "/about"),
                ("GitHub", "https://github.com/Etherealmber/atlas-commerce"),
                ("Nouveautés", "/whats-new"),
            ];
            let mut x = page.center().x - 233.85;
            for (label, path) in links {
                let galley = ui.painter().layout_no_wrap(
                    label.to_owned(),
                    crate::fonts::medium(12.0),
                    MUTED,
                );
                let bounds = rect(x, footer_y + 17.0, galley.size().x, 18.0);
                if ui
                    .interact(bounds, Id::new(("login-footer", label)), Sense::click())
                    .clicked()
                {
                    let url = if path.starts_with('/') {
                        format!("https://stock.afmarbre.com{path}")
                    } else {
                        path.to_owned()
                    };
                    ui.ctx().open_url(egui::OpenUrl::new_tab(url));
                }
                ui.painter().galley(bounds.min, galley, MUTED);
                x += bounds.width() + 10.0;
                if label != "Nouveautés" {
                    text(
                        ui,
                        pos2(x, footer_y + 26.0),
                        Align2::LEFT_CENTER,
                        "·",
                        crate::fonts::regular(12.0),
                        MUTED,
                    );
                    x += 12.4;
                }
            }
            text(
                ui,
                pos2(page.center().x, footer_y + 55.0),
                Align2::CENTER_CENTER,
                &format!("© 2026 Atlas Commerce · v{}", env!("CARGO_PKG_VERSION")),
                crate::fonts::regular(11.5),
                MUTED,
            );
        });
}
