use std::env;

use anyhow::{Context, Result};
use reqwest::Body;
use tokio::{
    fs::File,
    io::{BufReader, stdin},
};
use tokio_util::io::ReaderStream;

pub enum Input {
    File(String),
    Stdin,
    Body(Body),
}

pub async fn pub_main(input: Input, ext: &str) -> Result<()> {
    let upload_token = env::var("UPLOAD_TOKEN").context("UPLOAD_TOKEN must be set")?;
    let url = env::var("URL").context("URL must be set")?;

    let url = url.trim_end_matches('/');
    let url = format!("{url}/upload.{ext}");

    let body = match input {
        Input::File(file_path) => File::open(file_path).await?.into(),
        Input::Stdin => {
            let stdin = stdin();
            let reader = BufReader::new(stdin);
            Body::wrap_stream(ReaderStream::new(reader))
        }
        Input::Body(body) => body,
    };

    let client = reqwest::Client::new();
    let res = client
        .post(url)
        .header("Authorization", format!("Bearer {upload_token}"))
        .body(body)
        .send()
        .await?;
    res.error_for_status()?;

    Ok(())
}
