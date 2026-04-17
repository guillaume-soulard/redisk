mod commands;
use crate::cache::CacheLayer;
use crate::protocol::*;
use crate::server::commands::*;
use crate::storage::StorageEngine;
use anyhow::{Error, Result};
use bincode::ErrorKind;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

struct RediskCommandContext<'a> {
    redisk_protocol: &'a RediskProtocol,
    args: Vec<String>,
    storage: &'a Arc<Mutex<StorageEngine>>,
    memory: CacheLayer,
    db: u32,
}

pub type TTL = Option<u64>;

pub type RediskValue = Vec<u8>;

impl RediskCommandContext<'_> {
    pub fn mount(&mut self, key: &str, ttl: TTL) -> Option<RediskValue> {
        let storage = self.storage.lock().unwrap().get(self.db, key);
        match storage {
            Some(value) => {
                self.memory.set(self.db, key.to_string(), value, ttl)
                    .map(|v| v.1)
            },
            None => None,
        }
    }

    pub fn unmount(&mut self, key: &str, ttl: TTL) -> Option<RediskValue> {
        let memory = self.memory.delete(self.db, key);
        match memory {
            Some(value) => {
                self.storage.lock().unwrap().set(self.db, key.to_string(), value.clone(), ttl)
            },
            None => {
                self.storage.lock().unwrap().delete(self.db, key)
            },
        }
    }

    pub fn iter(&self) -> Box<dyn Iterator<Item = (&String, &(TTL, RediskValue))> + '_> {
        let memory_values = self.memory.iter(self.db);
        // let storage_values = self.storage.lock().unwrap().iter(self.db);
        memory_values
    }

    pub fn get(&mut self, key: &str) -> Option<RediskValue> {
        if self.memory.is_mounted(self.db, key) {
            return self.memory.get(self.db, key).map(|v| v.1);
        }
        match self.memory.get(self.db, key) {
            Some(value) => Some(value.1),
            None => {
                match self.storage.lock() {
                    Ok(mut storage) => storage.get(self.db, key),
                    Err(_) => None
                }
            },
        }
    }

    pub fn set(&mut self, key: &str, value: &RediskValue, ttl: TTL) -> Option<RediskValue> {
        if self.memory.is_mounted(self.db, key) {
            return self.memory.set(self.db, key.to_string(), value.clone(), ttl)
                .map(|v| v.1);
        }
        match self.storage.lock() {
            Ok(mut storage) => storage.set(self.db, key.to_string(), value.clone(), ttl),
            Err(_) => None,
        }
    }

    pub fn delete(&mut self, key: &str) -> Option<RediskValue> {
        let memory_result = self.memory.delete(self.db, key);
        if self.memory.is_mounted(self.db, key) {
            memory_result
        } else {
            match self.storage.lock() {
                Ok(mut storage) => storage.delete(self.db, key),
                Err(_) => None,
            }
        }
    }

    pub fn expire(&mut self, key: &str, ttl: TTL) -> Option<RediskValue> {
        if self.memory.is_mounted(self.db, key) {
            self.memory.expire(self.db, key, ttl)
        } else {
            match self.storage.lock() {
                Ok(mut storage) => storage.expire(self.db, key, ttl),
                Err(_) => None,
            }
        }
    }

    pub fn persist(&mut self, key: &str) -> Result<Option<RediskValue>> {
        if self.memory.is_mounted(self.db, key) {
            return Ok(self.memory.persist(self.db, key));
        }
        match self.storage.lock() {
            Ok(mut storage) => {
                Ok(storage.persist(self.db, key))
            },
            Err(e) => Err(Error::new(ErrorKind::Custom(String::from(e.to_string())))),
        }
    }
}

pub struct RedisServer {
    storage: Arc<Mutex<StorageEngine>>,
    cache: Arc<CacheLayer>,
    commands: Arc<HashMap<String, Box<dyn Command>>>,
}

impl RedisServer {
    pub fn new(storage: StorageEngine, nb_db: usize) -> Self {
        let commands: HashMap<String, Box<dyn Command>> = get_commands();
        Self {
            storage: Arc::new(Mutex::new(storage)),
            cache: Arc::new(CacheLayer::new(nb_db)),
            commands: Arc::new(commands),
        }
    }

    pub async fn run(&self, addr: &str) -> Result<()> {
        let listener = TcpListener::bind(addr).await?;
        println!("Server listening on {}", addr);

        loop {
            let (socket, _) = listener.accept().await?;
            let storage = self.storage.clone();
            let cache = self.cache.clone();
            let commands = self.commands.clone();
            tokio::spawn(async move {
                if let Err(e) = handle_connection(socket, storage, cache, commands).await {
                    eprintln!("Error handling connection: {}", e);
                }
            });
        }
    }
}

async fn handle_connection(
    mut socket: TcpStream,
    storage: Arc<Mutex<StorageEngine>>,
    cache: CacheLayer,
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
                let mut context = RediskCommandContext {
                    redisk_protocol: &protocol,
                    args,
                    storage: &storage,
                    memory: cache,
                    db: current_db,
                };
                let response = handle_command(&mut context, &commands).await;
                current_db = context.db;
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
            return context.redisk_protocol.serialize_error("wrong number of arguments for 'select' command");
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
        context.redisk_protocol.serialize_error(&format!("unknown command '{}'", cmd_name))
    }
}
