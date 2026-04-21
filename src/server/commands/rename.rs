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
        context: &'a mut RediskCommandContext,
    ) -> Pin<Box<dyn Future<Output = Vec<u8>> + Send + 'a>> {
        Box::pin(async move {
            if context.args.len() != 3 {
                return context.redisk_protocol.serialize_error("wrong number of arguments for 'rename' command");
            }
            let old_key = &context.args[1].clone();
            let new_key = &context.args[2].clone();

            if old_key == new_key {
                return context.redisk_protocol.serialize_ok();
            }

            match context.get(old_key) {
                Some(old) => {
                    context.set(new_key, old.value, old.ttl);
                    context.delete(old_key);
                    context.redisk_protocol.serialize_ok()
                },
                None => context.redisk_protocol.serialize_error("no such key"),
            }
        })
    }
}
