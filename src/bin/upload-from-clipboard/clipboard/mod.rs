#[cfg(unix)]
mod wl_paste;

use std::process::Stdio;

use anyhow::{Result, ensure};
use mime_guess::Mime;
use tokio::{
    io::{AsyncReadExt, BufReader},
    process::{ChildStdout, Command},
};

#[cfg(unix)]
pub use self::wl_paste::*;

async fn stream_command<T, Fut, F>(prog: &str, args: &[&str], mut f: F) -> Result<T>
where
    F: FnMut(BufReader<ChildStdout>) -> Fut,
    Fut: Future<Output = Result<T>>,
{
    let mut cmd = Command::new(prog)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;

    let ret = {
        let stdout = cmd.stdout.take().unwrap();
        let reader = BufReader::new(stdout);

        f(reader).await?
    };

    let status = cmd.wait().await?;
    ensure!(status.success());

    Ok(ret)
}

pub async fn get_existing_mimes() -> Result<Vec<Mime>> {
    let (prog, args) = make_existing_mimes_command();
    let stdout = stream_command(prog, args, |mut stdout| async move {
        let mut s = String::new();
        stdout.read_to_string(&mut s).await?;
        Ok(s)
    })
    .await?;

    let existing_mimes = parse_existing_mimes(stdout);

    Ok(existing_mimes)
}

pub async fn get_clipboard_stream<T, Fut, F>(mime: &str, f: F) -> Result<T>
where
    F: FnMut(BufReader<ChildStdout>) -> Fut,
    Fut: Future<Output = Result<T>>,
{
    let (prog, args) = make_clipboard_command(mime);
    stream_command(prog, &args, f).await
}

pub async fn get_clipboard_output(mime: &str) -> Result<String> {
    get_clipboard_stream(mime, |mut stdout| async move {
        let mut s = String::new();
        stdout.read_to_string(&mut s).await?;
        Ok(s)
    })
    .await
}
