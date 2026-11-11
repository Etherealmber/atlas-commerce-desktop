//! Isolated, offline visual fixture. Run only by the screenshot workflow.
//! It never creates an API client or loads a real account/server from disk.
use atlas_api::models::{Client, Dashboard, DeliveryNote, Expense, Invoice, Product, Vendor};
use atlas_core::{
    AccountStore, AppPaths, DeviceIdentity, JobRunner, ServerStore, SessionRecord, Settings,
};
use atlas_ui::{
    AtlasApp,
    state::{Route, Screen},
    theme::{self, Palette},
};
use std::sync::{Arc, Mutex};

struct Fixture {
    app: AtlasApp,
    palette: Palette,
}
impl eframe::App for Fixture {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        self.palette.window.to_normalized_gamma_f32()
    }
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.app.draw(ui, &ctx, &self.palette);
    }
}
fn build(route: &str) -> AtlasApp {
    let session = SessionRecord {
        server_url: "https://fixture.invalid".into(),
        username: "demo".into(),
        role: "owner".into(),
        display_name: "Compte fictif".into(),
        email: String::new(),
        avatar_url: String::new(),
        token: String::new(),
    };
    let settings = Settings {
        reduced_motion: true,
        theme: atlas_core::ThemeMode::Light,
        update_check_on_start: true,
        update_auto_download: true,
        autostart: true,
        notifications_enabled: false,
        ..Settings::default()
    };
    let mut app = AtlasApp::new(
        AppPaths::discover(),
        JobRunner::new(),
        DeviceIdentity::fresh(),
        settings,
        ServerStore {
            selected: Some("https://stock.exemple.invalid".into()),
            saved: vec![atlas_core::SavedServerEntry {
                label: "Atlas Commerce — exemple".into(),
                url: "https://stock.exemple.invalid".into(),
            }],
            onboarding_done: true,
            locked: true,
            requires_verification: false,
        },
        Some(session),
        AccountStore::default(),
        fastframe_shell::Waker::default(),
        fastframe_power::Power::spawn(|| {}),
        Arc::new(Mutex::new(Vec::new())),
        None,
    );
    app.screen = Screen::Shell(match route {
        "clients" | "client-editor" | "client-invoices" | "client-receivables" => Route::Clients,
        "products" | "stock" => Route::Products,
        "sales" | "quotes" => Route::Sales,
        "expenses" => Route::Expenses,
        "vendors" | "vendor-payables" => Route::Vendors,
        "employees" => Route::Employees,
        "settings" => Route::Settings,
        "delivery" => Route::DeliveryNotes,
        _ => Route::Dashboard,
    });
    if route == "login" || route == "login-qr" {
        // Error/retry state only: never start a network login in the fixture.
        app.screen = Screen::Login;
        app.session = None;
        app.login.browser_auto_started = true;
        app.login.error = Some("Connexion fictive indisponible.".into());
    }
    if route == "login-qr" {
        // Inert deep-link state: no token and no server request.
        app.login.error = None;
        app.qr.active = true;
        app.qr.status = "claimed".into();
    }
    if route == "settings" {
        // Fictional update state: proves the title-bar icon and the
        // "restart to install" banner without contacting GitHub.
        let manifest = atlas_core::update_feed::UpdateManifest {
            version: "0.2.0".into(),
            mandatory: false,
            minimum_version: None,
            notes: "Correctifs fictifs pour la capture.".into(),
            published_at: "2026-01-15T10:00:00Z".into(),
        };
        app.update.last_manifest = Some(manifest.clone());
        app.update.release_notes = Some(manifest);
        app.update.ready_version = Some("0.2.0".into());
    }
    app.dashboard.data = Some(Dashboard {
        currency: Some("MAD".into()),
        current_balance: 42000.0,
        stock_value: 17500.0,
        product_count: 2,
        ..Dashboard::default()
    });
    app.products.data = Some(vec![
        Product {
            id: 1,
            name: "Marbre blanc fictif".into(),
            sku: Some("DEMO-001".into()),
            price: 350.0,
            stock: 24.0,
            track_inventory: 1,
            is_active: 1,
            ..Product::default()
        },
        Product {
            id: 2,
            name: "Carrelage fictif".into(),
            sku: Some("DEMO-002".into()),
            price: 120.0,
            stock: 4.0,
            track_inventory: 1,
            is_active: 1,
            ..Product::default()
        },
    ]);
    app.clients.data = Some(vec![
        Client {
            id: 1,
            name: "Client Exemple".into(),
            city: "Casablanca".into(),
            phone: "0600000000".into(),
            balance_due: 1250.0,
            ..Client::default()
        },
        Client {
            id: 2,
            name: "Société Démo".into(),
            city: "Rabat".into(),
            balance_due: 0.0,
            ..Client::default()
        },
    ]);
    app.selected_client = Some(1);
    app.invoices.data = Some(vec![
        Invoice {
            id: 1,
            number: Some("FA-DEMO-001".into()),
            client_name: Some("Client Exemple".into()),
            issue_date: Some("2026-01-15".into()),
            total: 1500.0,
            amount_paid: Some(250.0),
            status: Some("partial".into()),
            ..Invoice::default()
        },
        Invoice {
            id: 3,
            number: Some("DV-DEMO-001".into()),
            client_name: Some("Client Exemple".into()),
            issue_date: Some("2026-01-21".into()),
            total: 2500.0,
            status: Some("quote".into()),
            ..Invoice::default()
        },
        Invoice {
            id: 2,
            number: Some("WR-DEMO-001".into()),
            client_name: Some("Client Exemple".into()),
            issue_date: Some("2026-01-20".into()),
            total: 300.0,
            amount_paid: Some(300.0),
            status: Some("remboursement".into()),
            doc_kind: Some("remboursement".into()),
            ..Invoice::default()
        },
    ]);
    app.vendors.data = Some(vec![Vendor {
        id: 1,
        name: "Fournisseur Exemple".into(),
        city: "Rabat".into(),
        balance_due: 500.0,
        ..Vendor::default()
    }]);
    app.employees.data = Some(
        serde_json::from_value(serde_json::json!({
            "rows": [
                {
                    "id": 1,
                    "emp_number": "EMP-001",
                    "display_name": "Amina Fictive",
                    "name": "Amina Fictive",
                    "phone": "0600000000",
                    "role_title": "Vendeuse",
                    "department": "Magasin",
                    "category": "Magasin",
                    "employment_status": "CDI",
                    "joined_date": "2025-03-01",
                    "national_id": "FI000001",
                    "pay_amount": 4200.0,
                    "pay_cycle_type": "monthly",
                    "cycle_days": 30,
                    "next_due": "2026-11-01",
                    "pay_overdue": false,
                    "active": 1,
                    "suggested_pay": 4200.0,
                    "gross_accrued": 4200.0,
                    "accrued_days": 30,
                    "period_from": "2026-10-01"
                },
                {
                    "id": 2,
                    "emp_number": "EMP-002",
                    "display_name": "Youssef Fictif",
                    "name": "Youssef Fictif",
                    "phone": "0611111111",
                    "role_title": "Magasinier",
                    "department": "Dépôt",
                    "category": "Chantier",
                    "national_id": "FI000002",
                    "pay_amount": 3600.0,
                    "pay_cycle_type": "days",
                    "cycle_days": 7,
                    "next_due": "2026-10-20",
                    "pay_overdue": true,
                    "active": 1,
                    "suggested_pay": 840.0,
                    "gross_accrued": 840.0,
                    "accrued_days": 7,
                    "period_from": "2026-10-13"
                },
                {
                    "id": 3,
                    "emp_number": "EMP-003",
                    "display_name": "Salma Fictive",
                    "name": "Salma Fictive",
                    "role_title": "Assistante",
                    "pay_amount": 3800.0,
                    "pay_cycle_type": "monthly",
                    "cycle_days": 30,
                    "active": 0,
                    "is_resigned": true
                }
            ],
            "categories": ["Bureau", "Chantier", "Magasin"]
        }))
        .expect("fixture employees"),
    );
    if route == "sales" {
        app.refunds_open = true;
    }
    if route == "quotes" {
        app.quotes_open = true;
    }
    #[cfg(feature = "visual-fixtures")]
    if route == "client-receivables" {
        app.show_fictional_receivables();
    }
    #[cfg(feature = "visual-fixtures")]
    if route == "vendor-payables" {
        app.show_fictional_payables();
    }
    if route == "client-invoices" {
        app.selected_client = Some(1);
        app.client_invoice_id = Some(1);
        app.client_invoices.data = app.invoices.data.clone();
    }
    if route == "employees" {
        app.employee_detail_id = Some(1);
        app.employee_detail.data = Some(
            serde_json::from_value(serde_json::json!({
                "id": 1,
                "emp_number": "EMP-001",
                "display_name": "Amina Fictive",
                "name": "Amina Fictive",
                "phone": "0600000000",
                "email": "amina@example.invalid",
                "role_title": "Vendeuse",
                "department": "Magasin",
                "national_id": "FI000001",
                "joined_date": "2025-03-01",
                "pay_amount": 4200.0,
                "pay_cycle_type": "monthly",
                "cycle_days": 30,
                "next_due": "2026-11-01",
                "active": 1,
                "suggested_pay": 4200.0,
                "gross_accrued": 4200.0,
                "accrued_days": 30,
                "period_from": "2026-10-01",
                "payments": [
                    {"id": 1, "amount": 4200.0, "paid_at": "2026-10-01", "period_label": "Période 2026-09-01 → 2026-10-01", "note": "Fictif"}
                ]
            }))
            .expect("fixture employee detail"),
        );
    }
    app.delivery_notes.data = Some(vec![DeliveryNote {
        id: 1,
        number: "BL-DEMO-001".into(),
        client_name: "Client Exemple".into(),
        issue_date: "2026-01-15".into(),
        status: "confirmed".into(),
        ..DeliveryNote::default()
    }]);
    app.expenses.data = Some(vec![serde_json::from_str::<Expense>(r#"{"id":1,"description":"Transport fictif","amount":125,"expense_date":"2026-01-15","entry_type":"expense"}"#).expect("fixture expense")]);
    if route == "stock" {
        app.stock_dialog = Some(atlas_ui::screens::products::StockDialog::new(
            &app.products.data.as_ref().expect("fixture products")[0],
        ));
    }
    if route == "client-editor" {
        app.client_editor = Some(atlas_ui::screens::clients::ClientEditor::for_client(
            &app.clients.data.as_ref().expect("fixture clients")[0],
        ));
    }
    if route == "agent" {
        app.agent_drawer_open = true;
    }
    if route == "palette-narrow" {
        app.palette_open = true;
    }
    app
}
fn main() -> eframe::Result<()> {
    let route = std::env::args().nth(1).unwrap_or_else(|| "clients".into());
    let size = if route == "palette-narrow" {
        [640.0, 480.0]
    } else {
        [1280.0, 800.0]
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size(size),
        ..Default::default()
    };
    eframe::run_native(
        "Atlas fixture — données fictives",
        options,
        Box::new(move |cc| {
            theme::install_fonts_and_icons(&cc.egui_ctx);
            cc.egui_ctx.set_theme(egui::ThemePreference::Light);
            let palette = Palette::light();
            theme::apply(&cc.egui_ctx, &palette, true);
            Ok(Box::new(Fixture {
                app: build(&route),
                palette,
            }))
        }),
    )
}
