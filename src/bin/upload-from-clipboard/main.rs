mod clipboard;

use std::{env::args, sync::LazyLock};

use anyhow::{Context, Error, Result, bail};
use futures::{StreamExt, stream};
use http_file_uploader::{guess_ext_from_reader_peek, logger, upload};
use mime::{
    IMAGE_BMP, IMAGE_JPEG, IMAGE_PNG, TEXT_HTML, TEXT_HTML_UTF_8, TEXT_PLAIN, TEXT_PLAIN_UTF_8,
};
use mime_guess::Mime;
use reqwest::Body;
use tokio::{fs::File, io::BufReader};
use tokio_util::io::ReaderStream;
use tracing::{debug, warn};
use url::Url;

use self::clipboard::{get_clipboard_output, get_clipboard_stream, get_existing_mimes};

static BEST_MIME_EXTS: LazyLock<Vec<(Mime, Option<&str>)>> = LazyLock::new(|| {
    vec![
        ("text/uri-list".parse().unwrap(), None),
        (IMAGE_PNG, Some("png")),
        (IMAGE_JPEG, Some("jpg")),
        ("image/jpg".parse().unwrap(), Some("jpg")),
        ("image/webp".parse().unwrap(), Some("webp")),
        (IMAGE_BMP, Some("bmp")),
        (TEXT_HTML_UTF_8, Some("html")),
        (TEXT_HTML, Some("html")),
        ("text/markdown".parse().unwrap(), Some("md")),
        (TEXT_PLAIN_UTF_8, Some("txt")),
        (TEXT_PLAIN, Some("txt")),
    ]
});

#[tokio::main]
async fn main() -> Result<()> {
    logger::initialize(true, Some(module_path!()));

    let existing_mimes = get_existing_mimes()
        .await
        .context("failed to get existing mimes")?;
    debug!(?existing_mimes);

    let (mime, maybe_ext) = match args().nth(1) {
        Some(s) => {
            let mime: Mime = s.parse()?;

            // make sure to return the mime given to us by `wl-paste` (space between params matters)
            let mime = existing_mimes
                .iter()
                .find_map(|existing_mime| {
                    if existing_mime == &mime {
                        Some(existing_mime.clone())
                    } else {
                        None
                    }
                })
                .unwrap_or(mime);

            let maybe_ext = if let Some(ext) = args().nth(2) {
                Some(ext)
            } else {
                BEST_MIME_EXTS.iter().find_map(|(best_mime, maybe_ext)| {
                    if best_mime == &mime {
                        //
                        maybe_ext.map(|s| s.to_string())
                    } else {
                        None
                    }
                })
            };

            (mime, maybe_ext)
        }
        None => {
            // make sure to return the mime given to us by `wl-paste` (space between params matters)
            let (mime, maybe_ext) = if let Some((mime, maybe_ext)) =
                BEST_MIME_EXTS.iter().find_map(|(best_mime, maybe_ext)| {
                    existing_mimes.iter().find_map(|existing_mime| {
                        if existing_mime == best_mime {
                            Some((existing_mime.clone(), maybe_ext.map(|s| s.to_string())))
                        } else {
                            None
                        }
                    })
                }) {
                (mime, maybe_ext)
            } else {
                // take first existing mime (not in best)
                (
                    existing_mimes
                        .into_iter()
                        .next()
                        .context("no existing mimes found")?,
                    None,
                )
            };

            (mime, maybe_ext)
        }
    };

    if mime != "text/uri-list" {
        // normal clipboard content, stream it

        get_clipboard_stream(mime.as_ref(), |stdout| {
            let mime = mime.clone();
            let maybe_ext = maybe_ext.clone();
            async move {
                let (ext, body) = if let Some(ext) = maybe_ext.or_else(|| {
                    mime_guess::get_mime_extensions(&mime).and_then(|exts| {
                        // take first extension
                        exts.first().map(|s| s.to_string())
                    })
                }) {
                    (ext, Body::wrap_stream(ReaderStream::new(stdout)))
                } else {
                    // guess extension from first MiB
                    let (ext, stream) = guess_ext_from_reader_peek(stdout).await?;
                    (ext, Body::wrap_stream(stream))
                };

                debug!(?mime, ?ext);

                upload(body, &ext).await.context("failed to upload")?;

                Ok(())
            }
        })
        .await?;
    } else {
        // read files if "text/uri-list"

        let output = get_clipboard_output(mime.as_ref()).await?;
        let output = output.trim();
        let file_paths = output
            .split_ascii_whitespace()
            .map(|s| {
                let url: Url = s
                    .parse()
                    .with_context(|| format!("failed to parse URI {s:?}"))?;

                if url.scheme() != "file" {
                    bail!("skipping non-file URI: {url}");
                }

                let path = url
                    .to_file_path()
                    .map_err(|_| anyhow::anyhow!("failed to convert URL to file path: {url:?}"))?;
                if !path.exists() {
                    bail!("file does not exist: {path:?}");
                }

                Ok(path)
            })
            .filter_map(|result| match result {
                Ok(path) => Some(path),
                Err(e) => {
                    warn!("{e}");
                    None
                }
            })
            .collect::<Vec<_>>();

        let _ = stream::iter(file_paths)
            .map(|path| async move {
                let result = {
                    let path = path.to_path_buf();
                    async move {
                        let maybe_ext = if let Some(ext) = path.extension() {
                            // use ext from path
                            let ext = ext.to_str().with_context(|| {
                                format!("failed to convert extension to str: {ext:?}")
                            })?;
                            Some(ext.to_string())
                        } else {
                            None
                        };

                        let f =
                            BufReader::new(File::open(&path).await.context("failed to open file")?);

                        let (ext, body) = if let Some(ext) = maybe_ext {
                            (ext, Body::wrap_stream(ReaderStream::new(f)))
                        } else {
                            debug!("peeking file to see if it's utf8...");
                            // peek file to see if it's text, else use "bin"

                            let (ext, stream) = guess_ext_from_reader_peek(f).await?;
                            (ext, Body::wrap_stream(stream))
                        };

                        upload(body, &ext).await.context("failed to upload")?;

                        Ok::<_, Error>(())
                    }
                }
                .await;

                (path, result)
            })
            .buffer_unordered(4)
            .filter_map(|(path, result)| async move {
                match result {
                    Ok(v) => Some(v),
                    Err(e) => {
                        warn!("Failed to upload file {path:?} {e:?}");
                        None
                    }
                }
            })
            .collect::<Vec<_>>()
            .await;
    }

    Ok(())
}
