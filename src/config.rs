use serde::Deserialize;
use std::fs;
use std::env;

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
    pub rpc_url: String,
    pub grpc_url: String,
    pub namespace: String,
    pub poster_mode: String,
    /// Mnemonic phrase (24 words) - will be converted to private key
    /// Either provide this OR private_key_hex (not both)
    pub mnemonic: Option<String>,
    /// Direct private key in hex format (64 characters)
    /// Either provide this OR mnemonic (not both)
    pub private_key_hex: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProofsConfig {
    pub enabled: bool,
    pub threshold_percent: f64,
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        // Load .env file if it exists (silently fail if not found)
        let _ = dotenvy::dotenv();
        
        let content = fs::read_to_string("config.toml")?;
        let mut config: Config = toml::from_str(&content)?;
        
        // Load from environment variables (takes precedence over config.toml)
        config.load_from_env()?;
        
        // Validate after loading from env
        config.validate()?;
        
        Ok(config)
    }

    fn load_from_env(&mut self) -> anyhow::Result<()> {
        // Check for mnemonic in environment
        if let Ok(mnemonic) = env::var("CELESTIA_MNEMONIC") {
            if !mnemonic.trim().is_empty() {
                tracing::info!("🔑 Loaded CELESTIA_MNEMONIC from environment");
                self.celestia.mnemonic = Some(mnemonic.trim().to_string());
                // Clear private_key_hex if mnemonic is set via env
                self.celestia.private_key_hex = None;
            }
        }
        
        // Check for private key in environment
        if let Ok(private_key) = env::var("CELESTIA_PRIVATE_KEY") {
            if !private_key.trim().is_empty() {
                tracing::info!("🔑 Loaded CELESTIA_PRIVATE_KEY from environment");
                self.celestia.private_key_hex = Some(private_key.trim().to_string());
                // Clear mnemonic if private_key is set via env
                self.celestia.mnemonic = None;
            }
        }
        
        Ok(())
    }

    fn validate(&self) -> anyhow::Result<()> {
        // Validate Celestia authentication config
        match (&self.celestia.mnemonic, &self.celestia.private_key_hex) {
            (None, None) => {
                anyhow::bail!(
                    "Celestia configuration error: Must provide authentication via environment variables.\n\
                    Set either CELESTIA_MNEMONIC or CELESTIA_PRIVATE_KEY in .env file or environment.\n\
                    See docs/ENV_SETUP.md for instructions."
                );
            }
            (Some(_), Some(_)) => {
                anyhow::bail!(
                    "Celestia configuration error: Provide only ONE of 'mnemonic' or 'private_key_hex', not both"
                );
            }
            (Some(_), None) => {
                tracing::info!("✅ Using mnemonic authentication (will be converted to private key)");
                Ok(())
            }
            (None, Some(_)) => {
                tracing::info!("✅ Using direct private key authentication");
                Ok(())
            }
        }
    }
}

impl CelestiaConfig {
    /// Get the private key hex, deriving it from mnemonic if necessary
    pub fn get_private_key_hex(&self) -> anyhow::Result<String> {
        if let Some(hex) = &self.private_key_hex {
            // Validate the hex key
            crate::crypto::validate_private_key_hex(hex)?;
            Ok(hex.clone())
        } else if let Some(mnemonic) = &self.mnemonic {
            // Derive from mnemonic
            crate::crypto::mnemonic_to_private_key_hex(mnemonic)
        } else {
            anyhow::bail!("No authentication method provided")
        }
    }
}

