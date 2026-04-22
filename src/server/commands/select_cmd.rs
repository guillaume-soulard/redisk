use std::future::Future;
use std::pin::Pin;
use crate::server::RediskCommandContext;
use crate::server::commands::Command;

pub struct SelectCommand;

impl Command for SelectCommand {
    fn name(&self) -> String {
        String::from("SELECT")
    }

    fn execute<'a>(
        &self,
        context: &'a mut RediskCommandContext,
    ) -> Pin<Box<dyn Future<Output = Vec<u8>> + Send + 'a>> {
        Box::pin(async move {
            match context.args[1].parse::<u32>() {
                Ok(db_index) => {
                    if context.db > context.nb_db {
                        context.redisk_protocol.serialize_error("DB index is out of range")
                    } else {
                        context.db = db_index;
                        context.redisk_protocol.serialize_ok()
                    }
                }
                Err(_) => return context.redisk_protocol.serialize_error("value is not an integer or out of range"),
            }
        })
    }
}
