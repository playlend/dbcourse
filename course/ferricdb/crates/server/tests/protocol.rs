//! Provided Milestone 10 codec test suite — pure bytes, no TCP.
//!
//! Run with `cargo test -p ferricdb-server -- --include-ignored ms10_`.
//! The live half of the milestone (psql connects, the slt suite passes over
//! the wire) is validated interactively per the milestone page.

use bytes::BytesMut;
use ferricdb_common::Error;
use ferricdb_server::protocol::{
    read_backend_message, read_message, write_frontend_message, write_message, BackendMessage,
    FieldDescription, FrontendMessage, PROTOCOL_VERSION, SSL_REQUEST_CODE,
};

fn frontend_roundtrip(msg: FrontendMessage, startup: bool) {
    let mut buf = BytesMut::new();
    write_frontend_message(&mut buf, &msg).expect("encode");
    let got = read_message(&mut buf, startup)
        .expect("decode")
        .expect("complete message");
    assert_eq!(got, msg);
    assert!(buf.is_empty(), "decoder consumed exactly one message");
}

fn backend_roundtrip(msg: BackendMessage) {
    let mut buf = BytesMut::new();
    write_message(&mut buf, &msg).expect("encode");
    let got = read_backend_message(&mut buf)
        .expect("decode")
        .expect("complete message");
    assert_eq!(got, msg);
    assert!(buf.is_empty());
}

// ------------------------------------------------------------ exact bytes

/// The module M11 byte explorer, as a test: Query("SELECT 1;") is exactly
/// 'Q', Int32 14 (self-inclusive, type-byte-exclusive), the text, a NUL.
#[test]
#[ignore = "milestone 10"]
fn ms10_query_select_1_exact_bytes() {
    let mut buf = BytesMut::new();
    write_frontend_message(
        &mut buf,
        &FrontendMessage::Query {
            sql: "SELECT 1;".into(),
        },
    )
    .unwrap();
    let expected: &[u8] = &[
        b'Q', 0x00, 0x00, 0x00, 0x0E, // type byte + Int32 length = 14
        b'S', b'E', b'L', b'E', b'C', b'T', b' ', b'1', b';', 0x00,
    ];
    assert_eq!(
        &buf[..],
        expected,
        "length includes itself, excludes the type byte"
    );
}

#[test]
#[ignore = "milestone 10"]
fn ms10_startup_message_has_no_type_byte() {
    let msg = FrontendMessage::StartupMessage {
        params: vec![
            ("user".into(), "ferris".into()),
            ("database".into(), "demo".into()),
        ],
    };
    let mut buf = BytesMut::new();
    write_frontend_message(&mut buf, &msg).unwrap();

    // first four bytes are the Int32 length of the WHOLE packet…
    let len = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    assert_eq!(len, buf.len(), "startup length counts the entire packet");
    // …then the protocol version, not a type byte
    let ver = u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]);
    assert_eq!(ver, PROTOCOL_VERSION);
    // and the packet ends with the pair-list terminator
    assert_eq!(buf[buf.len() - 1], 0x00);

    frontend_roundtrip(msg, true);
}

#[test]
#[ignore = "milestone 10"]
fn ms10_datarow_null_is_minus_one() {
    let msg = BackendMessage::DataRow {
        values: vec![Some(b"42".to_vec()), None, Some(vec![])],
    };
    let mut buf = BytesMut::new();
    write_message(&mut buf, &msg).unwrap();

    // find the NULL column: Int32 length -1, no value bytes after it —
    // and the empty string: Int32 length 0. They must differ on the wire.
    let bytes = buf.to_vec();
    let minus_one = (-1i32).to_be_bytes();
    let has_minus_one = bytes.windows(4).any(|w| w == minus_one);
    assert!(has_minus_one, "NULL travels as length -1");

    backend_roundtrip(msg);
}

// ------------------------------------------------------------ round-trips

#[test]
#[ignore = "milestone 10"]
fn ms10_frontend_roundtrips() {
    frontend_roundtrip(
        FrontendMessage::Query {
            sql: "SELECT * FROM t;".into(),
        },
        false,
    );
    frontend_roundtrip(FrontendMessage::Terminate, false);
    frontend_roundtrip(
        FrontendMessage::PasswordMessage {
            password: "hunter2".into(),
        },
        false,
    );
    frontend_roundtrip(FrontendMessage::Sync, false);
    frontend_roundtrip(
        FrontendMessage::Parse {
            statement: String::new(),
            sql: "SELECT * FROM t WHERE id = $1".into(),
            param_types: vec![20],
        },
        false,
    );
    frontend_roundtrip(
        FrontendMessage::Bind {
            portal: String::new(),
            statement: String::new(),
            params: vec![Some(b"7".to_vec()), None],
        },
        false,
    );
    frontend_roundtrip(
        FrontendMessage::Execute {
            portal: String::new(),
            max_rows: 0,
        },
        false,
    );
    frontend_roundtrip(
        FrontendMessage::Describe {
            kind: b'S',
            name: String::new(),
        },
        false,
    );
}

