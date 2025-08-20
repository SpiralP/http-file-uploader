pub mod logger;

use std::{
    env,
    io::{IsTerminal, stdout},
    path::PathBuf,
};

use anyhow::{Context, Error, Result};
use bytes::Bytes;
use futures::StreamExt;
use reqwest::Body;
use tokio::{
    fs::File,
    io::{AsyncRead, AsyncReadExt, BufReader, stdin},
};
use tokio_stream::Stream;
use tokio_util::io::ReaderStream;
use tracing::{debug, warn};

pub async fn upload(body: Body, ext: &str) -> Result<()> {
    let upload_token = env::var("UPLOAD_TOKEN").context("UPLOAD_TOKEN must be set")?;
    let url = env::var("URL").context("URL must be set")?;

    let url = url.trim_end_matches('/');
    let upload_url = format!("{url}/upload.{ext}");

    debug!(?ext, "uploading");
    let client = reqwest::Client::new();
    let res = client
        .post(upload_url)
        .header("Authorization", format!("Bearer {upload_token}"))
        .body(body)
        .send()
        .await?;
    let res = res.error_for_status()?;
    let text = res.text().await?;

    if stdout().is_terminal() {
        println!("{url}/{text}");
    } else {
        print!("{url}/{text}");
    }

    Ok(())
}

pub async fn upload_files(paths: Vec<PathBuf>) -> Result<()> {
    let _ = tokio_stream::iter(paths)
        .map(|path| async move {
            let result = {
                let path = path.to_path_buf();
                async move {
                    // handle stdin from `upload`
                    let (ext, body) = if path.to_string_lossy() == "-" {
                        let stdin = stdin();
                        let stdin = BufReader::new(stdin);
                        let (ext, stream) = guess_ext_from_reader_peek(stdin).await?;
                        (ext, Body::wrap_stream(stream))
                    } else {
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

                        if let Some(ext) = maybe_ext {
                            (ext, Body::wrap_stream(ReaderStream::new(f)))
                        } else {
                            debug!("peeking file to see if it's utf8...");
                            // peek file to see if it's text, else use "bin"

                            let (ext, stream) = guess_ext_from_reader_peek(f).await?;
                            (ext, Body::wrap_stream(stream))
                        }
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

    Ok(())
}

pub type BoxStream =
    Box<dyn Stream<Item = std::result::Result<Bytes, std::io::Error>> + Send + Unpin>;

pub async fn peek_as_stream<R, Ret, F>(mut reader: R, mut f: F) -> Result<(Ret, BoxStream)>
where
    R: AsyncRead + Unpin + Send + 'static,
    F: FnMut(&Bytes) -> Result<Ret>,
{
    let mut total_buf = vec![0; 1024 * 1024]; // 1 MiB
    let mut total_bytes_read = 0;
    let mut got_eof = false;
    while total_bytes_read < total_buf.len() {
        let buf = &mut total_buf[total_bytes_read..];
        let bytes_read = reader.read(buf).await?;
        if bytes_read == 0 {
            got_eof = true;
            break;
        }
        total_bytes_read += bytes_read;
    }
    let first_chunk = Bytes::from(total_buf[..total_bytes_read].to_vec());
    let ret = f(&first_chunk)?;

    let stream = tokio_stream::once(Ok(first_chunk));
    Ok((
        ret,
        if got_eof {
            Box::new(stream)
        } else {
            Box::new(stream.chain(ReaderStream::new(reader)))
        },
    ))
}

pub const UNKNOWN_EXT: &str = "bin";
pub const TEXT_EXT: &str = "txt";

pub fn guess_ext_from_bytes(bytes: &[u8]) -> String {
    if let Some(type_) = infer::get(bytes) {
        return type_.extension().to_string();
    }
    if str::from_utf8(bytes).is_ok() {
        return TEXT_EXT.to_string();
    }

    UNKNOWN_EXT.to_string()
}

pub async fn guess_ext_from_reader_peek<R>(reader: R) -> Result<(String, BoxStream)>
where
    R: AsyncRead + Unpin + Send + 'static,
{
    let (maybe_ext, stream) =
        peek_as_stream(reader, |first_chunk| Ok(guess_ext_from_bytes(first_chunk))).await?;

    Ok((maybe_ext, stream))
}
