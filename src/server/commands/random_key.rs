use crate::server::commands::Command;
use crate::server::RediskCommandContext;
use std::future::Future;
use std::pin::Pin;

pub struct RandomKey;

impl Command for RandomKey {
    fn name(&self) -> String {
        String::from("KEYS")
    }

    fn execute<'a>(
        &self,
        context: &'a mut RediskCommandContext,
    ) -> Pin<Box<dyn Future<Output = Vec<u8>> + Send + 'a>> {
        Box::pin(async move {
            if context.args.len() != 1 {
                return context
                    .redisk_protocol
                    .serialize_error("wrong number of arguments for 'randomkey' command");
            }
            let keys = context
                .iter()
                .choose(&mut rand::thread_rng())
                .map(|k| k.0.clone());

            let mut serialized_keys = Vec::new();
            for key in keys {
                serialized_keys.push(context.redisk_protocol.serialize_bulk_string(&key));
            }
            context.redisk_protocol.serialize_array(&serialized_keys)
        })
    }
}
