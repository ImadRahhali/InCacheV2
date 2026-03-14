use bytes::Bytes;
use crate::protocol::RespValue;
use crate::store::{now, Store, Value};

#[inline]
fn to_str(b: &Bytes) -> &str {
    std::str::from_utf8(b).unwrap_or("")
}

#[inline]
fn to_box(b: &Bytes) -> Box<str> {
    to_str(b).into()
}

pub fn cmd_set(store: &Store, args: &[Bytes]) -> RespValue {
    if args.len() < 2 {
        return RespValue::error("ERR wrong number of arguments for 'set' command".into());
    }
    let key = to_box(&args[0]);
    let value = args[1].clone();
    let mut expires_at: Option<f64> = None;
    let mut nx = false;
    let mut xx = false;
    let mut i = 2;
    while i < args.len() {
        let mut opt = args[i].to_vec();
        opt.make_ascii_uppercase();
        match opt.as_slice() {
            b"EX" => { expires_at = Some(now() + parse_f64(&args[i+1])); i += 2; }
            b"PX" => { expires_at = Some(now() + parse_f64(&args[i+1]) / 1000.0); i += 2; }
            b"NX" => { nx = true; i += 1; }
            b"XX" => { xx = true; i += 1; }
            _ => { i += 1; }
        }
    }
    if nx && store.exists(&key) { return RespValue::Null; }
    if xx && !store.exists(&key) { return RespValue::Null; }
    store.set_value(key, Value::String(value), expires_at);
    RespValue::ok()
}

pub fn cmd_get(store: &Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    store.with_entry(key, |e| match &e.value {
        Value::String(v) => RespValue::bulk(v.clone()),
        _ => RespValue::wrongtype(),
    }).unwrap_or(RespValue::Null)
}

pub fn cmd_getset(store: &Store, args: &[Bytes]) -> RespValue {
    let key = to_box(&args[0]);
    let new_val = args[1].clone();
    let old = store.with_entry(&key, |e| match &e.value {
        Value::String(v) => RespValue::bulk(v.clone()),
        _ => RespValue::Null,
    }).unwrap_or(RespValue::Null);
    store.set_value(key, Value::String(new_val), None);
    old
}

pub fn cmd_mset(store: &Store, args: &[Bytes]) -> RespValue {
    let mut i = 0;
    while i + 1 < args.len() {
        store.set_value(to_box(&args[i]), Value::String(args[i+1].clone()), None);
        i += 2;
    }
    RespValue::ok()
}

pub fn cmd_mget(store: &Store, args: &[Bytes]) -> RespValue {
    let items: Vec<RespValue> = args.iter().map(|a| {
        store.with_entry(to_str(a), |e| match &e.value {
            Value::String(v) => RespValue::bulk(v.clone()),
            _ => RespValue::Null,
        }).unwrap_or(RespValue::Null)
    }).collect();
    RespValue::Array(items)
}

pub fn cmd_del(store: &Store, args: &[Bytes]) -> RespValue {
    let count = args.iter().filter(|a| store.delete(to_str(a))).count();
    RespValue::Integer(count as i64)
}

pub fn cmd_exists(store: &Store, args: &[Bytes]) -> RespValue {
    let count = args.iter().filter(|a| store.exists(to_str(a))).count();
    RespValue::Integer(count as i64)
}

fn incr_by(store: &Store, key_bytes: &Bytes, amount: i64) -> RespValue {
    let key = to_str(key_bytes);
    // Try to update in-place first
    if let Some(result) = store.with_entry_mut(key, |entry| {
        match &mut entry.value {
            Value::String(v) => {
                match std::str::from_utf8(v).ok().and_then(|s| s.parse::<i64>().ok()) {
                    Some(n) => {
                        let new_val = n + amount;
                        *v = Bytes::from(new_val.to_string());
                        RespValue::Integer(new_val)
                    }
                    None => RespValue::error("ERR value is not an integer or out of range".into()),
                }
            }
            _ => RespValue::wrongtype(),
        }
    }) {
        return result;
    }
    // Key doesn't exist — create it
    store.set_value(key.into(), Value::String(Bytes::from(amount.to_string())), None);
    RespValue::Integer(amount)
}

