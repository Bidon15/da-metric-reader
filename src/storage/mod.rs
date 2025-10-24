use std::fs;
use crate::types::{Sample, Batch};

/// Save samples to file
pub fn save_samples(samples: &[Sample]) -> anyhow::Result<()> {
    let json = serde_json::to_string_pretty(samples)?;
    fs::write("data/samples.json", json)?;
    Ok(())
}

/// Save batch to file
pub fn save_batch(batch: &Batch) -> anyhow::Result<()> {
    let json = serde_json::to_string_pretty(batch)?;
    fs::write("data/batch.json", json)?;
    Ok(())
}

/// Save bitmap to hex file
pub fn save_bitmap(bitmap: &[u8]) -> anyhow::Result<()> {
    let hex: String = bitmap.iter().map(|b| format!("{:02x}", b)).collect();
    fs::write("data/bitmap.hex", hex)?;
    Ok(())
}

