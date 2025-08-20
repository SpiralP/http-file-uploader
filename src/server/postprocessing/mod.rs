mod html;
mod md;

use anyhow::{Context, Result};
use warp::reply::Reply;

use crate::server::postprocessing::{html::process_html, md::process_markdown};

pub async fn process(f: warp::fs::File) -> Result<impl Reply> {
    let ext = f.path().extension().context("extension() None")?;
    let reply: Box<dyn Reply> = match ext.to_str().context("to_str() None")? {
        "md" => Box::new(process_markdown(f).await?),
        "html" => Box::new(process_html(f).await?),
        _ => Box::new(f),
    };

    Ok(reply)
}
