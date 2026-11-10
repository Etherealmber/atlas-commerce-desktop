//! Offline OS-vault fixture. Only run under an isolated test keyring.
use atlas_core::{
    SessionRecord, vault,
    vault_authority::{RestoreMode, restore_mode},
    vault_worker::VaultWorker,
};

fn main() {
    let mode = std::env::args()
        .nth(1)
        .expect("expected save, load or remove");
    let server = "https://vault-ci.invalid:8443";
    let session = SessionRecord {
        server_url: server.into(),
        username: "fixture-account".into(),
        role: "owner".into(),
        display_name: "Compte fictif".into(),
        email: String::new(),
        avatar_url: String::new(),
        token: "fictional-token-for-isolated-ci".into(),
    };
    match mode.as_str() {
        "save" => vault::save(&session).expect("vault save failed"),
        "load" => assert_eq!(
            vault::load(server).expect("vault load failed"),
            Some(session)
        ),
        "remove" => {
            vault::remove(&session).expect("vault deletion failed");
            assert!(vault::load(server).expect("vault load failed").is_none());
        }
        "worker-save" => {
            let path = std::path::PathBuf::from(std::env::var_os("ATLAS_VAULT_AUTHORITY").unwrap());
            let worker = VaultWorker::new(path.clone());
            assert!(worker.save(session).unwrap().recv().unwrap().unwrap());
            assert_eq!(restore_mode(&path, server), RestoreMode::Vault);
        }
        "worker-load" => {
            let path = std::path::PathBuf::from(std::env::var_os("ATLAS_VAULT_AUTHORITY").unwrap());
            assert_eq!(restore_mode(&path, server), RestoreMode::Vault);
            let worker = VaultWorker::new(path);
            assert_eq!(
                worker.load(server.into()).unwrap().recv().unwrap().unwrap(),
                Some(session)
            );
        }
        "worker-remove" => {
            let path = std::path::PathBuf::from(std::env::var_os("ATLAS_VAULT_AUTHORITY").unwrap());
            let worker = VaultWorker::new(path.clone());
            worker.remove(session).unwrap().recv().unwrap().unwrap();
            assert_eq!(restore_mode(&path, server), RestoreMode::Blocked);
            assert!(vault::load(server).unwrap().is_none());
        }
        _ => panic!("unknown fixture operation"),
    }
}
