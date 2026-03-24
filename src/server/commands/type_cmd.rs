use std::future::Future;
use std::pin::Pin;
use crate::server::RediskCommandContext;
use crate::server::commands::Command;

pub struct Type;

impl Command for Type {
    fn name(&self) -> String {
        String::from("TYPE")
    }

    fn execute<'a>(
        &self,
        context: &'a RediskCommandContext,
    ) -> Pin<Box<dyn Future<Output = Vec<u8>> + Send + 'a>> {
        Box::pin(async move {
            if context.args.len() != 2 {
                return context.redisk_protocol.serialize_error("wrong number of arguments for 'type' command");
            }
            let key = &context.args[1];

            match context.get_type(key) {
                Ok(t) => context.redisk_protocol.serialize_simple_string(&t),
                Err(e) => context.redisk_protocol.serialize_error(&format!("storage error: {}", e)),
            }
        })
    }
}