pub fn cmd_incr(store: &Store, args: &[Bytes]) -> RespValue { incr_by(store, &args[0], 1) }
pub fn cmd_incrby(store: &Store, args: &[Bytes]) -> RespValue { incr_by(store, &args[0], parse_i64(&args[1])) }
pub fn cmd_decr(store: &Store, args: &[Bytes]) -> RespValue { incr_by(store, &args[0], -1) }
pub fn cmd_decrby(store: &Store, args: &[Bytes]) -> RespValue { incr_by(store, &args[0], -parse_i64(&args[1])) }

pub fn cmd_append(store: &Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    let val = &args[1];
    if let Some(result) = store.with_entry_mut(key, |entry| {
        match &mut entry.value {
            Value::String(v) => {
                let mut new = Vec::with_capacity(v.len() + val.len());
                new.extend_from_slice(v);
                new.extend_from_slice(val);
                let len = new.len();
                *v = Bytes::from(new);
                RespValue::Integer(len as i64)
            }
            _ => RespValue::wrongtype(),
        }
    }) {
        return result;
    }
    let len = val.len();
    store.set_value(key.into(), Value::String(val.clone()), None);
    RespValue::Integer(len as i64)
}

pub fn cmd_strlen(store: &Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    store.with_entry(key, |e| match &e.value {
        Value::String(v) => RespValue::Integer(v.len() as i64),
        _ => RespValue::Integer(0),
    }).unwrap_or(RespValue::Integer(0))
}

pub fn cmd_setnx(store: &Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    if store.exists(key) {
        RespValue::Integer(0)
    } else {
        store.set_value(key.into(), Value::String(args[1].clone()), None);
        RespValue::Integer(1)
    }
}

pub fn cmd_setex(store: &Store, args: &[Bytes]) -> RespValue {
    let key = to_box(&args[0]);
    let secs = parse_f64(&args[1]);
    store.set_value(key, Value::String(args[2].clone()), Some(now() + secs));
    RespValue::ok()
}

pub fn cmd_expire(store: &Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    let secs = parse_f64(&args[1]);
    RespValue::Integer(if store.set_expiry(key, now() + secs) { 1 } else { 0 })
}

pub fn cmd_ttl(store: &Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    let exp = store.get_expiry(key);
    if exp == -2.0 { RespValue::Integer(-2) }
    else if exp == -1.0 { RespValue::Integer(-1) }
    else {
        let rem = exp - now();
        if rem <= 0.0 { RespValue::Integer(-2) } else { RespValue::Integer(rem as i64) }
    }
}

pub fn cmd_persist(store: &Store, args: &[Bytes]) -> RespValue {
    RespValue::Integer(if store.persist(to_str(&args[0])) { 1 } else { 0 })
}

pub fn cmd_type(store: &Store, args: &[Bytes]) -> RespValue {
    RespValue::simple_from_str(store.get_type(to_str(&args[0])))
}

pub fn cmd_rename(store: &Store, args: &[Bytes]) -> RespValue {
    let old = to_str(&args[0]);
    let new_key = to_box(&args[1]);
    match store.rename(old, new_key) {
        Some(()) => RespValue::ok(),
        None => RespValue::error("ERR no such key".into()),
    }
}

pub fn cmd_keys(store: &Store, args: &[Bytes]) -> RespValue {
    let pattern = to_str(&args[0]);
    let keys = store.keys_matching(pattern);
    RespValue::Array(keys.into_iter().map(|k| RespValue::BulkString(Bytes::from(String::from(k)))).collect())
}

#[inline]
fn parse_i64(b: &Bytes) -> i64 {
    std::str::from_utf8(b).unwrap_or("0").parse().unwrap_or(0)
}

#[inline]
fn parse_f64(b: &Bytes) -> f64 {
    std::str::from_utf8(b).unwrap_or("0").parse().unwrap_or(0.0)
}
