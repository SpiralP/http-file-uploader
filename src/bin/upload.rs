use std::{env::args, io::IsTerminal, path::PathBuf};

use anyhow::{Result, bail};
use http_file_uploader::{logger, upload_files};

#[tokio::main]
async fn main() -> Result<()> {
    logger::initialize(true, Some(module_path!()));

    let mut paths = args().skip(1).map(PathBuf::from).collect::<Vec<_>>();

    if paths.is_empty() {
        if std::io::stdin().is_terminal() {
            bail!("No file paths provided and stdin is a terminal.");
        } else {
            paths.push("-".into());
        }
    }

    upload_files(paths).await?;

    Ok(())
}
