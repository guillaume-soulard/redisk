use std::future::Future;
use std::pin::Pin;
use crate::server::RediskCommandContext;
use crate::server::commands::Command;

pub struct Exists;

impl Command for Exists {
    fn name(&self) -> String {
        String::from("EXISTS")
    }

    fn execute<'a>(
        &self,
        context: &'a RediskCommandContext,
    ) -> Pin<Box<dyn Future<Output = Vec<u8>> + Send + 'a>> {
        Box::pin(async move {
            if context.args.len() < 2 {
                return context.redisk_protocol.serialize_error("wrong number of arguments for 'exists' command");
            }
            
            let mut count = 0;
            for key in context.args.iter().skip(1) {
                match context.exists(key) {
                    Ok(true) => count += 1,
                    Ok(false) => {},
                    Err(e) => return context.redisk_protocol.serialize_error(&format!("storage error: {}", e)),
                }
            }

            context.redisk_protocol.serialize_integer(count)
        })
    }
}
