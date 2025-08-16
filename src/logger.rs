use std::sync::Once;

use tracing_subscriber::{EnvFilter, util::SubscriberInitExt};

pub fn initialize(debug: bool, module_filter: Option<&str>) {
    static ONCE: Once = Once::new();
    ONCE.call_once(move || {
        let level = if debug { "debug" } else { "info" };

        let mut filter = EnvFilter::from_default_env();
        if let Some(module) = module_filter {
            filter = filter.add_directive(format!("{module}={level}").parse().unwrap());
        } else {
            filter = filter.add_directive(level.parse().unwrap());
        }

        let subscriber = tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_thread_ids(false)
            .with_thread_names(false)
            .with_ansi(true)
            .without_time()
            .finish();

        subscriber.init();
    });
}
