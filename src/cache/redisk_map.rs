use std::collections::{BTreeMap, HashMap, HashSet};
use std::ops::Add;
use std::time::{Duration, SystemTime};

pub struct RediskMap {
    map: HashMap<String, (Option<u64>, Vec<u8>)>,
    ttl_map: BTreeMap<u64, HashSet<String>>,
}

impl RediskMap {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
            ttl_map: BTreeMap::new(),
        }
    }

    pub fn put(&mut self, key: String, value: Vec<u8>, expire_seconds: Option<u64>) -> Option<Vec<u8>> {
        let existing = self.map.insert(key.clone(), (expire_seconds, value));
        if !self.ttl_map.contains_key(&expire_seconds) {
            self.ttl_map.insert(expire_seconds, HashSet::new());
        }
        let new_expiration = self.get_next_timestamp_by_duration(expire_seconds);
        let hashset = self.ttl_map.get_mut(&new_expiration);
        match hashset {
            Some(set) => {
                set.insert(key);
            }
            None => {
                let mut s = self.ttl_map.insert(new_expiration, HashSet::new()).unwrap();
                s.insert(key);
            }
        }
        match existing {
            Some((_, existing_value)) => Some(existing_value),
            None => None,
        }
    }

    pub fn get(&mut self, key: String) -> Option<Vec<u8>> {
        let existing = self.map.get(&key);
        match existing {
            Some(value) => {
                if self.get_now() > value.0 {
                    self.map.remove(&key);
                    None
                } else {
                    Some(value.1.clone())
                }
            },
            None => None,
        }
    }

    pub fn remove(&mut self, key: String) -> Option<Vec<u8>> {
        let existing = self.map.remove(&key);
        match existing {
            Some((ttl, existing_value)) => {
                self.ttl_map.get_mut(&ttl).unwrap().remove(&key);
                Some(existing_value)
            },
            None => None,
        }
    }

    pub fn clear(&mut self) {
        self.map.clear();
        self.ttl_map.clear();
    }

    pub fn size(&self) -> usize {
        self.map.len()
    }

    pub fn expire(&mut self, key: String, expire_seconds: u64) -> Option<Vec<u8>> {
        let existing = self.map.get(&key);
        match existing {
            Some((existing_expire_seconds, existing_value)) => {
                self.ttl_map.get_mut(&existing_expire_seconds).unwrap().remove(&key);
                if !self.ttl_map.contains_key(&expire_seconds) {
                    self.ttl_map.insert(expire_seconds, HashSet::new());
                }
                self.ttl_map.get_mut(&expire_seconds).unwrap().insert(key.clone());
                let map_value = self.map.insert(key, (expire_seconds, existing_value.clone()));
                map_value.map(|(_, value)| value.clone())
            }
            None => None,
        }
    }

    pub fn persist(&mut self, key: String) -> Option<Vec<u8>> {
        let existing = self.map.get(&key);
        match existing {
            Some((existing_expire_seconds, existing_value)) => {
                if let Some(ttl_set) = self.ttl_map.get_mut(&existing_expire_seconds) {
                    ttl_set.remove(&key);
                }
                self.map.insert(key, (0, existing_value.clone()))
                    .map(|(_, value)| value.clone())
            },
            None => None,
        }
    }

    pub fn ttl(&mut self, key: String) -> Option<u64> {
        let existing = self.map.get(&key);
        match existing {
            Some((existing_expire_seconds, _)) => {
                Some(*existing_expire_seconds)
            },
            None => None,
        }
    }

    fn get_now(&self) -> u64 {
        SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs()
    }

    fn get_next_timestamp_by_duration(&self, expire_seconds: u64) -> u64 {
        SystemTime::now()
            .add(Duration::from_secs(expire_seconds))
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }
}