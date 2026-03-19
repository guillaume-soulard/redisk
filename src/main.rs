mod storage;
mod cache;
mod protocol;
mod server;

use crate::storage::StorageEngine;
use crate::server::RedisServer;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let storage_path = "data.redisk";
    let storage = StorageEngine::new(storage_path)?;
    
    // Configurer le cache (capacité de 100 entrées pour l'exemple)
    let cache_size = 100;
    
    let server = RedisServer::new(storage, cache_size);
    server.run("127.0.0.1:36379").await?;

    Ok(())
}
