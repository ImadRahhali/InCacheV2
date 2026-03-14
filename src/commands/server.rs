use crate::protocol::RespValue;
use crate::store::Store;

const COMMAND_COUNT: i64 = 45;

pub fn cmd_ping(args: &[Vec<u8>]) -> RespValue {
    if let Some(msg) = args.first() {
        RespValue::BulkString(msg.clone())
    } else {
        RespValue::SimpleString("PONG".into())
    }
}

pub fn cmd_echo(args: &[Vec<u8>]) -> RespValue {
    if let Some(msg) = args.first() {
        RespValue::BulkString(msg.clone())
    } else {
        RespValue::Error("ERR wrong number of arguments for 'echo' command".into())
    }
}

pub fn cmd_hello(_args: &[Vec<u8>]) -> RespValue {
    RespValue::Array(vec![
        RespValue::BulkString(b"server".to_vec()),
        RespValue::BulkString(b"incache_v2".to_vec()),
        RespValue::BulkString(b"version".to_vec()),
        RespValue::BulkString(b"0.1.0".to_vec()),
        RespValue::BulkString(b"proto".to_vec()),
        RespValue::Integer(2),
        RespValue::BulkString(b"id".to_vec()),
        RespValue::Integer(1),
        RespValue::BulkString(b"mode".to_vec()),
        RespValue::BulkString(b"standalone".to_vec()),
        RespValue::BulkString(b"role".to_vec()),
        RespValue::BulkString(b"master".to_vec()),
        RespValue::BulkString(b"modules".to_vec()),
        RespValue::Array(vec![]),
    ])
}

pub fn cmd_flush(store: &mut Store) -> RespValue {
    store.flush();
    RespValue::SimpleString("OK".into())
}

pub fn cmd_dbsize(store: &mut Store) -> RespValue {
    RespValue::Integer(store.dbsize() as i64)
}

pub fn cmd_info() -> RespValue {
    let info = "# Server\r\nredis_version:0.1.0 (incache_v2/Rust)\r\ntcp_port:6399\r\n";
    RespValue::BulkString(info.as_bytes().to_vec())
}

pub fn cmd_select(args: &[Vec<u8>]) -> RespValue {
    if args.is_empty() {
        return RespValue::Error("ERR wrong number of arguments for 'select' command".into());
    }
    let db = String::from_utf8_lossy(&args[0]);
    if db == "0" {
        RespValue::SimpleString("OK".into())
    } else {
        RespValue::Error("ERR DB index is out of range".into())
    }
}

pub fn cmd_command(args: &[Vec<u8>]) -> RespValue {
    if let Some(sub) = args.first() {
        if sub.to_ascii_uppercase() == b"COUNT" {
            return RespValue::Integer(COMMAND_COUNT);
        }
    }
    RespValue::Integer(COMMAND_COUNT)
}

pub fn cmd_client(_args: &[Vec<u8>]) -> RespValue {
    RespValue::SimpleString("OK".into())
}
