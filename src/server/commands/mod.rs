use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use compact::Compact;
use crate::server::RediskCommandContext;

pub mod get;
pub mod set;
pub mod del;
pub mod ping;
pub mod ttl;
pub mod pttl;
pub mod exists;
pub mod expire;
pub mod persist;
pub mod keys;
pub mod scan;
pub mod type_cmd;
pub mod rename;
pub mod randomkey;
pub mod move_cmd;
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
        Box::new(Ttl),
        Box::new(Pttl),
        Box::new(Exists),
        Box::new(Expire),
        Box::new(Persist),
        Box::new(Keys),
        Box::new(Scan),
        Box::new(Type),
        Box::new(Rename),
        Box::new(RandomKey),
        Box::new(Move),
    ]
}

pub use get::Get;
pub use set::Set;
pub use del::Del;
pub use ping::Ping;
pub use ttl::Ttl;
pub use pttl::Pttl;
pub use exists::Exists;
pub use expire::Expire;
pub use persist::Persist;
pub use keys::Keys;
pub use scan::Scan;
pub use type_cmd::Type;
pub use rename::Rename;
pub use randomkey::RandomKey;
pub use move_cmd::Move;
