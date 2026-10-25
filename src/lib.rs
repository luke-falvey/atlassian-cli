//! Read-only command line client for Atlassian Data Center products.
//!
//! The binary is a thin wrapper around [`execute`], which tests call directly.

pub mod cli;
pub mod client;
pub mod credentials;
pub mod products;
pub mod request;

use anyhow::{Context, Result, bail};

pub use cli::Cli;
pub use client::{Client, Output};
use credentials::Credentials;

/// Resolve the base URL: `--base-url` first, then the product's env variable.
pub fn base_url(cli: &Cli) -> Result<String> {
    if let Some(url) = &cli.base_url {
        return Ok(url.clone());
    }
    let var = cli.product.url_env_var();
    match std::env::var(var) {
        Ok(url) if !url.trim().is_empty() => Ok(url),
        _ => bail!("no base URL: pass --base-url or set {var}"),
    }
}

/// Run a parsed command and return what it fetched.
pub async fn execute(cli: &Cli) -> Result<Output> {
    let request = cli.to_request()?;
    let base_url = base_url(cli)?;
    let credentials_file = match &cli.credentials_file {
        Some(path) => path.clone(),
        None => Credentials::default_path()?,
    };
    let credentials = Credentials::from_file(&credentials_file)?;
    let client = Client::new(&base_url, credentials, cli.ca_cert.as_deref())?;
    client.execute(&request).await
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