#[test]
#[ignore = "milestone 10"]
fn ms10_backend_roundtrips() {
    backend_roundtrip(BackendMessage::AuthenticationOk);
    backend_roundtrip(BackendMessage::AuthenticationCleartextPassword);
    backend_roundtrip(BackendMessage::ParameterStatus {
        name: "server_version".into(),
        value: "16.0 (FerricDB)".into(),
    });
    backend_roundtrip(BackendMessage::BackendKeyData {
        process_id: 42,
        secret_key: 0xC0FFEE,
    });
    backend_roundtrip(BackendMessage::ReadyForQuery { txn_status: b'I' });
    backend_roundtrip(BackendMessage::RowDescription {
        fields: vec![
            FieldDescription {
                name: "id".into(),
                type_oid: 20,
                type_len: 8,
            },
            FieldDescription {
                name: "name".into(),
                type_oid: 25,
                type_len: -1,
            },
        ],
    });
    backend_roundtrip(BackendMessage::CommandComplete {
        tag: "INSERT 0 5".into(),
    });
    backend_roundtrip(BackendMessage::ErrorResponse {
        severity: "ERROR".into(),
        code: "40001".into(),
        message: "serialization failure — retry the transaction".into(),
    });
    backend_roundtrip(BackendMessage::EmptyQueryResponse);
    backend_roundtrip(BackendMessage::ParseComplete);
    backend_roundtrip(BackendMessage::BindComplete);
}

// --------------------------------------------------------- framing edges

#[test]
#[ignore = "milestone 10"]
fn ms10_partial_message_returns_none_and_keeps_bytes() {
    let mut buf = BytesMut::new();
    write_frontend_message(
        &mut buf,
        &FrontendMessage::Query {
            sql: "SELECT 1;".into(),
        },
    )
    .unwrap();
    let full = buf.clone();

    // feed only the first 7 bytes: not a complete message yet
    let mut partial = BytesMut::from(&full[..7]);
    assert!(matches!(read_message(&mut partial, false), Ok(None)));
    assert_eq!(partial.len(), 7, "incomplete input must not be consumed");

    // append the rest: now it parses
    partial.extend_from_slice(&full[7..]);
    let msg = read_message(&mut partial, false).unwrap().unwrap();
    assert_eq!(
        msg,
        FrontendMessage::Query {
            sql: "SELECT 1;".into()
        }
    );
}

#[test]
#[ignore = "milestone 10"]
fn ms10_malformed_length_is_a_protocol_error() {
    // type byte 'Q', then a length of 2 — impossible, the field counts
    // itself (minimum 4)
    let mut buf = BytesMut::from(&[b'Q', 0x00, 0x00, 0x00, 0x02, 0x00][..]);
    match read_message(&mut buf, false) {
        Err(Error::Protocol(_)) => {}
        other => panic!("expected a protocol error, got {other:?}"),
    }
}

#[test]
#[ignore = "milestone 10"]
fn ms10_two_messages_in_one_buffer_parse_in_order() {
    let mut buf = BytesMut::new();
    write_frontend_message(
        &mut buf,
        &FrontendMessage::Query {
            sql: "SELECT 1;".into(),
        },
    )
    .unwrap();
    write_frontend_message(&mut buf, &FrontendMessage::Terminate).unwrap();

    let first = read_message(&mut buf, false).unwrap().unwrap();
    assert_eq!(
        first,
        FrontendMessage::Query {
            sql: "SELECT 1;".into()
        }
    );
    let second = read_message(&mut buf, false).unwrap().unwrap();
    assert_eq!(second, FrontendMessage::Terminate);
    assert!(buf.is_empty());
}

#[test]
#[ignore = "milestone 10"]
fn ms10_ssl_request_roundtrip_and_exact_bytes() {
    let mut buf = BytesMut::new();
    write_frontend_message(&mut buf, &FrontendMessage::SslRequest).unwrap();
    let expected = [
        0x00,
        0x00,
        0x00,
        0x08, // Int32 length = 8
        ((SSL_REQUEST_CODE >> 24) & 0xff) as u8,
        ((SSL_REQUEST_CODE >> 16) & 0xff) as u8,
        ((SSL_REQUEST_CODE >> 8) & 0xff) as u8,
        (SSL_REQUEST_CODE & 0xff) as u8,
    ];
    assert_eq!(&buf[..], &expected[..]);

    let decoded = read_message(&mut buf, true).unwrap().unwrap();
    assert_eq!(decoded, FrontendMessage::SslRequest);
    assert!(buf.is_empty());
}

