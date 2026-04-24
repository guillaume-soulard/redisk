mod commands;
use crate::cache::CacheLayer;
use crate::protocol::*;
use crate::server::commands::*;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

struct RediskCommandContext<'a> {
    redisk_protocol: &'a RediskProtocol,
    args: Vec<String>,
    storage: &'a mut CacheLayer,
    db: u32,
    nb_db: u32,
}

pub type TTL = Option<u64>;

pub type RediskValue = Vec<u8>;

#[derive(Clone)]
pub struct RediskStorageAddress {
    pub offset: u64,
}

#[derive(Clone)]
pub struct RediskKeyValue {
    pub value: RediskValue,
    pub mounted: bool,
    pub deleted: bool,
    pub ttl: TTL,
    pub storage_address: Option<RediskStorageAddress>,
}

impl RediskCommandContext<'_> {
    pub fn mount(&mut self, key: &str) -> Option<RediskKeyValue> {
        self.storage.mount(self.db, key)
    }

    pub fn unmount(&mut self, key: &str) -> Option<RediskKeyValue> {
        self.storage.unmount(self.db, key)
    }

    pub fn iter(&self) -> Box<dyn Iterator<Item = (&String, &RediskKeyValue)> + '_> {
        self.storage.iter(self.db)
    }

    pub fn get(&mut self, key: &str) -> Option<RediskKeyValue> {
        self.storage.get(self.db, key)
    }

    pub fn set(&mut self, key: &str, value: RediskValue, ttl: TTL) -> Option<RediskKeyValue> {
        self.storage.set(self.db, key.to_string(), value, ttl)
    }

    pub fn delete(&mut self, key: &str) -> Option<RediskKeyValue> {
        self.storage.delete(self.db, key)
    }

    pub fn expire(&mut self, key: &str, ttl: TTL) -> Option<RediskKeyValue> {
        self.storage.expire(self.db, key, ttl)
    }

    pub fn persist(&mut self, key: &str) -> Option<RediskKeyValue> {
        self.storage.persist(self.db, key)
    }
}

pub struct RedisServer {
    cache: Arc<Mutex<CacheLayer>>,
    commands: Arc<HashMap<String, Box<dyn Command>>>,
    nb_db: u32,
}

impl RedisServer {
    pub fn new(_cache_size: usize, nb_db: u32) -> Self {
        let commands: HashMap<String, Box<dyn Command>> = get_commands();
        Self {
            cache: Arc::new(Mutex::new(CacheLayer::new(nb_db))),
            commands: Arc::new(commands),
            nb_db,
        }
    }

    pub async fn run(&self, addr: &str) -> Result<()> {
        let listener = TcpListener::bind(addr).await?;
        println!("Server listening on {}", addr);

        loop {
            let (socket, _) = listener.accept().await?;
            let cache = self.cache.clone();
            let commands = self.commands.clone();
            let nb_db = self.nb_db.clone();
            tokio::spawn(async move {
                if let Err(e) = handle_connection(socket, cache, commands, nb_db).await {
                    eprintln!("Error handling connection: {}", e);
                }
            });
        }
    }
}

async fn handle_connection(
    mut socket: TcpStream,
    cache: Arc<Mutex<CacheLayer>>,
    commands: Arc<HashMap<String, Box<dyn Command>>>,
    nb_db: u32,
) -> Result<()> {
    let mut buffer = vec![0; 1024];
    let mut current_db = 0;
    let protocol = new_redisk_protocol(2);

    loop {
        let n = socket.read(&mut buffer).await?;
        if n == 0 {
            return Ok(());
        }

        match parse_command(&buffer[..n]) {
            Ok((args, _)) => {
                let response = {
                    let mut memory_lock = cache.lock().await;
                    let mut context = RediskCommandContext {
                        redisk_protocol: &protocol,
                        args,
                        storage: &mut *memory_lock,
                        db: current_db,
                        nb_db,
                    };
                    let response = handle_command(&mut context, &commands).await;
                    current_db = context.db;
                    response
                };
                socket.write_all(&response).await?;
            }
            Err(e) => {
                let error_msg = protocol.serialize_error(&e.to_string());
                socket.write_all(&error_msg).await?;
            }
        }
    }
}

async fn handle_command(
    context: &mut RediskCommandContext<'_>,
    commands: &HashMap<String, Box<dyn Command>>,
) -> Vec<u8> {
    if context.args.is_empty() {
        return context.redisk_protocol.serialize_error("empty command");
    }

    let cmd_name = context.args[0].to_uppercase();
    if cmd_name == "SELECT" {
        if context.args.len() != 2 {
            return context
                .redisk_protocol
                .serialize_error("wrong number of arguments for 'select' command");
        }
        if let Ok(db) = context.args[1].parse::<u32>() {
            context.db = db;
            return context.redisk_protocol.serialize_ok();
        } else {
            return context.redisk_protocol.serialize_error("invalid DB index");
        }
    }

    if let Some(command) = commands.get(&cmd_name) {
        command.execute(context).await
    } else {
        context
            .redisk_protocol
            .serialize_error(&format!("unknown command '{}'", cmd_name))
    }
}
