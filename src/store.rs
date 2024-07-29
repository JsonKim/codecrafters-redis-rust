use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

#[derive(Clone)]
pub struct Store {
    data: Arc<RwLock<HashMap<String, String>>>,
}

impl Store {
    pub fn new() -> Self {
        Store {
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn get(&self, key: &str) -> Option<String> {
        let data = self.data.read().unwrap();
        data.get(key).cloned()
    }

    pub fn set(&self, key: String, value: String) {
        let mut data = self.data.write().unwrap();
        data.insert(key, value);
    }
}
