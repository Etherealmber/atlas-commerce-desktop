//! Ordered system-vault operations, away from the interface thread.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use crate::{SessionRecord, vault, vault_authority::Authority};

/// A failed vault operation. No backend details or sensitive values are exposed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    /// The local authority could not be written.
    Authority,
    /// The operating-system vault is unavailable or rejected the value.
    Vault,
    /// The worker was unavailable.
    Worker,
}

enum Command {
    Save(SessionRecord, u64, Sender<Result<bool, Failure>>),
    Load(String, Sender<Result<Option<SessionRecord>, Failure>>),
    Remove(SessionRecord, Sender<Result<(), Failure>>),
}

/// One FIFO worker for the lifetime of the app. All keychain access happens
/// on this thread, so a later sign-out deletion follows an earlier save.
pub struct VaultWorker {
    sender: Sender<Command>,
    authority: Authority,
}

impl VaultWorker {
    /// Starts the worker with an authority file inside the application's state.
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        let authority = Authority::new(path);
        let worker_authority = authority.clone();
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            while let Ok(command) = receiver.recv() {
                match command {
                    Command::Save(session, generation, reply) => {
                        let result =
                            vault::save(&session)
                                .map_err(|_| Failure::Vault)
                                .and_then(|()| {
                                    worker_authority
                                        .activate(&session.server_url, generation)
                                        .map_err(|_| Failure::Authority)
                                });
                        let _ = reply.send(result);
                    }
                    Command::Load(server, reply) => {
                        let _ = reply.send(vault::load(&server).map_err(|_| Failure::Vault));
                    }
                    Command::Remove(session, reply) => {
                        let _ = reply.send(vault::remove(&session).map_err(|_| Failure::Vault));
                    }
                }
            }
        });
        Self { sender, authority }
    }

    /// Blocks restoration before queueing a save; a late save cannot override
    /// a subsequent sign-out. OS vault operations never run on the caller.
    ///
    /// # Errors
    /// The authority or worker is unavailable.
    pub fn save(&self, session: SessionRecord) -> Result<Receiver<Result<bool, Failure>>, Failure> {
        let generation = self
            .authority
            .block(&session.server_url)
            .map_err(|_| Failure::Authority)?;
        let (sender, receiver) = mpsc::channel();
        self.sender
            .send(Command::Save(session, generation, sender))
            .map_err(|_| Failure::Worker)?;
        Ok(receiver)
    }

    /// Queues a vault read, returning a pollable receiver.
    ///
    /// # Errors
    /// The worker is unavailable.
    pub fn load(
        &self,
        server: String,
    ) -> Result<Receiver<Result<Option<SessionRecord>, Failure>>, Failure> {
        let (sender, receiver) = mpsc::channel();
        self.sender
            .send(Command::Load(server, sender))
            .map_err(|_| Failure::Worker)?;
        Ok(receiver)
    }

    /// Blocks restoration before queueing removal, even if keychain deletion
    /// later fails. It does not claim to revoke the session on the server.
    ///
    /// # Errors
    /// The authority or worker is unavailable.
    pub fn remove(&self, session: SessionRecord) -> Result<Receiver<Result<(), Failure>>, Failure> {
        self.authority
            .block(&session.server_url)
            .map_err(|_| Failure::Authority)?;
        let (sender, receiver) = mpsc::channel();
        self.sender
            .send(Command::Remove(session, sender))
            .map_err(|_| Failure::Worker)?;
        Ok(receiver)
    }

    /// Prevents restoration without requiring a vault operation.
    ///
    /// # Errors
    /// The local authority cannot be written.
    pub fn block(&self, server: &str) -> Result<(), Failure> {
        self.authority
            .block(server)
            .map(|_| ())
            .map_err(|_| Failure::Authority)
    }

    /// Marks a subsequent legacy login authoritative and invalidates saves.
    ///
    /// # Errors
    /// The authority cannot be written.
    pub fn legacy(&self, server: &str) -> Result<(), Failure> {
        self.authority
            .legacy(server)
            .map_err(|_| Failure::Authority)
    }
}
