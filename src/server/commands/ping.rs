use crate::protocol::serialize_pong;

pub async fn handle_ping() -> Vec<u8> {
    serialize_pong()
}
