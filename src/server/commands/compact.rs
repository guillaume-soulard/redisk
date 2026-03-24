use std::future::Future;
use std::pin::Pin;
use crate::server::RediskCommandContext;
use crate::server::commands::Command;

pub struct Compact;

impl Command for Compact {
    fn name(&self) -> String {
        String::from("COMPACT")
    }

    fn execute<'a>(
        &self,
        context: &'a RediskCommandContext,
    ) -> Pin<Box<dyn Future<Output = Vec<u8>> + Send + 'a>> {
        Box::pin(async move {
            context.storage.lock().unwrap().compact().unwrap();
            context.redisk_protocol.serialize_ok()
        })
    }
}
