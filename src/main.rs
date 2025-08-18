mod logger;
mod naming;

use std::{env, path::PathBuf, sync::LazyLock, time::Duration};

use anyhow::{Context, Result};
use bytes::Buf;
use tempfile::TempDir;
use tokio::{
    fs::File,
    io::{AsyncWriteExt, BufWriter},
    sync::Mutex,
};
use tracing::{debug, info, warn};
use warp::{
    Filter,
    filters::log::log,
    reject::{self, MethodNotAllowed, Rejection},
};

use crate::naming::get_random_word_string;

const RETENTION_DURATION: Duration = Duration::from_secs(60 * 60 * 24 * 7); // 7 days

static TEMP_DIR: LazyLock<Mutex<Option<TempDir>>> = LazyLock::new(|| {
    Mutex::new(Some(
        tempfile::tempdir().expect("Failed to create temporary directory"),
    ))
});

async fn get_temp_dir_path() -> PathBuf {
    let temp_dir = TEMP_DIR.lock().await;
    temp_dir.as_ref().unwrap().path().to_path_buf()
}

#[derive(Debug)]
struct ServerError;

impl reject::Reject for ServerError {}

#[tokio::main]
async fn main() -> Result<()> {
    let upload_token = env::var("UPLOAD_TOKEN").context("UPLOAD_TOKEN must be set")?;
    let port = env::var("PORT")
        .unwrap_or_else(|_| "3030".to_string())
        .parse::<u16>()
        .context("PORT must be a valid u16")?;

    logger::initialize(true, Some(module_path!()));

    let temp_dir = get_temp_dir_path().await;
    debug!("{}", temp_dir.display());

    let file_route = warp::fs::dir(temp_dir.to_path_buf());
    let upload_route = warp::post()
        .and(warp::path::param())
        .and_then(|param: String| async move {
            if let Some((before, ext)) = param.split_once('.') {
                if before == "upload"
                    && !ext.is_empty()
                    && ext.len() <= 5
                    && ext.chars().all(|c| c.is_ascii_alphanumeric())
                {
                    return Ok(ext.to_string());
                }
            }

            Err(warp::reject::not_found())
        })
        .and(warp::path::end())
        .and(warp::header::header("authorization"))
        .and_then(move |ext: String, auth: String| {
            let upload_token = upload_token.clone();
            async move {
                if auth == format!("Bearer {upload_token}") {
                    Ok(ext)
                } else {
                    Err(warp::reject::not_found())
                }
            }
        })
        .and(warp::body::aggregate())
        .and_then(|ext, stream| async move {
            upload_file(ext, stream).await.map_err(|e| {
                warn!("Error uploading file: {e}");
                warp::reject::custom(ServerError)
            })
        });

    debug!("Starting server on 0.0.0.0:{port}");
    warp::serve(
        file_route
            .or(upload_route)
            .with(log(module_path!()))
            .or_else(|rejection: Rejection| async move {
                if rejection.find::<MethodNotAllowed>().is_some() {
                    Err(warp::reject::not_found())
                } else {
                    Err(rejection)
                }
            }),
    )
    .bind(([0, 0, 0, 0], port))
    .await
    .graceful(async move {
        info!("Listening on 0.0.0.0:{port}");

        #[cfg(unix)]
        {
            use tokio::signal::unix::{SignalKind, signal};

            let mut interrupt = signal(SignalKind::interrupt()).unwrap();
            let mut terminate = signal(SignalKind::terminate()).unwrap();
            let mut hangup = signal(SignalKind::hangup()).unwrap();
            tokio::select! {
                _ = interrupt.recv() => { debug!("got SIGINT"); },
                _ = terminate.recv() => { debug!("got SIGTERM"); },
                _ = hangup.recv() => { debug!("got SIGHUP"); },
            };
        }
        if cfg!(not(unix)) {
            use tokio::signal::ctrl_c;

            ctrl_c().await.unwrap();
            debug!("got Ctrl-C");
        }

        info!("Shutting down");

        if let Some(temp_dir) = TEMP_DIR.lock().await.take() {
            if let Err(e) = temp_dir.close() {
                warn!("Failed to clean up temporary directory: {e}");
            }
        }
    })
    .run()
    .await;

    Ok(())
}

async fn upload_file(ext: String, mut buf: impl Buf) -> Result<impl warp::Reply> {
    let temp_dir = get_temp_dir_path().await;

    let filename = format!("{}.{ext}", get_random_word_string());
    let filepath = temp_dir.join(&filename);
    if filepath.exists() {
        warn!("file {filename} already exists, replacing!!");
    }
    debug!("writing {filename}");

    let bytes_written = {
        let f = File::create(filepath).await?;
        let mut writer = BufWriter::new(f);
        let mut bytes_written = 0;
        while buf.has_remaining() {
            let chunk = buf.chunk();
            let len = chunk.len();
            writer.write_all(chunk).await?;
            buf.advance(len);
            bytes_written += len;
        }
        writer.flush().await?;
        bytes_written
    };
    debug!("wrote {bytes_written} bytes to {filename}");

    tokio::spawn({
        let filename = filename.clone();
        async move {
            tokio::time::sleep(RETENTION_DURATION).await;
            info!("Deleting {filename}");
            if let Err(e) = tokio::fs::remove_file(temp_dir.join(&filename)).await {
                warn!("Failed to delete file {filename}: {e}");
            }
        }
    });

    Ok(filename)
}
