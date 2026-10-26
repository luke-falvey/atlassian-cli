//! Top-level command line definition: global flags and one subcommand per product.

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::config::Section;
use crate::products::agile::AgileCommand;
use crate::products::bitbucket::BitbucketCommand;
use crate::products::confluence::ConfluenceCommand;
use crate::products::jira::JiraCommand;
use crate::request::ApiRequest;

/// Read-only command line client for Atlassian Data Center products.
///
/// Server URLs and Basic-auth credentials are read from ~/.atlassian.toml,
/// with one section per product: [jira] (also used by agile), [confluence]
/// and [bitbucket].
#[derive(Debug, Parser)]
#[command(name = "atlassian", version)]
pub struct Cli {
    /// Base URL of the product, e.g. https://jira.example.com. Overrides
    /// `base_url` in the product's config section.
    #[arg(long, global = true, value_name = "URL", help_heading = GLOBAL)]
    pub base_url: Option<String>,

    /// Config file [default: ~/.atlassian.toml].
    #[arg(long, global = true, value_name = "PATH", help_heading = GLOBAL)]
    pub config: Option<PathBuf>,

    /// Extra PEM CA certificate to trust, for servers with a private CA.
    /// Overrides `ca_cert` in the product's config section.
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
}

impl Product {
    /// The config section that holds this product's settings.
    pub fn section(&self) -> Section {
        match self {
            Product::Jira(_) | Product::Agile(_) => Section::Jira,
            Product::Confluence(_) => Section::Confluence,
            Product::Bitbucket(_) => Section::Bitbucket,
        }
    }

    pub fn to_request(&self) -> Result<ApiRequest> {
        match self {
            Product::Jira(c) => c.to_request(),
            Product::Agile(c) => c.to_request(),
            Product::Confluence(c) => c.to_request(),
            Product::Bitbucket(c) => c.to_request(),
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
    fn config_sections() {
        let cli = Cli::try_parse_from(["atlassian", "agile", "board", "get", "1"]).unwrap();
        assert_eq!(cli.product.section(), Section::Jira);
        let cli = Cli::try_parse_from(["atlassian", "bitbucket", "project", "list"]).unwrap();
        assert_eq!(cli.product.section(), Section::Bitbucket);
    }

    #[test]
    fn global_flags_after_subcommand() {
        let cli = Cli::try_parse_from([
            "atlassian",
            "jira",
            "myself",
            "--base-url",
            "https://jira.example.com",
            "--config",
            "/tmp/c.toml",
            "--compact",
        ])
        .unwrap();
        assert_eq!(cli.base_url.as_deref(), Some("https://jira.example.com"));
        assert_eq!(cli.config, Some(PathBuf::from("/tmp/c.toml")));
        assert!(cli.compact);
    }

    /// Every command and argument must explain itself in `--help`: people
    /// and LLM agents learn the CLI from it.
    #[test]
    fn every_command_and_argument_has_help() {
        fn walk(cmd: &clap::Command, path: &str, missing: &mut Vec<String>) {
            for arg in cmd.get_arguments() {
                let id = arg.get_id().as_str();
                if id == "help" || id == "version" || arg.is_global_set() && !path.is_empty() {
                    continue;
                }
                if arg.get_help().is_none() {
                    missing.push(format!("argument `{id}` of `atlassian{path}`"));
                }
            }
            for sub in cmd.get_subcommands() {
                if sub.get_name() == "help" {
                    continue;
                }
                let sub_path = format!("{path} {}", sub.get_name());
                if sub.get_about().is_none() {
                    missing.push(format!("command `atlassian{sub_path}`"));
                }
                walk(sub, &sub_path, missing);
            }
        }
        let mut missing = Vec::new();
        walk(&Cli::command(), "", &mut missing);
        assert!(
            missing.is_empty(),
            "missing help text:\n  {}",
            missing.join("\n  ")
        );
    }
}
