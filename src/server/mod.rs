mod naming;
mod postprocessing;
mod routes;
mod utils;

use futures::future::{self, BoxFuture};
use tracing::{debug, info};

use crate::server::{
    naming::init_combinations,
    routes::get_routes,
    utils::{cleanup_temp_dir, get_temp_dir_path},
};

pub async fn run_server<F>(port: u16, upload_token: String, stop_signal: F)
where
    F: Future<Output = &'static str> + Send + 'static,
{
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

            let mut futures: Vec<BoxFuture<'static, &'static str>> = Vec::new();

            #[cfg(unix)]
            {
                use tokio::signal::unix::{SignalKind, signal};

                futures.push(Box::pin(async {
                    let mut interrupt = signal(SignalKind::interrupt()).unwrap();
                    interrupt.recv().await;
                    "interrupt"
                }));
                futures.push(Box::pin(async {
                    let mut terminate = signal(SignalKind::terminate()).unwrap();
                    terminate.recv().await;
                    "terminate"
                }));
                futures.push(Box::pin(async {
                    let mut hangup = signal(SignalKind::hangup()).unwrap();
                    hangup.recv().await;
                    "hangup"
                }));
            }

            if cfg!(not(unix)) {
                use tokio::signal::ctrl_c;

                futures.push(Box::pin(async {
                    ctrl_c().await.unwrap();
                    "Ctrl-C"
                }));
            }

            futures.push(Box::pin(stop_signal));

            let (reason, ..) = future::select_all(futures).await;
            info!("Shutting down due to {reason}");

            cleanup_temp_dir().await;
        })
        .run()
        .await;
}

#[tokio::test]
async fn test_run_server() {
    let dir = get_temp_dir_path().await;

    let (stop_signal_tx, stop_signal_rx) = tokio::sync::oneshot::channel();
    let server_task = tokio::task::spawn(async move {
        let stop_signal = async move {
            stop_signal_rx.await.unwrap();
            "signal"
        };
        run_server(8080, "test".to_string(), stop_signal).await;
    });

    let file_name_1 = {
        unsafe {
            std::env::set_var("UPLOAD_TOKEN", "test");
            std::env::set_var("URL", "http://localhost:8080/");
        }

        http_file_uploader::upload("test1".into(), "txt")
            .await
            .unwrap();

        let entry = tokio::fs::read_dir(&dir)
            .await
            .unwrap()
            .next_entry()
            .await
            .unwrap()
            .unwrap();
        let contents = tokio::fs::read_to_string(entry.path()).await.unwrap();
        assert_eq!(contents, "test1");
        entry.file_name().to_string_lossy().to_string()
    };

    let file_name_2 = {
        let client = reqwest::Client::new();
        let res = client
            .post("http://localhost:8080/upload.txt")
            .header("Authorization", "Bearer test")
            .body("test2")
            .send()
            .await
            .unwrap();
        let path = res.text().await.unwrap();
        let contents = tokio::fs::read(dir.join(&path)).await.unwrap();
        assert_eq!(contents, b"test2");
        path
    };

    {
        let contents = reqwest::get(format!("http://localhost:8080/{file_name_1}"))
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        assert_eq!(contents, "test1");
    }

    {
        let contents = reqwest::get(format!("http://localhost:8080/{file_name_2}"))
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        assert_eq!(contents, "test2");
    }

    stop_signal_tx.send(()).unwrap();
    server_task.await.unwrap();

    assert!(!dir.exists());
}
