use crate::protocol::{serialize_bulk, serialize_error, serialize_null};
use crate::storage::StorageEngine;
use crate::cache::CacheLayer;
use std::sync::{Arc, Mutex};

pub async fn handle_get(
    args: Vec<String>,
    storage: &Arc<Mutex<StorageEngine>>,
    cache: &Arc<CacheLayer>,
) -> Vec<u8> {
    if args.len() != 2 {
        return serialize_error("wrong number of arguments for 'get' command");
    }
    let key = &args[1];
    
    // Check cache
    if let Some(val) = cache.get(key) {
        return serialize_bulk(&String::from_utf8_lossy(&val));
    }

    // Check storage
    let mut storage = storage.lock().unwrap();
    match storage.get(key) {
        Ok(Some(val)) => {
            cache.set(key.clone(), val.clone());
            serialize_bulk(&String::from_utf8_lossy(&val))
        }
        Ok(None) => serialize_null(),
        Err(e) => serialize_error(&format!("storage error: {}", e)),
    }
}
