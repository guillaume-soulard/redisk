use std::future::Future;
use std::pin::Pin;
use crate::server::RediskCommandContext;
use crate::server::commands::Command;

pub struct MoveCommand;

impl Command for MoveCommand {
    fn name(&self) -> String {
        String::from("MOVE")
    }

    fn execute<'a>(
        &self,
        context: &'a mut RediskCommandContext,
    ) -> Pin<Box<dyn Future<Output = Vec<u8>> + Send + 'a>> {
        Box::pin(async move {
            if context.args.len() != 3 {
                return context.redisk_protocol.serialize_error("wrong number of arguments for 'move' command");
            }
            let key = context.args[1].clone();
            let db = context.args[2].parse::<u32>().unwrap_or(0);
            if db > context.nb_db - 1 {
                return context.redisk_protocol.serialize_integer(0);
            }
            let current_db = context.db;
            match context.get(&key) {
                Some(old_db_value) => {
                    context.db = db;
                    if context.get(&key).is_some() {
                        context.db = current_db;
                        return context.redisk_protocol.serialize_integer(0);
                    }
                    context.set(&key, old_db_value.value.clone(), old_db_value.ttl);
                    context.db = current_db;
                    context.delete(&key);
                    context.redisk_protocol.serialize_integer(1)
                },
                None => {
                    context.redisk_protocol.serialize_integer(0)
                },
            }
        })
    }
}
