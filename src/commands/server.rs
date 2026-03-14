use bytes::Bytes;
use crate::protocol::RespValue;
use crate::store::Store;

const COMMAND_COUNT: i64 = 45;

pub fn cmd_ping(args: &[Bytes]) -> RespValue {
    if let Some(msg) = args.first() {
        RespValue::bulk(msg.clone())
    } else {
        RespValue::pong()
    }
}

pub fn cmd_echo(args: &[Bytes]) -> RespValue {
    if let Some(msg) = args.first() {
        RespValue::bulk(msg.clone())
    } else {
        RespValue::error("ERR wrong number of arguments for 'echo' command".into())
    }
}

pub fn cmd_hello(_args: &[Bytes]) -> RespValue {
    RespValue::Array(vec![
        RespValue::bulk_from(b"server"), RespValue::bulk_from(b"incache_v2"),
        RespValue::bulk_from(b"version"), RespValue::bulk_from(b"0.2.0"),
        RespValue::bulk_from(b"proto"), RespValue::Integer(2),
        RespValue::bulk_from(b"id"), RespValue::Integer(1),
        RespValue::bulk_from(b"mode"), RespValue::bulk_from(b"standalone"),
        RespValue::bulk_from(b"role"), RespValue::bulk_from(b"master"),
        RespValue::bulk_from(b"modules"), RespValue::Array(vec![]),
    ])
}

pub fn cmd_flush(store: &Store) -> RespValue {
    store.flush();
    RespValue::ok()
}

pub fn cmd_dbsize(store: &Store) -> RespValue {
    RespValue::Integer(store.dbsize() as i64)
}

pub fn cmd_info() -> RespValue {
    RespValue::BulkString(Bytes::from_static(
        b"# Server\r\nredis_version:0.2.0 (incache_v2/Rust)\r\ntcp_port:6399\r\n"
    ))
}

pub fn cmd_select(args: &[Bytes]) -> RespValue {
    if args.is_empty() {
        return RespValue::error("ERR wrong number of arguments for 'select' command".into());
    }
    if args[0] == &b"0"[..] {
        RespValue::ok()
    } else {
        RespValue::error("ERR DB index is out of range".into())
    }
}

pub fn cmd_command(args: &[Bytes]) -> RespValue {
    if let Some(sub) = args.first() {
        let mut u = sub.to_vec();
        u.make_ascii_uppercase();
        if u == b"COUNT" {
            return RespValue::Integer(COMMAND_COUNT);
        }
    }
    RespValue::Integer(COMMAND_COUNT)
}
