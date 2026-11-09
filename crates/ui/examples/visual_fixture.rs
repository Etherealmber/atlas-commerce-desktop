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
        update_check_on_start: false,
        notifications_enabled: false,
        ..Settings::default()
    };
    let mut app = AtlasApp::new(
        AppPaths::discover(),
        JobRunner::new(),
        DeviceIdentity::fresh(),
        settings,
        ServerStore::default(),
        Some(session),
        AccountStore::default(),
        fastframe_shell::Waker::default(),
        fastframe_power::Power::spawn(|| {}),
        Arc::new(Mutex::new(Vec::new())),
        None,
    );
    app.screen = Screen::Shell(match route {
        "clients" | "client-editor" => Route::Clients,
        "products" | "stock" => Route::Products,
        "sales" => Route::Sales,
        "expenses" => Route::Expenses,
        "vendors" => Route::Vendors,
        "delivery" => Route::DeliveryNotes,
        _ => Route::Dashboard,
    });
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
            ..Product::default()
        },
        Product {
            id: 2,
            name: "Carrelage fictif".into(),
            sku: Some("DEMO-002".into()),
            price: 120.0,
            stock: 4.0,
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
    app.invoices.data = Some(vec![Invoice {
        id: 1,
        number: Some("FA-DEMO-001".into()),
        client_name: Some("Client Exemple".into()),
        issue_date: Some("2026-01-15".into()),
        total: 1500.0,
        amount_paid: Some(250.0),
        status: Some("partial".into()),
        ..Invoice::default()
    }]);
    app.vendors.data = Some(vec![Vendor {
        id: 1,
        name: "Fournisseur Exemple".into(),
        city: "Rabat".into(),
        balance_due: 500.0,
        ..Vendor::default()
    }]);
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
    app
}
fn main() -> eframe::Result<()> {
    let route = std::env::args().nth(1).unwrap_or_else(|| "clients".into());
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1280.0, 800.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Atlas fixture — données fictives",
        options,
        Box::new(move |cc| {
            theme::install_fonts_and_icons(&cc.egui_ctx);
            let palette = Palette::light();
            theme::apply(&cc.egui_ctx, &palette, true);
            Ok(Box::new(Fixture {
                app: build(&route),
                palette,
            }))
        }),
    )
}
