//! Small generic dialogs shared by screens.

/// A right-aligned confirmation dialog (title, body, confirm/dismiss).
#[derive(Clone)]
pub struct Confirm {
    pub title: String,
    pub body: String,
    pub confirm_label: String,
    pub danger: bool,
    /// What to run when confirmed. The dialog is closed in either case.
    pub action: ConfirmAction,
}

/// Actions the confirm dialog can trigger, represented as data so the UI
/// thread stays free of closures capturing borrows.
#[derive(Clone, Debug)]
pub enum ConfirmAction {
    EnableAttendance,
    CompleteMobileAction(i64),
    RejectMobileAction {
        id: i64,
        reason: String,
    },
    AcknowledgeInventoryUncertainty,
    AcknowledgeCashShiftUncertainty,
    AcknowledgeLedgerUncertainty,
    OpenCashShift {
        till: String,
        opening: f64,
        notes: String,
    },
    CashShiftMovement {
        id: i64,
        direction: &'static str,
        method: &'static str,
        amount: f64,
        label: String,
        reference: String,
    },
    CloseCashShift {
        id: i64,
        amount: f64,
        notes: String,
    },
    CountInventory {
        product_id: i64,
        quantity: f64,
        note: String,
    },
    RevokeDevice(String),
    ForgetServer(String),
    SignOut,
    DiscardEditor,
    RevokeApiKey(i64),
}
