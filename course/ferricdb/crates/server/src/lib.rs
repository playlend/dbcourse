//! FerricDB network server — **you build this in Milestone 10**.
//!
//! A tokio TCP server speaking the PostgreSQL frontend/backend protocol v3:
//! the [`protocol`] module owns the byte-exact message codec; [`Server`]
//! owns the accept loop and the per-connection session state machine
//! (SSL negotiation → startup → auth → query cycles → termination), routing
//! SQL through [`ferricdb_exec::execute`] on a dedicated
//! [`ExecContext::new_session`] per client.
//!
//! When this milestone is done, `psql -h 127.0.0.1 -p 5433` connects to
//! your database out of the box (including declining `SSLRequest` cleanly
//! with `'N'`) and does not know the difference.
//!
//! Milestone page: `course/project/milestone-10/index.html`.
//! Concept module: `course/modules/m11-wire-protocol-server/index.html`.

#![allow(unused_variables, dead_code)] // stubs: remove once you implement

pub mod protocol;

use std::sync::Arc;

use ferricdb_common::Result;
use ferricdb_exec::ExecContext;
use tokio::net::TcpListener;

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
/// connection. Every session gets its own isolated transaction slot via
/// [`ExecContext::new_session`]; a dropped connection rolls back its open
/// transaction.
pub struct Server {
    config: ServerConfig,
    ctx: Arc<ExecContext>,
}

impl Server {
    pub fn new(config: ServerConfig, ctx: Arc<ExecContext>) -> Self {
        Server { config, ctx }
    }

    /// Bind `config.addr` and serve until the process ends (delegates to
    /// [`Server::serve`]).
    pub async fn run(self) -> Result<()> {
        let listener = TcpListener::bind(&self.config.addr).await?;
        self.serve(listener).await
    }

    /// Accept connections from an already-bound `listener` (handy for
    /// integration tests binding `127.0.0.1:0`).
    ///
    /// Per-connection session outline (see the milestone page's state
    /// machine):
    /// 1. Construct an isolated session with `self.ctx.new_session()`.
    /// 2. Read startup packet (`startup = true`): if `SslRequest`, write a
    ///    single `b'N'` byte and loop to read `StartupMessage` (or upgrade
    ///    to TLS with `b'S'` if you implement the optional TLS extension).
    /// 3. Auth dance (`AuthenticationOk` for trust, or
    ///    `AuthenticationCleartextPassword` → `PasswordMessage` →
    ///    `AuthenticationOk` / `ErrorResponse`).
    /// 4. Send `ParameterStatus` (`server_version`, `client_encoding`),
    ///    `BackendKeyData`, and `ReadyForQuery { txn_status: b'I' }`.
    /// 5. Query loop (`startup = false`): execute SQL via
    ///    `tokio::task::spawn_blocking` (so synchronous disk/mutex work
    ///    never blocks tokio workers), emit `RowDescription` + `DataRow`* +
    ///    `CommandComplete { tag: result.tag }` (or `ErrorResponse`), then
    ///    `ReadyForQuery` (`b'I'` / `b'T'` / `b'E'`) after *every* cycle.
    /// 6. On `Terminate` or EOF, abort any open transaction cleanly.
    pub async fn serve(self, listener: TcpListener) -> Result<()> {
        todo!("Milestone 10")
    }
}
