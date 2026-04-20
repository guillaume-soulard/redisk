use std::future::Future;
use std::pin::Pin;
use crate::server::RediskCommandContext;
use crate::server::commands::Command;

pub struct Persist;

impl Command for Persist {
    fn name(&self) -> String {
        String::from("PERSIST")
    }

    fn execute<'a>(
        &self,
        context: &'a mut RediskCommandContext,
    ) -> Pin<Box<dyn Future<Output = Vec<u8>> + Send + 'a>> {
        Box::pin(async move {
            if context.args.len() != 2 {
                return context.redisk_protocol.serialize_error("wrong number of arguments for 'persist' command");
            }
            
            let key = &context.args[1];

            match context.persist(key) {
                Ok(true) => context.redisk_protocol.serialize_integer(1),
                Ok(false) => context.redisk_protocol.serialize_integer(0),
                Err(e) => context.redisk_protocol.serialize_error(&format!("storage error: {}", e)),
            }
        })
    }
}
