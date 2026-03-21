mod commands;
use crate::cache::CacheLayer;
use crate::protocol::*;
use crate::server::commands::*;
use crate::storage::StorageEngine;
use anyhow::{Error, Result};
use bincode::ErrorKind;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

struct RediskCommandContext<'a> {
    redisk_protocol: &'a RediskProtocol,
    args: Vec<String>,
    storage: &'a Arc<Mutex<StorageEngine>>,
    memory: &'a Arc<CacheLayer>,
}

impl RediskCommandContext<'_> {
    pub fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        match self.memory.get(key) {
            Some(value) => Ok(Some(value)),
            None => {
                match self.storage.lock() {
                    Ok(mut storage) => match storage.get(key) {
                        Ok(value) => Ok(value),
                        Err(err) => Err(err),
                    },
                    Err(e) => Err(Error::new(ErrorKind::Custom(String::from(e.to_string())))),
                }
            },
        }
    }

    pub fn set(&self, key: &str, value: &Vec<u8>) -> Result<()> {
        match self.storage.lock() {
            Ok(mut storage) => match storage.set(key.to_string(), value.clone()) {
                Ok(_) => {
                    self.memory.set(key.to_string(), value.clone());
                    Ok(())
                },
                Err(err) => Err(Error::new(ErrorKind::Custom(String::from(err.to_string())))),
            },
            Err(e) => Err(Error::new(ErrorKind::Custom(String::from(e.to_string())))),
        }
    }

    pub fn delete(&self, key: &str) -> Result<()> {
        self.memory.delete(key);
        match self.storage.lock() {
            Ok(mut storage) => match storage.delete(key) {
                Ok(_) => Ok(()),
                Err(err) => Err(Error::new(ErrorKind::Custom(String::from(err.to_string())))),
            },
            Err(e) => Err(Error::new(ErrorKind::Custom(String::from(e.to_string())))),
        }
    }
}

type CommandHandler = Box<
    dyn for<'a> Fn(
            &'a RediskCommandContext,
        ) -> Pin<Box<dyn Future<Output = Vec<u8>> + Send + 'a>>
        + Send
        + Sync,
>;

pub struct RedisServer {
    storage: Arc<Mutex<StorageEngine>>,
    cache: Arc<CacheLayer>,
    commands: Arc<HashMap<String, CommandHandler>>,
}

impl RedisServer {
    pub fn new(storage: StorageEngine, cache_size: usize) -> Self {
        let mut commands: HashMap<String, CommandHandler> = HashMap::new();

        commands.insert(
            "GET".to_string(),
            Box::new(|context| Box::pin(handle_get(context))),
        );
        commands.insert(
            "SET".to_string(),
            Box::new(|context| Box::pin(handle_set(context))),
        );
        commands.insert(
            "DEL".to_string(),
            Box::new(|context| Box::pin(handle_del(context))),
        );
        commands.insert(
            "PING".to_string(),
            Box::new(|context| Box::pin(handle_ping(context))),
        );

        Self {
            storage: Arc::new(Mutex::new(storage)),
            cache: Arc::new(CacheLayer::new(cache_size)),
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
    commands: Arc<HashMap<String, CommandHandler>>,
) -> Result<()> {
    let mut buffer = vec![0; 1024];
    let mut context: RediskCommandContext = RediskCommandContext {
      redisk_protocol: &new_redisk_protocol(2),
        args: vec![],
        storage: &storage,
        memory: &cache,
    };
    loop {
        let n = socket.read(&mut buffer).await?;
        if n == 0 {
            return Ok(());
        }

        match parse_command(&buffer[..n]) {
            Ok((args, _)) => {
                context.args = args;
                let response = handle_command(&context, &commands).await;
                socket.write_all(&response).await?;
            }
            Err(e) => {
                let error_msg = context.redisk_protocol.serialize_error(&e.to_string());
                socket.write_all(&error_msg).await?;
            }
        }
    }
}

async fn handle_command(
    context: &RediskCommandContext<'_>,
    commands: &HashMap<String, CommandHandler>,
) -> Vec<u8> {
    if context.args.is_empty() {
        return context.redisk_protocol.serialize_error("empty command");
    }

    let cmd_name = context.args[0].to_uppercase();
    if let Some(handler) = commands.get(&cmd_name) {
        handler(context).await
    } else {
        context.redisk_protocol.serialize_error(&format!("unknown command '{}'", cmd_name))
    }
}
