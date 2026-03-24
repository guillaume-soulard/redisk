mod storage;
mod cache;
mod protocol;
mod server;

use crate::storage::StorageEngine;
use crate::server::RedisServer;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let storage_path = "data.rdat";
    let storage = StorageEngine::new(storage_path)?;
    let cache_size = 100;
    let server = RedisServer::new(storage, cache_size);
    server.run("127.0.0.1:36379").await?;
    Ok(())
}
