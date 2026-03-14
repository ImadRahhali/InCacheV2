/// Command dispatcher — maps command name → handler.
pub mod strings;
pub mod lists;
pub mod hashes;
pub mod sets;
pub mod server;

use crate::protocol::RespValue;
use crate::store::Store;

pub fn dispatch(store: &mut Store, cmd: &str, args: &[Vec<u8>]) -> RespValue {
    match cmd {
        // Server
        "PING" => server::cmd_ping(args),
        "ECHO" => server::cmd_echo(args),
        "HELLO" => server::cmd_hello(args),
        "FLUSHALL" | "FLUSHDB" => server::cmd_flush(store),
        "DBSIZE" => server::cmd_dbsize(store),
        "INFO" => server::cmd_info(),
        "SELECT" => server::cmd_select(args),
        "COMMAND" => server::cmd_command(args),
        "CLIENT" => server::cmd_client(args),

        // Strings + keys
        "SET" => strings::cmd_set(store, args),
        "GET" => strings::cmd_get(store, args),
        "GETSET" => strings::cmd_getset(store, args),
        "MSET" => strings::cmd_mset(store, args),
        "MGET" => strings::cmd_mget(store, args),
        "DEL" => strings::cmd_del(store, args),
        "EXISTS" => strings::cmd_exists(store, args),
        "INCR" => strings::cmd_incr(store, args),
        "INCRBY" => strings::cmd_incrby(store, args),
        "DECR" => strings::cmd_decr(store, args),
        "DECRBY" => strings::cmd_decrby(store, args),
        "APPEND" => strings::cmd_append(store, args),
        "STRLEN" => strings::cmd_strlen(store, args),
        "SETNX" => strings::cmd_setnx(store, args),
        "SETEX" => strings::cmd_setex(store, args),
        "EXPIRE" => strings::cmd_expire(store, args),
        "TTL" => strings::cmd_ttl(store, args),
        "PERSIST" => strings::cmd_persist(store, args),
        "TYPE" => strings::cmd_type(store, args),
        "RENAME" => strings::cmd_rename(store, args),
        "KEYS" => strings::cmd_keys(store, args),

        // Lists
        "LPUSH" => lists::cmd_lpush(store, args),
        "RPUSH" => lists::cmd_rpush(store, args),
        "LPOP" => lists::cmd_lpop(store, args),
        "RPOP" => lists::cmd_rpop(store, args),
        "LRANGE" => lists::cmd_lrange(store, args),
        "LLEN" => lists::cmd_llen(store, args),
        "LINDEX" => lists::cmd_lindex(store, args),
        "LSET" => lists::cmd_lset(store, args),
        "LINSERT" => lists::cmd_linsert(store, args),
        "LREM" => lists::cmd_lrem(store, args),

        // Hashes
        "HSET" => hashes::cmd_hset(store, args),
        "HGET" => hashes::cmd_hget(store, args),
        "HMSET" => hashes::cmd_hmset(store, args),
        "HMGET" => hashes::cmd_hmget(store, args),
        "HGETALL" => hashes::cmd_hgetall(store, args),
        "HDEL" => hashes::cmd_hdel(store, args),
        "HEXISTS" => hashes::cmd_hexists(store, args),
        "HLEN" => hashes::cmd_hlen(store, args),
        "HKEYS" => hashes::cmd_hkeys(store, args),
        "HVALS" => hashes::cmd_hvals(store, args),
        "HINCRBY" => hashes::cmd_hincrby(store, args),

        // Sets
        "SADD" => sets::cmd_sadd(store, args),
        "SMEMBERS" => sets::cmd_smembers(store, args),
        "SREM" => sets::cmd_srem(store, args),
        "SISMEMBER" => sets::cmd_sismember(store, args),
        "SCARD" => sets::cmd_scard(store, args),
        "SUNION" => sets::cmd_sunion(store, args),
        "SINTER" => sets::cmd_sinter(store, args),
        "SDIFF" => sets::cmd_sdiff(store, args),
        "SMOVE" => sets::cmd_smove(store, args),
        "SPOP" => sets::cmd_spop(store, args),

        _ => RespValue::Error(format!("ERR unknown command '{}'", cmd.to_lowercase())),
    }
}
