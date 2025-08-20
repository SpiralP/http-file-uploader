use anyhow::{Context, Result};
use headers::{ContentType, HeaderMapExt};
use tokio::fs;
use warp::reply::Response;

const HTML: &str = include_str!("./html.html");
const HTML_HEAD: &str = include_str!("./html_head.html");

pub async fn process_html(f: warp::fs::File) -> Result<Response> {
    let file_name = f.path().file_name().context("file_name() None")?;
    let contents = fs::read_to_string(f.path()).await?;

    let (title, description) = (file_name.to_str().context("to_str() None")?, "");

    let html_head = HTML_HEAD
        .replace("%TITLE%", title)
        .replace("%DESCRIPTION%", description);

    let html = if contents.contains("<html") && contents.contains("<head>") {
        let html_head = html_head.replace("rgb(31, 31, 31)", "#121212");
        contents
            .replace("<head>", &format!("<head>{html_head}"))
            .replace("background-color:#ffffff;", "background-color:#121212;")
            .replace("color:#000000;", "color:#ffffff;")
            .replace(
                "font-family:monospace",
                r#"font-family: 'Hack Nerd Font', 'Hack', monospace"#,
            )
    } else {
        HTML.replace("%HEAD%", &html_head)
            .replace("%CONTENTS%", &contents)
    };
    let mut resp = Response::new(html.into());
    resp.headers_mut().typed_insert(ContentType::html());

    Ok(resp)
}
