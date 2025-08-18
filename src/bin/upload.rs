use std::env::args;

use anyhow::Result;
use http_file_uploader::{Input, pub_main};

#[tokio::main]
async fn main() -> Result<()> {
    let file_path = args().nth(1).unwrap_or_else(|| "-".to_string());
    let input = if file_path == "-" {
        Input::Stdin
    } else {
        Input::File(file_path)
    };
    let ext = args().nth(2).unwrap_or_else(|| "txt".to_string());

    pub_main(input, &ext).await
}
