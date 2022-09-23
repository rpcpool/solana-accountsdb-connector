use prometheus::{
    labels, opts, register_counter, register_histogram_vec, Counter, HistogramVec, IntGaugeVec,
    Opts, Registry,
};

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
        "slot_update_duratioh",
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
