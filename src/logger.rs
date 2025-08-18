use std::{io, sync::Once};

use tracing_subscriber::{EnvFilter, util::SubscriberInitExt};

pub fn initialize(debug: bool, module_filter: Option<&str>) {
    static ONCE: Once = Once::new();
    ONCE.call_once(move || {
        let level = if debug { "debug" } else { "info" };

        let mut filter = EnvFilter::from_default_env();
        if let Some(module) = module_filter {
            let local_modules = ["http_file_uploader", "upload_from_clipboard", "upload"];
            assert!(local_modules.contains(&module), "update hardcoded list!!");

            for module in local_modules {
                filter = filter.add_directive(format!("{module}={level}").parse().unwrap());
            }
        } else {
            filter = filter.add_directive(level.parse().unwrap());
        }

        let subscriber = tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_thread_ids(false)
            .with_thread_names(false)
            .with_ansi(true)
            .without_time()
            .with_writer(io::stderr)
            .finish();

        subscriber.init();
    });
}
