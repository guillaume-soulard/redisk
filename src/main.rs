mod storage;
mod cache;
mod protocol;
mod server;

use crate::server::RedisServer;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let cache_size = 100;
    let nb_db = 16;
    let server = RedisServer::new(cache_size, nb_db);
    server.run("127.0.0.1:36379").await?;
    Ok(())
}
