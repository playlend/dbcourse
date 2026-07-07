//! Provided Milestone 10 codec test suite — pure bytes, no TCP.
//!
//! Run with `cargo test -p ferricdb-server -- --include-ignored ms10_`.
//! The live half of the milestone (psql connects, the slt suite passes over
//! the wire) is validated interactively per the milestone page.

use bytes::BytesMut;
use ferricdb_common::Error;
use ferricdb_server::protocol::{
    read_backend_message, read_message, write_frontend_message, write_message, BackendMessage,
    FieldDescription, FrontendMessage, PROTOCOL_VERSION,
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
