//! Request log: one JSON line per request the CLI sends or refuses, so people
//! can review what was read (for example by an LLM agent). Credentials are
//! never logged.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde_json::{Value, json};

/// Appends entries to a JSON Lines file.
#[derive(Debug, Clone)]
pub struct RequestLog {
    path: PathBuf,
    product: &'static str,
}

impl RequestLog {
    pub fn new(path: PathBuf, product: &'static str) -> Self {
        Self { path, product }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// A request that got a response (or failed to get one).
    pub fn request(&self, url: &str, status: Option<u16>, items: Option<usize>) -> Result<()> {
        let outcome = match status {
            Some(s) if (200..300).contains(&s) => "ok",
            Some(_) => "http_error",
            None => "network_error",
        };
        self.write(json!({
            "time": now_rfc3339(),
            "product": self.product,
            "method": "GET",
            "url": url,
            "status": status,
            "items": items,
            "outcome": outcome,
        }))
    }

    /// A command the scope refused before sending anything.
    pub fn denied(&self, command: &str, reason: &str) -> Result<()> {
        self.write(json!({
            "time": now_rfc3339(),
            "product": self.product,
            "command": command,
            "outcome": "denied",
            "reason": reason,
        }))
    }

    fn write(&self, entry: Value) -> Result<()> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)
                .with_context(|| format!("could not create log directory {}", dir.display()))?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .with_context(|| format!("could not open request log {}", self.path.display()))?;
        writeln!(file, "{entry}")
            .with_context(|| format!("could not write request log {}", self.path.display()))
    }
}

/// The current UTC time as `YYYY-MM-DDTHH:MM:SSZ`.
pub fn now_rfc3339() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format_rfc3339(secs)
}

/// Format seconds since the Unix epoch as UTC RFC 3339.
pub fn format_rfc3339(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // Civil-from-days (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_utc_times() {
        assert_eq!(format_rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_rfc3339(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(format_rfc3339(1_793_014_245), "2026-10-26T11:30:45Z");
    }

    #[test]
    fn appends_json_lines() {
        let dir = std::env::temp_dir().join(format!("atl-log-{}", std::process::id()));
        let path = dir.join("nested").join("requests.log");
        let log = RequestLog::new(path.clone(), "jira");
        log.request("https://j/rest/api/2/myself", Some(200), None)
            .unwrap();
        log.denied("jira issue get XYZ-1", "not allowed").unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<Value> = text
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0]["outcome"], "ok");
        assert_eq!(lines[0]["product"], "jira");
        assert_eq!(lines[1]["outcome"], "denied");
        assert_eq!(lines[1]["command"], "jira issue get XYZ-1");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
