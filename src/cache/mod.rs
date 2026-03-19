use lru::LruCache;
use std::num::NonZeroUsize;
use std::sync::Mutex;

pub struct CacheLayer {
    cache: Mutex<LruCache<String, Vec<u8>>>,
}

impl CacheLayer {
    pub fn new(capacity: usize) -> Self {
        Self {
            cache: Mutex::new(LruCache::new(NonZeroUsize::new(capacity).unwrap())),
        }
    }

    pub fn get(&self, key: &str) -> Option<Vec<u8>> {
        let mut cache = self.cache.lock().unwrap();
        cache.get(key).cloned()
    }

    pub fn set(&self, key: String, value: Vec<u8>) {
        let mut cache = self.cache.lock().unwrap();
        cache.put(key, value);
    }

    pub fn delete(&self, key: &str) {
        let mut cache = self.cache.lock().unwrap();
        cache.pop(key);
    }
}
