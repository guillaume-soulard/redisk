mod commands;
use crate::cache::CacheLayer;
use crate::protocol::*;
use crate::server::commands::*;
use crate::storage::StorageEngine;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

struct RediskCommandContext<'a> {
    redisk_protocol: &'a RediskProtocol,
    args: Vec<String>,
    memory: &'a mut CacheLayer,
    storage: &'a mut StorageEngine,
    db: u32,
}

pub type TTL = Option<u64>;

pub type RediskValue = Vec<u8>;

#[derive(Clone)]
pub struct RediskKeyValue {
    pub value: RediskValue,
    pub offset: Option<u64>,
    pub mounted: bool,
    pub deleted: bool,
    pub ttl: TTL,
}

impl RediskCommandContext<'_> {
    pub fn mount(&mut self, key: &str) -> Option<RediskKeyValue> {
        self.memory.mount(self.db, key)
    }

    pub fn unmount(&mut self, key: &str) -> Option<RediskKeyValue> {
        self.memory.unmount(self.db, key)
    }

    pub fn iter(&self) -> Box<dyn Iterator<Item = (&String, &RediskKeyValue)> + '_> {
        let memory_values = self.memory.iter(self.db);
        // let storage_values = self.storage.lock().unwrap().iter(self.db);
        memory_values
    }

    pub fn get(&mut self, key: &str) -> Option<RediskKeyValue> {
        self.memory.get(self.db, key)
    }

    pub fn set(&mut self, key: &str, value: RediskValue, ttl: TTL) -> Option<RediskKeyValue> {
        self.memory.set(self.db, key.to_string(), value, ttl)
    }

    pub fn delete(&mut self, key: &str) -> Option<RediskKeyValue> {
        self.memory.delete(self.db, key)
    }

    pub fn expire(&mut self, key: &str, ttl: TTL) -> Option<RediskKeyValue> {
        self.memory.expire(self.db, key, ttl)
    }

    pub fn persist(&mut self, key: &str) -> Option<RediskKeyValue> {
        self.memory.persist(self.db, key)
    }
}

pub struct RedisServer {
    storage: Arc<Mutex<StorageEngine>>,
    cache: Arc<Mutex<CacheLayer>>,
    commands: Arc<HashMap<String, Box<dyn Command>>>,
}

impl RedisServer {
    pub fn new(storage: StorageEngine, _cache_size: usize, nb_db: usize) -> Self {
        let commands: HashMap<String, Box<dyn Command>> = get_commands();
        Self {
            storage: Arc::new(Mutex::new(storage)),
            cache: Arc::new(Mutex::new(CacheLayer::new(nb_db))),
            commands: Arc::new(commands),
        }
    }

    pub async fn run(&self, addr: &str) -> Result<()> {
        let listener = TcpListener::bind(addr).await?;
        println!("Server listening on {}", addr);

        loop {
            let (socket, _) = listener.accept().await?;
            let cache = self.cache.clone();
            let storage = self.storage.clone();
            let commands = self.commands.clone();
            tokio::spawn(async move {
                if let Err(e) = handle_connection(socket, cache, storage, commands).await {
                    eprintln!("Error handling connection: {}", e);
                }
            });
        }
    }
}

async fn handle_connection(
    mut socket: TcpStream,
    cache: Arc<Mutex<CacheLayer>>,
    storage: Arc<Mutex<StorageEngine>>,
    commands: Arc<HashMap<String, Box<dyn Command>>>,
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
                    let mut storage_lock = storage.lock().await;
                    let mut context = RediskCommandContext {
                        redisk_protocol: &protocol,
                        args,
                        memory: &mut *memory_lock,
                        storage: &mut *storage_lock,
                        db: current_db,
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
