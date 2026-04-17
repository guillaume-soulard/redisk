mod redisk_map;

use crate::cache::redisk_map::RediskMap;
use crate::server::{RediskValue, TTL};
use std::sync::{Mutex, MutexGuard};

pub struct CacheLayer {
    cache: Vec<RediskMap>,
}

impl CacheLayer {
    pub fn new(nb_db: usize) -> Self {
        Self {
            cache: (0..nb_db).map(|_| Mutex::new(RediskMap::new())).collect(),
        }
    }

    fn get_db(&self, db: u32) -> Option<MutexGuard<RediskMap>> {
        match self.cache.get(db as usize) {
            Some(cache) => {
                match cache.lock() {
                    Ok(cache) => Some(cache),
                    Err(_) => None,
                }
            },
            None => None,
        }
    }

    pub fn iter(&self, db: u32) -> Box<dyn Iterator<Item = (String, (TTL, RediskValue))> + '_> {
        match self.get_db(db) {
            Some(cache) => Box::new(cache.into_iter()), // into_iter consomme le cache et donne les valeurs
            None => Box::new(std::iter::empty()),
        }
    }

    pub fn mount(&self, db: u32, key: &str) {
        match self.get_db(db) {
            Some(mut cache) => {
                cache.mount(key.to_string())
            },
            None => {},
        }
    }

    pub fn unmount(&self, db: u32, key: &str) -> Option<(TTL, RediskValue)> {
        match self.get_db(db) {
            Some(mut cache) => {
                cache.unmount(key)
            },
            None => None,
        }
    }

    pub fn is_mounted(&self, db: u32, key: &str) -> bool {
        match self.get_db(db) {
            Some(cache) => {
                cache.is_mounted(key)
            },
            None => false,
        }
    }

    pub fn get(&self, db: u32, key: &str) -> Option<(TTL, RediskValue)> {
        match self.get_db(db) {
            Some(mut cache) => {
                cache.get(key.to_string())
            },
            None => None,
        }
    }

    pub fn set(&self, db: u32, key: String, value: RediskValue, ttl: TTL) -> Option<(TTL, RediskValue)> {
        match self.get_db(db) {
            Some(mut cache) => {
                cache.set(key.to_string(), value, ttl)
            },
            None => None,
        }
    }

    pub fn delete(&self, db: u32, key: &str) -> Option<RediskValue> {
        match self.get_db(db) {
            Some(mut cache) => {
                cache.delete(key.to_string())
            },
            None => None,
        }
    }

    pub fn expire(&self, db: u32, key: &str, ttl: TTL) -> Option<RediskValue> {
        match self.get_db(db) {
            Some(mut cache) => {
                cache.expire(key.to_string(), ttl)
            },
            None => None,
        }
    }

    pub fn persist(&self, db: u32, key: &str) -> Option<RediskValue> {
        match self.get_db(db) {
            Some(mut cache) => {
                cache.persist(key.to_string())
            },
            None => None,
        }
    }
}
