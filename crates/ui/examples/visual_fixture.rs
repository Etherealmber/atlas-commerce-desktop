//! Isolated, offline visual fixture. Run only by the screenshot workflow.
//! It never creates an API client or loads a real account/server from disk.
use atlas_api::models::{
    Client, Dashboard, DeliveryNote, Expense, Invoice, MonthlyPoint, Product, Vendor,
};
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
    route: String,
    shortcut_reported: bool,
    wizard_step: Option<usize>,
}
impl eframe::App for Fixture {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        self.palette.window.to_normalized_gamma_f32()
    }
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if self.route == "alerts-popover" {
            let id = egui::Id::new("notification-popover-open");
            ctx.data_mut(|data| {
                if data.get_temp::<bool>(id).is_none() {
                    data.insert_temp(id, true);
                }
            });
        }
        if self.route == "stock-shortcut" && ctx.input(|input| input.key_pressed(egui::Key::K)) {
            println!(
                "FIXTURE_KEY mods={:?} events={:?} focus={:?}",
                ctx.input(|input| input.modifiers),
                ctx.input(|input| input.events.clone()),
                ctx.memory(|memory| memory.focused())
            );
        }
        self.app.draw(ui, &ctx, &self.palette);
        if matches!(
            self.route.as_str(),
            "product-wizard" | "client-wizard" | "employee-wizard"
        ) {
            let step = if self.route == "product-wizard" {
                self.app.product_editor.as_ref().map(|editor| editor.step)
            } else if self.route == "client-wizard" {
                self.app.client_editor.as_ref().map(|editor| editor.step)
            } else {
                self.app.employee_new.as_ref().map(|editor| editor.step)
            };
            if step != self.wizard_step {
                self.wizard_step = step;
                // Fixed fixture markers only: never log application data.
                match step {
                    Some(0) => println!("FIXTURE_WIZARD_STEP 0"),
                    Some(1) => println!("FIXTURE_WIZARD_STEP 1"),
                    Some(2) => println!("FIXTURE_WIZARD_STEP 2"),
                    Some(3) => println!("FIXTURE_WIZARD_STEP 3"),
                    Some(4) => println!("FIXTURE_WIZARD_STEP 4"),
                    _ => {}
                }
            }
        }
        if self.route == "stock-shortcut"
            && !self.shortcut_reported
            && matches!(self.app.screen, Screen::Shell(Route::Products))
            && self.app.product_search == "SKU42"
        {
            println!("FIXTURE_STOCK_SHORTCUT_OK");
            self.shortcut_reported = true;
        }
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
        "clients" | "clients-narrow" | "client-editor" | "client-wizard" | "client-invoices"
        | "client-receivables" => Route::Clients,
        "products" | "products-wide" | "products-narrow" | "product-wizard" | "stock"
        | "stock-report" => Route::Products,
        "sales" | "quotes" => Route::Sales,
        "expenses" | "expenses-wide" | "finance-report" => Route::Expenses,
        "vendors" | "vendor-payables" => Route::Vendors,
        "employees" | "employee-editor" | "employee-editor-narrow" | "employee-wizard" => {
            Route::Employees
        }
        "attendance" | "attendance-disabled" => Route::Attendance,
        "inventory-ops" => Route::InventoryOps,
        "cash-shifts" => Route::CashShifts,
        "payroll" => Route::Payroll,
        "mobile-actions" => Route::Actions,
        "settings" | "settings-narrow" | "settings-dark" => Route::Dashboard,
        "delivery" => Route::DeliveryNotes,
        _ => Route::Dashboard,
    });
    if route == "login" || route == "login-qr" || route == "login-wait" || route == "login-v2" {
        // Error/retry state only: never start a network login in the fixture.
        app.screen = Screen::Login;
        app.session = None;
        app.login.browser_auto_started = true;
        app.login.error = Some("Connexion fictive indisponible.".into());
    }
    #[cfg(feature = "visual-fixtures")]
    if route == "login-wait" {
        app.login.error = None;
        app.show_fictional_browser_wait();
    }
    #[cfg(feature = "visual-fixtures")]
    if route == "login-v2" {
        app.login.error = None;
        app.show_fictional_native_v2();
    }
    if route == "login-qr" {
        // Inert deep-link state: no token and no server request.
        app.login.error = None;
        app.qr.active = true;
        app.qr.status = "claimed".into();
        app.qr.qr_url = "https://fixture.invalid/login/qr?t=fictional&mode=pc".into();
    }
    if matches!(route, "settings" | "settings-narrow" | "settings-dark") {
        app.settings_dialog_open = true;
        app.settings.observe_version("0.2.0", 1_700_000_000);
        app.settings
            .observe_version(env!("CARGO_PKG_VERSION"), 1_710_000_000);
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
    if matches!(route, "update-dialog" | "update-dialog-dark") {
        app.update.last_manifest = Some(atlas_core::update_feed::UpdateManifest {
            version: "9.1.0".into(),
            mandatory: false,
            minimum_version: None,
            notes: "## Nouveautés\n- Exemple fictif hors ligne.".into(),
            published_at: "2026-01-15T10:00:00Z".into(),
        });
        app.update.dialog_open = true;
    }
    app.nav_badges.data = Some(atlas_api::models::NavBadges {
        invoices_unpaid: 3,
        bills_unpaid: 2,
        bills_receive: 4,
        products_low: 1,
        actions_pending: 2,
        quotes_open: 1,
        vr_open: 1,
        employees_pay_due: 1,
    });
    app.alerts.data = Some(vec![
        atlas_api::models::Alert {
            kind: "stock_bas".into(),
            label: "Produit fictif : stock bas".into(),
            severity: Some("medium".into()),
            href: Some("/products".into()),
            count: None,
        },
        atlas_api::models::Alert {
            kind: "facture_echue".into(),
            label: "Facture fictive échue".into(),
            severity: Some("high".into()),
            href: Some("/invoices/1".into()),
            count: None,
        },
    ]);
    app.attendance.data = Some(atlas_api::attendance::AttendanceDay {
        date: "2026-10-22".into(),
        summary: atlas_api::attendance::AttendanceSummary {
            present: 1,
            absent: 1,
            late: 0,
            excused: 0,
            total: 2,
        },
        rows: vec![
            atlas_api::attendance::AttendanceRow {
                employee_id: 1,
                name: "Employé fictif".into(),
                role_title: "Magasin".into(),
                check_in: Some("08:45".into()),
                check_out: None,
                status: "present".into(),
                note: String::new(),
            },
            atlas_api::attendance::AttendanceRow {
                employee_id: 2,
                name: "Collaboratrice fictive".into(),
                role_title: "Ventes".into(),
                check_in: None,
                check_out: None,
                status: "absent".into(),
                note: String::new(),
            },
        ],
    });
    app.dashboard.data = Some(Dashboard {
        currency: Some("MAD".into()),
        current_balance: Some(42000.0),
        sales_received: Some(53000.0),
        other_income_total: Some(1000.0),
        owner_contrib_total: Some(2000.0),
        purchases_paid: Some(6000.0),
        expenses_total: Some(5000.0),
        owner_draw_total: Some(3000.0),
        stock_value: 17500.0,
        product_count: 2,
        expense_categories: Some(vec![
            atlas_api::models::DashboardExpenseCategory {
                category: "Transport".into(),
                total: 2450.0,
            },
            atlas_api::models::DashboardExpenseCategory {
                category: "Services".into(),
                total: 1200.0,
            },
        ]),
        monthly_series: (1..=12)
            .map(|month| MonthlyPoint {
                key: format!("2026-{month:02}"),
                label: format!("{month:02}/26"),
                sales: f64::from(month) * 900.0,
                expenses: f64::from(month) * 350.0,
                profit: f64::from(month) * 550.0,
                invoices_fa: Some(f64::from(month) * 600.0),
                sales_vr: Some(f64::from(month) * 300.0),
                bills: Some(f64::from(month) * 200.0),
                salaries: Some(f64::from(month) * 150.0),
                receipts: Some(f64::from(month) * 700.0),
                clients_new: Some(f64::from(month % 4)),
            })
            .collect(),
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
    app.cash_shifts.data = Some(vec![atlas_api::cash_shifts::CashShift {
        id: 1,
        till_id: "CAISSE-1".into(),
        operator: "demo".into(),
        opened_at: "2026-10-22T08:00:00Z".into(),
        closed_at: None,
        opening_float: 1000.0,
        counted_amount: None,
        expected_amount: None,
        variance: None,
        status: "open".into(),
        notes: String::new(),
    }]);
    app.inventory_ops.data = Some(atlas_api::inventory_ops::InventoryOverview {
        locations: vec![atlas_api::inventory_ops::StockLocation {
            id: 1,
            code: "MAG".into(),
            name: "Magasin fictif".into(),
            is_default: 1,
            total_units: Some(28.0),
            sku_count: Some(2),
        }],
        products: app.products.data.clone().unwrap_or_default(),
    });
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
    #[cfg(feature = "visual-fixtures")]
    if route == "stock-report" {
        app.show_fictional_stock_report();
    }
    #[cfg(feature = "visual-fixtures")]
    if route == "finance-report" {
        app.show_fictional_finance_report();
    }
    if route == "attendance-disabled" {
        app.attendance.data = None;
        let error = atlas_api::ApiError {
            status: 403,
            code: Some("ATTENDANCE_DISABLED".into()),
            message: "Module présence désactivé (fiction).".into(),
        };
        app.attendance.error = Some(error.message.clone());
        app.attendance.last_error = Some(error);
    }
    if route == "client-invoices" {
        app.selected_client = Some(1);
        app.client_invoice_id = Some(1);
        app.client_invoices.data = app.invoices.data.clone();
    }
    if matches!(route, "employee-editor" | "employee-editor-narrow") {
        app.employee_new = Some(atlas_ui::screens::employees::NewEmployeeForm::default());
    }
    if route == "employee-wizard" {
        let form = atlas_ui::screens::employees::NewEmployeeForm {
            first_name: "Amina".into(),
            ..Default::default()
        };
        app.employee_new = Some(form);
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
    app.payroll_runs.data = Some(vec![serde_json::from_value(serde_json::json!({"id":1,"period_month":"2026-01","status":"draft","run_kind":"regular","corrects_run_id":null,"total_gross":12000.0,"total_cnss_emp":537.6,"total_amo_emp":271.2,"total_igr":600.0,"total_net":10591.2,"total_employer_cost":14000.0})).expect("fixture payroll")]);
    app.mobile_actions.data = Some(vec![serde_json::from_str::<atlas_api::mobile_actions::MobileAction>(r#"{"id":12,"type":"stock_check","title":"Contrôle fictif","status":"pending","detail":"Vérifier le stock","created_at":"2026-01-15"}"#).expect("fixture mobile action")]);
    app.expenses.data = Some(vec![serde_json::from_str::<Expense>(r#"{"id":1,"description":"Transport fictif","amount":125,"expense_date":"2026-01-15","entry_type":"expense"}"#).expect("fixture expense")]);
    if route == "stock" {
        app.stock_dialog = Some(atlas_ui::screens::products::StockDialog::new(
            &app.products.data.as_ref().expect("fixture products")[0],
        ));
    }
    if route == "product-wizard" {
        let mut editor = atlas_ui::screens::products::ProductEditor::new();
        editor.name = "Produit fictif".into();
        app.product_editor = Some(editor);
    }
    if matches!(route, "client-editor" | "client-wizard") {
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
    let size = if matches!(
        route.as_str(),
        "dashboard-wide" | "products-wide" | "expenses-wide"
    ) {
        [1760.0, 850.0]
    } else if route == "palette-narrow" {
        [640.0, 480.0]
    } else if matches!(
        route.as_str(),
        "products-narrow"
            | "settings-narrow"
            | "clients-narrow"
            | "employee-editor-narrow"
            | "product-wizard"
            | "client-wizard"
            | "employee-wizard"
    ) {
        [720.0, 520.0]
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
            let dark = matches!(route.as_str(), "settings-dark" | "update-dialog-dark");
            cc.egui_ctx.set_theme(if dark {
                egui::ThemePreference::Dark
            } else {
                egui::ThemePreference::Light
            });
            let palette = if dark {
                Palette::dark()
            } else {
                Palette::light()
            };
            theme::apply(&cc.egui_ctx, &palette, true);
            Ok(Box::new(Fixture {
                app: build(&route),
                palette,
                route: route.clone(),
                shortcut_reported: false,
                wizard_step: None,
            }))
        }),
    )
}
