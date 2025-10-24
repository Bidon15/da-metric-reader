use serde::Deserialize;
use std::fs;

/// Configuration loaded from config.toml
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub sampling: SamplingConfig,
    pub metrics: MetricsConfig,
    pub da_posting: DaPostingConfig,
    pub batching: BatchingConfig,
    pub celestia: CelestiaConfig,
    pub proofs: ProofsConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SamplingConfig {
    pub tick_secs: u64,
    pub max_staleness_secs: u64,
    pub grace_period_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DaPostingConfig {
    pub enabled: bool,
    pub post_every_sample: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BatchingConfig {
    pub window_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MetricsConfig {
    pub head_metric: String,
    pub headers_metric: String,
    pub min_increment: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CelestiaConfig {
    pub node_url: String,
    pub namespace: String,
    pub poster_mode: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProofsConfig {
    pub enabled: bool,
    pub threshold_percent: f64,
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let content = fs::read_to_string("config.toml")?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }
}

