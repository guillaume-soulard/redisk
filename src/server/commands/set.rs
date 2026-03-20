use crate::server::RediskCommandContext;

pub async fn handle_set(
    context: &RediskCommandContext<'_>,
) -> Vec<u8> {
    if context.args.len() != 3 {
        return context.redisk_protocol.serialize_error("wrong number of arguments for 'set' command");
    }
    let key = context.args[1].clone();
    let value = context.args[2].as_bytes().to_vec();

    // Update storage
    let mut storage = context.storage.lock().unwrap();
    if let Err(e) = storage.set(key.clone(), value.clone()) {
        return context.redisk_protocol.serialize_error(&format!("storage error: {}", e));
    }

    // Update cache
    context.memory.set(key, value);
    context.redisk_protocol.serialize_ok()
}
