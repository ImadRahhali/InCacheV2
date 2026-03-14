pub mod strings;
pub mod lists;
pub mod hashes;
pub mod sets;
pub mod server;

use bytes::Bytes;
use crate::protocol::RespValue;
use crate::store::Store;

#[inline(always)]
pub fn dispatch(store: &mut Store, cmd_name: &Bytes, args: &[Bytes]) -> RespValue {
    let mut upper = [0u8; 16];
    let len = cmd_name.len().min(16);
    upper[..len].copy_from_slice(&cmd_name[..len]);
    upper[..len].make_ascii_uppercase();
    let name = &upper[..len];

    match name {
        b"SET" => strings::cmd_set(store, args),
        b"GET" => strings::cmd_get(store, args),
        b"PING" => server::cmd_ping(args),
        b"INCR" => strings::cmd_incr(store, args),
        b"LPUSH" => lists::cmd_lpush(store, args),
        b"HSET" => hashes::cmd_hset(store, args),
        b"DEL" => strings::cmd_del(store, args),

        b"ECHO" => server::cmd_echo(args),
        b"HELLO" => server::cmd_hello(args),
        b"FLUSHALL" | b"FLUSHDB" => server::cmd_flush(store),
        b"DBSIZE" => server::cmd_dbsize(store),
        b"INFO" => server::cmd_info(),
        b"SELECT" => server::cmd_select(args),
        b"COMMAND" => server::cmd_command(args),
        b"CLIENT" => RespValue::ok(),

        b"GETSET" => strings::cmd_getset(store, args),
        b"MSET" => strings::cmd_mset(store, args),
        b"MGET" => strings::cmd_mget(store, args),
        b"EXISTS" => strings::cmd_exists(store, args),
        b"INCRBY" => strings::cmd_incrby(store, args),
        b"DECR" => strings::cmd_decr(store, args),
        b"DECRBY" => strings::cmd_decrby(store, args),
        b"APPEND" => strings::cmd_append(store, args),
        b"STRLEN" => strings::cmd_strlen(store, args),
        b"SETNX" => strings::cmd_setnx(store, args),
        b"SETEX" => strings::cmd_setex(store, args),
        b"EXPIRE" => strings::cmd_expire(store, args),
        b"TTL" => strings::cmd_ttl(store, args),
        b"PERSIST" => strings::cmd_persist(store, args),
        b"TYPE" => strings::cmd_type(store, args),
        b"RENAME" => strings::cmd_rename(store, args),
        b"KEYS" => strings::cmd_keys(store, args),

        b"RPUSH" => lists::cmd_rpush(store, args),
        b"LPOP" => lists::cmd_lpop(store, args),
        b"RPOP" => lists::cmd_rpop(store, args),
        b"LRANGE" => lists::cmd_lrange(store, args),
        b"LLEN" => lists::cmd_llen(store, args),
        b"LINDEX" => lists::cmd_lindex(store, args),
        b"LSET" => lists::cmd_lset(store, args),
        b"LINSERT" => lists::cmd_linsert(store, args),
        b"LREM" => lists::cmd_lrem(store, args),

        b"HGET" => hashes::cmd_hget(store, args),
        b"HMSET" => hashes::cmd_hmset(store, args),
        b"HMGET" => hashes::cmd_hmget(store, args),
        b"HGETALL" => hashes::cmd_hgetall(store, args),
        b"HDEL" => hashes::cmd_hdel(store, args),
        b"HEXISTS" => hashes::cmd_hexists(store, args),
        b"HLEN" => hashes::cmd_hlen(store, args),
        b"HKEYS" => hashes::cmd_hkeys(store, args),
        b"HVALS" => hashes::cmd_hvals(store, args),
        b"HINCRBY" => hashes::cmd_hincrby(store, args),

        b"SADD" => sets::cmd_sadd(store, args),
        b"SMEMBERS" => sets::cmd_smembers(store, args),
        b"SREM" => sets::cmd_srem(store, args),
        b"SISMEMBER" => sets::cmd_sismember(store, args),
        b"SCARD" => sets::cmd_scard(store, args),
        b"SUNION" => sets::cmd_sunion(store, args),
        b"SINTER" => sets::cmd_sinter(store, args),
        b"SDIFF" => sets::cmd_sdiff(store, args),
        b"SMOVE" => sets::cmd_smove(store, args),
        b"SPOP" => sets::cmd_spop(store, args),

        _ => {
            let s = String::from_utf8_lossy(cmd_name);
            RespValue::error(format!("ERR unknown command '{}'", s.to_lowercase()))
        }
    }
}
