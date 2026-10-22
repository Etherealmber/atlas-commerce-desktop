//! `atlas-core`: platform services shared by every other crate.
//!
//! Folders and portable installs, the stable device fingerprint, settings
//! and server/account persistence, session isolation, deep-link parsing,
//! the background job runner and the update-feed decision logic. No egui and
//! no network client live here, so every module is unit-testable headless.

pub mod deep_link;
pub mod device;
pub mod dirs;
pub mod format;
pub mod jobs;
pub mod servers;
pub mod session;
pub mod settings;
pub mod update_feed;

pub use deep_link::{QrTokenParser, VerifyLink};
pub use device::DeviceIdentity;
pub use dirs::{APP_ID, APP_NAME, AppPaths, SLUG};
pub use jobs::{Job, JobRunner};
pub use servers::{PRODUCTION_URL, SavedServerEntry, ServerError, ServerStore, ServerUrl};
pub use session::{AccountStore, SavedAccount, SessionRecord};
pub use settings::{Settings, ThemeMode, UpdateChannel};
pub use update_feed::{MANIFEST_ASSET, UpdateDecision, UpdateManifest};
