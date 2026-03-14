use crate::protocol::RespValue;
use crate::store::{Store, Value};
use std::time::{SystemTime, UNIX_EPOCH};

fn now() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs_f64()
}

fn to_str(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

pub fn cmd_set(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    if args.len() < 2 {
        return RespValue::Error("ERR wrong number of arguments for 'set' command".into());
    }
    let key = to_str(&args[0]);
    let value = args[1].clone();
    let mut expires_at: Option<f64> = None;
    let mut nx = false;
    let mut xx = false;
    let mut i = 2;
    while i < args.len() {
        let opt = to_str(&args[i]).to_uppercase();
        match opt.as_str() {
            "EX" => {
                let secs: f64 = to_str(&args[i + 1]).parse().unwrap_or(0.0);
                expires_at = Some(now() + secs);
                i += 2;
            }
            "PX" => {
                let ms: f64 = to_str(&args[i + 1]).parse().unwrap_or(0.0);
                expires_at = Some(now() + ms / 1000.0);
                i += 2;
            }
            "NX" => { nx = true; i += 1; }
            "XX" => { xx = true; i += 1; }
            _ => { i += 1; }
        }
    }
    if nx && store.exists(&key) { return RespValue::Null; }
    if xx && !store.exists(&key) { return RespValue::Null; }
    store.set_value(key, Value::String(value), expires_at);
    RespValue::SimpleString("OK".into())
}

pub fn cmd_get(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    let key = to_str(&args[0]);
    match store.get_entry(&key) {
        None => RespValue::Null,
        Some(e) => match &e.value {
            Value::String(v) => RespValue::BulkString(v.clone()),
            _ => RespValue::Error("WRONGTYPE Operation against a key holding the wrong kind of value".into()),
        },
    }
}

pub fn cmd_getset(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    let key = to_str(&args[0]);
    let new_val = args[1].clone();
    let old = match store.get_entry(&key) {
        Some(e) => match &e.value {
            Value::String(v) => RespValue::BulkString(v.clone()),
            _ => RespValue::Null,
        },
        None => RespValue::Null,
    };
    store.set_value(key, Value::String(new_val), None);
    old
}

pub fn cmd_mset(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    let mut i = 0;
    while i + 1 < args.len() {
        let key = to_str(&args[i]);
        store.set_value(key, Value::String(args[i + 1].clone()), None);
        i += 2;
    }
    RespValue::SimpleString("OK".into())
}

pub fn cmd_mget(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    let items: Vec<RespValue> = args.iter().map(|a| {
        let key = to_str(a);
        match store.get_entry(&key) {
            Some(e) => match &e.value {
                Value::String(v) => RespValue::BulkString(v.clone()),
                _ => RespValue::Null,
            },
            None => RespValue::Null,
        }
    }).collect();
    RespValue::Array(items)
}

pub fn cmd_del(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    let count = args.iter().filter(|a| store.delete(&to_str(a))).count();
    RespValue::Integer(count as i64)
}

pub fn cmd_exists(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    let count = args.iter().filter(|a| store.exists(&to_str(a))).count();
    RespValue::Integer(count as i64)
}

fn incr_by(store: &mut Store, key_bytes: &[u8], amount: i64) -> RespValue {
    let key = to_str(key_bytes);
    match store.get_entry_mut(&key) {
        None => {
            store.set_value(key, Value::String(amount.to_string().into_bytes()), None);
            RespValue::Integer(amount)
        }
        Some(entry) => match &mut entry.value {
            Value::String(v) => {
                match std::str::from_utf8(v).ok().and_then(|s| s.parse::<i64>().ok()) {
                    Some(n) => {
                        let new_val = n + amount;
                        *v = new_val.to_string().into_bytes();
                        RespValue::Integer(new_val)
                    }
                    None => RespValue::Error("ERR value is not an integer or out of range".into()),
                }
            }
            _ => RespValue::Error("WRONGTYPE Operation against a key holding the wrong kind of value".into()),
        },
    }
}

pub fn cmd_incr(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    incr_by(store, &args[0], 1)
}

pub fn cmd_incrby(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    let amount: i64 = to_str(&args[1]).parse().unwrap_or(0);
    incr_by(store, &args[0], amount)
}

pub fn cmd_decr(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    incr_by(store, &args[0], -1)
}

pub fn cmd_decrby(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    let amount: i64 = to_str(&args[1]).parse().unwrap_or(0);
    incr_by(store, &args[0], -amount)
}

pub fn cmd_append(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    let key = to_str(&args[0]);
    let val = &args[1];
    match store.get_entry_mut(&key) {
        None => {
            let len = val.len();
            store.set_value(key, Value::String(val.clone()), None);
            RespValue::Integer(len as i64)
        }
        Some(entry) => match &mut entry.value {
            Value::String(v) => {
                v.extend_from_slice(val);
                RespValue::Integer(v.len() as i64)
            }
            _ => RespValue::Error("WRONGTYPE Operation against a key holding the wrong kind of value".into()),
        },
    }
}

pub fn cmd_strlen(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    let key = to_str(&args[0]);
    match store.get_entry(&key) {
        None => RespValue::Integer(0),
        Some(e) => match &e.value {
            Value::String(v) => RespValue::Integer(v.len() as i64),
            _ => RespValue::Integer(0),
        },
    }
}

pub fn cmd_setnx(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    let key = to_str(&args[0]);
    if store.exists(&key) {
        RespValue::Integer(0)
    } else {
        store.set_value(key, Value::String(args[1].clone()), None);
        RespValue::Integer(1)
    }
}

pub fn cmd_setex(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    let key = to_str(&args[0]);
    let secs: f64 = to_str(&args[1]).parse().unwrap_or(0.0);
    store.set_value(key, Value::String(args[2].clone()), Some(now() + secs));
    RespValue::SimpleString("OK".into())
}

pub fn cmd_expire(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    let key = to_str(&args[0]);
    let secs: f64 = to_str(&args[1]).parse().unwrap_or(0.0);
    if store.set_expiry(&key, now() + secs) {
        RespValue::Integer(1)
    } else {
        RespValue::Integer(0)
    }
}

pub fn cmd_ttl(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    let key = to_str(&args[0]);
    let exp = store.get_expiry(&key);
    if exp == -2.0 {
        RespValue::Integer(-2)
    } else if exp == -1.0 {
        RespValue::Integer(-1)
    } else {
        let remaining = exp - now();
        if remaining <= 0.0 {
            RespValue::Integer(-2)
        } else {
            RespValue::Integer(remaining as i64)
        }
    }
}

pub fn cmd_persist(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    let key = to_str(&args[0]);
    RespValue::Integer(if store.persist(&key) { 1 } else { 0 })
}

pub fn cmd_type(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    let key = to_str(&args[0]);
    RespValue::SimpleString(store.get_type(&key).into())
}

pub fn cmd_rename(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    let old_key = to_str(&args[0]);
    let new_key = to_str(&args[1]);
    let entry = match store.raw_mut().remove(&old_key) {
        Some(e) => e,
        None => return RespValue::Error("ERR no such key".into()),
    };
    store.raw_mut().insert(new_key, entry);
    RespValue::SimpleString("OK".into())
}

pub fn cmd_keys(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    let pattern = to_str(&args[0]);
    let keys = store.keys_matching(&pattern);
    RespValue::Array(keys.into_iter().map(|k| RespValue::BulkString(k.into_bytes())).collect())
}
