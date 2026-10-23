use prometheus::{
    HistogramOpts, HistogramVec, IntCounterVec, IntGauge, Opts, Registry, TextEncoder,
};
use std::sync::Arc;
use tokio::sync::Semaphore;

pub(crate) struct Metrics {
    registry: Registry,
    requests: IntCounterVec,
    input_bytes: HistogramVec,
    output_bytes: HistogramVec,
    duration_seconds: HistogramVec,
    tokio_workers: IntGauge,
    tokio_alive_tasks: IntGauge,
    tokio_global_queue_depth: IntGauge,
    permits_available: IntGauge,
}

impl Metrics {
    pub(crate) fn new() -> Arc<Self> {
        let registry = Registry::new();

        #[cfg(target_os = "linux")]
        registry
            .register(Box::new(
                prometheus::process_collector::ProcessCollector::for_self(),
            ))
            .unwrap();

        let requests = IntCounterVec::new(
            Opts::new(
                "wymmd_minify_requests_total",
                "Total number of minify requests.",
            ),
            &["media_type", "status"],
        )
        .unwrap();
        let byte_buckets = prometheus::exponential_buckets(256.0, 4.0, 8).unwrap();
        let input_bytes = HistogramVec::new(
            HistogramOpts::new(
                "wymmd_minify_input_bytes",
                "Size of minify request bodies in bytes.",
            )
            .buckets(byte_buckets.clone()),
            &["media_type"],
        )
        .unwrap();
        let output_bytes = HistogramVec::new(
            HistogramOpts::new(
                "wymmd_minify_output_bytes",
                "Size of minified responses in bytes.",
            )
            .buckets(byte_buckets),
            &["media_type"],
        )
        .unwrap();
        let duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "wymmd_minify_duration_seconds",
                "Time spent minifying on the blocking pool.",
            ),
            &["media_type"],
        )
        .unwrap();
        let tokio_workers = IntGauge::new(
            "tokio_workers",
            "Number of worker threads in the tokio runtime.",
        )
        .unwrap();
        let tokio_alive_tasks = IntGauge::new(
            "tokio_alive_tasks",
            "Number of alive tasks in the tokio runtime.",
        )
        .unwrap();
        let tokio_global_queue_depth = IntGauge::new(
            "tokio_global_queue_depth",
            "Number of tasks in the tokio global queue.",
        )
        .unwrap();
        let permits_available = IntGauge::new(
            "wymmd_minify_permits_available",
            "Number of available minify concurrency permits.",
        )
        .unwrap();

        registry.register(Box::new(requests.clone())).unwrap();
        registry.register(Box::new(input_bytes.clone())).unwrap();
        registry.register(Box::new(output_bytes.clone())).unwrap();
        registry
            .register(Box::new(duration_seconds.clone()))
            .unwrap();
        registry.register(Box::new(tokio_workers.clone())).unwrap();
        registry
            .register(Box::new(tokio_alive_tasks.clone()))
            .unwrap();
        registry
            .register(Box::new(tokio_global_queue_depth.clone()))
            .unwrap();
        registry
            .register(Box::new(permits_available.clone()))
            .unwrap();

        Arc::new(Self {
            registry,
            requests,
            input_bytes,
            output_bytes,
            duration_seconds,
            tokio_workers,
            tokio_alive_tasks,
            tokio_global_queue_depth,
            permits_available,
        })
    }

    pub(crate) fn record_request(&self, media_type: &str, status: &str) {
        self.requests.with_label_values(&[media_type, status]).inc();
    }

    pub(crate) fn observe_input(&self, media_type: &str, bytes: usize) {
        self.input_bytes
            .with_label_values(&[media_type])
            .observe(bytes as f64);
    }

    pub(crate) fn observe_output(&self, media_type: &str, bytes: usize) {
        self.output_bytes
            .with_label_values(&[media_type])
            .observe(bytes as f64);
    }

    pub(crate) fn observe_duration(&self, media_type: &str, seconds: f64) {
        self.duration_seconds
            .with_label_values(&[media_type])
            .observe(seconds);
    }

    pub(crate) fn render(&self, limit: &Semaphore) -> String {
        let runtime = tokio::runtime::Handle::current().metrics();
        self.tokio_workers.set(runtime.num_workers() as i64);
        self.tokio_alive_tasks.set(runtime.num_alive_tasks() as i64);
        self.tokio_global_queue_depth
            .set(runtime.global_queue_depth() as i64);
        self.permits_available.set(limit.available_permits() as i64);

        TextEncoder::new()
            .encode_to_string(&self.registry.gather())
            .unwrap()
    }
}
