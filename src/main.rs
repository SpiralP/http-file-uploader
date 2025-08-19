mod logger;
mod server;

use std::env;

use anyhow::{Context, Result};

use crate::server::run_server;

#[tokio::main]
async fn main() -> Result<()> {
    let upload_token = env::var("UPLOAD_TOKEN").context("UPLOAD_TOKEN must be set")?;
    let port = env::var("PORT")
        .unwrap_or_else(|_| "3030".to_string())
        .parse::<u16>()
        .context("PORT must be a valid u16")?;

    logger::initialize(true, Some(module_path!()));

    run_server(port, upload_token).await;

    Ok(())
}
