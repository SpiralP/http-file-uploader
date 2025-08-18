use std::{env::args, process::Stdio, str::FromStr, sync::LazyLock};

use anyhow::{Context, Result, ensure};
use http_file_uploader::{Input, pub_main};
use mime::{
    FromStrError, IMAGE_BMP, IMAGE_JPEG, IMAGE_PNG, TEXT_HTML, TEXT_HTML_UTF_8, TEXT_PLAIN,
    TEXT_PLAIN_UTF_8,
};
use mime_guess::Mime;
use reqwest::Body;
use tokio::process::Command;
use tokio_util::io::ReaderStream;

static BEST_MIME_EXTS: LazyLock<Vec<(Mime, &str)>> = LazyLock::new(|| {
    vec![
        (IMAGE_PNG, "png"),
        (IMAGE_JPEG, "jpg"),
        ("image/jpg".parse().unwrap(), "jpg"),
        ("image/webp".parse().unwrap(), "webp"),
        (IMAGE_BMP, "bmp"),
        (TEXT_HTML_UTF_8, "html"),
        (TEXT_HTML, "html"),
        (TEXT_PLAIN_UTF_8, "txt"),
        (TEXT_PLAIN, "txt"),
    ]
});

#[tokio::main]
async fn main() -> Result<()> {
    let (mime, ext) = match args().nth(1) {
        Some(s) => {
            let mime: Mime = s.parse()?;
            let ext = args().nth(2).unwrap_or_else(|| {
                let (_mime, ext) = get_best_mime_and_preferred_ext(&[&mime]).expect("TODO");
                ext.to_string()
            });

            (mime, ext)
        }
        None => {
            let existing_mimes = get_existing_mimes()
                .await
                .context("failed to get existing mimes")?;

            let (mime, ext) =
                get_best_mime_and_preferred_ext(existing_mimes.iter().collect::<Vec<_>>().as_ref())
                    .context("couldn't find any mime types???")?;

            (mime.clone(), ext.to_string())
        }
    };

    println!("{mime:?}");
    println!("{ext:?}");

    let stdout = Stdio::piped();
    let cmd = Command::new("wl-paste")
        .args(["--type", mime.as_ref()])
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(Stdio::inherit())
        .spawn()?;

    let stdout = cmd.stdout.unwrap();

    // cmd.wait_with_output().await?;
    // ensure!(output.status.success());

    let body = Body::wrap_stream(ReaderStream::new(stdout));
    pub_main(Input::Body(body), &ext).await?;

    Ok(())
}

fn get_best_mime_and_preferred_ext<'a>(mimes: &[&'a Mime]) -> Option<(&'a Mime, &'static str)> {
    BEST_MIME_EXTS
        .iter()
        .find_map(|(mime, ext)| {
            // make sure to return the mime given to us by `wl-paste` (space between params matters)
            mimes
                .iter()
                .find(|&&existing_mime| existing_mime == mime)
                .map(|mime| (*mime, *ext))
        })
        .or_else(|| {
            mimes.first().map(|mime| {
                let ext = mime_guess::get_mime_extensions(mime)
                    .and_then(|ext| ext.first())
                    .unwrap_or(&"txt");
                (*mime, *ext)
            })
        })
}

async fn get_existing_mimes() -> Result<Vec<Mime>> {
    let cmd = Command::new("wl-paste")
        .arg("--list-types")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;

    let output = cmd.wait_with_output().await?;
    ensure!(output.status.success());

    let output = String::from_utf8(output.stdout)?;
    let output = output.trim();
    let existing_mimes = output
        .split_ascii_whitespace()
        .map(Mime::from_str)
        .collect::<std::result::Result<Vec<_>, FromStrError>>()?;

    Ok(existing_mimes)
}
