use std::future::Future;
use std::pin::Pin;
use crate::server::RediskCommandContext;
use crate::server::commands::Command;

pub struct Ping;

impl Command for Ping {
    fn name(&self) -> String {
        String::from("PING")
    }

    fn execute<'a>(
        &self,
        context: &'a RediskCommandContext,
    ) -> Pin<Box<dyn Future<Output = Vec<u8>> + Send + 'a>> {
        Box::pin(async move {
            match context.args.len() {
                1 => context.redisk_protocol.serialize_simple_string(&"PONG".to_string()),
                2 => context.redisk_protocol.serialize_bulk_string(&context.args[1]),
                _ => context.redisk_protocol.serialize_error("wrong number of arguments for 'ping' command"),
            }
        })
    }
}
