use crate::protocol::{serialize_integer, serialize_error};
use crate::storage::StorageEngine;
use crate::cache::CacheLayer;
use std::sync::{Arc, Mutex};

pub async fn handle_del(
    args: Vec<String>,
    storage: &Arc<Mutex<StorageEngine>>,
    cache: &Arc<CacheLayer>,
) -> Vec<u8> {
    if args.len() < 2 {
        return serialize_error("wrong number of arguments for 'del' command");
    }
    let mut count = 0;
    let mut storage = storage.lock().unwrap();
    for key in args.iter().skip(1) {
        let exists = match storage.get(key) {
            Ok(Some(_)) => true,
            _ => false,
        };
        if exists {
            if let Ok(_) = storage.delete(key) {
                cache.delete(key);
                count += 1;
            }
        }
    }
    serialize_integer(count)
}
