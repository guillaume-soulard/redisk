use std::future::Future;
use std::pin::Pin;
use crate::server::{RediskCommandContext, RediskValue};
use crate::server::commands::Command;

pub struct Set;

impl Command for Set {
    fn name(&self) -> String {
        String::from("SET")
    }

    fn execute<'a>(
        &self,
        context: &'a mut RediskCommandContext,
    ) -> Pin<Box<dyn Future<Output = Vec<u8>> + Send + 'a>> {
        Box::pin(async move {
            if context.args.len() != 3 && context.args.len() != 5 {
                return context.redisk_protocol.serialize_error("wrong number of arguments for 'set' command");
            }
            let key = context.args[1].clone();
            let value = context.args[2].as_bytes().to_vec() as RediskValue;
            let mut ttl = None;

            if context.args.len() == 5 {
                if context.args[3].to_uppercase() == "EX" {
                    if let Ok(seconds) = context.args[4].parse::<u64>() {
                        ttl = Some(seconds);
                    } else {
                        return context.redisk_protocol.serialize_error("invalid expire time in 'set' command");
                    }
                } else {
                    return context.redisk_protocol.serialize_error("syntax error in 'set' command");
                }
            }

            match context.set(&key, value, ttl) {
                Some(_) => {
                    context.redisk_protocol.serialize_ok()
                }
                None => {
                    context.redisk_protocol.serialize_error(&"key not set:")
                }
            }
        })
    }
}
