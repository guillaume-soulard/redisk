use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use compact::Compact;
use crate::server::RediskCommandContext;

pub mod get;
pub mod set;
pub mod del;
pub mod ping;
mod compact;

pub trait Command: Send + Sync {
    fn name(&self) -> String;
    fn execute<'a>(
        &self,
        context: &'a RediskCommandContext,
    ) -> Pin<Box<dyn Future<Output = Vec<u8>> + Send + 'a>>;
}

pub fn get_commands() -> HashMap<String, Box<dyn Command>> {
    let mut commands = HashMap::new();
    for command in get_command_list() {
        commands.insert(command.name(), command);
    }
    commands
}

fn get_command_list() -> Vec<Box<dyn Command>> {
    vec![
        Box::new(Get),
        Box::new(Set),
        Box::new(Del),
        Box::new(Ping),
        Box::new(Compact),
    ]
}

pub use get::Get;
pub use set::Set;
pub use del::Del;
pub use ping::Ping;
