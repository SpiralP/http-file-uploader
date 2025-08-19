use std::{path::PathBuf, time::Duration};

use anyhow::Result;
use bytes::Buf;
use tokio::{
    fs::File,
    io::{AsyncWriteExt, BufWriter},
};
use tracing::{debug, info, warn};
use warp::{
    Filter,
    filters::{BoxedFilter, log::log},
    reject::{self, MethodNotAllowed, Rejection},
    reply::Reply,
};

use crate::server::{naming::get_random_word_string, utils::get_temp_dir_path};

const RETENTION_DURATION: Duration = Duration::from_secs(60 * 60 * 24 * 7); // 7 days

#[derive(Debug)]
struct ServerError;

impl reject::Reject for ServerError {}

pub fn get_routes(dir: PathBuf, upload_token: String) -> BoxedFilter<(impl Reply,)> {
    let file_route = warp::fs::dir(dir.to_path_buf());
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

    file_route
        .or(upload_route)
        .with(log(module_path!()))
        .or_else(|rejection: Rejection| async move {
            if rejection.find::<MethodNotAllowed>().is_some() {
                Err(warp::reject::not_found())
            } else {
                Err(rejection)
            }
        })
        .boxed()
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
