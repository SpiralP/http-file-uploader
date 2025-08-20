use std::str::Lines;

use anyhow::{Context, Result};
use headers::{ContentType, HeaderMapExt};
use tokio::fs;
use warp::reply::Response;

const HTML: &str = include_str!("./md.html");

pub async fn process_markdown(f: warp::fs::File) -> Result<Response> {
    let file_name = f.path().file_name().context("file_name() None")?;
    let contents = fs::read_to_string(f.path()).await?;

    let mut lines = contents.lines();
    let (title, description) = if let Some(line) = lines.find(|line| line.starts_with("# ")) {
        (line.trim_start_matches("# "), find_description(lines))
    } else {
        (
            file_name.to_str().context("to_str() None")?,
            find_description(contents.lines()),
        )
    };

    let mut resp = Response::new(
        HTML.replace("%TITLE%", title)
            .replace("%DESCRIPTION%", &description)
            .replace("%CONTENTS%", &contents)
            .into(),
    );
    resp.headers_mut().typed_insert(ContentType::html());

    Ok(resp)
}

fn find_description(lines: Lines) -> String {
    // skip empty and ![image](links)
    lines
        .filter(|line| !line.trim().is_empty() && !line.starts_with("!["))
        .take(10)
        .collect::<Vec<_>>()
        .join("\n")
}
