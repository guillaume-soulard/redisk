use crate::server::{RediskKeyValue, RediskValue, TTL};
use std::collections::HashMap;
use std::ops::Add;
use std::time::{Duration, SystemTime};

pub struct RediskMap {
    map: HashMap<String, RediskKeyValue>,
}

impl RediskMap {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item=(&String, &RediskKeyValue)> {
        let now = get_now();
        self.map.iter()
            .filter(move |(_, v)| v.ttl.is_none() || v.ttl.unwrap() > now)
    }

    pub fn mount(&mut self, key: String) -> Option<RediskKeyValue> {
        match self.map.get_mut(&key) {
            Some(v) => {
                v.mounted = true;
                Some(v.clone())
            }
            None => {
                let val = RediskKeyValue {
                    mounted: true,
                    offset: None,
                    ttl: None,
                    deleted: false,
                    value: vec![],
                    storage_address: None,
                };
                self.map.insert(key, val.clone());
                Some(val)
            }
        }
    }

    pub fn is_mounted(&self, key: &str) -> bool {
        self.map.get(key).map_or(false, |v| v.mounted)
    }

    pub fn unmount(&mut self, key: &str) -> Option<RediskKeyValue> {
        let value = self.map.get_mut(key);
        match value {
            Some(v) => {
                (*v).mounted = false;
                Some(v.clone())
            },
            None => None,
        }
    }

    pub fn set(&mut self, key: String, value: RediskValue, ttl: TTL) -> Option<RediskKeyValue> {
        let expiration = get_next_timestamp_by_duration(ttl);
        match self.map.get_mut(&key) {
            Some(v) => {
                v.ttl = expiration;
                v.value = value;
                None
            },
            None => {
                self.map.insert(key, RediskKeyValue {
                    mounted: false,
                    offset: None,
                    ttl: expiration,
                    deleted: false,
                    value,
                    storage_address: None,
                });
                None
            }
        }
    }

    pub fn get(&mut self, key: String) -> Option<RediskKeyValue> {
        if let Some(existing) = self.map.get(&key) {
            if let Some(ttl) = existing.ttl {
                if ttl < get_now() {
                    self.map.remove(&key);
                    return None;
                }
            }
            return Some(existing.clone());
        }
        None
    }

    pub fn delete(&mut self, key: String) -> Option<RediskKeyValue> {
        self.map.remove(&key)
    }

    pub fn expire(&mut self, key: String, ttl: TTL) -> Option<RediskKeyValue> {
        match self.map.get_mut(&key) {
            Some(v) => {
                let expiration = get_next_timestamp_by_duration(ttl);
                v.ttl = expiration;
                Some(v.clone())
            },
            None => None
        }
    }

    pub fn persist(&mut self, key: String) -> Option<RediskKeyValue> {
        match self.map.get_mut(&key) {
            Some(v) => {
                v.ttl = None;
                Some(v.clone())
            },
            None => None
        }
    }
}

fn get_now() -> u64 {
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs()
}

fn get_next_timestamp_by_duration(ttl: TTL) -> TTL {
    match ttl {
        Some(t) => {
            Some(SystemTime::now()
                .add(Duration::from_secs(t))
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs())
        },
        None => None
    }
}