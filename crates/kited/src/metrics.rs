//! Prometheus metrics registration.

use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};

pub const DIRECTORY_RECORDS_TOTAL: &str = "kite_mesh_directory_records_total";
pub const DIRECTORY_LOOKUP_SECONDS: &str = "kite_mesh_directory_lookup_seconds";
pub const MATCH_ACCEPT_TOTAL: &str = "kite_mesh_match_accept_total";
pub const RECEIPTS_TOTAL: &str = "kite_mesh_receipts_total";

/// Install a per-daemon Prometheus recorder. Returns a handle that can be
/// scraped via `handle.render()`.
pub fn install() -> anyhow::Result<PrometheusHandle> {
    let handle = PrometheusBuilder::new().install_recorder()?;

    metrics::describe_counter!(
        DIRECTORY_RECORDS_TOTAL,
        "Total DHT directory records published by this daemon."
    );
    metrics::describe_histogram!(
        DIRECTORY_LOOKUP_SECONDS,
        metrics::Unit::Seconds,
        "Wall-clock time to complete a DHT directory lookup."
    );
    metrics::describe_counter!(
        MATCH_ACCEPT_TOTAL,
        "Total candidates that passed the facet filter and cosine rerank."
    );
    metrics::describe_counter!(
        RECEIPTS_TOTAL,
        "Total signed receipts persisted by this daemon."
    );

    Ok(handle)
}
