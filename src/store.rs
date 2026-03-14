/// In-memory data store with TTL expiry — lock-free via DashMap.
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use dashmap::DashMap;
use tokio::time::{interval, Duration};
use bytes::Bytes;

#[inline]
pub fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs_f64()
}

#[derive(Debug, Clone)]
pub enum Value {
    String(Bytes),
    List(VecDeque<Bytes>),
    Hash(HashMap<Box<str>, Bytes>),
    Set(HashSet<Box<str>>),
}

impl Value {
    #[inline]
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::String(_) => "string",
            Value::List(_) => "list",
            Value::Hash(_) => "hash",
            Value::Set(_) => "set",
        }
    }

    #[inline]
    pub fn collection_len(&self) -> usize {
        match self {
            Value::String(v) => v.len(),
            Value::List(v) => v.len(),
            Value::Hash(v) => v.len(),
            Value::Set(v) => v.len(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub value: Value,
    pub expires_at: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct Store {
    data: Arc<DashMap<Box<str>, Entry>>,
}

impl Store {
    pub fn new() -> Self {
        Store {
            data: Arc::new(DashMap::with_capacity(1024)),
        }
    }

    pub fn start_expiry_sweep(&self) {
        let data = self.data.clone();
        tokio::spawn(async move {
            let mut tick = interval(Duration::from_millis(100));
            loop {
                tick.tick().await;
                let n = now();
                data.retain(|_, entry: &mut Entry| {
                    entry.expires_at.map_or(true, |exp| exp > n)
                });
            }
        });
    }

    #[inline]
    fn check_expired(&self, key: &str) -> bool {
        if let Some(entry) = self.data.get(key) {
            if let Some(exp) = entry.expires_at {
                if exp <= now() {
                    drop(entry);
                    self.data.remove(key);
                    return true;
                }
            }
        }
        false
    }

    #[inline]
    pub fn get_type(&self, key: &str) -> &'static str {
        self.check_expired(key);
        match self.data.get(key) {
            Some(e) => e.value.type_name(),
            None => "none",
        }
    }

    #[inline]
    pub fn set_value(&self, key: Box<str>, value: Value, expires_at: Option<f64>) {
        self.data.insert(key, Entry { value, expires_at });
    }

    #[inline]
    pub fn delete(&self, key: &str) -> bool {
        self.check_expired(key);
        self.data.remove(key).is_some()
    }

    #[inline]
    pub fn exists(&self, key: &str) -> bool {
        self.check_expired(key);
        self.data.contains_key(key)
    }

    pub fn keys_matching(&self, pattern: &str) -> Vec<Box<str>> {
        let n = now();
        self.data.retain(|_, e| e.expires_at.map_or(true, |exp| exp > n));
        self.data.iter().map(|r| r.key().clone()).filter(|k| glob_match(pattern, k)).collect()
    }

    pub fn flush(&self) {
        self.data.clear();
    }

    pub fn dbsize(&self) -> usize {
        let n = now();
        self.data.retain(|_, e| e.expires_at.map_or(true, |exp| exp > n));
        self.data.len()
    }

    pub fn set_expiry(&self, key: &str, expires_at: f64) -> bool {
        self.check_expired(key);
        if let Some(mut entry) = self.data.get_mut(key) {
            entry.expires_at = Some(expires_at);
            true
        } else {
            false
        }
    }

    pub fn get_expiry(&self, key: &str) -> f64 {
        self.check_expired(key);
        match self.data.get(key) {
            None => -2.0,
            Some(e) => match e.expires_at {
                None => -1.0,
                Some(exp) => exp,
            },
        }
    }

    pub fn persist(&self, key: &str) -> bool {
        self.check_expired(key);
        if let Some(mut entry) = self.data.get_mut(key) {
            if entry.expires_at.is_some() {
                entry.expires_at = None;
                return true;
            }
        }
        false
    }

    #[inline]
    pub fn remove_if_empty(&self, key: &str) {
        if let Some(entry) = self.data.get(key) {
            if entry.value.collection_len() == 0 {
                drop(entry);
                self.data.remove(key);
            }
        }
    }

    // --- Accessors that work with DashMap refs ---

    /// Execute a closure with read access to an entry. Returns None if key doesn't exist.
    #[inline]
    pub fn with_entry<F, R>(&self, key: &str, f: F) -> Option<R>
    where F: FnOnce(&Entry) -> R {
        self.check_expired(key);
        self.data.get(key).map(|r| f(r.value()))
    }

    /// Execute a closure with mutable access to an entry. Returns None if key doesn't exist.
    #[inline]
    pub fn with_entry_mut<F, R>(&self, key: &str, f: F) -> Option<R>
    where F: FnOnce(&mut Entry) -> R {
        self.check_expired(key);
        self.data.get_mut(key).map(|mut r| f(r.value_mut()))
    }

    /// Get or create a list, then execute closure on it.
    pub fn with_list<F, R>(&self, key: &str, f: F) -> Result<R, &'static str>
    where F: FnOnce(&mut VecDeque<Bytes>) -> R {
        self.check_expired(key);
        if !self.data.contains_key(key) {
            self.data.insert(
                key.into(),
                Entry { value: Value::List(VecDeque::new()), expires_at: None },
            );
        }
        let mut entry = self.data.get_mut(key).unwrap();
        match &mut entry.value {
            Value::List(d) => Ok(f(d)),
            _ => Err("WRONGTYPE Operation against a key holding the wrong kind of value"),
        }
    }

    /// Get or create a hash, then execute closure on it.
    pub fn with_hash<F, R>(&self, key: &str, f: F) -> Result<R, &'static str>
    where F: FnOnce(&mut HashMap<Box<str>, Bytes>) -> R {
        self.check_expired(key);
        if !self.data.contains_key(key) {
            self.data.insert(
                key.into(),
                Entry { value: Value::Hash(HashMap::new()), expires_at: None },
            );
        }
        let mut entry = self.data.get_mut(key).unwrap();
        match &mut entry.value {
            Value::Hash(h) => Ok(f(h)),
            _ => Err("WRONGTYPE Operation against a key holding the wrong kind of value"),
        }
    }

    /// Get or create a set, then execute closure on it.
    pub fn with_set<F, R>(&self, key: &str, f: F) -> Result<R, &'static str>
    where F: FnOnce(&mut HashSet<Box<str>>) -> R {
        self.check_expired(key);
        if !self.data.contains_key(key) {
            self.data.insert(
                key.into(),
                Entry { value: Value::Set(HashSet::new()), expires_at: None },
            );
        }
        let mut entry = self.data.get_mut(key).unwrap();
        match &mut entry.value {
            Value::Set(s) => Ok(f(s)),
            _ => Err("WRONGTYPE Operation against a key holding the wrong kind of value"),
        }
    }

    /// Rename: remove old key, insert under new key.
    pub fn rename(&self, old: &str, new_key: Box<str>) -> Option<()> {
        self.check_expired(old);
        let (_, entry) = self.data.remove(old)?;
        self.data.insert(new_key, entry);
        Some(())
    }
}

fn glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    glob_inner(&p, &t)
}

fn glob_inner(p: &[char], t: &[char]) -> bool {
    match (p.first(), t.first()) {
        (None, None) => true,
        (Some('*'), _) => glob_inner(&p[1..], t) || (!t.is_empty() && glob_inner(p, &t[1..])),
        (Some('?'), Some(_)) => glob_inner(&p[1..], &t[1..]),
        (Some(pc), Some(tc)) if *pc == *tc => glob_inner(&p[1..], &t[1..]),
        _ => false,
    }
}
