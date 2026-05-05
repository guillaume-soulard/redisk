mod redisk_map;

use crate::cache::redisk_map::RediskMap;
use crate::server::{RediskKeyValue, RediskValue, TTL};

pub struct CacheLayer {
    cache: Vec<RediskMap>,
}

impl CacheLayer {
    pub fn new(nb_db: u32) -> Self {
        Self {
            cache: (0..nb_db).map(|db| RediskMap::new(db)).collect(),
        }
    }

    pub fn iter(&self, db: u32) -> Box<dyn Iterator<Item = (&String, &RediskKeyValue)> + '_> {
        match self.cache.get(db as usize) {
            Some(map) => Box::new(map.iter()),
            None => Box::new(std::iter::empty()),
        }
    }

    pub fn mount(&mut self, db: u32, key: &str) -> Option<RediskKeyValue> {
        match self.cache.get_mut(db as usize) {
            Some(map) => map.mount(key.to_string()),
            None => None,
        }
    }

    pub fn unmount(&mut self, db: u32, key: &str) -> Option<RediskKeyValue> {
        match self.cache.get_mut(db as usize) {
            Some(cache) => {
                cache.unmount(key)
            },
            None => None,
        }
    }

    pub fn get(&mut self, db: u32, key: &str) -> Option<RediskKeyValue> {
        match self.cache.get_mut(db as usize) {
            Some(cache) => {
                cache.get(key.to_string())
            },
            None => None,
        }
    }

    pub fn set(&mut self, db: u32, key: String, value: RediskValue, ttl: TTL) -> Option<RediskKeyValue> {
        match self.cache.get_mut(db as usize) {
            Some(cache) => {
                cache.set(key.to_string(), value, ttl)
            },
            None => None,
        }
    }

    pub fn delete(&mut self, db: u32, key: &str) -> Option<RediskKeyValue> {
        match self.cache.get_mut(db as usize) {
            Some(cache) => {
                cache.delete(key.to_string())
            },
            None => None,
        }
    }

    pub fn expire(&mut self, db: u32, key: &str, ttl: TTL) -> Option<RediskKeyValue> {
        match self.cache.get_mut(db as usize) {
            Some(cache) => {
                cache.expire(key.to_string(), ttl)
            },
            None => None,
        }
    }

    pub fn persist(&mut self, db: u32, key: &str) -> Option<RediskKeyValue> {
        match self.cache.get_mut(db as usize) {
            Some(cache) => {
                cache.persist(key.to_string())
            },
            None => None,
        }
    }

    pub fn db_size(&self, db: u32) -> usize {
        self.cache.len()
    }
}
