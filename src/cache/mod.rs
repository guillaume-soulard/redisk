mod redisk_map;

use crate::cache::redisk_map::RediskMap;
use rand::seq::IteratorRandom;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct CacheLayer {
    cache: Vec<Mutex<RediskMap>>,
}

impl CacheLayer {
    pub fn new(capacity: usize, nb_db: usize) -> Self {
        Self {
            cache: (0..nb_db).map(|_| Mutex::new(RediskMap::new())).collect(),
        }
    }

    pub fn get(&self, db: u32, key: &str) -> Option<Vec<u8>> {
        let optional_cache = self.cache.get(db as usize);
        if let None = optional_cache {
            return None;
        }
        let mut cache = optional_cache.unwrap().lock().unwrap();
        if let Some(value) = cache.get(key.to_string()) {
            return Some(value.clone());
        }
        None
    }

    pub fn set(&self, db: u32, key: String, value: Vec<u8>, ttl: Option<u64>) {
        let optional_cache = self.cache.get(db as usize);
        if let None = optional_cache {
            return;
        }
        let mut cache = optional_cache.unwrap().lock().unwrap();
        cache.put(key, value, ttl);
    }

    pub fn delete(&self, db: u32, key: &str) {
        let optional_cache = self.cache.get(db as usize);
        if let None = optional_cache {
            return;
        }
        let mut cache = optional_cache.unwrap().lock().unwrap();
        cache.remove(key.to_string());
    }

    pub fn get_ttl(&self, db: u32, key: &str) -> Option<u64> {
        let optional_cache = self.cache.get(db as usize);
        if let None = optional_cache {
            return None;
        }
        let mut cache = optional_cache.unwrap().lock().unwrap();
        cache.ttl(key.to_string())
    }

    pub fn exists(&self, db: u32, key: &str) -> bool {
        let optional_cache = self.cache.get(db as usize);
        if let None = optional_cache {
            return false;
        }
        let mut cache = optional_cache.unwrap().lock().unwrap();
        cache.get(key.to_string()).is_some()
    }

    pub fn expire(&self, db: u32, key: &str, seconds: u64) -> bool {
        let optional_cache = self.cache.get(db as usize);
        if let None = optional_cache {
            return false;
        }
        let mut cache = optional_cache.unwrap().lock().unwrap();
        cache.expire(key.to_string(), seconds).is_some()
    }

    pub fn persist(&self, db: u32, key: &str) -> bool {
        let optional_cache = self.cache.get(db as usize);
        if let None = optional_cache {
            return false;
        }
        let mut cache = optional_cache.unwrap().lock().unwrap();
        cache.persist(key.to_string()).is_some()
    }

    pub fn keys(&self, db: u32, pattern: &str) -> Vec<String> {
        let regex_pattern = pattern.replace("*", ".*").replace("?", ".");
        let regex = match regex::Regex::new(&format!("^{}$", regex_pattern)) {
            Ok(r) => r,
            Err(_) => return vec![],
        };
        match self.cache.get(db as usize) {
            Some(cache) => {
                let cache = cache.lock().unwrap();
                cache
                    .iter()
                    .filter(|(key, _)| regex.is_match(key))
                    .map(|(key, _)| key.clone())
                    .collect()
            }
            None => vec![],
        }
    }

    pub fn rename(&self, db: u32, old_key: &str, new_key: &str) -> bool {
        match self.cache.get(db as usize) {
            Some(cache) => {
                let mut cache = cache.lock().unwrap();
                let ttl = cache.ttl(old_key.to_string());
                if let Some(value) = cache.remove(old_key.to_string()) {
                    cache.put((new_key.to_string()), value, ttl);
                    return true;
                }
            }
            None => {}
        }
        false
    }

    pub fn random_key(&self, db: u32) -> Option<String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let cache = self.cache.lock().unwrap();
        let mut rng = rand::thread_rng();
        cache
            .iter()
            .filter(|((key_db, _), entry)| {
                if *key_db != db {
                    return false;
                }
                let (_, expires_at) = entry;
                if let Some(exp) = *expires_at {
                    if exp <= now {
                        return false;
                    }
                }
                true
            })
            .choose(&mut rng)
            .map(|((_, key), _)| key.clone())
    }

    pub fn move_key(&self, src_db: u32, dest_db: u32, key: &str) {
        let mut cache = self.cache.lock().unwrap();
        if let Some(val) = cache.pop(&(src_db, key.to_string())) {
            cache.put((dest_db, key.to_string()), val);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_ttl() {
        let cache = CacheLayer::new(10);
        cache.set(0, "key1".to_string(), b"val1".to_vec(), Some(5));

        let ttl = cache.get_ttl(0, "key1");
        assert!(ttl.is_some());
        assert!(ttl.unwrap().unwrap() <= 5);

        std::thread::sleep(std::time::Duration::from_secs(6));
        assert!(cache.get_ttl(0, "key1").is_none());
    }

    #[test]
    fn test_cache_ttl_no_expire() {
        let cache = CacheLayer::new(10);
        cache.set(0, "key1".to_string(), b"val1".to_vec(), None);
        let ttl = cache.get_ttl(0, "key1");
        assert_eq!(ttl, Some(None));
    }

    #[test]
    fn test_cache_exists() {
        let cache = CacheLayer::new(10);
        cache.set(0, "key1".to_string(), b"val1".to_vec(), None);
        assert!(cache.exists(0, "key1"));
        assert!(!cache.exists(0, "key2"));
    }

    #[test]
    fn test_cache_expire() {
        let cache = CacheLayer::new(10);
        cache.set(0, "key1".to_string(), b"val1".to_vec(), None);
        assert!(cache.expire(0, "key1", 10));
        let ttl = cache.get_ttl(0, "key1");
        assert!(ttl.is_some());
        assert!(ttl.unwrap().is_some());
    }

    #[test]
    fn test_cache_persist() {
        let cache = CacheLayer::new(10);
        cache.set(0, "key1".to_string(), b"val1".to_vec(), Some(10));
        assert!(cache.persist(0, "key1"));
        let ttl = cache.get_ttl(0, "key1");
        assert_eq!(ttl, Some(None));
    }

    #[test]
    fn test_cache_db_isolation() {
        let cache = CacheLayer::new(10);
        cache.set(0, "key1".to_string(), b"val0".to_vec(), None);
        cache.set(1, "key1".to_string(), b"val1".to_vec(), None);

        assert_eq!(cache.get(0, "key1"), Some(b"val0".to_vec()));
        assert_eq!(cache.get(1, "key1"), Some(b"val1".to_vec()));
    }
}
