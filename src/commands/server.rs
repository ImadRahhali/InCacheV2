use bytes::Bytes;
use crate::protocol::{Command, RespValue};
use crate::store::Store;

const COMMAND_COUNT: i64 = 45;

#[inline(always)]
pub fn cmd_ping(cmd: &Command, buf: &[u8]) -> RespValue {
    if cmd.argc() > 1 { RespValue::bulk(Bytes::copy_from_slice(cmd.arg(1, buf))) }
    else { RespValue::pong() }
}

#[inline(always)]
pub fn cmd_echo(cmd: &Command, buf: &[u8]) -> RespValue {
    if cmd.argc() > 1 { RespValue::bulk(Bytes::copy_from_slice(cmd.arg(1, buf))) }
    else { RespValue::error("ERR wrong number of arguments for 'echo' command".into()) }
}

pub fn cmd_hello() -> RespValue {
    RespValue::Array(vec![
        RespValue::bulk_from(b"server"), RespValue::bulk_from(b"incache_v2"),
        RespValue::bulk_from(b"version"), RespValue::bulk_from(b"0.4.0"),
        RespValue::bulk_from(b"proto"), RespValue::Integer(2),
        RespValue::bulk_from(b"id"), RespValue::Integer(1),
        RespValue::bulk_from(b"mode"), RespValue::bulk_from(b"standalone"),
        RespValue::bulk_from(b"role"), RespValue::bulk_from(b"master"),
        RespValue::bulk_from(b"modules"), RespValue::Array(vec![]),
    ])
}

pub fn cmd_info() -> RespValue {
    RespValue::BulkString(Bytes::from_static(b"# Server\r\nredis_version:0.4.0 (incache_v2/Rust)\r\ntcp_port:6399\r\n"))
}

pub fn cmd_select(cmd: &Command, buf: &[u8]) -> RespValue {
    if cmd.argc() < 2 { return RespValue::error("ERR wrong number of arguments for 'select' command".into()); }
    if cmd.arg(1, buf) == b"0" { RespValue::ok() }
    else { RespValue::error("ERR DB index is out of range".into()) }
}

pub fn cmd_command(cmd: &Command, buf: &[u8]) -> RespValue {
    if cmd.argc() > 1 {
        let sub = cmd.arg(1, buf);
        if sub.eq_ignore_ascii_case(b"COUNT") { return RespValue::Integer(COMMAND_COUNT); }
    }
    RespValue::Integer(COMMAND_COUNT)
}
