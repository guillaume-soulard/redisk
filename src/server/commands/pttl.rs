use std::future::Future;
use std::pin::Pin;
use crate::server::RediskCommandContext;
use crate::server::commands::Command;

pub struct Pttl;

impl Command for Pttl {
    fn name(&self) -> String {
        String::from("PTTL")
    }

    fn execute<'a>(
        &self,
        context: &'a RediskCommandContext,
    ) -> Pin<Box<dyn Future<Output = Vec<u8>> + Send + 'a>> {
        Box::pin(async move {
            if context.args.len() != 2 {
                return context.redisk_protocol.serialize_error("wrong number of arguments for 'pttl' command");
            }
            let key = &context.args[1];

            match context.get_ttl(key) {
                Ok(Some(Some(ttl))) => {
                    context.redisk_protocol.serialize_integer((ttl * 1000) as i64)
                }
                Ok(Some(None)) => {
                    context.redisk_protocol.serialize_integer(-1)
                }
                Ok(None) => {
                    context.redisk_protocol.serialize_integer(-2)
                }
                Err(e) => context.redisk_protocol.serialize_error(&format!("storage error: {}", e)),
            }
        })
    }
}
