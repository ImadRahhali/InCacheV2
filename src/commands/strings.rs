use bytes::Bytes;
use crate::protocol::{Command, RespValue};
use crate::store::{now, Store, Value};

#[inline(always)]
fn arg_str<'a>(cmd: &Command, i: usize, buf: &'a [u8]) -> &'a str {
    unsafe { std::str::from_utf8_unchecked(cmd.arg(i, buf)) }
}
#[inline(always)]
fn arg_bytes(cmd: &Command, i: usize, buf: &[u8]) -> Bytes {
    Bytes::copy_from_slice(cmd.arg(i, buf))
}
#[inline(always)]
fn arg_i64(cmd: &Command, i: usize, buf: &[u8]) -> i64 {
    arg_str(cmd, i, buf).parse().unwrap_or(0)
}
#[inline(always)]
fn arg_f64(cmd: &Command, i: usize, buf: &[u8]) -> f64 {
    arg_str(cmd, i, buf).parse().unwrap_or(0.0)
}

pub fn cmd_set(store: &mut Store, cmd: &Command, buf: &[u8]) -> RespValue {
    if cmd.argc() < 3 { return RespValue::error("ERR wrong number of arguments for 'set' command".into()); }
    let key = arg_str(cmd, 1, buf).to_string();
    let value = arg_bytes(cmd, 2, buf);
    let mut expires_at: Option<f64> = None;
    let (mut nx, mut xx) = (false, false);
    let mut i = 3;
    while i < cmd.argc() {
        let opt = cmd.arg(i, buf);
        match opt[0] | 0x20 {
            b'e' => { expires_at = Some(now() + arg_f64(cmd, i+1, buf)); i += 2; }
            b'p' => { expires_at = Some(now() + arg_f64(cmd, i+1, buf) / 1000.0); i += 2; }
            b'n' => { nx = true; i += 1; }
            b'x' => { xx = true; i += 1; }
            _ => { i += 1; }
        }
    }
    if nx && store.exists(&key) { return RespValue::Null; }
    if xx && !store.exists(&key) { return RespValue::Null; }
    store.set_value(key, Value::String(value), expires_at);
    RespValue::ok()
}

#[inline(always)]
pub fn cmd_get(store: &mut Store, cmd: &Command, buf: &[u8]) -> RespValue {
    let key = arg_str(cmd, 1, buf);
    match store.get_entry(key) {
        None => RespValue::Null,
        Some(e) => match &e.value { Value::String(v) => RespValue::bulk(v.clone()), _ => RespValue::wrongtype() },
    }
}

pub fn cmd_getset(store: &mut Store, cmd: &Command, buf: &[u8]) -> RespValue {
    let key = arg_str(cmd, 1, buf);
    let old = match store.get_entry(key) {
        Some(e) => match &e.value { Value::String(v) => RespValue::bulk(v.clone()), _ => RespValue::Null },
        None => RespValue::Null,
    };
    store.set_value(key.to_string(), Value::String(arg_bytes(cmd, 2, buf)), None);
    old
}

pub fn cmd_mset(store: &mut Store, cmd: &Command, buf: &[u8]) -> RespValue {
    let mut i = 1;
    while i + 1 < cmd.argc() {
        store.set_value(arg_str(cmd, i, buf).to_string(), Value::String(arg_bytes(cmd, i+1, buf)), None);
        i += 2;
    }
    RespValue::ok()
}

pub fn cmd_mget(store: &mut Store, cmd: &Command, buf: &[u8]) -> RespValue {
    let mut items = Vec::with_capacity(cmd.argc() - 1);
    for i in 1..cmd.argc() {
        items.push(match store.get_entry(arg_str(cmd, i, buf)) {
            Some(e) => match &e.value { Value::String(v) => RespValue::bulk(v.clone()), _ => RespValue::Null },
            None => RespValue::Null,
        });
    }
    RespValue::Array(items)
}

pub fn cmd_del(store: &mut Store, cmd: &Command, buf: &[u8]) -> RespValue {
    let mut c = 0i64;
    for i in 1..cmd.argc() { if store.delete(arg_str(cmd, i, buf)) { c += 1; } }
    RespValue::Integer(c)
}

pub fn cmd_exists(store: &mut Store, cmd: &Command, buf: &[u8]) -> RespValue {
    let mut c = 0i64;
    for i in 1..cmd.argc() { if store.exists(arg_str(cmd, i, buf)) { c += 1; } }
    RespValue::Integer(c)
}

