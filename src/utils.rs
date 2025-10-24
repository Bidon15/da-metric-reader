use chrono::{DateTime, Utc};

/// Format Unix timestamp to human-readable string
pub fn format_timestamp(ts: u64) -> String {
    let dt = DateTime::<Utc>::from_timestamp(ts as i64, 0)
        .unwrap_or_else(|| DateTime::<Utc>::from_timestamp(0, 0).unwrap());
    dt.format("%Y-%m-%d %H:%M:%S UTC").to_string()
}

