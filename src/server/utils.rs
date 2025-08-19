use std::{path::PathBuf, sync::LazyLock};

use tempfile::TempDir;
use tokio::sync::Mutex;
use tracing::warn;

static TEMP_DIR: LazyLock<Mutex<Option<TempDir>>> = LazyLock::new(|| {
    Mutex::new(Some(
        tempfile::tempdir().expect("Failed to create temporary directory"),
    ))
});

pub async fn get_temp_dir_path() -> PathBuf {
    let temp_dir = TEMP_DIR.lock().await;
    temp_dir.as_ref().unwrap().path().to_path_buf()
}

pub async fn cleanup_temp_dir() {
    if let Some(temp_dir) = TEMP_DIR.lock().await.take() {
        if let Err(e) = temp_dir.close() {
            warn!("Failed to clean up temporary directory: {e}");
        }
    }
}
