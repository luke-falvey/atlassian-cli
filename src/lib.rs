//! Read-only command line client for Atlassian Data Center products.
//!
//! The binary is a thin wrapper around [`execute`], which tests call directly.

pub mod cli;
pub mod client;
pub mod config;
pub mod log;
pub mod products;
pub mod request;
pub mod scope;

use anyhow::{Context, Result};

pub use cli::Cli;
pub use client::{Client, Output};
use config::{Config, Resolved};

/// Load the config file and resolve the settings for the chosen product.
pub fn resolve_settings(cli: &Cli) -> Result<Resolved> {
    let path = match &cli.config {
        Some(path) => path.clone(),
        None => Config::default_path()?,
    };
    let config = Config::from_file(&path)?;
    Ok(config.resolve(
        cli.product.section(),
        cli.base_url.as_deref(),
        cli.ca_cert.as_deref(),
        &path,
    )?)
}

/// Run a parsed command and return what it fetched.
///
/// The command is checked against the configured scope before anything is
/// sent; refused commands are logged and fail. List results are filtered
/// down to the scope afterwards.
pub async fn execute(cli: &Cli) -> Result<Output> {
    let mut request = cli.to_request()?;
    let settings = resolve_settings(cli)?;
    let log = settings
        .log
        .clone()
        .map(|path| log::RequestLog::new(path, cli.product.name()));
    let client = Client::new(
        &settings.base_url,
        settings.credentials,
        settings.ca_cert.as_deref(),
    )?
    .with_log(log.clone())
    .with_max_items(settings.scope.max_results());

    let filter = match scope::authorize(&mut request, &settings.scope, &client).await {
        Ok(filter) => filter,
        Err(err) => {
            if let Some(log) = &log {
                log.denied(&command_line(), &format!("{err:#}"))?;
            }
            return Err(err);
        }
    };
    let output = client.execute(&request).await?;
    Ok(filter.apply(output, &settings.scope))
}

/// The command as typed, for the request log.
fn command_line() -> String {
    std::env::args().collect::<Vec<_>>().join(" ")
}

/// Format output for printing.
pub fn render(output: &Output, compact: bool) -> Result<String> {
    Ok(match output {
        Output::Json(v) if compact => serde_json::to_string(v)?,
        Output::Json(v) => serde_json::to_string_pretty(v)?,
        Output::Text(t) => t.clone(),
    })
}

/// Run a parsed command and print the result to stdout.
pub async fn run(cli: Cli) -> Result<()> {
    let output = execute(&cli).await?;
    let text = render(&output, cli.compact).context("could not format output")?;
    if text.ends_with('\n') {
        print!("{text}");
    } else {
        println!("{text}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn render_pretty_and_compact() {
        let out = Output::Json(json!({"a": 1}));
        assert_eq!(render(&out, true).unwrap(), r#"{"a":1}"#);
        assert_eq!(render(&out, false).unwrap(), "{\n  \"a\": 1\n}");
        assert_eq!(render(&Output::Text("hi".into()), false).unwrap(), "hi");
    }
}
