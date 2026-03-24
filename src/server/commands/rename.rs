use std::future::Future;
use std::pin::Pin;
use crate::server::RediskCommandContext;
use crate::server::commands::Command;

pub struct Rename;

impl Command for Rename {
    fn name(&self) -> String {
        String::from("RENAME")
    }

    fn execute<'a>(
        &self,
        context: &'a RediskCommandContext,
    ) -> Pin<Box<dyn Future<Output = Vec<u8>> + Send + 'a>> {
        Box::pin(async move {
            if context.args.len() != 3 {
                return context.redisk_protocol.serialize_error("wrong number of arguments for 'rename' command");
            }
            let old_key = &context.args[1];
            let new_key = &context.args[2];

            if old_key == new_key {
                return context.redisk_protocol.serialize_ok();
            }

            match context.rename(old_key, new_key) {
                Ok(_) => context.redisk_protocol.serialize_ok(),
                Err(e) => {
                    if e.to_string() == "no such key" {
                        context.redisk_protocol.serialize_error("no such key")
                    } else {
                        context.redisk_protocol.serialize_error(&format!("storage error: {}", e))
                    }
                }
            }
        })
    }
}
