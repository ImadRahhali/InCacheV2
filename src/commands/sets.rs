use bytes::Bytes;
use std::collections::HashSet;
use crate::protocol::RespValue;
use crate::store::{Store, Value};

#[inline]
fn to_str(b: &Bytes) -> &str { std::str::from_utf8(b).unwrap_or("") }
#[inline]
fn to_box(b: &Bytes) -> Box<str> { to_str(b).into() }

pub fn cmd_sadd(store: &Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    match store.with_set(key, |s| {
        let mut added = 0i64;
        for a in &args[1..] { if s.insert(to_box(a)) { added += 1; } }
        added
    }) {
        Ok(n) => RespValue::Integer(n),
        Err(e) => RespValue::error(e.into()),
    }
}

pub fn cmd_smembers(store: &Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    store.with_entry(key, |e| match &e.value {
        Value::Set(s) => RespValue::Array(s.iter().map(|v| RespValue::BulkString(Bytes::copy_from_slice(v.as_bytes()))).collect()),
        _ => RespValue::wrongtype(),
    }).unwrap_or(RespValue::Array(vec![]))
}

pub fn cmd_srem(store: &Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    let count = store.with_entry_mut(key, |entry| {
        match &mut entry.value {
            Value::Set(s) => {
                let mut c = 0i64;
                for a in &args[1..] { if s.remove(to_str(a)) { c += 1; } }
                Ok(c)
            }
            _ => Err(()),
        }
    });
    match count {
        None => RespValue::Integer(0),
        Some(Ok(c)) => { store.remove_if_empty(key); RespValue::Integer(c) }
        Some(Err(())) => RespValue::wrongtype(),
    }
}

pub fn cmd_sismember(store: &Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    let member = to_str(&args[1]);
    store.with_entry(key, |e| match &e.value {
        Value::Set(s) => RespValue::Integer(if s.contains(member) { 1 } else { 0 }),
        _ => RespValue::wrongtype(),
    }).unwrap_or(RespValue::Integer(0))
}

pub fn cmd_scard(store: &Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    store.with_entry(key, |e| match &e.value {
        Value::Set(s) => RespValue::Integer(s.len() as i64),
        _ => RespValue::wrongtype(),
    }).unwrap_or(RespValue::Integer(0))
}

fn get_set_members(store: &Store, key: &str) -> HashSet<Box<str>> {
    store.with_entry(key, |e| match &e.value {
        Value::Set(s) => s.clone(),
        _ => HashSet::new(),
    }).unwrap_or_default()
}

fn set_to_resp(s: HashSet<Box<str>>) -> RespValue {
    RespValue::Array(s.into_iter().map(|v| RespValue::BulkString(Bytes::from(String::from(v)))).collect())
}

pub fn cmd_sunion(store: &Store, args: &[Bytes]) -> RespValue {
    let mut result = HashSet::new();
    for a in args { result.extend(get_set_members(store, to_str(a))); }
    set_to_resp(result)
}

pub fn cmd_sinter(store: &Store, args: &[Bytes]) -> RespValue {
    let mut result: Option<HashSet<Box<str>>> = None;
    for a in args {
        let members = get_set_members(store, to_str(a));
        result = Some(match result {
            None => members,
            Some(r) => r.intersection(&members).cloned().collect(),
        });
    }
    set_to_resp(result.unwrap_or_default())
}

pub fn cmd_sdiff(store: &Store, args: &[Bytes]) -> RespValue {
    let mut result: Option<HashSet<Box<str>>> = None;
    for a in args {
        let members = get_set_members(store, to_str(a));
        result = Some(match result {
            None => members,
            Some(r) => r.difference(&members).cloned().collect(),
        });
    }
    set_to_resp(result.unwrap_or_default())
}

pub fn cmd_smove(store: &Store, args: &[Bytes]) -> RespValue {
    let src_key = to_str(&args[0]);
    let dst_key = to_str(&args[1]);
    let member = to_box(&args[2]);
    let removed = store.with_entry_mut(src_key, |entry| {
        match &mut entry.value {
            Value::Set(s) => Ok(s.remove(&*member)),
            _ => Err(()),
        }
    });
    match removed {
        None | Some(Ok(false)) => RespValue::Integer(0),
        Some(Err(())) => RespValue::wrongtype(),
        Some(Ok(true)) => {
            store.remove_if_empty(src_key);
            match store.with_set(dst_key, |s| { s.insert(member); }) {
                Ok(()) => RespValue::Integer(1),
                Err(e) => RespValue::error(e.into()),
            }
        }
    }
}

pub fn cmd_spop(store: &Store, args: &[Bytes]) -> RespValue {
    let key = to_str(&args[0]);
    let val = store.with_entry_mut(key, |entry| {
        match &mut entry.value {
            Value::Set(s) => {
                if s.is_empty() { return Ok(None); }
                let v = s.iter().next().unwrap().clone();
                s.remove(&*v);
                Ok(Some(v))
            }
            _ => Err(()),
        }
    });
    match val {
        None => RespValue::Null,
        Some(Err(())) => RespValue::wrongtype(),
        Some(Ok(None)) => RespValue::Null,
        Some(Ok(Some(v))) => {
            store.remove_if_empty(key);
            RespValue::BulkString(Bytes::from(String::from(v)))
        }
    }
}
