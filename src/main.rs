use anyhow::{bail, Result};
use gumdrop::{Options, ParsingStyle};
use metrics_exporter_prometheus::PrometheusBuilder;
use metrics_process::Collector;
use sekshibot::{ConnectionOptions, SekshiBot, UnauthorizedError};
use std::{process::ExitCode, time::Duration};

/// The chat moderation-and-more bot for the WLK community.
#[derive(Debug, Clone, Options)]
pub struct Cli {
    /// HTTP API endpoint of the üWave server to connect to.
    #[options(required)]
    pub api_url: String,
    /// WebSocket API endpoint of the üWave server to connect to.
    #[options(required)]
    pub socket_url: String,
    pub help: bool,
    pub metrics: Option<String>,
}

fn setup_metrics(endpoint: &str) -> anyhow::Result<flume::Sender<()>> {
    let (tx, rx) = flume::bounded::<()>(1);

    let process_metrics = Collector::default();
    process_metrics.describe();

    let prom_handle = PrometheusBuilder::new()
        .install_recorder()
        .expect("failed to install recorder");

    let server = tiny_http::Server::http(endpoint).unwrap();

    let collect_exit = rx.clone();
    let _collect_handle = std::thread::Builder::new()
        .name("prometheus collector".into())
        .spawn(move || loop {
            match collect_exit.recv_timeout(Duration::from_secs(3)) {
                Ok(_) => break,
                Err(flume::RecvTimeoutError::Disconnected) => break,
                Err(_) => {
                    process_metrics.collect();
                }
            }
        })
        .unwrap();

    let server_exit = rx.clone();
    let _server_handle = std::thread::Builder::new()
        .name("prometheus exporter".into())
        .spawn(move || {
            loop {
                match server_exit.recv_timeout(Duration::from_millis(50)) {
                    Ok(_) => break,
                    Err(flume::RecvTimeoutError::Disconnected) => break,
                    Err(_) => {}
                }
                if let Some(request) = server.recv_timeout(Duration::from_millis(50))? {
                    if request.url() == "/metrics" || request.url().starts_with("/metrics?") {
                        let response = tiny_http::Response::from_string(prom_handle.render())
                            .with_header(
                                tiny_http::Header::from_bytes(b"content-type", b"text/plain")
                                    .unwrap(),
                            );
                        request.respond(response)?;
                    } else {
                        request.respond(tiny_http::Response::empty(tiny_http::StatusCode(404)))?;
                    }
                }
            }
            anyhow::Ok(())
        })
        .unwrap();

    Ok(tx)
}

fn main() -> Result<ExitCode> {
    femme::with_level(log::LevelFilter::Info);
    let args = Cli::parse_args_or_exit(ParsingStyle::AllOptions);
    log::info!("args: {:?}", args);

    let email = match std::env::var("SEKSHIBOT_EMAIL") {
        Ok(email) => email,
        _ => bail!("missing SEKSHIBOT_EMAIL env var"),
    };
    let password = match std::env::var("SEKSHIBOT_PASSWORD") {
        Ok(password) => password,
        _ => bail!("missing SEKSHIBOT_PASSWORD env var"),
    };

    let metrics_endpoint = args.metrics.as_deref().unwrap_or("0.0.0.0:3003");
    let tx = setup_metrics(metrics_endpoint)?;

    let result = (|| {
        let bot = SekshiBot::connect(ConnectionOptions {
            api_url: args.api_url,
            socket_url: args.socket_url,
            email,
            password,
        })?;

        bot.run()
    })();

    let _ = tx.send(());
    drop(tx);

    match result {
        Ok(_) => Ok(ExitCode::SUCCESS),
        Err(err) => {
            if err.is::<UnauthorizedError>() {
                eprintln!("Error: {err}");
                Ok(ExitCode::from(75))
            } else {
                Err(err)
            }
        }
    }
}
