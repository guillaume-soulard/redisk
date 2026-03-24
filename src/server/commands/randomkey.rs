use std::future::Future;
use std::pin::Pin;
use crate::server::RediskCommandContext;
use crate::server::commands::Command;

pub struct RandomKey;

impl Command for RandomKey {
    fn name(&self) -> String {
        String::from("RANDOMKEY")
    }

    fn execute<'a>(
        &self,
        context: &'a RediskCommandContext,
    ) -> Pin<Box<dyn Future<Output = Vec<u8>> + Send + 'a>> {
        Box::pin(async move {
            if context.args.len() != 1 {
                return context.redisk_protocol.serialize_error("wrong number of arguments for 'randomkey' command");
            }

            match context.randomkey() {
                Ok(Some(key)) => context.redisk_protocol.serialize_bulk_string(&key),
                Ok(None) => context.redisk_protocol.serialize_null(),
                Err(e) => context.redisk_protocol.serialize_error(&format!("storage error: {}", e)),
            }
        })
    }
}
