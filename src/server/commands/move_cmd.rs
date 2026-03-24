use std::future::Future;
use std::pin::Pin;
use crate::server::RediskCommandContext;
use crate::server::commands::Command;

pub struct Move;

impl Command for Move {
    fn name(&self) -> String {
        String::from("MOVE")
    }

    fn execute<'a>(
        &self,
        context: &'a RediskCommandContext,
    ) -> Pin<Box<dyn Future<Output = Vec<u8>> + Send + 'a>> {
        Box::pin(async move {
            if context.args.len() != 3 {
                return context.redisk_protocol.serialize_error("wrong number of arguments for 'move' command");
            }
            let key = &context.args[1];
            let dest_db = match context.args[2].parse::<u32>() {
                Ok(db) => db,
                Err(_) => return context.redisk_protocol.serialize_error("invalid DB index"),
            };

            match context.move_key(context.db, dest_db, key) {
                Ok(true) => context.redisk_protocol.serialize_integer(1),
                Ok(false) => context.redisk_protocol.serialize_integer(0),
                Err(e) => context.redisk_protocol.serialize_error(&e.to_string()),
            }
        })
    }
}
