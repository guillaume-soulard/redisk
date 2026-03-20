use crate::server::RediskCommandContext;

pub async fn handle_get(
    context: &RediskCommandContext<'_>,
) -> Vec<u8> {
    if context.args.len() != 2 {
        return context.redisk_protocol.serialize_error("wrong number of arguments for 'get' command");
    }
    let key = &context.args[1];
    
    // Check cache
    if let Some(val) = context.memory.get(key) {
        return context.redisk_protocol.serialize_bulk_string(&String::from_utf8_lossy(&val));
    }

    // Check storage
    let mut storage = context.storage.lock().unwrap();
    match storage.get(key) {
        Ok(Some(val)) => {
            context.memory.set(key.clone(), val.clone());
            context.redisk_protocol.serialize_bulk_string(&String::from_utf8_lossy(&val))
        }
        Ok(None) => context.redisk_protocol.serialize_null(),
        Err(e) => context.redisk_protocol.serialize_error(&format!("storage error: {}", e)),
    }
}
