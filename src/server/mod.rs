mod commands;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use crate::storage::StorageEngine;
use crate::cache::CacheLayer;
use crate::protocol::*;
use crate::server::commands::*;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use anyhow::Result;

struct RediskCommandContext<'a> {
    redisk_protocol: &'a RediskProtocol,
    args: Vec<String>,
    storage: &'a Arc<Mutex<StorageEngine>>,
    memory: &'a Arc<CacheLayer>,
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
            Box::new(|args, storage, cache| Box::pin(handle_get(args, storage, cache))),
        );
        commands.insert(
            "SET".to_string(),
            Box::new(|args, storage, cache| Box::pin(handle_set(args, storage, cache))),
        );
        commands.insert(
            "DEL".to_string(),
            Box::new(|args, storage, cache| Box::pin(handle_del(args, storage, cache))),
        );
        commands.insert(
            "PING".to_string(),
            Box::new(|args, storage, cache| Box::pin(handle_ping(args, storage, cache))),
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
        memory: &Arc::new(()),
    };
    loop {
        let n = socket.read(&mut buffer).await?;
        if n == 0 {
            return Ok(());
        }

        match parse_command(&buffer[..n]) {
            Ok((args, _)) => {
                let response = handle_command(args, &storage, &cache, &commands).await;
                socket.write_all(&response).await?;
            }
            Err(e) => {
                let error_msg = serialize_error(&e.to_string());
                socket.write_all(&error_msg).await?;
            }
        }
    }
}

async fn handle_command(
    args: Vec<String>,
    storage: &Arc<Mutex<StorageEngine>>,
    cache: &Arc<CacheLayer>,
    commands: &HashMap<String, CommandHandler>,
) -> Vec<u8> {
    if args.is_empty() {
        return serialize_error("empty command");
    }

    let cmd_name = args[0].to_uppercase();
    if let Some(handler) = commands.get(&cmd_name) {
        handler(args, storage, cache).await
    } else {
        serialize_error(&format!("unknown command '{}'", cmd_name))
    }
}
