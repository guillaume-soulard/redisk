use crate::server::RediskCommandContext;

pub async fn handle_del(
    context: &RediskCommandContext<'_>,
) -> Vec<u8> {
    if context.args.len() < 2 {
        return context.redisk_protocol.serialize_error("wrong number of arguments for 'del' command");
    }
    let mut count = 0;
    let mut storage = context.storage.lock().unwrap();
    for key in context.args.iter().skip(1) {
        let exists = match storage.get(key) {
            Ok(Some(_)) => true,
            _ => false,
        };
        if exists {
            if let Ok(_) = storage.delete(key) {
                context.memory.delete(key);
                count += 1;
            }
        }
    }
    context.redisk_protocol.serialize_integer(count)
}
