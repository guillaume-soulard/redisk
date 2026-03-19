use crate::protocol::{serialize_ok, serialize_error};
use crate::storage::StorageEngine;
use crate::cache::CacheLayer;
use std::sync::{Arc, Mutex};

pub async fn handle_set(
    args: Vec<String>,
    storage: &Arc<Mutex<StorageEngine>>,
    cache: &Arc<CacheLayer>,
) -> Vec<u8> {
    if args.len() != 3 {
        return serialize_error("wrong number of arguments for 'set' command");
    }
    let key = args[1].clone();
    let value = args[2].as_bytes().to_vec();

    // Update storage
    let mut storage = storage.lock().unwrap();
    if let Err(e) = storage.set(key.clone(), value.clone()) {
        return serialize_error(&format!("storage error: {}", e));
    }

    // Update cache
    cache.set(key, value);
    serialize_ok()
}
