use crate::server::{RediskValue, TTL};
use std::collections::{HashMap, HashSet};
use std::ops::Add;
use std::time::{Duration, SystemTime};

pub struct RediskMap {
    map: HashMap<String, (TTL, RediskValue)>,
    mounted_keys: HashSet<String>,
}

impl RediskMap {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
            mounted_keys: HashSet::new(),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item=(&String, &(TTL, RediskValue))> {
        self.map.iter()
            .filter(|(_, (expires_at, _))| expires_at.is_none() || expires_at.unwrap() > SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs())
    }

    pub fn mount(&mut self, key: String) {
        self.mounted_keys.insert(key);
    }

    pub fn is_mounted(&self, key: &str) -> bool {
        self.mounted_keys.contains(key)
    }

    pub fn unmount(&mut self, key: &str) -> Option<(TTL, RediskValue)> {
        self.mounted_keys.remove(key);
        self.map.remove(key)
    }

    pub fn set(&mut self, key: String, value: RediskValue, ttl: TTL) -> Option<(TTL, RediskValue)> {
        let expiration = get_next_timestamp_by_duration(ttl);
        self.map.insert(key.clone(), (expiration, value))
    }

    pub fn get(&mut self, key: String) -> Option<(TTL, RediskValue)> {
        let existing = self.map.get(&key);
        match existing {
            Some(existing) => {
                if let Some(ttl) = existing.0 {
                    if ttl < get_now() {
                        // TODO clean expired keys from the map
                        return None;
                    }
                }
                Some(existing.clone())
            }
            None => None,
        }
    }

    pub fn delete(&mut self, key: String) -> Option<RediskValue> {
        self.map.remove(&key).map(|(_, value)| value)
    }

    pub fn expire(&mut self, key: String, ttl: TTL) -> Option<RediskValue> {
        let existing = self.map.get_mut(&key);
        match existing {
            Some(v) => {
                let expiration = get_next_timestamp_by_duration(ttl);
                v.0 = expiration;
                Some(v.1.clone())
            },
            None => None
        }
    }

    pub fn persist(&mut self, key: String) -> Option<RediskValue> {
        let existing = self.map.get_mut(&key);
        match existing {
            Some(v) => {
                v.0 = None;
                Some(v.1.clone())
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