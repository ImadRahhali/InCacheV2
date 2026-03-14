use bytes::Bytes;
use rustc_hash::FxHashSet as HashSet;
use crate::protocol::{Command, RespValue};
use crate::store::{Store, Value};

#[inline(always)]
fn arg_str<'a>(cmd: &Command, i: usize, buf: &'a [u8]) -> &'a str { unsafe { std::str::from_utf8_unchecked(cmd.arg(i, buf)) } }

pub fn cmd_sadd(store: &mut Store, cmd: &Command, buf: &[u8]) -> RespValue {
    let key = arg_str(cmd, 1, buf);
    match store.get_or_create_set(key) {
        Err(e) => RespValue::error(e.into()),
        Ok(s) => {
            let mut added = 0i64;
            for i in 2..cmd.argc() { if s.insert(arg_str(cmd, i, buf).into()) { added += 1; } }
            RespValue::Integer(added)
        }
    }
}

pub fn cmd_smembers(store: &mut Store, cmd: &Command, buf: &[u8]) -> RespValue {
    match store.get_entry(arg_str(cmd, 1, buf)) {
        None => RespValue::Array(vec![]),
        Some(e) => match &e.value {
            Value::Set(s) => RespValue::Array(s.iter().map(|v| RespValue::bulk_from(v.as_bytes())).collect()),
            _ => RespValue::wrongtype(),
        },
    }
}

pub fn cmd_srem(store: &mut Store, cmd: &Command, buf: &[u8]) -> RespValue {
    let key = arg_str(cmd, 1, buf);
    let c = match store.get_entry_mut(key) {
        None => return RespValue::Integer(0),
        Some(e) => match &mut e.value {
            Value::Set(s) => { let mut c = 0i64; for i in 2..cmd.argc() { if s.remove(arg_str(cmd, i, buf)) { c += 1; } } c }
            _ => return RespValue::wrongtype(),
        },
    };
    store.remove_if_empty(key);
    RespValue::Integer(c)
}

pub fn cmd_sismember(store: &mut Store, cmd: &Command, buf: &[u8]) -> RespValue {
    match store.get_entry(arg_str(cmd, 1, buf)) {
        None => RespValue::Integer(0),
        Some(e) => match &e.value {
            Value::Set(s) => RespValue::Integer(if s.contains(arg_str(cmd, 2, buf)) { 1 } else { 0 }),
            _ => RespValue::wrongtype(),
        },
    }
}

pub fn cmd_scard(store: &mut Store, cmd: &Command, buf: &[u8]) -> RespValue {
    match store.get_entry(arg_str(cmd, 1, buf)) {
        None => RespValue::Integer(0),
        Some(e) => match &e.value { Value::Set(s) => RespValue::Integer(s.len() as i64), _ => RespValue::wrongtype() },
    }
}

fn get_members(store: &mut Store, key: &str) -> HashSet<Box<str>> {
    match store.get_entry(key) {
        None => HashSet::default(),
        Some(e) => match &e.value { Value::Set(s) => s.clone(), _ => HashSet::default() },
    }
}

fn set_resp(s: HashSet<Box<str>>) -> RespValue {
    RespValue::Array(s.into_iter().map(|v| RespValue::BulkString(Bytes::from(String::from(v)))).collect())
}

pub fn cmd_sunion(store: &mut Store, cmd: &Command, buf: &[u8]) -> RespValue {
    let mut r = HashSet::default();
    for i in 1..cmd.argc() { r.extend(get_members(store, arg_str(cmd, i, buf))); }
    set_resp(r)
}

pub fn cmd_sinter(store: &mut Store, cmd: &Command, buf: &[u8]) -> RespValue {
    let mut r: Option<HashSet<Box<str>>> = None;
    for i in 1..cmd.argc() {
        let m = get_members(store, arg_str(cmd, i, buf));
        r = Some(match r { None => m, Some(r) => r.intersection(&m).cloned().collect() });
    }
    set_resp(r.unwrap_or_default())
}

pub fn cmd_sdiff(store: &mut Store, cmd: &Command, buf: &[u8]) -> RespValue {
    let mut r: Option<HashSet<Box<str>>> = None;
    for i in 1..cmd.argc() {
        let m = get_members(store, arg_str(cmd, i, buf));
        r = Some(match r { None => m, Some(r) => r.difference(&m).cloned().collect() });
    }
    set_resp(r.unwrap_or_default())
}

pub fn cmd_smove(store: &mut Store, cmd: &Command, buf: &[u8]) -> RespValue {
    let src = arg_str(cmd, 1, buf);
    let dst = arg_str(cmd, 2, buf);
    let member: Box<str> = arg_str(cmd, 3, buf).into();
    let removed = match store.get_entry_mut(src) {
        None => return RespValue::Integer(0),
        Some(e) => match &mut e.value { Value::Set(s) => s.remove(&*member), _ => return RespValue::wrongtype() },
    };
    if !removed { return RespValue::Integer(0); }
    store.remove_if_empty(src);
    match store.get_or_create_set(dst) {
        Ok(s) => { s.insert(member); RespValue::Integer(1) }
        Err(e) => RespValue::error(e.into()),
    }
}

pub fn cmd_spop(store: &mut Store, cmd: &Command, buf: &[u8]) -> RespValue {
    let key = arg_str(cmd, 1, buf);
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