#[test]
#[ignore = "milestone 10"]
fn ms10_negative_length_and_bad_startup_version_error() {
    // Negative Int32 length (-1) must error immediately, not wait for 4 GiB.
    let mut neg = BytesMut::from(&[b'Q', 0xFF, 0xFF, 0xFF, 0xFF][..]);
    assert!(matches!(
        read_message(&mut neg, false),
        Err(Error::Protocol(_))
    ));

    // Unknown startup protocol version (neither 196608 nor SSL_REQUEST_CODE).
    let mut bad_ver = BytesMut::from(&[0x00, 0x00, 0x00, 0x08, 0x00, 0x02, 0x00, 0x00][..]);
    assert!(matches!(
        read_message(&mut bad_ver, true),
        Err(Error::Protocol(_))
    ));
}

#[tokio::test]
#[ignore = "milestone 10"]
async fn ms10_server_ssl_decline_and_simple_query_over_tcp() {
    use ferricdb_buffer::{BufferPoolManager, ClockReplacer};
    use ferricdb_catalog::Catalog;
    use ferricdb_exec::ExecContext;
    use ferricdb_server::{Server, ServerConfig};
    use ferricdb_storage::{DiskManager, PageStore};
    use ferricdb_txn::TransactionManager;
    use std::sync::Arc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};

    let dir = tempfile::tempdir().unwrap();
    let disk = DiskManager::open(&dir.path().join("srv.db")).unwrap();
    let pool: Arc<dyn PageStore> = Arc::new(BufferPoolManager::new(
        32,
        disk,
        Box::new(ClockReplacer::new(32)),
    ));
    let catalog = Arc::new(Catalog::bootstrap(Arc::clone(&pool)).unwrap());
    let ctx = Arc::new(ExecContext::new(
        catalog,
        pool,
        Arc::new(TransactionManager::new()),
    ));

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = Server::new(
        ServerConfig {
            addr: addr.to_string(),
            password: None,
        },
        ctx,
    );
    let srv_handle = tokio::spawn(async move {
        let _ = server.serve(listener).await;
    });

    let mut stream = TcpStream::connect(addr).await.unwrap();

    // 1. Send SSLRequest; server declines cleanly with 'N'.
    let mut out = BytesMut::new();
    write_frontend_message(&mut out, &FrontendMessage::SslRequest).unwrap();
    stream.write_all(&out).await.unwrap();
    let mut ssl_reply = [0u8; 1];
    stream.read_exact(&mut ssl_reply).await.unwrap();
    assert_eq!(ssl_reply, [b'N'], "server must decline SSLRequest with 'N'");

    // 2. Send StartupMessage and read until ReadyForQuery('I').
    out.clear();
    write_frontend_message(
        &mut out,
        &FrontendMessage::StartupMessage {
            params: vec![("user".into(), "ferris".into())],
        },
    )
    .unwrap();
    stream.write_all(&out).await.unwrap();

    let mut in_buf = BytesMut::new();
    let mut saw_auth_ok = false;
    let mut saw_ready = false;
    while !saw_ready {
        while let Some(msg) = read_backend_message(&mut in_buf).unwrap() {
            match msg {
                BackendMessage::AuthenticationOk => saw_auth_ok = true,
                BackendMessage::ReadyForQuery { txn_status } => {
                    assert_eq!(txn_status, b'I');
                    assert!(saw_auth_ok);
                    saw_ready = true;
                    break;
                }
                _ => {}
            }
        }
        if saw_ready {
            break;
        }
        let n = stream.read_buf(&mut in_buf).await.unwrap();
        assert!(n > 0, "server closed before ReadyForQuery");
    }

    // 3. Send a simple query and verify DataRow + CommandComplete + ReadyForQuery.
    out.clear();
    write_frontend_message(
        &mut out,
        &FrontendMessage::Query {
            sql: "SELECT 1 + 1;".into(),
        },
    )
    .unwrap();
    stream.write_all(&out).await.unwrap();

    let mut saw_two = false;
    let mut done = false;
    while !done {
        let n = stream.read_buf(&mut in_buf).await.unwrap();
        assert!(n > 0, "server closed mid-query");
        while let Some(msg) = read_backend_message(&mut in_buf).unwrap() {
            match msg {
                BackendMessage::DataRow { values } => {
                    if values == vec![Some(b"2".to_vec())] {
                        saw_two = true;
                    }
                }
                BackendMessage::ReadyForQuery { .. } => {
                    done = true;
                    break;
                }
                _ => {}
            }
        }
    }
    assert!(saw_two, "server must return DataRow([\"2\"])");

    // 4. Terminate cleanly.
    out.clear();
    write_frontend_message(&mut out, &FrontendMessage::Terminate).unwrap();
    stream.write_all(&out).await.unwrap();
    srv_handle.abort();
}
