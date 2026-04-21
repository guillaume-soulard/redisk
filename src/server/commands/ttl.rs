use crate::server::commands::Command;
use crate::server::RediskCommandContext;
use std::future::Future;
use std::pin::Pin;

pub struct Ttl;

impl Command for Ttl {
    fn name(&self) -> String {
        String::from("TTL")
    }

    fn execute<'a>(
        &self,
        context: &'a mut RediskCommandContext,
    ) -> Pin<Box<dyn Future<Output = Vec<u8>> + Send + 'a>> {
        Box::pin(async move {
            if context.args.len() != 2 {
                return context.redisk_protocol.serialize_error("wrong number of arguments for 'ttl' command");
            }

            let key = &context.args[1].clone();

            match context.get(key) {
                Some(value) => {
                    match value.ttl {
                        Some(ttl) => context.redisk_protocol.serialize_integer(ttl as i64),
                        None => context.redisk_protocol.serialize_integer(-1)
                    }
                },
                None => context.redisk_protocol.serialize_integer(-2)
            }
        })
    }
}
