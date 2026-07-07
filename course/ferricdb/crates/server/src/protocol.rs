//! PostgreSQL frontend/backend protocol v3 — hand-rolled framing (Milestone 10).
//!
//! You implement the wire codec in this module by hand: no protocol crates,
//! just [`bytes::BytesMut`] and big-endian integer wrangling. Once it works,
//! real `psql` can talk to FerricDB (`psql "host=127.0.0.1 port=5433
//! sslmode=disable"`).
//!
//! # Framing
//!
//! Every message except the startup packet is framed as:
//!
//! ```text
//! +-----------+----------------+----------------------+
//! | type byte | length (Int32) | body (length-4 bytes)|
//! +-----------+----------------+----------------------+
//! ```
//!
//! * All integers are **big-endian** (network byte order).
//! * The length field counts **itself plus the body** — it *includes* its own
//!   4 bytes and *excludes* the type byte. An empty-body message (e.g.
//!   `Sync`) therefore has length 4.
//! * A length smaller than 4 can never be valid and must be rejected with
//!   [`Error::Protocol`].
//! * Strings on the wire are NUL-terminated UTF-8 ("C strings") unless noted.
//!
//! The **startup packet is the one exception**: it has *no type byte*. Its
//! body starts with the protocol version number [`PROTOCOL_VERSION`]
//! (196608 = 3.0), followed by `key\0value\0` parameter pairs (e.g.
//! `user`, `database`), terminated by one extra NUL byte.
//!
//! # Message reference (the subset FerricDB speaks)
//!
//! Frontend (client → server) — decoded by [`read_message`]:
//!
//! | type | message | body |
//! |------|---------|------|
//! | —    | [`FrontendMessage::StartupMessage`] | Int32 version, `key\0value\0`… pairs, final `\0` |
//! | `Q`  | [`FrontendMessage::Query`] | SQL text as C string |
//! | `P`  | [`FrontendMessage::Parse`] | statement name C string, SQL C string, Int16 n, n × Int32 param type OIDs |
//! | `B`  | [`FrontendMessage::Bind`] | portal C string, statement C string, Int16 0 (format codes, starter always 0), Int16 n params, n × (Int32 len or -1 for NULL, then bytes), Int16 0 (result formats) |
//! | `D`  | [`FrontendMessage::Describe`] | kind byte (`S` statement / `P` portal), name C string |
//! | `E`  | [`FrontendMessage::Execute`] | portal C string, Int32 max rows (0 = unlimited) |
//! | `S`  | [`FrontendMessage::Sync`] | empty |
//! | `X`  | [`FrontendMessage::Terminate`] | empty |
//! | `p`  | [`FrontendMessage::PasswordMessage`] | password C string (lowercase `p`!) |
//!
//! Backend (server → client) — encoded by [`write_message`]:
//!
//! | type | message | body |
//! |------|---------|------|
//! | `R`  | [`BackendMessage::AuthenticationOk`] | Int32 0 |
//! | `R`  | [`BackendMessage::AuthenticationCleartextPassword`] | Int32 3 |
//! | `S`  | [`BackendMessage::ParameterStatus`] | name C string, value C string |
//! | `K`  | [`BackendMessage::BackendKeyData`] | Int32 process id, Int32 secret key |
//! | `Z`  | [`BackendMessage::ReadyForQuery`] | 1 status byte: `I` idle, `T` in txn, `E` failed txn |
//! | `T`  | [`BackendMessage::RowDescription`] | Int16 n, n × field (see [`FieldDescription`]) |
//! | `D`  | [`BackendMessage::DataRow`] | Int16 n, n × (Int32 len or **-1 for NULL**, then bytes) |
//! | `C`  | [`BackendMessage::CommandComplete`] | tag C string, e.g. `SELECT 3`, `INSERT 0 2` |
//! | `E`  | [`BackendMessage::ErrorResponse`] | fields: byte `S` + severity C string, byte `C` + SQLSTATE C string, byte `M` + message C string, final `\0` |
//! | `I`  | [`BackendMessage::EmptyQueryResponse`] | empty |
//! | `1`  | [`BackendMessage::ParseComplete`] | empty |
//! | `2`  | [`BackendMessage::BindComplete`] | empty |
//!
//! (`D` and `E` appear in both tables — the two directions are independent
//! namespaces, exactly as in PostgreSQL.)
//!
//! # Out of scope
//!
//! Real clients may open with an `SSLRequest` or `CancelRequest` packet
//! (special magic version numbers 80877103 / 80877102 instead of 196608).
//! The starter does not model them: decode them as [`Error::Protocol`] and
//! tell `psql` to skip TLS with `sslmode=disable`. (Stretch goal: answer
//! `SSLRequest` with a single `'N'` byte in the accept loop before the
//! startup packet, so a default `psql` invocation works too.)

use bytes::BytesMut;
use ferricdb_common::Result;

