//! FerricDB network server — **you build this in Milestone 10**.
//!
//! A tokio TCP server speaking the PostgreSQL frontend/backend protocol v3:
//! the [`protocol`] module owns the byte-exact message codec; [`Server`]
//! owns the accept loop and the per-connection session state machine
//! (startup → auth → query cycles → termination), routing SQL through
//! [`ferricdb_exec::execute`].
//!
//! When this milestone is done, `psql -h 127.0.0.1 -p 5433` connects to
//! your database and does not know the difference.
//!
//! Milestone page: `course/project/milestone-10/index.html`.
//! Concept module: `course/modules/m11-wire-protocol-server/index.html`.

#![allow(unused_variables, dead_code)] // stubs: remove once you implement

pub mod protocol;

use std::sync::Arc;

use ferricdb_common::Result;
use ferricdb_exec::ExecContext;

/// Server configuration.
pub struct ServerConfig {
    /// Listen address. FerricDB's course default is `127.0.0.1:5433` —
    /// one above real Postgres, so both can run side by side.
    pub addr: String,
    /// `None` = trust authentication (accept anyone). `Some(pw)` = demand
    /// a cleartext password equal to `pw`.
    pub password: Option<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig {
            addr: "127.0.0.1:5433".into(),
            password: None,
        }
    }
}

/// The FerricDB server: a tokio accept loop spawning one session task per
/// connection. Every session gets its own transaction slot (a fresh view
/// into the shared [`ExecContext`]); a dropped connection rolls back its
/// open transaction.
pub struct Server {
    config: ServerConfig,
    ctx: Arc<ExecContext>,
}

impl Server {
    pub fn new(config: ServerConfig, ctx: Arc<ExecContext>) -> Self {
        Server { config, ctx }
    }

    /// Bind, accept, and serve until the process ends.
    ///
    /// Session outline (see the milestone page's state machine):
    /// startup message (decline SSLRequest with `'N'`), auth dance,
    /// ParameterStatus + BackendKeyData + ReadyForQuery, then the query
    /// loop — with ReadyForQuery after *every* cycle, error cycles
    /// included.
    pub async fn run(self) -> Result<()> {
        todo!("Milestone 10")
    }
}
