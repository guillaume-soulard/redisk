mod redisk_map;

use lru::LruCache;
use std::num::NonZeroUsize;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use rand::seq::IteratorRandom;

pub struct CacheLayer {
    cache: Mutex<LruCache<(u32, String), (Vec<u8>, Option<u64>)>>,
}

impl CacheLayer {
    pub fn new(capacity: usize) -> Self {
        Self {
            cache: Mutex::new(LruCache::new(NonZeroUsize::new(capacity).unwrap())),
        }
    }

    pub fn get(&self, db: u32, key: &str) -> Option<Vec<u8>> {
        let mut cache = self.cache.lock().unwrap();
        if let Some((value, expires_at)) = cache.get(&(db, key.to_string())) {
            if let Some(exp) = expires_at {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                if *exp <= now {
                    cache.pop(&(db, key.to_string()));
                    return None;
                }
            }
            return Some(value.clone());
        }
        None
    }

    pub fn set(&self, db: u32, key: String, value: Vec<u8>, ttl: Option<u64>) {
        let expires_at = ttl.map(|t| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
                + t
        });
        let mut cache = self.cache.lock().unwrap();
        cache.put((db, key), (value, expires_at));
    }

    pub fn delete(&self, db: u32, key: &str) {
        let mut cache = self.cache.lock().unwrap();
        cache.pop(&(db, key.to_string()));
    }

    pub fn get_ttl(&self, db: u32, key: &str) -> Option<Option<u64>> {
        let mut cache = self.cache.lock().unwrap();
        if let Some((_, expires_at)) = cache.get(&(db, key.to_string())) {
            if let Some(exp) = expires_at {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                if *exp <= now {
                    cache.pop(&(db, key.to_string()));
                    return None;
                }
                return Some(Some(*exp - now));
            } else {
                return Some(None);
            }
        }
        None
    }

    pub fn exists(&self, db: u32, key: &str) -> bool {
        let mut cache = self.cache.lock().unwrap();
        if let Some((_, expires_at)) = cache.get(&(db, key.to_string())) {
            if let Some(exp) = expires_at {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                if *exp <= now {
                    cache.pop(&(db, key.to_string()));
                    return false;
                }
            }
            return true;
        }
        false
    }

    pub fn expire(&self, db: u32, key: &str, seconds: u64) -> bool {
        let mut cache = self.cache.lock().unwrap();
        if let Some((value, _)) = cache.get(&(db, key.to_string())) {
            let expires_at = Some(SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() + seconds);
            let val = value.clone();
            cache.put((db, key.to_string()), (val, expires_at));
            return true;
        }
        false
    }

    pub fn persist(&self, db: u32, key: &str) -> bool {
        let mut cache = self.cache.lock().unwrap();
        if let Some((value, expires_at)) = cache.get(&(db, key.to_string())) {
            if expires_at.is_none() {
                return false;
            }
            let val = value.clone();
            cache.put((db, key.to_string()), (val, None));
            return true;
        }
        false
    }

    pub fn keys(&self, db: u32, pattern: &str) -> Vec<String> {
        let regex_pattern = pattern
            .replace("*", ".*")
            .replace("?", ".");
        let regex = match regex::Regex::new(&format!("^{}$", regex_pattern)) {
            Ok(r) => r,
            Err(_) => return vec![],
        };

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let cache = self.cache.lock().unwrap();
        cache.iter()
            .filter(|((key_db, key), entry)| {
                if *key_db != db {
                    return false;
                }
                let (_, expires_at) = entry;
                if let Some(exp) = *expires_at {
                    if exp <= now {
                        return false;
                    }
                }
                regex.is_match(key)
            })
            .map(|((_, key), _)| key.clone())
            .collect()
    }

    pub fn rename(&self, db: u32, old_key: &str, new_key: &str) {
        let mut cache = self.cache.lock().unwrap();
        if let Some((value, expires_at)) = cache.pop(&(db, old_key.to_string())) {
            cache.put((db, new_key.to_string()), (value, expires_at));
        }
    }

    pub fn randomkey(&self, db: u32) -> Option<String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let cache = self.cache.lock().unwrap();
        let mut rng = rand::thread_rng();
        cache.iter()
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
