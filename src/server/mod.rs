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
