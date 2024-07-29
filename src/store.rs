use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::Instant,
};

struct Data {
    value: String,
    expires_at: Option<Instant>,
}

#[derive(Clone)]
pub struct Store {
    data: Arc<RwLock<HashMap<String, Data>>>,
}

impl Store {
    pub fn new() -> Self {
        Store {
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn get(&self, key: &str) -> Option<String> {
        let data = self.data.read().unwrap();
        data.get(key)
            .and_then(|Data { value, expires_at }| match expires_at {
                Some(expires_at) if expires_at.clone() <= Instant::now() => None,
                _ => Some(value.clone()),
            })
    }

    pub fn set(&self, key: String, value: String, px: Option<u64>) {
        let mut data = self.data.write().unwrap();
        let value = Data {
            value,
            expires_at: px.map(|px| Instant::now() + std::time::Duration::from_millis(px)),
        };
        data.insert(key, value);
    }
}
