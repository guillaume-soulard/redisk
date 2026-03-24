use std::future::Future;
use std::pin::Pin;
use crate::server::RediskCommandContext;
use crate::server::commands::Command;

pub struct Expire;

impl Command for Expire {
    fn name(&self) -> String {
        String::from("EXPIRE")
    }

    fn execute<'a>(
        &self,
        context: &'a RediskCommandContext,
    ) -> Pin<Box<dyn Future<Output = Vec<u8>> + Send + 'a>> {
        Box::pin(async move {
            if context.args.len() != 3 {
                return context.redisk_protocol.serialize_error("wrong number of arguments for 'expire' command");
            }
            
            let key = &context.args[1];
            let seconds = match context.args[2].parse::<u64>() {
                Ok(s) => s,
                Err(_) => return context.redisk_protocol.serialize_error("value is not an integer or out of range"),
            };

            match context.expire(key, seconds) {
                Ok(true) => context.redisk_protocol.serialize_integer(1),
                Ok(false) => context.redisk_protocol.serialize_integer(0),
                Err(e) => context.redisk_protocol.serialize_error(&format!("storage error: {}", e)),
            }
        })
    }
}
