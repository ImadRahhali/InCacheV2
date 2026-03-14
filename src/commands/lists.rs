use bytes::Bytes;
use crate::protocol::RespValue;
use crate::store::{Store, Value};

#[inline(always)]
fn to_str(b: &Bytes) -> &str { unsafe { std::str::from_utf8_unchecked(b) } }
#[inline(always)]
fn parse_i64(b: &Bytes) -> i64 { to_str(b).parse().unwrap_or(0) }

pub fn cmd_lpush(store: &mut Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    match store.get_or_create_list(key) {
        Err(e) => RespValue::error(e.into()),
        Ok(d) => { for v in &args[1..] { d.push_front(v.clone()); } RespValue::Integer(d.len() as i64) }
    }
}

pub fn cmd_rpush(store: &mut Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    match store.get_or_create_list(key) {
        Err(e) => RespValue::error(e.into()),
        Ok(d) => { for v in &args[1..] { d.push_back(v.clone()); } RespValue::Integer(d.len() as i64) }
    }
}

pub fn cmd_lpop(store: &mut Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    let val = match store.get_entry_mut(key) {
        None => return RespValue::Null,
        Some(e) => match &mut e.value {
            Value::List(d) => d.pop_front(),
            _ => return RespValue::wrongtype(),
        },
    };
    store.remove_if_empty(key);
    val.map_or(RespValue::Null, RespValue::bulk)
}

pub fn cmd_rpop(store: &mut Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    let val = match store.get_entry_mut(key) {
        None => return RespValue::Null,
        Some(e) => match &mut e.value {
            Value::List(d) => d.pop_back(),
            _ => return RespValue::wrongtype(),
        },
    };
    store.remove_if_empty(key);
    val.map_or(RespValue::Null, RespValue::bulk)
}

pub fn cmd_lrange(store: &mut Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    let start = parse_i64(&args[1]);
    let stop = parse_i64(&args[2]);
    match store.get_entry(key) {
        None => RespValue::Array(vec![]),
        Some(e) => match &e.value {
            Value::List(d) => {
                let len = d.len() as i64;
                let s = if start < 0 { (len + start).max(0) } else { start } as usize;
                let e = if stop < 0 { len + stop } else { stop } as usize;
                if s > e || s >= d.len() { return RespValue::Array(vec![]); }
                let e = e.min(d.len() - 1);
                RespValue::Array(d.iter().skip(s).take(e - s + 1).map(|v| RespValue::bulk(v.clone())).collect())
            }
            _ => RespValue::wrongtype(),
        },
    }
}

pub fn cmd_llen(store: &mut Store, args: &[Bytes]) -> RespValue {
    match store.get_entry(to_str(&args[0])) {
        None => RespValue::Integer(0),
        Some(e) => match &e.value { Value::List(d) => RespValue::Integer(d.len() as i64), _ => RespValue::wrongtype() },
    }
}

pub fn cmd_lindex(store: &mut Store, args: &[Bytes]) -> RespValue {
    let mut index = parse_i64(&args[1]);
    match store.get_entry(to_str(&args[0])) {
        None => RespValue::Null,
        Some(e) => match &e.value {
            Value::List(d) => {
                if index < 0 { index += d.len() as i64; }
                if index < 0 || index as usize >= d.len() { RespValue::Null }
                else { RespValue::bulk(d[index as usize].clone()) }
            }
            _ => RespValue::wrongtype(),
        },
    }
}

pub fn cmd_lset(store: &mut Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    let mut index = parse_i64(&args[1]);
    let value = args[2].clone();
    match store.get_entry_mut(key) {
        None => RespValue::error("ERR no such key".into()),
        Some(e) => match &mut e.value {
            Value::List(d) => {
                if index < 0 { index += d.len() as i64; }
                if index < 0 || index as usize >= d.len() { RespValue::error("ERR index out of range".into()) }
                else { d[index as usize] = value; RespValue::ok() }
            }
            _ => RespValue::wrongtype(),
        },
    }
}

pub fn cmd_linsert(store: &mut Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    let before = args[1][0] | 0x20 == b'b';
    let pivot = &args[2];
    let value = args[3].clone();
    match store.get_entry_mut(key) {
        None => RespValue::Integer(0),
        Some(e) => match &mut e.value {
            Value::List(d) => {
                for i in 0..d.len() {
                    if &d[i] == pivot {
                        d.insert(if before { i } else { i + 1 }, value);
                        return RespValue::Integer(d.len() as i64);
                    }
                }
                RespValue::Integer(-1)
            }
            _ => RespValue::wrongtype(),
        },
    }
}

pub fn cmd_lrem(store: &mut Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    let count = parse_i64(&args[1]);
    let value = &args[2];
    let removed = match store.get_entry_mut(key) {
        None => return RespValue::Integer(0),
        Some(e) => match &mut e.value {
            Value::List(d) => {
                let mut removed = 0i64;
                if count > 0 {
                    let mut i = 0;
                    while i < d.len() {
                        if &d[i] == value && removed < count { d.remove(i); removed += 1; }
                        else { i += 1; }
                    }
                } else if count < 0 {
                    let limit = count.unsigned_abs() as i64;
                    let mut i = d.len();
                    while i > 0 { i -= 1; if &d[i] == value && removed < limit { d.remove(i); removed += 1; } }
                } else {
                    d.retain(|v| if v == value { removed += 1; false } else { true });
                }
                removed
            }
            _ => return RespValue::wrongtype(),
        },
    };
    store.remove_if_empty(key);
    RespValue::Integer(removed)
}
