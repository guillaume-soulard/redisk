use crate::server::RediskCommandContext;

pub async fn handle_del(
    context: &RediskCommandContext<'_>,
) -> Vec<u8> {
    if context.args.len() < 2 {
        return context.redisk_protocol.serialize_error("wrong number of arguments for 'del' command");
    }
    let mut count = 0;
    for key in context.args.iter().skip(1) {
        match context.delete(key) {
            Ok(_) => count += 1,
            Err(e) => {
                context.redisk_protocol.serialize_error(&format!("storage error: {}", e));
                break
            },
        }
        count += 1;
    }
    context.redisk_protocol.serialize_integer(count)
}
