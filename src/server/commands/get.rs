use std::future::Future;
use std::pin::Pin;
use crate::server::RediskCommandContext;
use crate::server::commands::Command;

pub struct Get;

impl Command for Get {
    fn name(&self) -> String {
        String::from("GET")
    }

    fn execute<'a>(
        &self,
        context: &'a RediskCommandContext,
    ) -> Pin<Box<dyn Future<Output = Vec<u8>> + Send + 'a>> {
        Box::pin(async move {
            if context.args.len() != 2 {
                return context.redisk_protocol.serialize_error("wrong number of arguments for 'get' command");
            }
            let key = &context.args[1];

            match context.get(key) {
                Ok(Some(value)) => {
                    context.memory.set(key.clone(), value.clone());
                    context.redisk_protocol.serialize_bulk_string(&String::from_utf8_lossy(&value))
                }
                Ok(None) => context.redisk_protocol.serialize_null(),
                Err(e) => context.redisk_protocol.serialize_error(&format!("storage error: {}", e)),
            }
        })
    }
}