fn incr_by(store: &mut Store, key: &str, amount: i64) -> RespValue {
    if let Some(entry) = store.get_entry_mut(key) {
        match &mut entry.value {
            Value::String(v) => match std::str::from_utf8(v).ok().and_then(|s| s.parse::<i64>().ok()) {
                Some(n) => { let r = n + amount; *v = Bytes::from(r.to_string()); RespValue::Integer(r) }
                None => RespValue::error("ERR value is not an integer or out of range".into()),
            },
            _ => RespValue::wrongtype(),
        }
    } else {
        store.set_value(key.to_string(), Value::String(Bytes::from(amount.to_string())), None);
        RespValue::Integer(amount)
    }
}

pub fn cmd_incr(store: &mut Store, cmd: &Command, buf: &[u8]) -> RespValue { incr_by(store, arg_str(cmd, 1, buf), 1) }
pub fn cmd_incrby(store: &mut Store, cmd: &Command, buf: &[u8]) -> RespValue { incr_by(store, arg_str(cmd, 1, buf), arg_i64(cmd, 2, buf)) }
pub fn cmd_decr(store: &mut Store, cmd: &Command, buf: &[u8]) -> RespValue { incr_by(store, arg_str(cmd, 1, buf), -1) }
pub fn cmd_decrby(store: &mut Store, cmd: &Command, buf: &[u8]) -> RespValue { incr_by(store, arg_str(cmd, 1, buf), -arg_i64(cmd, 2, buf)) }

pub fn cmd_append(store: &mut Store, cmd: &Command, buf: &[u8]) -> RespValue {
    let key = arg_str(cmd, 1, buf);
    let val = cmd.arg(2, buf);
    if let Some(entry) = store.get_entry_mut(key) {
        match &mut entry.value {
            Value::String(v) => {
                let mut new = Vec::with_capacity(v.len() + val.len());
                new.extend_from_slice(v); new.extend_from_slice(val);
                let len = new.len(); *v = Bytes::from(new); RespValue::Integer(len as i64)
            }
            _ => RespValue::wrongtype(),
        }
    } else {
        let len = val.len();
        store.set_value(key.to_string(), Value::String(Bytes::copy_from_slice(val)), None);
        RespValue::Integer(len as i64)
    }
}

pub fn cmd_strlen(store: &mut Store, cmd: &Command, buf: &[u8]) -> RespValue {
    match store.get_entry(arg_str(cmd, 1, buf)) {
        Some(e) => match &e.value { Value::String(v) => RespValue::Integer(v.len() as i64), _ => RespValue::Integer(0) },
        None => RespValue::Integer(0),
    }
}

pub fn cmd_setnx(store: &mut Store, cmd: &Command, buf: &[u8]) -> RespValue {
    let key = arg_str(cmd, 1, buf);
    if store.exists(key) { RespValue::Integer(0) }
    else { store.set_value(key.to_string(), Value::String(arg_bytes(cmd, 2, buf)), None); RespValue::Integer(1) }
}

pub fn cmd_setex(store: &mut Store, cmd: &Command, buf: &[u8]) -> RespValue {
    let key = arg_str(cmd, 1, buf).to_string();
    store.set_value(key, Value::String(arg_bytes(cmd, 3, buf)), Some(now() + arg_f64(cmd, 2, buf)));
    RespValue::ok()
}

pub fn cmd_expire(store: &mut Store, cmd: &Command, buf: &[u8]) -> RespValue {
    RespValue::Integer(if store.set_expiry(arg_str(cmd, 1, buf), now() + arg_f64(cmd, 2, buf)) { 1 } else { 0 })
}

pub fn cmd_ttl(store: &mut Store, cmd: &Command, buf: &[u8]) -> RespValue {
    let exp = store.get_expiry(arg_str(cmd, 1, buf));
    if exp == -2.0 { RespValue::Integer(-2) }
    else if exp == -1.0 { RespValue::Integer(-1) }
    else { let r = exp - now(); if r <= 0.0 { RespValue::Integer(-2) } else { RespValue::Integer(r as i64) } }
}

pub fn cmd_persist(store: &mut Store, cmd: &Command, buf: &[u8]) -> RespValue {
    RespValue::Integer(if store.persist(arg_str(cmd, 1, buf)) { 1 } else { 0 })
}

pub fn cmd_type(store: &mut Store, cmd: &Command, buf: &[u8]) -> RespValue {
    RespValue::simple_from_str(store.get_type(arg_str(cmd, 1, buf)))
}

pub fn cmd_rename(store: &mut Store, cmd: &Command, buf: &[u8]) -> RespValue {
    if store.rename(arg_str(cmd, 1, buf), arg_str(cmd, 2, buf).to_string()) { RespValue::ok() }
    else { RespValue::error("ERR no such key".into()) }
}

pub fn cmd_keys(store: &mut Store, cmd: &Command, buf: &[u8]) -> RespValue {
    let keys = store.keys_matching(arg_str(cmd, 1, buf));
    RespValue::Array(keys.into_iter().map(|k| RespValue::BulkString(Bytes::from(k))).collect())
}
