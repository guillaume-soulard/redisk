use crate::server::RediskCommandContext;

pub async fn handle_ping(
    context: &RediskCommandContext<'_>,
) -> Vec<u8> {
    context.redisk_protocol.serialize_simple_string(&"PONG".to_string())
}
