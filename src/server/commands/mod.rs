use crate::server::RediskCommandContext;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

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
pub mod select_cmd;
pub mod mount;
pub mod unmount;
pub mod move_cmd;

pub trait Command: Send + Sync {
    fn name(&self) -> String;
    fn execute<'a>(
        &self,
        context: &'a mut RediskCommandContext,
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
        Box::new(Ttl),
        Box::new(Pttl),
        Box::new(Exists),
        Box::new(Expire),
        Box::new(Persist),
        Box::new(Keys),
        Box::new(Scan),
        Box::new(Type),
        Box::new(Rename),
        Box::new(SelectCommand),
        Box::new(Mount),
        Box::new(Unmount),
        Box::new(MoveCommand)
    ]
}

pub use del::Del;
pub use exists::Exists;
pub use expire::Expire;
pub use get::Get;
pub use keys::Keys;
pub use persist::Persist;
pub use ping::Ping;
pub use pttl::Pttl;
pub use rename::Rename;
pub use scan::Scan;
pub use set::Set;
pub use ttl::Ttl;
pub use type_cmd::Type;
pub use mount::Mount;
pub use select_cmd::SelectCommand;
pub use unmount::Unmount;
pub use move_cmd::MoveCommand;
