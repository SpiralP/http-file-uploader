use anyhow::Result;
use mime::Mime;

pub fn make_existing_mimes_command() -> (&'static str, &'static [&'static str]) {
    ("wl-paste", &["--list-types"])
}

pub fn parse_existing_mimes(stdout: String) -> Result<Vec<Mime>> {
    let stdout = stdout.trim();
    let existing_mimes = stdout
        .split_ascii_whitespace()
        .map(|s| Ok(s.parse()?))
        .collect::<Result<Vec<Mime>>>()?;

    Ok(existing_mimes)
}

pub fn make_clipboard_command(mime: &str) -> (&str, Vec<&str>) {
    ("wl-paste", vec!["--type", mime])
}
