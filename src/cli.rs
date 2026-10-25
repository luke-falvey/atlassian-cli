//! Top-level command line definition: global flags and one subcommand per product.

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::products::agile::AgileCommand;
use crate::products::bamboo::BambooCommand;
use crate::products::bitbucket::BitbucketCommand;
use crate::products::confluence::ConfluenceCommand;
use crate::products::jira::JiraCommand;
use crate::request::ApiRequest;

/// Read-only command line client for Atlassian Data Center products.
///
/// Authenticates with HTTP Basic auth using `username:password` from
/// ~/.atlassian_credentials.
#[derive(Debug, Parser)]
#[command(name = "atlassian", version)]
pub struct Cli {
    /// Base URL of the product, e.g. https://jira.example.com. Defaults to
    /// JIRA_URL, CONFLUENCE_URL, BITBUCKET_URL or BAMBOO_URL.
    #[arg(long, global = true, value_name = "URL", help_heading = GLOBAL)]
    pub base_url: Option<String>,

    /// Credentials file holding `username:password` [default: ~/.atlassian_credentials].
    #[arg(long, global = true, value_name = "PATH", help_heading = GLOBAL)]
    pub credentials_file: Option<PathBuf>,

    /// Extra PEM CA certificate to trust, for servers with a private CA.
    #[arg(long, global = true, value_name = "PATH", help_heading = GLOBAL)]
    pub ca_cert: Option<PathBuf>,

    /// Print compact JSON instead of pretty-printed JSON.
    #[arg(long, global = true, help_heading = GLOBAL)]
    pub compact: bool,

    #[command(subcommand)]
    pub product: Product,
}

const GLOBAL: &str = "Global options";

#[derive(Debug, Subcommand)]
pub enum Product {
    /// Jira (issues, projects, users, search, ...).
    #[command(subcommand)]
    Jira(JiraCommand),
    /// Jira Software (boards, sprints, epics). Uses the Jira base URL.
    #[command(subcommand, alias = "jira-software")]
    Agile(AgileCommand),
    /// Confluence (spaces, content, search, ...).
    #[command(subcommand)]
    Confluence(ConfluenceCommand),
    /// Bitbucket (projects, repositories, pull requests, ...).
    #[command(subcommand)]
    Bitbucket(BitbucketCommand),
    /// Bamboo (plans, build results, deployments, ...).
    #[command(subcommand)]
    Bamboo(BambooCommand),
}

impl Product {
    /// Environment variable that holds this product's base URL.
    pub fn url_env_var(&self) -> &'static str {
        match self {
            Product::Jira(_) | Product::Agile(_) => "JIRA_URL",
            Product::Confluence(_) => "CONFLUENCE_URL",
            Product::Bitbucket(_) => "BITBUCKET_URL",
            Product::Bamboo(_) => "BAMBOO_URL",
        }
    }

    pub fn to_request(&self) -> Result<ApiRequest> {
        match self {
            Product::Jira(c) => c.to_request(),
            Product::Agile(c) => c.to_request(),
            Product::Confluence(c) => c.to_request(),
            Product::Bitbucket(c) => c.to_request(),
            Product::Bamboo(c) => c.to_request(),
        }
    }
}

impl Cli {
    pub fn to_request(&self) -> Result<ApiRequest> {
        self.product.to_request()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn url_env_vars() {
        let cli = Cli::try_parse_from(["atlassian", "agile", "board", "get", "1"]).unwrap();
        assert_eq!(cli.product.url_env_var(), "JIRA_URL");
        let cli = Cli::try_parse_from(["atlassian", "bamboo", "info"]).unwrap();
        assert_eq!(cli.product.url_env_var(), "BAMBOO_URL");
    }

    #[test]
    fn global_flags_after_subcommand() {
        let cli = Cli::try_parse_from([
            "atlassian",
            "jira",
            "myself",
            "--base-url",
            "https://jira.example.com",
            "--credentials-file",
            "/tmp/c",
            "--compact",
        ])
        .unwrap();
        assert_eq!(cli.base_url.as_deref(), Some("https://jira.example.com"));
        assert_eq!(cli.credentials_file, Some(PathBuf::from("/tmp/c")));
        assert!(cli.compact);
    }
}
