use std::future::Future;
use std::pin::Pin;
use crate::server::RediskCommandContext;
use crate::server::commands::Command;

pub struct Del;

impl Command for Del {
    fn name(&self) -> String {
        String::from("DEL")
    }

    fn execute<'a>(
        &self,
        context: &'a mut RediskCommandContext,
    ) -> Pin<Box<dyn Future<Output = Vec<u8>> + Send + 'a>> {
        Box::pin(async move {
            if context.args.len() < 2 {
                return context.redisk_protocol.serialize_error("wrong number of arguments for 'del' command");
            }
            let mut count = 0;
            for key in context.args.iter().skip(1) {
                match context.delete(key) {
                    Ok(_) => count += 1,
                    Err(e) => {
                        return context.redisk_protocol.serialize_error(&format!("storage error: {}", e));
                    },
                }
            }
            context.redisk_protocol.serialize_integer(count)
        })
    }
}
