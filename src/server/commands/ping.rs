use crate::server::RediskCommandContext;

pub async fn handle_ping(
    context: &RediskCommandContext<'_>,
) -> Vec<u8> {
    match context.args.len() {
        1 => context.redisk_protocol.serialize_simple_string(&"PONG".to_string()),
        2_ => context.redisk_protocol.serialize_bulk_string(&context.args[1]),
        _ => context.redisk_protocol.serialize_error("wrong number of arguments for 'ping' command"),
    }
}
