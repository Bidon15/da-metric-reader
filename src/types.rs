use serde::{Serialize, Deserialize};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use crate::config::Config;

/// Stores the latest DAS metrics
#[derive(Debug, Clone, Default)]
pub struct DasMetrics {
    pub head: Option<i64>,
    pub headers: Option<i64>,
    pub last_update: Option<u64>, // Unix timestamp in seconds
}

/// Application state shared across handlers and background tasks
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub das_metrics: Arc<Mutex<DasMetrics>>,
    pub ring_buffer: Arc<Mutex<VecDeque<SampleBit>>>,
    pub samples: Arc<Mutex<Vec<Sample>>>,
}

/// A single sample bit with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SampleBit {
    pub timestamp: u64,
    pub ok: bool,
    pub reason: String,
}

/// Raw sample data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sample {
    pub timestamp: u64,
    pub head: Option<i64>,
    pub headers: Option<i64>,
    pub ok: bool,
    pub reason: String,
}

/// Batch structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Batch {
    pub n: usize,
    pub good: usize,
    pub threshold: usize,
    pub bitmap_hash: String,
    pub window: TimeWindow,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeWindow {
    pub start: u64,
    pub end: u64,
}

/// Normalized metric structure for easier processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedMetric {
    /// Metric name (e.g., "http.server.duration")
    pub name: String,
    /// Type of metric (Sum, Gauge, Histogram, etc.)
    pub metric_type: String,
    /// The actual metric value
    pub value: MetricValue,
    /// Labels/attributes attached to this data point
    pub attributes: HashMap<String, String>,
    /// Resource attributes (service.name, host.name, etc.)
    pub resource_attributes: HashMap<String, String>,
    /// Instrumentation scope (library name and version)
    pub scope_name: Option<String>,
    pub scope_version: Option<String>,
    /// Timestamps in nanoseconds since Unix epoch
    pub time_unix_nano: Option<u64>,
    pub start_time_unix_nano: Option<u64>,
}

/// Enum to represent different metric value types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MetricValue {
    Int(i64),
    Double(f64),
    Histogram {
        count: u64,
        sum: Option<f64>,
        buckets: Vec<HistogramBucket>,
    },
    Summary {
        count: u64,
        sum: f64,
        quantiles: Vec<SummaryQuantile>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistogramBucket {
    pub count: u64,
    pub upper_bound: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryQuantile {
    pub quantile: f64,
    pub value: f64,
}

