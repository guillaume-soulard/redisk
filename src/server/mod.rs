mod commands;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use crate::storage::StorageEngine;
use crate::cache::CacheLayer;
use crate::protocol::*;
use crate::server::commands::*;
use std::sync::{Arc, Mutex};
use anyhow::Result;

pub struct RedisServer {
    storage: Arc<Mutex<StorageEngine>>,
    cache: Arc<CacheLayer>,
}

impl RedisServer {
    pub fn new(storage: StorageEngine, cache_size: usize) -> Self {
        Self {
            storage: Arc::new(Mutex::new(storage)),
            cache: Arc::new(CacheLayer::new(cache_size)),
        }
    }

    pub async fn run(&self, addr: &str) -> Result<()> {
        let listener = TcpListener::bind(addr).await?;
        println!("Server listening on {}", addr);

        loop {
            let (socket, _) = listener.accept().await?;
            let storage = self.storage.clone();
            let cache = self.cache.clone();
            tokio::spawn(async move {
                if let Err(e) = handle_connection(socket, storage, cache).await {
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
) -> Result<()> {
    let mut buffer = vec![0; 1024];

    loop {
        let n = socket.read(&mut buffer).await?;
        if n == 0 {
            return Ok(());
        }

        match parse_command(&buffer[..n]) {
            Ok((args, _)) => {
                let response = handle_command(args, &storage, &cache).await;
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
) -> Vec<u8> {
    if args.is_empty() {
        return serialize_error("empty command");
    }

    let cmd = args[0].to_uppercase();
    match cmd.as_str() {
        "GET" => handle_get(args, storage, cache).await,
        "SET" => handle_set(args, storage, cache).await,
        "DEL" => handle_del(args, storage, cache).await,
        "PING" => handle_ping().await,
        _ => serialize_error(&format!("unknown command '{}'", cmd)),
    }
}
