use hyper::{
    service::{make_service_fn, service_fn},
    Body, Request, Response, Server,
};
use prometheus::{
    labels, opts, register_counter, register_histogram_vec, Counter, Encoder, HistogramVec,
    IntGaugeVec, Opts, Registry, TextEncoder,
};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tokio::runtime::Runtime;

lazy_static::lazy_static! {
    pub(crate) static ref REGISTRY: Registry = Registry::new();

    pub(crate) static ref ONLOAD_COUNTER: Counter = register_counter!(opts!(
        "onload_count",
        "Number of times `onload()` method was called.",
        labels! {"handler" => "all",}
    ))
    .unwrap();

    pub(crate) static ref ONLOAD_HISTOGRAM: HistogramVec = register_histogram_vec!(
        "plugin_loading_duration",
        "The latencies in seconds for performing plugin initialization.",
        &["handler"]
    )
    .unwrap();


    pub(crate) static ref UNLOAD_COUNTER: Counter = register_counter!(opts!(
        "unload_count",
        "Number of times `unload()` method was called.",
        labels! {"handler" => "all",}
    ))
    .unwrap();

    pub(crate) static ref UNLOAD_HISTOGRAM: HistogramVec = register_histogram_vec!(
        "plugin_unloading_duration",
        "The latencies in seconds for performing plugin unwinding.",
        &["handler"]
    )
    .unwrap();


    pub(crate) static ref ACCOUNT_UPDATE_COUNTER: Counter = register_counter!(opts!(
        "unload_count",
        "Number of times `unload()` method was called.",
        labels! {"handler" => "all",}
    ))
    .unwrap();

    pub(crate) static ref ACCOUNT_UPDATE_HISTOGRAM: HistogramVec = register_histogram_vec!(
        "plugin_unloading_duration",
        "The latencies in seconds for performing plugin unwinding.",
        &["handler"]
    )
    .unwrap();


    pub(crate) static ref ACCOUNT_UPDATE: IntGaugeVec = IntGaugeVec::new(
        Opts::new("account_info", "An account has been updated"),
        &["type"]
    ).unwrap();

    pub(crate) static ref SLOT_UPDATE_COUNTER: Counter = register_counter!(opts!(
        "slot_update_count",
        "Number of times slot was update.",
        labels! {"handler" => "all",}
    ))
    .unwrap();

    pub(crate) static ref SLOT_UPDATE_HISTOGRAM: HistogramVec = register_histogram_vec!(
        "slot_update_duration",
        "The latencies in seconds for performing of a slot update.",
        &["handler"]
    )
    .unwrap();


    pub(crate) static ref SLOT_UPDATE: IntGaugeVec = IntGaugeVec::new(
        Opts::new("slot_no", "A slot has been updated"),
        &["type"]
    ).unwrap();

    pub(crate) static ref END_OF_STARTUP: Counter = register_counter!(opts!(
        "startup_finished",
        "Startup finished notification received",
        labels! {"handler" => "all",}
    ))
    .unwrap();
}

pub fn spawn_metric_thread(runtime: &Runtime, socket_addr: SocketAddr) {
    runtime.spawn(async move {
        let addr: SocketAddr = socket_addr;

        match crate::CHANNEL.1.write().await.recv().await {
            Some(_) => {
                tracing::info!(
                    "GeyserPlugin Prometheus Metrics -> Listening on http://{}",
                    addr
                );

                let serve_future = Server::bind(&addr).serve(make_service_fn(|_| async {
                    Ok::<_, hyper::Error>(service_fn(serve_metrics))
                }));

                if let Err(err) = serve_future.await {
                    tracing::error!("server error: {}", err);
                }
            }
            None => {
                tracing::error!("`Sender` has been dropped!");
            }
        }
    });
}

async fn serve_metrics(_req: Request<Body>) -> Result<Response<Body>, hyper::http::Error> {
    let encoder = TextEncoder::new();
    let metric_families = REGISTRY.gather();
    let mut buffer = vec![];
    match encoder.encode(&metric_families, &mut buffer) {
        Ok(_) => (),
        Err(error) => {
            tracing::error!("Error while encoding metrics: `{}`", error.to_string());
        }
    }

    match Response::builder().status(200).body(Body::from(buffer)) {
        Ok(response) => Ok(response),
        Err(error) => {
            tracing::error!("Hyper error when serving metrics: `{}`", error.to_string());

            Err(error)
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrometheusConfig {
    pub(crate) ip: String,
    pub(crate) port: u16,
}

impl PrometheusConfig {
    pub fn to_socket_addr(&self) -> SocketAddr {
        let ip = match self.ip.parse::<Ipv4Addr>() {
            Ok(ip_addr) => ip_addr,
            Err(error) => {
                tracing::error!("Error parsing IP address: `{:?}`", error);

                std::process::exit(1);
            }
        };

        SocketAddr::new(IpAddr::V4(ip), self.port)
    }
}
