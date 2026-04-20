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
        context: &'a mut RediskCommandContext,
    ) -> Pin<Box<dyn Future<Output = Vec<u8>> + Send + 'a>> {
        Box::pin(async move {
            if context.args.len() != 2 {
                return context.redisk_protocol.serialize_error("wrong number of arguments for 'type' command");
            }
            let key = &context.args[1];

            match context.get(key) {
                Some(_) => context.redisk_protocol.serialize_simple_string(&String::from("string")),
                None => context.redisk_protocol.serialize_simple_string(&String::from("none")),
            }
        })
    }
}
