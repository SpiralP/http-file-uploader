use mime::Mime;

pub fn make_existing_mimes_command() -> (&'static str, &'static [&'static str]) {
    ("wl-paste", &["--list-types"])
}

pub fn parse_existing_mimes(stdout: String) -> Vec<Mime> {
    let stdout = stdout.trim();
    stdout
        .split_ascii_whitespace()
        .filter_map(|s| s.parse().ok())
        .collect::<Vec<Mime>>()
}

pub fn make_clipboard_command(mime: &str) -> (&str, Vec<&str>) {
    ("wl-paste", vec!["--type", mime])
}
