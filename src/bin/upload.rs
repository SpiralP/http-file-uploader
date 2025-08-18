use std::{env::args, io::IsTerminal};

use anyhow::{Context, Result, bail};
use http_file_uploader::{guess_ext_from_reader_peek, logger, upload};
use reqwest::Body;
use tokio::{
    fs::File,
    io::{BufReader, stdin},
};

#[tokio::main]
async fn main() -> Result<()> {
    logger::initialize(true, Some(module_path!()));

    let path = if let Some(path) = args().nth(1) {
        path
    } else if std::io::stdin().is_terminal() {
        bail!("No file path provided and stdin is a terminal.");
    } else {
        "-".to_string()
    };

    let (ext, stream) = if path == "-" {
        let stdin = stdin();
        let stdin = BufReader::new(stdin);
        guess_ext_from_reader_peek(stdin).await?
    } else {
        let f = BufReader::new(File::open(&path).await.context("failed to open file")?);
        guess_ext_from_reader_peek(f).await?
    };
    let ext = args().nth(2).unwrap_or(ext);

    let body = Body::wrap_stream(stream);

    upload(body, &ext).await
}