/// Protocol version 3.0, encoded as `(3 << 16) | 0` = 196608. The first
/// Int32 of every startup packet body must equal this.
pub const PROTOCOL_VERSION: u32 = 196608;

/// A message sent by the client (frontend) to the server (backend).
///
/// Field layouts are documented in the [module docs](self); the extended
/// query protocol messages (`Parse`/`Bind`/`Describe`/`Execute`/`Sync`) are
/// a Milestone 10 stretch goal — the simple-query flow only needs
/// `StartupMessage`, `Query`, `Terminate`, and (with auth) `PasswordMessage`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FrontendMessage {
    /// First message on every connection. **No type byte** — just length,
    /// version, and `key\0value\0` parameter pairs (ordered as sent).
    /// Interesting keys: `user`, `database`.
    StartupMessage {
        /// Parameter pairs in wire order, e.g. `("user", "ferris")`.
        params: Vec<(String, String)>,
    },
    /// `'Q'` — simple query: one or more SQL statements in a single string.
    Query {
        /// The SQL text (without the wire NUL terminator).
        sql: String,
    },
    /// `'P'` — extended protocol: parse SQL into a named prepared statement.
    Parse {
        /// Prepared-statement name; empty string = the unnamed statement.
        statement: String,
        /// The SQL text to parse.
        sql: String,
        /// Parameter type OIDs the client pre-declares (may be empty).
        param_types: Vec<u32>,
    },
    /// `'B'` — extended protocol: bind parameter values to a prepared
    /// statement, producing a portal.
    Bind {
        /// Destination portal name; empty string = the unnamed portal.
        portal: String,
        /// Source prepared-statement name.
        statement: String,
        /// One entry per parameter, `None` = NULL (wire length -1). The
        /// starter uses text format only (format-code counts written as 0).
        params: Vec<Option<Vec<u8>>>,
    },
    /// `'E'` — extended protocol: execute a portal.
    Execute {
        /// Portal name to run.
        portal: String,
        /// Row limit; 0 means "no limit".
        max_rows: i32,
    },
    /// `'D'` — extended protocol: describe a statement or portal so the
    /// client learns its result shape.
    Describe {
        /// `b'S'` to describe a prepared statement, `b'P'` for a portal.
        kind: u8,
        /// Name of the statement/portal (empty = unnamed).
        name: String,
    },
    /// `'S'` — extended protocol: end of a command sequence; the server
    /// answers with `ReadyForQuery`. Empty body.
    Sync,
    /// `'X'` — client is closing the connection. Empty body.
    Terminate,
    /// `'p'` (lowercase!) — cleartext password, in response to
    /// `AuthenticationCleartextPassword`.
    PasswordMessage {
        /// The password (without the wire NUL terminator).
        password: String,
    },
}

/// A message sent by the server (backend) to the client (frontend).
///
/// Field layouts are documented in the [module docs](self). The simple-query
/// happy path is: `AuthenticationOk`, `ParameterStatus`*, `BackendKeyData`,
/// `ReadyForQuery`, then per query `RowDescription`, `DataRow`*,
/// `CommandComplete`, `ReadyForQuery`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BackendMessage {
    /// `'R'` with Int32 code 0 — authentication succeeded.
    AuthenticationOk,
    /// `'R'` with Int32 code 3 — server demands a cleartext password
    /// (sent when [`crate::ServerConfig::password`] is `Some`).
    AuthenticationCleartextPassword,
    /// `'S'` — run-time parameter report sent during session startup
    /// (e.g. `server_version`, `client_encoding`).
    ParameterStatus {
        /// Parameter name.
        name: String,
        /// Parameter value.
        value: String,
    },
    /// `'K'` — cancellation key data. FerricDB does not implement query
    /// cancellation; send made-up values so that libpq is happy.
    BackendKeyData {
        /// Backend process id (fake it — e.g. a connection counter).
        process_id: u32,
        /// Per-connection secret for CancelRequest (unused by FerricDB).
        secret_key: u32,
    },
    /// `'Z'` — server is ready for the next query.
    ReadyForQuery {
        /// Transaction status: `b'I'` idle, `b'T'` in a transaction,
        /// `b'E'` in a failed transaction (commands rejected until
        /// ROLLBACK).
        txn_status: u8,
    },
    /// `'T'` — shape of the rows that follow: one entry per output column.
    RowDescription {
        /// Column descriptors in output order.
        fields: Vec<FieldDescription>,
    },
    /// `'D'` — one result row. Values are in **text format** (the decimal /
    /// UTF-8 rendering of each value), `None` = SQL NULL, encoded as
    /// length **-1** with no value bytes.
    DataRow {
        /// One entry per column of the `RowDescription`.
        values: Vec<Option<Vec<u8>>>,
    },
    /// `'C'` — a statement finished. The tag mirrors Postgres:
    /// `SELECT <rows>`, `INSERT 0 <rows>`, `UPDATE <rows>`,
    /// `DELETE <rows>`, `CREATE TABLE`, `BEGIN`, `COMMIT`, `ROLLBACK`, …
    CommandComplete {
        /// The command tag string.
        tag: String,
    },
    /// `'E'` — a statement failed. Maps a [`ferricdb_common::Error`] onto
    /// the wire; the connection stays usable afterwards (send
    /// `ReadyForQuery` next).
    ErrorResponse {
        /// Severity, normally `"ERROR"`.
        severity: String,
        /// SQLSTATE code, e.g. `"42601"` syntax error, `"40001"`
        /// serialization failure, `"XX000"` internal error.
        code: String,
        /// Human-readable message.
        message: String,
    },
    /// `'I'` — the client sent an empty query string. Empty body.
    EmptyQueryResponse,
    /// `'1'` — Parse succeeded. Empty body.
    ParseComplete,
    /// `'2'` — Bind succeeded. Empty body.
    BindComplete,
}

