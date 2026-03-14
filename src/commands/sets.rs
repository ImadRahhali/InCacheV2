use crate::protocol::RespValue;
use crate::store::{Store, Value};
use std::collections::HashSet;

fn to_str(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

pub fn cmd_sadd(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    let key = to_str(&args[0]);
    let s = match store.get_or_create_set(&key) {
        Ok(s) => s,
        Err(e) => return RespValue::Error(e.into()),
    };
    let mut added = 0i64;
    for a in &args[1..] {
        if s.insert(to_str(a)) { added += 1; }
    }
    RespValue::Integer(added)
}

pub fn cmd_smembers(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    let key = to_str(&args[0]);
    match store.get_entry(&key) {
        None => RespValue::Array(vec![]),
        Some(e) => match &e.value {
            Value::Set(s) => {
                RespValue::Array(s.iter().map(|v| RespValue::BulkString(v.as_bytes().to_vec())).collect())
            }
            _ => RespValue::Error("WRONGTYPE Operation against a key holding the wrong kind of value".into()),
        },
    }
}

pub fn cmd_srem(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    let key = to_str(&args[0]);
    let count = {
        let entry = match store.get_entry_mut(&key) {
            None => return RespValue::Integer(0),
            Some(e) => e,
        };
        let s = match &mut entry.value {
            Value::Set(s) => s,
            _ => return RespValue::Error("WRONGTYPE Operation against a key holding the wrong kind of value".into()),
        };
        let mut c = 0i64;
        for a in &args[1..] {
            if s.remove(&to_str(a)) { c += 1; }
        }
        c
    };
    store.remove_if_empty(&key);
    RespValue::Integer(count)
}

pub fn cmd_sismember(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    let key = to_str(&args[0]);
    let member = to_str(&args[1]);
    match store.get_entry(&key) {
        None => RespValue::Integer(0),
        Some(e) => match &e.value {
            Value::Set(s) => RespValue::Integer(if s.contains(&member) { 1 } else { 0 }),
            _ => RespValue::Error("WRONGTYPE Operation against a key holding the wrong kind of value".into()),
        },
    }
}

pub fn cmd_scard(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    let key = to_str(&args[0]);
    match store.get_entry(&key) {
        None => RespValue::Integer(0),
        Some(e) => match &e.value {
            Value::Set(s) => RespValue::Integer(s.len() as i64),
            _ => RespValue::Error("WRONGTYPE Operation against a key holding the wrong kind of value".into()),
        },
    }
}

fn get_set_members(store: &mut Store, key: &str) -> HashSet<String> {
    match store.get_entry(key) {
        None => HashSet::new(),
        Some(e) => match &e.value {
            Value::Set(s) => s.clone(),
            _ => HashSet::new(),
        },
    }
}

pub fn cmd_sunion(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    let mut result = HashSet::new();
    for a in args {
        result.extend(get_set_members(store, &to_str(a)));
    }
    RespValue::Array(result.into_iter().map(|v| RespValue::BulkString(v.into_bytes())).collect())
}

pub fn cmd_sinter(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    let mut result: Option<HashSet<String>> = None;
    for a in args {
        let members = get_set_members(store, &to_str(a));
        result = Some(match result {
            None => members,
            Some(r) => r.intersection(&members).cloned().collect(),
        });
    }
    let r = result.unwrap_or_default();
    RespValue::Array(r.into_iter().map(|v| RespValue::BulkString(v.into_bytes())).collect())
}

pub fn cmd_sdiff(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    let mut result: Option<HashSet<String>> = None;
    for a in args {
        let members = get_set_members(store, &to_str(a));
        result = Some(match result {
            None => members,
            Some(r) => r.difference(&members).cloned().collect(),
        });
    }
    let r = result.unwrap_or_default();
    RespValue::Array(r.into_iter().map(|v| RespValue::BulkString(v.into_bytes())).collect())
}

pub fn cmd_smove(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    let src_key = to_str(&args[0]);
    let dst_key = to_str(&args[1]);
    let member = to_str(&args[2]);
    // Remove from source
    let removed = {
        let entry = match store.get_entry_mut(&src_key) {
            None => return RespValue::Integer(0),
            Some(e) => e,
        };
        match &mut entry.value {
            Value::Set(s) => s.remove(&member),
            _ => return RespValue::Error("WRONGTYPE Operation against a key holding the wrong kind of value".into()),
        }
    };
    if !removed {
        return RespValue::Integer(0);
    }
    store.remove_if_empty(&src_key);
    // Add to dest
    let ds = match store.get_or_create_set(&dst_key) {
        Ok(s) => s,
        Err(e) => return RespValue::Error(e.into()),
    };
    ds.insert(member);
    RespValue::Integer(1)
}

pub fn cmd_spop(store: &mut Store, args: &[Vec<u8>]) -> RespValue {
    let key = to_str(&args[0]);
    let val = {
        let entry = match store.get_entry_mut(&key) {
            None => return RespValue::Null,
            Some(e) => e,
        };
        let s = match &mut entry.value {
            Value::Set(s) => s,
            _ => return RespValue::Error("WRONGTYPE Operation against a key holding the wrong kind of value".into()),
        };
        if s.is_empty() {
            return RespValue::Null;
        }
        let v = s.iter().next().unwrap().clone();
        s.remove(&v);
        v
    };
    store.remove_if_empty(&key);
    RespValue::BulkString(val.into_bytes())
}
