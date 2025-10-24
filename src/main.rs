mod config;
mod types;
mod utils;
mod otlp;
mod metrics;
mod da;
mod storage;
mod crypto;

use axum::{routing::post, Router};
use std::{
    collections::VecDeque,
    fs,
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use tokio::net::TcpListener;
use tracing::info;

use config::Config;
use types::{AppState, DasMetrics};
use otlp::handle_metrics;
use metrics::{run_sampler, run_batch_generator};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    // Load configuration
    let config = Arc::new(Config::load()?);
    info!("Loaded config: {:?}", config);
    
    // Create data directory if it doesn't exist
    fs::create_dir_all("data")?;
    
    // Initialize shared state
    let state = AppState {
        config: config.clone(),
        das_metrics: Arc::new(Mutex::new(DasMetrics::default())),
        ring_buffer: Arc::new(Mutex::new(VecDeque::new())),
        samples: Arc::new(Mutex::new(Vec::new())),
    };
    
    // Spawn background sampler task
    let sampler_state = state.clone();
    tokio::spawn(async move {
        run_sampler(sampler_state).await;
    });
    
    // Spawn background batch generator task
    let batch_state = state.clone();
    tokio::spawn(async move {
        run_batch_generator(batch_state).await;
    });
    
    // Start HTTP server
    let app = Router::new()
        .route("/v1/metrics", post(handle_metrics))
        .with_state(state);

    let addr: SocketAddr = "0.0.0.0:4318".parse()?;
    info!("🚀 Listening for OTLP/HTTP on http://{addr}");
    info!("📊 Sampler will tick every {} seconds", config.sampling.tick_secs);
    
    if config.da_posting.enabled {
        if config.da_posting.post_every_sample {
            info!("📡 DA posting: ENABLED - Will post each sample to Celestia DA");
        } else {
            info!("📡 DA posting: ENABLED - Will post batched samples to Celestia DA");
        }
    } else {
        info!("📡 DA posting: DISABLED - Samples will be stored locally only");
    }
    
    info!("📦 Batches (for ZK proofs) will be generated every {} seconds ({} minutes)", 
          config.batching.window_secs, 
          config.batching.window_secs / 60);
    
    let listener = TcpListener::bind(&addr).await?;
    axum::serve(listener, app.into_make_service()).await?;

    Ok(())
}
