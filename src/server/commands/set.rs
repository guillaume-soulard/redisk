use std::future::Future;
use std::pin::Pin;
use crate::server::RediskCommandContext;
use crate::server::commands::Command;

pub struct Set;

impl Command for Set {
    fn name(&self) -> String {
        String::from("SET")
    }

    fn execute<'a>(
        &self,
        context: &'a RediskCommandContext,
    ) -> Pin<Box<dyn Future<Output = Vec<u8>> + Send + 'a>> {
        Box::pin(async move {
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
        })
    }
}
