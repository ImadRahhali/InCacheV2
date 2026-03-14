pub mod strings;
pub mod lists;
pub mod hashes;
pub mod sets;
pub mod server;

use crate::protocol::{Command, RespValue};
use crate::store::Store;

/// Dispatch a command. Args are slices into the read buffer.
#[inline(always)]
pub fn dispatch(store: &mut Store, cmd: &Command, buf: &[u8]) -> RespValue {
    let name_raw = cmd.arg(0, buf);
    let mut upper = [0u8; 16];
    let len = name_raw.len().min(16);
    upper[..len].copy_from_slice(&name_raw[..len]);
    upper[..len].make_ascii_uppercase();
    let name = &upper[..len];

    match name {
        b"SET" => strings::cmd_set(store, cmd, buf),
        b"GET" => strings::cmd_get(store, cmd, buf),
        b"PING" => server::cmd_ping(cmd, buf),
        b"INCR" => strings::cmd_incr(store, cmd, buf),
        b"LPUSH" => lists::cmd_lpush(store, cmd, buf),
        b"HSET" => hashes::cmd_hset(store, cmd, buf),
        b"DEL" => strings::cmd_del(store, cmd, buf),

        b"ECHO" => server::cmd_echo(cmd, buf),
        b"HELLO" => server::cmd_hello(),
        b"FLUSHALL" | b"FLUSHDB" => { store.flush(); RespValue::ok() }
        b"DBSIZE" => RespValue::Integer(store.dbsize() as i64),
        b"INFO" => server::cmd_info(),
        b"SELECT" => server::cmd_select(cmd, buf),
        b"COMMAND" => server::cmd_command(cmd, buf),
        b"CLIENT" => RespValue::ok(),

        b"GETSET" => strings::cmd_getset(store, cmd, buf),
        b"MSET" => strings::cmd_mset(store, cmd, buf),
        b"MGET" => strings::cmd_mget(store, cmd, buf),
        b"EXISTS" => strings::cmd_exists(store, cmd, buf),
        b"INCRBY" => strings::cmd_incrby(store, cmd, buf),
        b"DECR" => strings::cmd_decr(store, cmd, buf),
        b"DECRBY" => strings::cmd_decrby(store, cmd, buf),
        b"APPEND" => strings::cmd_append(store, cmd, buf),
        b"STRLEN" => strings::cmd_strlen(store, cmd, buf),
        b"SETNX" => strings::cmd_setnx(store, cmd, buf),
        b"SETEX" => strings::cmd_setex(store, cmd, buf),
        b"EXPIRE" => strings::cmd_expire(store, cmd, buf),
        b"TTL" => strings::cmd_ttl(store, cmd, buf),
        b"PERSIST" => strings::cmd_persist(store, cmd, buf),
        b"TYPE" => strings::cmd_type(store, cmd, buf),
        b"RENAME" => strings::cmd_rename(store, cmd, buf),
        b"KEYS" => strings::cmd_keys(store, cmd, buf),

        b"RPUSH" => lists::cmd_rpush(store, cmd, buf),
        b"LPOP" => lists::cmd_lpop(store, cmd, buf),
        b"RPOP" => lists::cmd_rpop(store, cmd, buf),
        b"LRANGE" => lists::cmd_lrange(store, cmd, buf),
        b"LLEN" => lists::cmd_llen(store, cmd, buf),
        b"LINDEX" => lists::cmd_lindex(store, cmd, buf),
        b"LSET" => lists::cmd_lset(store, cmd, buf),
        b"LINSERT" => lists::cmd_linsert(store, cmd, buf),
        b"LREM" => lists::cmd_lrem(store, cmd, buf),

        b"HGET" => hashes::cmd_hget(store, cmd, buf),
        b"HMSET" => hashes::cmd_hmset(store, cmd, buf),
        b"HMGET" => hashes::cmd_hmget(store, cmd, buf),
        b"HGETALL" => hashes::cmd_hgetall(store, cmd, buf),
        b"HDEL" => hashes::cmd_hdel(store, cmd, buf),
        b"HEXISTS" => hashes::cmd_hexists(store, cmd, buf),
        b"HLEN" => hashes::cmd_hlen(store, cmd, buf),
        b"HKEYS" => hashes::cmd_hkeys(store, cmd, buf),
        b"HVALS" => hashes::cmd_hvals(store, cmd, buf),
        b"HINCRBY" => hashes::cmd_hincrby(store, cmd, buf),

        b"SADD" => sets::cmd_sadd(store, cmd, buf),
        b"SMEMBERS" => sets::cmd_smembers(store, cmd, buf),
        b"SREM" => sets::cmd_srem(store, cmd, buf),
        b"SISMEMBER" => sets::cmd_sismember(store, cmd, buf),
        b"SCARD" => sets::cmd_scard(store, cmd, buf),
        b"SUNION" => sets::cmd_sunion(store, cmd, buf),
        b"SINTER" => sets::cmd_sinter(store, cmd, buf),
        b"SDIFF" => sets::cmd_sdiff(store, cmd, buf),
        b"SMOVE" => sets::cmd_smove(store, cmd, buf),
        b"SPOP" => sets::cmd_spop(store, cmd, buf),

        _ => {
            let s = String::from_utf8_lossy(name_raw);
            RespValue::error(format!("ERR unknown command '{}'", s.to_lowercase()))
        }
    }
}
