/// In-memory data store — single-threaded, zero locking (like Redis).
use std::collections::VecDeque;
use rustc_hash::FxHashMap as HashMap;
use rustc_hash::FxHashSet as HashSet;
use std::time::{SystemTime, UNIX_EPOCH};
use bytes::Bytes;

#[inline(always)]
pub fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs_f64()
}

#[derive(Debug)]
pub enum Value {
    String(Bytes),
    List(VecDeque<Bytes>),
    Hash(HashMap<Box<str>, Bytes>),
    Set(HashSet<Box<str>>),
}

impl Value {
    #[inline(always)]
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::String(_) => "string",
            Value::List(_) => "list",
            Value::Hash(_) => "hash",
            Value::Set(_) => "set",
        }
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        match self {
            Value::String(v) => v.is_empty(),
            Value::List(v) => v.is_empty(),
            Value::Hash(v) => v.is_empty(),
            Value::Set(v) => v.is_empty(),
        }
    }
}

#[derive(Debug)]
pub struct Entry {
    pub value: Value,
    pub expires_at: Option<f64>,
}

#[derive(Debug)]
pub struct Store {
    data: HashMap<String, Entry>,
}

impl Store {
    pub fn new() -> Self {
        Store { data: HashMap::default() }
    }

    pub fn sweep_expired(&mut self) {
        let n = now();
        self.data.retain(|_, e| e.expires_at.map_or(true, |exp| exp > n));
    }

    #[inline(always)]
    fn check_expired(&mut self, key: &str) -> bool {
        if let Some(entry) = self.data.get(key) {
            if let Some(exp) = entry.expires_at {
                if exp <= now() {
                    self.data.remove(key);
                    return true;
                }
            }
        }
        false
    }

    #[inline(always)]
    pub fn get_entry(&mut self, key: &str) -> Option<&Entry> {
        self.check_expired(key);
        self.data.get(key)
    }

    #[inline(always)]
    pub fn get_entry_mut(&mut self, key: &str) -> Option<&mut Entry> {
        self.check_expired(key);
        self.data.get_mut(key)
    }

    #[inline(always)]
    pub fn get_type(&mut self, key: &str) -> &'static str {
        match self.get_entry(key) {
            Some(e) => e.value.type_name(),
            None => "none",
        }
    }

    #[inline(always)]
    pub fn set_value(&mut self, key: String, value: Value, expires_at: Option<f64>) {
        self.data.insert(key, Entry { value, expires_at });
    }

    #[inline(always)]
    pub fn delete(&mut self, key: &str) -> bool {
        self.check_expired(key);
        self.data.remove(key).is_some()
    }

    #[inline(always)]
    pub fn exists(&mut self, key: &str) -> bool {
        self.check_expired(key);
        self.data.contains_key(key)
    }

    pub fn keys_matching(&mut self, pattern: &str) -> Vec<String> {
        self.sweep_expired();
        self.data.keys().filter(|k| glob_match(pattern, k)).cloned().collect()
    }

    #[inline(always)]
    pub fn flush(&mut self) { self.data.clear(); }

    pub fn dbsize(&mut self) -> usize {
        self.sweep_expired();
        self.data.len()
    }

    pub fn set_expiry(&mut self, key: &str, expires_at: f64) -> bool {
        self.check_expired(key);
        if let Some(entry) = self.data.get_mut(key) {
            entry.expires_at = Some(expires_at);
            true
        } else { false }
    }

    pub fn get_expiry(&mut self, key: &str) -> f64 {
        self.check_expired(key);
        match self.data.get(key) {
            None => -2.0,
            Some(e) => e.expires_at.unwrap_or(-1.0),
        }
    }

    pub fn persist(&mut self, key: &str) -> bool {
        self.check_expired(key);
        if let Some(entry) = self.data.get_mut(key) {
            if entry.expires_at.is_some() {
                entry.expires_at = None;
                return true;
            }
        }
        false
    }

    #[inline(always)]
    pub fn remove_if_empty(&mut self, key: &str) {
        if let Some(entry) = self.data.get(key) {
            if entry.value.is_empty() {
                self.data.remove(key);
            }
        }
    }

    pub fn get_or_create_list(&mut self, key: &str) -> Result<&mut VecDeque<Bytes>, &'static str> {
        self.check_expired(key);
        if !self.data.contains_key(key) {
            self.data.insert(key.to_string(), Entry { value: Value::List(VecDeque::new()), expires_at: None });
        }
        match &mut self.data.get_mut(key).unwrap().value {
            Value::List(d) => Ok(d),
            _ => Err("WRONGTYPE Operation against a key holding the wrong kind of value"),
        }
    }

    pub fn get_or_create_hash(&mut self, key: &str) -> Result<&mut HashMap<Box<str>, Bytes>, &'static str> {
        self.check_expired(key);
        if !self.data.contains_key(key) {
            self.data.insert(key.to_string(), Entry { value: Value::Hash(HashMap::default()), expires_at: None });
        }
        match &mut self.data.get_mut(key).unwrap().value {
            Value::Hash(h) => Ok(h),
            _ => Err("WRONGTYPE Operation against a key holding the wrong kind of value"),
        }
    }

    pub fn get_or_create_set(&mut self, key: &str) -> Result<&mut HashSet<Box<str>>, &'static str> {
        self.check_expired(key);
        if !self.data.contains_key(key) {
            self.data.insert(key.to_string(), Entry { value: Value::Set(HashSet::default()), expires_at: None });
        }
        match &mut self.data.get_mut(key).unwrap().value {
            Value::Set(s) => Ok(s),
            _ => Err("WRONGTYPE Operation against a key holding the wrong kind of value"),
        }
    }

    pub fn rename(&mut self, old: &str, new_key: String) -> bool {
        self.check_expired(old);
        if let Some(entry) = self.data.remove(old) {
            self.data.insert(new_key, entry);
            true
        } else { false }
    }
}

fn glob_match(pattern: &str, text: &str) -> bool {
    glob_inner(pattern.as_bytes(), text.as_bytes())
}

fn glob_inner(p: &[u8], t: &[u8]) -> bool {
    match (p.first(), t.first()) {
        (None, None) => true,
        (Some(b'*'), _) => glob_inner(&p[1..], t) || (!t.is_empty() && glob_inner(p, &t[1..])),
        (Some(b'?'), Some(_)) => glob_inner(&p[1..], &t[1..]),
        (Some(a), Some(b)) if a == b => glob_inner(&p[1..], &t[1..]),
        _ => false,
    }
}
