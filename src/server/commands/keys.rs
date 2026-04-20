use std::future::Future;
use std::pin::Pin;
use crate::server::RediskCommandContext;
use crate::server::commands::Command;

pub struct Keys;

impl Command for Keys {
    fn name(&self) -> String {
        String::from("KEYS")
    }

    fn execute<'a>(
        &self,
        context: &'a mut RediskCommandContext,
    ) -> Pin<Box<dyn Future<Output = Vec<u8>> + Send + 'a>> {
        Box::pin(async move {
            if context.args.len() != 2 {
                return context.redisk_protocol.serialize_error("wrong number of arguments for 'keys' command");
            }
            let pattern = &context.args[1];

            match context.keys(pattern) {
                Ok(keys) => {
                    let mut serialized_keys = Vec::new();
                    for key in keys {
                        serialized_keys.push(context.redisk_protocol.serialize_bulk_string(&key));
                    }
                    context.redisk_protocol.serialize_array(&serialized_keys)
                }
                Err(e) => context.redisk_protocol.serialize_error(&format!("storage error: {}", e)),
            }
        })
    }
}
