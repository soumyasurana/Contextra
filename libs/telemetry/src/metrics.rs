use metrics::{counter, histogram};
use std::time::Duration;

/// Record the duration of an HTTP request
pub fn record_request_duration(method: &str, path: &str, duration: Duration) {
    histogram!(
        "http_requests_duration_seconds",
        "method" => method.to_string(),
        "path" => path.to_string()
    )
    .record(duration.as_secs_f64());
}

/// Increment the count of an HTTP error
pub fn increment_error_count(method: &str, path: &str, error_type: &str) {
    counter!(
        "http_requests_errors_total",
        "method" => method.to_string(),
        "path" => path.to_string(),
        "error_type" => error_type.to_string()
    )
    .increment(1);
}
