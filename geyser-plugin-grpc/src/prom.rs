use {
    crate::version::VERSION as VERSION_INFO,
    futures_util::FutureExt,
    hyper::{
        server::conn::AddrStream,
        service::{make_service_fn, service_fn},
        Body, Request, Response, Server, StatusCode,
    },
    log::*,
    prometheus::{
        labels, opts, register_counter, register_histogram_vec, Counter, HistogramVec,
        IntCounterVec, IntGaugeVec, Opts, Registry, TextEncoder,
    },
    serde::Deserialize,
    std::{net::SocketAddr, sync::Once},
    tokio::{runtime::Runtime, sync::oneshot},
};

lazy_static::lazy_static! {
    pub static ref REGISTRY: Registry = Registry::new();

    static ref VERSION: IntCounterVec = IntCounterVec::new(
        Opts::new("version", "Plugin version info"),
        &["key", "value"]
    ).unwrap();

    pub static ref ONLOAD_COUNTER: Counter = register_counter!(opts!(
        "onload_count",
        "Number of times `onload()` method was called.",
        labels! {"handler" => "all",}
    ))
    .unwrap();

    pub static ref ONLOAD_HISTOGRAM: HistogramVec = register_histogram_vec!(
        "plugin_loading_duration",
        "The latencies in seconds for performing plugin initialization.",
        &["handler"]
    )
    .unwrap();


    pub static ref UNLOAD_COUNTER: Counter = register_counter!(opts!(
        "unload_count",
        "Number of times `unload()` method was called.",
        labels! {"handler" => "all",}
    ))
    .unwrap();

    pub static ref UNLOAD_HISTOGRAM: HistogramVec = register_histogram_vec!(
        "plugin_unloading_duration",
        "The latencies in seconds for performing plugin unwinding.",
        &["handler"]
    )
    .unwrap();


    pub static ref ACCOUNT_UPDATE_COUNTER: Counter = register_counter!(opts!(
        "unload_count",
        "Number of times `unload()` method was called.",
        labels! {"handler" => "all",}
    ))
    .unwrap();

    pub static ref ACCOUNT_UPDATE_HISTOGRAM: HistogramVec = register_histogram_vec!(
        "plugin_unloading_duration",
        "The latencies in seconds for performing plugin unwinding.",
        &["handler"]
    )
    .unwrap();


    pub static ref ACCOUNT_UPDATE: IntGaugeVec = IntGaugeVec::new(
        Opts::new("account_info", "An account has been updated"),
        &["type"]
    ).unwrap();

    pub static ref SLOT_UPDATE_COUNTER: Counter = register_counter!(opts!(
        "slot_update_count",
        "Number of times slot was update.",
        labels! {"handler" => "all",}
    ))
    .unwrap();

    pub static ref SLOT_UPDATE_HISTOGRAM: HistogramVec = register_histogram_vec!(
        "slot_update_duration",
        "The latencies in seconds for performing of a slot update.",
        &["handler"]
    )
    .unwrap();


    pub static ref SLOT_UPDATE: IntGaugeVec = IntGaugeVec::new(
        Opts::new("slot_no", "A slot has been updated"),
        &["type"]
    ).unwrap();

    pub static ref END_OF_STARTUP: Counter = register_counter!(opts!(
        "startup_finished",
        "Startup finished notification received",
        labels! {"handler" => "all",}
    ))
    .unwrap();
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrometheusConfig {
    /// Address of Prometheus service.
    pub address: SocketAddr,
}

#[derive(Debug)]
pub struct PrometheusService {
    shutdown_signal: oneshot::Sender<()>,
}

impl PrometheusService {
    pub fn new(runtime: &Runtime, config: Option<PrometheusConfig>) -> Self {
        static REGISTER: Once = Once::new();
        REGISTER.call_once(|| {
            macro_rules! register {
                ($collector:ident) => {
                    REGISTRY
                        .register(Box::new($collector.clone()))
                        .expect("collector can't be registered");
                };
            }
            register!(VERSION);

            for (key, value) in &[
                ("version", VERSION_INFO.version),
                ("solana", VERSION_INFO.solana),
                ("git", VERSION_INFO.git),
                ("rustc", VERSION_INFO.rustc),
                ("buildts", VERSION_INFO.buildts),
            ] {
                VERSION.with_label_values(&[key, value]).inc()
            }
        });

        let (tx, rx) = oneshot::channel();
        if let Some(PrometheusConfig { address }) = config {
            runtime.spawn(async move {
                let make_service = make_service_fn(move |_: &AddrStream| async move {
                    Ok::<_, hyper::Error>(service_fn(move |req: Request<Body>| async move {
                        let response = match req.uri().path() {
                            "/metrics" => metrics_handler(),
                            _ => not_found_handler(),
                        };
                        Ok::<_, hyper::Error>(response)
                    }))
                });
                let server = Server::bind(&address).serve(make_service);
                if let Err(error) = tokio::try_join!(server, rx.map(|_| Ok(()))) {
                    error!("prometheus service failed: {}", error);
                }
            });
        }

        PrometheusService {
            shutdown_signal: tx,
        }
    }

    pub fn shutdown(self) {
        let _ = self.shutdown_signal.send(());
    }
}

fn metrics_handler() -> Response<Body> {
    let metrics = TextEncoder::new()
        .encode_to_string(&REGISTRY.gather())
        .unwrap_or_else(|error| {
            error!("could not encode custom metrics: {}", error);
            String::new()
        });
    Response::builder().body(Body::from(metrics)).unwrap()
}

fn not_found_handler() -> Response<Body> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::empty())
        .unwrap()
}
