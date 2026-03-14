/// In-memory data store with TTL expiry.
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;
use tokio::time::{interval, Duration};

fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs_f64()
}

#[derive(Debug, Clone)]
pub enum Value {
    String(Vec<u8>),
    List(VecDeque<Vec<u8>>),
    Hash(HashMap<String, Vec<u8>>),
    Set(HashSet<String>),
}

impl Value {
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::String(_) => "string",
            Value::List(_) => "list",
            Value::Hash(_) => "hash",
            Value::Set(_) => "set",
        }
    }

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

#[derive(Debug)]
pub struct Store {
    data: HashMap<String, Entry>,
}

pub type SharedStore = Arc<Mutex<Store>>;

pub fn new_shared_store() -> SharedStore {
    Arc::new(Mutex::new(Store {
        data: HashMap::new(),
    }))
}

/// Start the background expiry sweep task (every 100ms).
pub fn start_expiry_sweep(store: SharedStore) {
    tokio::spawn(async move {
        let mut tick = interval(Duration::from_millis(100));
        loop {
            tick.tick().await;
            let n = now();
            let mut s = store.lock().await;
            s.data.retain(|_, entry| {
                entry.expires_at.map_or(true, |exp| exp > n)
            });
        }
    });
}

impl Store {
    fn is_expired(&mut self, key: &str) -> bool {
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

    pub fn get_entry(&mut self, key: &str) -> Option<&Entry> {
        self.is_expired(key);
        self.data.get(key)
    }

    pub fn get_entry_mut(&mut self, key: &str) -> Option<&mut Entry> {
        self.is_expired(key);
        self.data.get_mut(key)
    }

    pub fn get_type(&mut self, key: &str) -> &'static str {
        match self.get_entry(key) {
            Some(e) => e.value.type_name(),
            None => "none",
        }
    }

    pub fn set_value(&mut self, key: String, value: Value, expires_at: Option<f64>) {
        self.data.insert(key, Entry { value, expires_at });
    }

    pub fn delete(&mut self, key: &str) -> bool {
        self.is_expired(key);
        self.data.remove(key).is_some()
    }

    pub fn exists(&mut self, key: &str) -> bool {
        self.is_expired(key);
        self.data.contains_key(key)
    }

    pub fn keys_matching(&mut self, pattern: &str) -> Vec<String> {
        let n = now();
        self.data.retain(|_, e| e.expires_at.map_or(true, |exp| exp > n));
        self.data
            .keys()
            .filter(|k| glob_match(pattern, k))
            .cloned()
            .collect()
    }

    pub fn flush(&mut self) {
        self.data.clear();
    }

    pub fn dbsize(&mut self) -> usize {
        let n = now();
        self.data.retain(|_, e| e.expires_at.map_or(true, |exp| exp > n));
        self.data.len()
    }

    pub fn set_expiry(&mut self, key: &str, expires_at: f64) -> bool {
        self.is_expired(key);
        if let Some(entry) = self.data.get_mut(key) {
            entry.expires_at = Some(expires_at);
            true
        } else {
            false
        }
    }

    pub fn get_expiry(&mut self, key: &str) -> f64 {
        self.is_expired(key);
        match self.data.get(key) {
            None => -2.0,
            Some(e) => match e.expires_at {
                None => -1.0,
                Some(exp) => exp,
            },
        }
    }

    pub fn persist(&mut self, key: &str) -> bool {
        self.is_expired(key);
        if let Some(entry) = self.data.get_mut(key) {
            if entry.expires_at.is_some() {
                entry.expires_at = None;
                return true;
            }
        }
        false
    }

    pub fn remove_if_empty(&mut self, key: &str) {
        if let Some(entry) = self.data.get(key) {
            if entry.value.collection_len() == 0 {
                self.data.remove(key);
            }
        }
    }

    // --- Type-specific getters that auto-create ---

    pub fn get_or_create_list(&mut self, key: &str) -> Result<&mut VecDeque<Vec<u8>>, &'static str> {
        self.is_expired(key);
        if !self.data.contains_key(key) {
            self.data.insert(
                key.to_string(),
                Entry {
                    value: Value::List(VecDeque::new()),
                    expires_at: None,
                },
            );
        }
        match &mut self.data.get_mut(key).unwrap().value {
            Value::List(d) => Ok(d),
            _ => Err("WRONGTYPE Operation against a key holding the wrong kind of value"),
        }
    }

    pub fn get_or_create_hash(&mut self, key: &str) -> Result<&mut HashMap<String, Vec<u8>>, &'static str> {
        self.is_expired(key);
        if !self.data.contains_key(key) {
            self.data.insert(
                key.to_string(),
                Entry {
                    value: Value::Hash(HashMap::new()),
                    expires_at: None,
                },
            );
        }
        match &mut self.data.get_mut(key).unwrap().value {
            Value::Hash(h) => Ok(h),
            _ => Err("WRONGTYPE Operation against a key holding the wrong kind of value"),
        }
    }

    pub fn get_or_create_set(&mut self, key: &str) -> Result<&mut HashSet<String>, &'static str> {
        self.is_expired(key);
        if !self.data.contains_key(key) {
            self.data.insert(
                key.to_string(),
                Entry {
                    value: Value::Set(HashSet::new()),
                    expires_at: None,
                },
            );
        }
        match &mut self.data.get_mut(key).unwrap().value {
            Value::Set(s) => Ok(s),
            _ => Err("WRONGTYPE Operation against a key holding the wrong kind of value"),
        }
    }

    /// Borrow the raw data map (for rename, etc.)
    pub fn raw_mut(&mut self) -> &mut HashMap<String, Entry> {
        &mut self.data
    }
}

/// Simple glob matching supporting * and ?
fn glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    glob_match_inner(&p, &t)
}

fn glob_match_inner(p: &[char], t: &[char]) -> bool {
    match (p.first(), t.first()) {
        (None, None) => true,
        (Some('*'), _) => {
            // Match zero chars or one+ chars
            glob_match_inner(&p[1..], t) || (!t.is_empty() && glob_match_inner(p, &t[1..]))
        }
        (Some('?'), Some(_)) => glob_match_inner(&p[1..], &t[1..]),
        (Some(pc), Some(tc)) if *pc == *tc => glob_match_inner(&p[1..], &t[1..]),
        _ => false,
    }
}