/// One column entry inside [`BackendMessage::RowDescription`].
///
/// Wire layout per field: name C string, Int32 table OID, Int16 attribute
/// number, Int32 type OID, Int16 type length, Int32 type modifier, Int16
/// format code. FerricDB only keeps the three interesting values; the
/// encoder writes `0` for table OID / attribute number, `-1` for the type
/// modifier, and `0` (text) for the format code.
///
/// Suggested Postgres type OIDs for FerricDB's [`ferricdb_common::DataType`]s:
///
/// | DataType | OID | type_len |
/// |----------|-----|----------|
/// | Boolean  | 16 (`bool`) | 1 |
/// | Integer  | 20 (`int8`) | 8 |
/// | Float    | 701 (`float8`) | 8 |
/// | Text     | 25 (`text`) | -1 (varlena) |
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldDescription {
    /// Column name shown by the client.
    pub name: String,
    /// PostgreSQL type OID (see table above).
    pub type_oid: u32,
    /// Fixed byte width of the type, or -1 for variable-length types.
    pub type_len: i16,
}

/// Decode one frontend message from the front of `buf` (Milestone 10).
///
/// This is the **incremental, non-blocking** decoder the session loop calls
/// after every TCP read:
///
/// * `Ok(Some(msg))` — a complete message was parsed; its bytes (type byte,
///   length field, body) have been consumed from the front of `buf`.
/// * `Ok(None)` — `buf` does not yet hold a complete message (fewer bytes
///   than the length field promises). `buf` is left untouched; read more
///   bytes from the socket and call again.
/// * `Err(Error::Protocol)` — the bytes can never form a valid message:
///   a length field < 4 (it must at least count itself), an unknown type
///   byte, a missing NUL terminator, invalid UTF-8, or a startup packet
///   whose version is not [`PROTOCOL_VERSION`]. The caller should report
///   and close the connection.
///
/// `startup` selects the framing mode: pass `true` for the first message of
/// a connection (startup packet, **no type byte**) and `false` afterwards
/// (normal type-byte framing).
pub fn read_message(buf: &mut BytesMut, startup: bool) -> Result<Option<FrontendMessage>> {
    todo!("Milestone 10")
}

/// Encode one backend message onto the end of `buf` (Milestone 10).
///
/// Appends the type byte, the Int32 length (counting itself and the body,
/// not the type byte), and the body, per the [module docs](self). The
/// session loop encodes into a `BytesMut` and hands the bytes to the socket
/// in one write.
///
/// Errors with [`Error::Protocol`] on unencodable input — e.g. a string
/// field that contains an interior NUL byte, or more than `i16::MAX`
/// columns in a row.
pub fn write_message(buf: &mut BytesMut, msg: &BackendMessage) -> Result<()> {
    todo!("Milestone 10")
}

/// Encode one *frontend* message onto the end of `buf` (Milestone 10).
///
/// Mirror image of [`read_message`] — the client-side half of the codec.
/// The server itself never sends frontend messages; this exists so the
/// provided tests (and any toy client you write) can round-trip messages
/// through the codec without a live TCP connection.
///
/// `StartupMessage` is encoded **without a type byte**; everything else
/// gets normal type-byte framing. Same error contract as
/// [`write_message`].
pub fn write_frontend_message(buf: &mut BytesMut, msg: &FrontendMessage) -> Result<()> {
    todo!("Milestone 10")
}

/// Decode one *backend* message from the front of `buf` (Milestone 10).
///
/// Mirror image of [`write_message`] — the client-side half of the codec,
/// used by the provided round-trip tests and any toy client. Incremental
/// contract identical to [`read_message`]: `Ok(None)` means "need more
/// bytes, buffer untouched"; on success the message's bytes are consumed.
/// (Backend framing always has a type byte, so there is no `startup` flag.)
pub fn read_backend_message(buf: &mut BytesMut) -> Result<Option<BackendMessage>> {
    todo!("Milestone 10")
}
