use bytes::Bytes;
use crate::protocol::RespValue;
use crate::store::{now, Store, Value};

#[inline(always)]
fn to_str(b: &Bytes) -> &str { unsafe { std::str::from_utf8_unchecked(b) } }

#[inline(always)]
fn parse_i64(b: &Bytes) -> i64 { to_str(b).parse().unwrap_or(0) }

#[inline(always)]
fn parse_f64(b: &Bytes) -> f64 { to_str(b).parse().unwrap_or(0.0) }

pub fn cmd_set(store: &mut Store, args: &[Bytes]) -> RespValue {
    if args.len() < 2 { return RespValue::error("ERR wrong number of arguments for 'set' command".into()); }
    let key = to_str(&args[0]).to_string();
    let value = args[1].clone();
    let mut expires_at: Option<f64> = None;
    let (mut nx, mut xx) = (false, false);
    let mut i = 2;
    while i < args.len() {
        match args[i][0] | 0x20 {
            b'e' => { let v = parse_f64(&args[i+1]); expires_at = Some(now() + if args[i][0] | 0x20 == b'e' && args[i].len() == 2 && (args[i][1] | 0x20) == b'x' { v } else { v / 1000.0 }); i += 2; }
            b'p' => { expires_at = Some(now() + parse_f64(&args[i+1]) / 1000.0); i += 2; }
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
pub fn cmd_get(store: &mut Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    match store.get_entry(key) {
        None => RespValue::Null,
        Some(e) => match &e.value {
            Value::String(v) => RespValue::bulk(v.clone()),
            _ => RespValue::wrongtype(),
        },
    }
}

pub fn cmd_getset(store: &mut Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    let old = match store.get_entry(key) {
        Some(e) => match &e.value { Value::String(v) => RespValue::bulk(v.clone()), _ => RespValue::Null },
        None => RespValue::Null,
    };
    store.set_value(key.to_string(), Value::String(args[1].clone()), None);
    old
}

pub fn cmd_mset(store: &mut Store, args: &[Bytes]) -> RespValue {
    let mut i = 0;
    while i + 1 < args.len() {
        store.set_value(to_str(&args[i]).to_string(), Value::String(args[i+1].clone()), None);
        i += 2;
    }
    RespValue::ok()
}

pub fn cmd_mget(store: &mut Store, args: &[Bytes]) -> RespValue {
    let items: Vec<RespValue> = args.iter().map(|a| {
        match store.get_entry(to_str(a)) {
            Some(e) => match &e.value { Value::String(v) => RespValue::bulk(v.clone()), _ => RespValue::Null },
            None => RespValue::Null,
        }
    }).collect();
    RespValue::Array(items)
}

pub fn cmd_del(store: &mut Store, args: &[Bytes]) -> RespValue {
    let c = args.iter().filter(|a| store.delete(to_str(a))).count();
    RespValue::Integer(c as i64)
}

pub fn cmd_exists(store: &mut Store, args: &[Bytes]) -> RespValue {
    let c = args.iter().filter(|a| store.exists(to_str(a))).count();
    RespValue::Integer(c as i64)
}

fn incr_by(store: &mut Store, key_bytes: &Bytes, amount: i64) -> RespValue {
    let key = to_str(key_bytes);
    if let Some(entry) = store.get_entry_mut(key) {
        match &mut entry.value {
            Value::String(v) => {
                match std::str::from_utf8(v).ok().and_then(|s| s.parse::<i64>().ok()) {
                    Some(n) => { let r = n + amount; *v = Bytes::from(r.to_string()); RespValue::Integer(r) }
                    None => RespValue::error("ERR value is not an integer or out of range".into()),
                }
            }
            _ => RespValue::wrongtype(),
        }
    } else {
        store.set_value(key.to_string(), Value::String(Bytes::from(amount.to_string())), None);
        RespValue::Integer(amount)
    }
}

#[inline(always)] pub fn cmd_incr(store: &mut Store, args: &[Bytes]) -> RespValue { incr_by(store, &args[0], 1) }
#[inline(always)] pub fn cmd_incrby(store: &mut Store, args: &[Bytes]) -> RespValue { incr_by(store, &args[0], parse_i64(&args[1])) }
#[inline(always)] pub fn cmd_decr(store: &mut Store, args: &[Bytes]) -> RespValue { incr_by(store, &args[0], -1) }
#[inline(always)] pub fn cmd_decrby(store: &mut Store, args: &[Bytes]) -> RespValue { incr_by(store, &args[0], -parse_i64(&args[1])) }

pub fn cmd_append(store: &mut Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    let val = &args[1];
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
        store.set_value(key.to_string(), Value::String(val.clone()), None);
        RespValue::Integer(len as i64)
    }
}

pub fn cmd_strlen(store: &mut Store, args: &[Bytes]) -> RespValue {
    match store.get_entry(to_str(&args[0])) {
        Some(e) => match &e.value { Value::String(v) => RespValue::Integer(v.len() as i64), _ => RespValue::Integer(0) },
        None => RespValue::Integer(0),
    }
}

pub fn cmd_setnx(store: &mut Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    if store.exists(key) { RespValue::Integer(0) }
    else { store.set_value(key.to_string(), Value::String(args[1].clone()), None); RespValue::Integer(1) }
}

pub fn cmd_setex(store: &mut Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]).to_string();
    store.set_value(key, Value::String(args[2].clone()), Some(now() + parse_f64(&args[1])));
    RespValue::ok()
}

pub fn cmd_expire(store: &mut Store, args: &[Bytes]) -> RespValue {
    RespValue::Integer(if store.set_expiry(to_str(&args[0]), now() + parse_f64(&args[1])) { 1 } else { 0 })
}

pub fn cmd_ttl(store: &mut Store, args: &[Bytes]) -> RespValue {
    let exp = store.get_expiry(to_str(&args[0]));
    if exp == -2.0 { RespValue::Integer(-2) }
    else if exp == -1.0 { RespValue::Integer(-1) }
    else { let r = exp - now(); if r <= 0.0 { RespValue::Integer(-2) } else { RespValue::Integer(r as i64) } }
}

pub fn cmd_persist(store: &mut Store, args: &[Bytes]) -> RespValue {
    RespValue::Integer(if store.persist(to_str(&args[0])) { 1 } else { 0 })
}

pub fn cmd_type(store: &mut Store, args: &[Bytes]) -> RespValue {
    RespValue::simple_from_str(store.get_type(to_str(&args[0])))
}

pub fn cmd_rename(store: &mut Store, args: &[Bytes]) -> RespValue {
    if store.rename(to_str(&args[0]), to_str(&args[1]).to_string()) { RespValue::ok() }
    else { RespValue::error("ERR no such key".into()) }
}

pub fn cmd_keys(store: &mut Store, args: &[Bytes]) -> RespValue {
    let keys = store.keys_matching(to_str(&args[0]));
    RespValue::Array(keys.into_iter().map(|k| RespValue::BulkString(Bytes::from(k))).collect())
}
