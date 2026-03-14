use bytes::Bytes;
use std::collections::HashSet;
use crate::protocol::RespValue;
use crate::store::{Store, Value};

#[inline(always)]
fn to_str(b: &Bytes) -> &str { unsafe { std::str::from_utf8_unchecked(b) } }

pub fn cmd_sadd(store: &mut Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    match store.get_or_create_set(key) {
        Err(e) => RespValue::error(e.into()),
        Ok(s) => {
            let mut added = 0i64;
            for a in &args[1..] { if s.insert(to_str(a).into()) { added += 1; } }
            RespValue::Integer(added)
        }
    }
}

pub fn cmd_smembers(store: &mut Store, args: &[Bytes]) -> RespValue {
    match store.get_entry(to_str(&args[0])) {
        None => RespValue::Array(vec![]),
        Some(e) => match &e.value {
            Value::Set(s) => RespValue::Array(s.iter().map(|v| RespValue::bulk_from(v.as_bytes())).collect()),
            _ => RespValue::wrongtype(),
        },
    }
}

pub fn cmd_srem(store: &mut Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    let c = match store.get_entry_mut(key) {
        None => return RespValue::Integer(0),
        Some(e) => match &mut e.value {
            Value::Set(s) => { let mut c = 0i64; for a in &args[1..] { if s.remove(to_str(a)) { c += 1; } } c }
            _ => return RespValue::wrongtype(),
        },
    };
    store.remove_if_empty(key);
    RespValue::Integer(c)
}

pub fn cmd_sismember(store: &mut Store, args: &[Bytes]) -> RespValue {
    match store.get_entry(to_str(&args[0])) {
        None => RespValue::Integer(0),
        Some(e) => match &e.value {
            Value::Set(s) => RespValue::Integer(if s.contains(to_str(&args[1])) { 1 } else { 0 }),
            _ => RespValue::wrongtype(),
        },
    }
}

pub fn cmd_scard(store: &mut Store, args: &[Bytes]) -> RespValue {
    match store.get_entry(to_str(&args[0])) {
        None => RespValue::Integer(0),
        Some(e) => match &e.value { Value::Set(s) => RespValue::Integer(s.len() as i64), _ => RespValue::wrongtype() },
    }
}

fn get_members(store: &mut Store, key: &str) -> HashSet<Box<str>> {
    match store.get_entry(key) {
        None => HashSet::new(),
        Some(e) => match &e.value { Value::Set(s) => s.clone(), _ => HashSet::new() },
    }
}

fn set_resp(s: HashSet<Box<str>>) -> RespValue {
    RespValue::Array(s.into_iter().map(|v| RespValue::BulkString(Bytes::from(String::from(v)))).collect())
}

pub fn cmd_sunion(store: &mut Store, args: &[Bytes]) -> RespValue {
    let mut r = HashSet::new();
    for a in args { r.extend(get_members(store, to_str(a))); }
    set_resp(r)
}

pub fn cmd_sinter(store: &mut Store, args: &[Bytes]) -> RespValue {
    let mut r: Option<HashSet<Box<str>>> = None;
    for a in args {
        let m = get_members(store, to_str(a));
        r = Some(match r { None => m, Some(r) => r.intersection(&m).cloned().collect() });
    }
    set_resp(r.unwrap_or_default())
}

pub fn cmd_sdiff(store: &mut Store, args: &[Bytes]) -> RespValue {
    let mut r: Option<HashSet<Box<str>>> = None;
    for a in args {
        let m = get_members(store, to_str(a));
        r = Some(match r { None => m, Some(r) => r.difference(&m).cloned().collect() });
    }
    set_resp(r.unwrap_or_default())
}

pub fn cmd_smove(store: &mut Store, args: &[Bytes]) -> RespValue {
    let src = to_str(&args[0]);
    let dst = to_str(&args[1]);
    let member: Box<str> = to_str(&args[2]).into();
    let removed = match store.get_entry_mut(src) {
        None => return RespValue::Integer(0),
        Some(e) => match &mut e.value {
            Value::Set(s) => s.remove(&*member),
            _ => return RespValue::wrongtype(),
        },
    };
    if !removed { return RespValue::Integer(0); }
    store.remove_if_empty(src);
    match store.get_or_create_set(dst) {
        Ok(s) => { s.insert(member); RespValue::Integer(1) }
        Err(e) => RespValue::error(e.into()),
    }
}

pub fn cmd_spop(store: &mut Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    let val = match store.get_entry_mut(key) {
        None => return RespValue::Null,
        Some(e) => match &mut e.value {
            Value::Set(s) => {
                if s.is_empty() { return RespValue::Null; }
                let v = s.iter().next().unwrap().clone();
                s.remove(&*v);
                v
            }
            _ => return RespValue::wrongtype(),
        },
    };
    store.remove_if_empty(key);
    RespValue::BulkString(Bytes::from(String::from(val)))
}
