use crate::server::RediskCommandContext;

pub async fn handle_set(
    context: &RediskCommandContext<'_>,
) -> Vec<u8> {
    if context.args.len() != 3 {
        return context.redisk_protocol.serialize_error("wrong number of arguments for 'set' command");
    }
    let key = context.args[1].clone();
    let value = context.args[2].as_bytes().to_vec();

    match context.set(&key, &value) {
        Ok(_) => {
            context.redisk_protocol.serialize_ok()
        }
        Err(e) => {
            context.redisk_protocol.serialize_error(&format!("storage error: {}", e))
        }
    }    
}
