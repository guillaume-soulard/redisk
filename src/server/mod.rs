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
                match self.storage.lock().unwrap().set(self.db, key.to_string(), value.clone(), ttl) {
                    Ok(Some(v)) => Some(v),
                    Ok(None) => None,
                    Err(_) => None,
                }
            },
            None => None,
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, (TTL, &RediskValue))> {
        self.memory.iter(self.db)
    }
    pub fn get(&self, key: &str) -> Result<Option<RediskValue>> {
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
        match self.storage.lock() {
            Ok(mut storage) => match storage.set(self.db, key.to_string(), value.clone(), ttl) {
                Ok(_) => {
                    self.memory.set(self.db, key.to_string(), value.clone(), ttl);
                    Ok(())
                },
                Err(err) => Err(Error::new(ErrorKind::Custom(String::from(err.to_string())))),
            },
            Err(e) => Err(Error::new(ErrorKind::Custom(String::from(e.to_string())))),
        }
    }

    pub fn delete(&self, key: &str) -> Result<Option<RediskValue>> {
        self.memory.delete(self.db, key);
        match self.storage.lock() {
            Ok(mut storage) => match storage.delete(self.db, key) {
                Ok(_) => Ok(()),
                Err(err) => Err(Error::new(ErrorKind::Custom(String::from(err.to_string())))),
            },
            Err(e) => Err(Error::new(ErrorKind::Custom(String::from(e.to_string())))),
        }
    }

    pub fn expire(&self, key: &str, ttl: TTL) -> Result<Option<RediskValue>> {
        match self.storage.lock() {
            Ok(mut storage) => {
                let success = storage.expire(self.db, key, seconds)?;
                if success {
                    self.memory.expire(self.db, key, seconds);
                }
                Ok(success)
            },
            Err(e) => Err(Error::new(ErrorKind::Custom(String::from(e.to_string())))),
        }
    }

    pub fn persist(&self, key: &str) -> Result<Option<RediskValue>> {
        match self.storage.lock() {
            Ok(mut storage) => {
                let success = storage.persist(self.db, key)?;
                if success {
                    self.memory.persist(self.db, key);
                }
                Ok(success)
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
    pub fn new(storage: StorageEngine, cache_size: usize, nb_db: usize) -> Self {
        let commands: HashMap<String, Box<dyn Command>> = get_commands();
        Self {
            storage: Arc::new(Mutex::new(storage)),
            cache: Arc::new(CacheLayer::new(cache_size, nb_db)),
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
