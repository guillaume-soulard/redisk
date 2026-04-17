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
    memory: &'a Arc<CacheLayer>,
    db: u32,
}

pub type TTL = Option<u64>;

pub type RediskValue = Vec<u8>;

impl RediskCommandContext<'_> {
    pub fn mount(&self, key: &str, ttl: TTL) -> Option<RediskValue> {
        let storage = self.storage.lock().unwrap().get(self.db, key);
        match storage {
            Ok(Some(value)) => {
                self.memory.set(self.db, key.to_string(), value, ttl)
                    .map(|v| v.1)
            },
            Ok(None) => None,
            Err(_) => None,
        }
    }

    pub fn unmount(&self, key: &str, ttl: TTL) -> Option<RediskValue> {
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

    pub fn iter(&self) -> impl Iterator<Item = (&String, (TTL, &RediskValue))> {
        let memory_values = self.memory.iter(self.db);
        memory_values.chain(self.storage.lock().unwrap().iter(self.db))
    }

    pub fn get(&self, key: &str) -> Result<Option<RediskValue>> {
        if self.memory.is_mounted(self.db, key) {
            return Ok(self.memory.get(self.db, key).map(|v| v.1));
        }
        match self.memory.get(self.db, key) {
            Some(value) => Ok(Some(value.1)),
            None => {
                match self.storage.lock() {
                    Ok(mut storage) => match storage.get(self.db, key) {
                        Ok(value) => Ok(value),
                        Err(err) => Err(err),
                    },
                    Err(e) => Err(Error::new(ErrorKind::Custom(String::from(e.to_string())))),
                }
            },
        }
    }

    pub fn set(&self, key: &str, value: &RediskValue, ttl: TTL) -> Result<Option<RediskValue>> {
        if self.memory.is_mounted(self.db, key) {
            self.memory.set(self.db, key.to_string(), value.clone(), ttl);
            return Ok(None);
        }

        match self.storage.lock() {
            Ok(mut storage) => match storage.set(self.db, key.to_string(), value.clone(), ttl) {
                Ok(_) => {
                    Ok(
                        self.memory.set(self.db, key.to_string(), value.clone(), ttl)
                        .map(|v| v.1)
                    )
                },
                Err(err) => Err(Error::new(ErrorKind::Custom(String::from(err.to_string())))),
            },
            Err(e) => Err(Error::new(ErrorKind::Custom(String::from(e.to_string())))),
        }
    }

    pub fn delete(&self, key: &str) -> Result<Option<RediskValue>> {
        let memory_result = self.memory.delete(self.db, key);
        if self.memory.is_mounted(self.db, key) {
            Ok(memory_result)
        } else {
            match self.storage.lock() {
                Ok(mut storage) => match storage.delete(self.db, key) {
                    Ok(v) => Ok(v),
                    Err(err) => Err(Error::new(ErrorKind::Custom(String::from(err.to_string())))),
                },
                Err(e) => Err(Error::new(ErrorKind::Custom(String::from(e.to_string())))),
            }
        }
    }

    pub fn expire(&self, key: &str, ttl: TTL) -> Result<Option<RediskValue>> {
        if self.memory.is_mounted(self.db, key) {
            return Ok(self.memory.expire(self.db, key, ttl));
        }
        match self.storage.lock() {
            Ok(mut storage) => {
                Ok(storage.expire(self.db, key, ttl))
            },
            Err(e) => Err(Error::new(ErrorKind::Custom(String::from(e.to_string())))),
        }
    }

    pub fn persist(&self, key: &str) -> Result<Option<RediskValue>> {
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
    cache: Arc<CacheLayer>,
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
                    memory: &cache,
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
