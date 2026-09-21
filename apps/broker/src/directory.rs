//! Directory boundary (FreeIPA LDAP in production, fixtures in tests).

use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;

use crate::hbac::{DirectoryView, UserView};

#[derive(Debug)]
pub enum DirError {
    /// The bind worked but the broker principal may not read something.
    InsufficientAccess(String),
    Unavailable(String),
}

impl std::fmt::Display for DirError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DirError::InsufficientAccess(s) => write!(f, "insufficient access: {s}"),
            DirError::Unavailable(s) => write!(f, "directory unavailable: {s}"),
        }
    }
}

#[async_trait]
pub trait Directory: Send + Sync {
    /// Rules, remote host group members (with their DNS A records) and group
    /// nesting. Cached by the caller.
    async fn view(&self) -> Result<DirectoryView, DirError>;
    /// `None` when the user does not exist.
    async fn user(&self, uid: &str) -> Result<Option<UserView>, DirError>;
}

pub struct FakeDirectory {
    pub view: Mutex<DirectoryView>,
    pub users: Mutex<HashMap<String, UserView>>,
    pub calls: Mutex<u32>,
}

impl FakeDirectory {
    pub fn new(view: DirectoryView, users: Vec<(&str, UserView)>) -> FakeDirectory {
        FakeDirectory {
            view: Mutex::new(view),
            users: Mutex::new(users.into_iter().map(|(k, v)| (k.to_string(), v)).collect()),
            calls: Mutex::new(0),
        }
    }
}

#[async_trait]
impl Directory for FakeDirectory {
    async fn view(&self) -> Result<DirectoryView, DirError> {
        *self.calls.lock().unwrap() += 1;
        Ok(self.view.lock().unwrap().clone())
    }
    async fn user(&self, uid: &str) -> Result<Option<UserView>, DirError> {
        Ok(self.users.lock().unwrap().get(uid).cloned())
    }
}
