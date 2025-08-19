mod naming;
mod routes;
mod utils;

use tracing::{debug, info};

use crate::server::{
    naming::init_combinations,
    routes::get_routes,
    utils::{cleanup_temp_dir, get_temp_dir_path},
};

pub async fn run_server(port: u16, upload_token: String) {
    let temp_dir = get_temp_dir_path().await;
    debug!("{}", temp_dir.display());

    tokio::task::spawn_blocking(|| {
        init_combinations();
    });

    debug!("Starting server on 0.0.0.0:{port}");
    warp::serve(get_routes(temp_dir, upload_token))
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

            cleanup_temp_dir().await;
        })
        .run()
        .await;
}
