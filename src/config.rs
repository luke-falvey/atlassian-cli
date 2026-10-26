//! Configuration read from `~/.atlassian.toml`.
//!
//! ```toml
//! # Default Basic-auth credentials, used by every product without its own.
//! username = "alice"
//! password = "secret"
//!
//! [jira]          # also used by `agile` (Jira Software)
//! base_url = "https://jira.example.com"
//!
//! [jira.scope]    # optional: limit what may be read (see `crate::scope`)
//! projects = ["ABC"]
//!
//! [confluence]
//! base_url = "https://wiki.example.com/confluence"
//! username = "svc-confluence"   # optional per-product override
//! password = "other-secret"
//!
//! [bitbucket]
//! base_url = "https://bitbucket.example.com"
//! ca_cert = "/etc/ssl/corp-ca.pem"  # optional extra CA certificate
//! ```

use std::fmt;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::scope::{ProductScope, Scope};
use thiserror::Error;

/// File name looked up in the user's home directory.
pub const DEFAULT_FILE_NAME: &str = ".atlassian.toml";

/// The config sections, one per product. Jira Software uses `[jira]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Jira,
    Confluence,
    Bitbucket,
}

impl Section {
    pub fn name(self) -> &'static str {
        match self {
            Section::Jira => "jira",
            Section::Confluence => "confluence",
            Section::Bitbucket => "bitbucket",
        }
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("could not determine the home directory; pass --config")]
    NoHomeDir,
    #[error("could not read config file {path}: {error}")]
    Read {
        path: PathBuf,
        error: std::io::Error,
    },
    #[error("invalid config file {path}: {message}")]
    Parse { path: PathBuf, message: String },
    #[error(
        "no base URL for {section}: set `base_url` in the [{section}] section of {path}, \
         or pass --base-url"
    )]
    MissingBaseUrl {
        section: &'static str,
        path: PathBuf,
    },
    #[error(
        "no {field} for {section}: set `{field}` at the top of {path} \
         or in its [{section}] section"
    )]
    MissingCredential {
        field: &'static str,
        section: &'static str,
        path: PathBuf,
    },
    #[error("the username for {section} in {path} is empty")]
    EmptyUsername {
        section: &'static str,
        path: PathBuf,
    },
}

/// Settings for one product section.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductConfig {
    pub base_url: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub ca_cert: Option<PathBuf>,
    /// Limits on what this product may read, e.g. `[jira.scope]`.
    /// Absent: no limits.
    pub scope: Option<ProductScope>,
}

/// The whole config file.
#[derive(Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub username: Option<String>,
    pub password: Option<String>,
    #[serde(default)]
    pub jira: ProductConfig,
    #[serde(default)]
    pub confluence: ProductConfig,
    #[serde(default)]
    pub bitbucket: ProductConfig,
    #[serde(default)]
    pub log: LogConfig,
}

/// The `[log]` section: where to record requests.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LogConfig {
    /// Record requests (default: true).
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Log file (default: `~/.local/state/atlassian/requests.log`). A leading
    /// `~/` means the home directory.
    pub path: Option<PathBuf>,
}

fn default_true() -> bool {
    true
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            path: None,
        }
    }
}

/// Default request log location.
pub const DEFAULT_LOG_PATH: &str = ".local/state/atlassian/requests.log";

impl LogConfig {
    /// The log file to write, or `None` when logging is off.
    pub fn file(&self) -> Result<Option<PathBuf>, ConfigError> {
        if !self.enabled {
            return Ok(None);
        }
        let home = || dirs::home_dir().ok_or(ConfigError::NoHomeDir);
        let path = match &self.path {
            None => home()?.join(DEFAULT_LOG_PATH),
            Some(p) => match p.strip_prefix("~") {
                Ok(rest) => home()?.join(rest),
                Err(_) => p.clone(),
            },
        };
        Ok(Some(path))
    }
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let redact = |p: &Option<String>| p.as_ref().map(|_| "<redacted>");
        let section = |s: &ProductConfig| {
            format!(
                "{{ base_url: {:?}, username: {:?}, password: {:?}, ca_cert: {:?}, scope: {:?} }}",
                s.base_url,
                s.username,
                redact(&s.password),
                s.ca_cert,
                s.scope
            )
        };
        f.debug_struct("Config")
            .field("username", &self.username)
            .field("password", &redact(&self.password))
            .field("jira", &section(&self.jira))
            .field("confluence", &section(&self.confluence))
            .field("bitbucket", &section(&self.bitbucket))
            .field("log", &self.log)
            .finish()
    }
}

/// Basic-auth credentials.
#[derive(Clone, PartialEq, Eq)]
pub struct Credentials {
    pub username: String,
    pub password: String,
}

impl fmt::Debug for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Credentials")
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .finish()
    }
}

/// Everything needed to talk to one product.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    pub base_url: String,
    pub credentials: Credentials,
    pub ca_cert: Option<PathBuf>,
    pub scope: Scope,
    /// Request log file, or `None` when logging is off.
    pub log: Option<PathBuf>,
}

impl Config {
    /// Path of the default config file, `~/.atlassian.toml`.
    pub fn default_path() -> Result<PathBuf, ConfigError> {
        dirs::home_dir()
            .map(|home| home.join(DEFAULT_FILE_NAME))
            .ok_or(ConfigError::NoHomeDir)
    }

    /// Read the config from `path`, warning on stderr if other users can
    /// read it (it holds passwords).
    pub fn from_file(path: &Path) -> Result<Self, ConfigError> {
        let contents = std::fs::read_to_string(path).map_err(|error| ConfigError::Read {
            path: path.to_path_buf(),
            error,
        })?;
        warn_if_world_readable(path);
        Self::parse(&contents, path)
    }

    /// Parse config text. `path` is only used in errors.
    pub fn parse(contents: &str, path: &Path) -> Result<Self, ConfigError> {
        let config: Self = toml::from_str(contents).map_err(|e| ConfigError::Parse {
            path: path.to_path_buf(),
            message: e.message().to_string(),
        })?;
        for section in [Section::Jira, Section::Confluence, Section::Bitbucket] {
            if let Some(scope) = &config.section(section).scope {
                scope
                    .validate(section.name())
                    .map_err(|message| ConfigError::Parse {
                        path: path.to_path_buf(),
                        message,
                    })?;
            }
        }
        Ok(config)
    }

    pub fn section(&self, section: Section) -> &ProductConfig {
        match section {
            Section::Jira => &self.jira,
            Section::Confluence => &self.confluence,
            Section::Bitbucket => &self.bitbucket,
        }
    }

    /// Settings for `section`. Section credentials win over the top-level
    /// ones; `base_url` and `ca_cert` overrides (from flags) win over both.
    pub fn resolve(
        &self,
        section: Section,
        base_url: Option<&str>,
        ca_cert: Option<&Path>,
        path: &Path,
    ) -> Result<Resolved, ConfigError> {
        let s = self.section(section);
        let name = section.name();
        let base_url = base_url
            .map(str::to_string)
            .or_else(|| s.base_url.clone())
            .filter(|u| !u.trim().is_empty())
            .ok_or_else(|| ConfigError::MissingBaseUrl {
                section: name,
                path: path.to_path_buf(),
            })?;
        let missing = |field| ConfigError::MissingCredential {
            field,
            section: name,
            path: path.to_path_buf(),
        };
        let username = s
            .username
            .clone()
            .or_else(|| self.username.clone())
            .ok_or_else(|| missing("username"))?;
        if username.trim().is_empty() {
            return Err(ConfigError::EmptyUsername {
                section: name,
                path: path.to_path_buf(),
            });
        }
        let password = s
            .password
            .clone()
            .or_else(|| self.password.clone())
            .ok_or_else(|| missing("password"))?;
        Ok(Resolved {
            base_url,
            credentials: Credentials { username, password },
            ca_cert: ca_cert.map(Path::to_path_buf).or_else(|| s.ca_cert.clone()),
            scope: match &s.scope {
                Some(scope) => Scope::restricted(scope.clone()),
                None => Scope::unrestricted(),
            },
            log: self.log.file()?,
        })
    }
}

#[cfg(unix)]
fn warn_if_world_readable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(meta) = std::fs::metadata(path) {
        let mode = meta.permissions().mode() & 0o777;
        if mode & 0o077 != 0 {
            eprintln!(
                "warning: {} has permissions {:o}; run `chmod 600 {}` to keep your password private",
                path.display(),
                mode,
                path.display()
            );
        }
    }
}

#[cfg(not(unix))]
fn warn_if_world_readable(_path: &Path) {}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Result<Config, ConfigError> {
        Config::parse(s, Path::new("cfg.toml"))
    }

    fn resolve(s: &str, section: Section) -> Result<Resolved, ConfigError> {
        parse(s)?.resolve(section, None, None, Path::new("cfg.toml"))
    }

    const FULL: &str = r#"
username = "alice"
password = "pa:ss \"word\""

[jira]
base_url = "https://jira.example.com"

[confluence]
base_url = "https://wiki.example.com/confluence"
username = "svc"
password = "other"
ca_cert = "/etc/ca.pem"

[bitbucket]
base_url = "https://bitbucket.example.com"
"#;

    #[test]
    fn top_level_credentials_apply_to_every_section() {
        let r = resolve(FULL, Section::Jira).unwrap();
        assert_eq!(r.base_url, "https://jira.example.com");
        assert_eq!(r.credentials.username, "alice");
        assert_eq!(r.credentials.password, "pa:ss \"word\"");
        assert_eq!(r.ca_cert, None);
        let r = resolve(FULL, Section::Bitbucket).unwrap();
        assert_eq!(r.base_url, "https://bitbucket.example.com");
        assert_eq!(r.credentials.username, "alice");
    }

    #[test]
    fn section_credentials_and_ca_cert_override() {
        let r = resolve(FULL, Section::Confluence).unwrap();
        assert_eq!(r.base_url, "https://wiki.example.com/confluence");
        assert_eq!(r.credentials.username, "svc");
        assert_eq!(r.credentials.password, "other");
        assert_eq!(r.ca_cert, Some(PathBuf::from("/etc/ca.pem")));
    }

    #[test]
    fn flags_override_config() {
        let cfg = parse(FULL).unwrap();
        let r = cfg
            .resolve(
                Section::Confluence,
                Some("http://localhost:1990/confluence"),
                Some(Path::new("/tmp/other.pem")),
                Path::new("cfg.toml"),
            )
            .unwrap();
        assert_eq!(r.base_url, "http://localhost:1990/confluence");
        assert_eq!(r.ca_cert, Some(PathBuf::from("/tmp/other.pem")));
    }

    #[test]
    fn missing_base_url_names_the_section() {
        let err = resolve("username = \"a\"\npassword = \"b\"", Section::Bitbucket).unwrap_err();
        assert!(matches!(
            err,
            ConfigError::MissingBaseUrl {
                section: "bitbucket",
                ..
            }
        ));
        let msg = err.to_string();
        assert!(
            msg.contains("[bitbucket]") && msg.contains("--base-url"),
            "{msg}"
        );
    }

    #[test]
    fn missing_credentials_are_errors() {
        let err = resolve("[jira]\nbase_url = \"https://j\"", Section::Jira).unwrap_err();
        assert!(matches!(
            err,
            ConfigError::MissingCredential {
                field: "username",
                ..
            }
        ));
        let err = resolve(
            "username = \"a\"\n[jira]\nbase_url = \"https://j\"",
            Section::Jira,
        )
        .unwrap_err();
        assert!(matches!(
            err,
            ConfigError::MissingCredential {
                field: "password",
                ..
            }
        ));
    }

    #[test]
    fn empty_username_is_an_error() {
        let err = resolve(
            "username = \" \"\npassword = \"p\"\n[jira]\nbase_url = \"https://j\"",
            Section::Jira,
        )
        .unwrap_err();
        assert!(matches!(err, ConfigError::EmptyUsername { .. }));
    }

    #[test]
    fn empty_password_is_allowed() {
        let r = resolve(
            "username = \"a\"\npassword = \"\"\n[jira]\nbase_url = \"https://j\"",
            Section::Jira,
        )
        .unwrap();
        assert_eq!(r.credentials.password, "");
    }

    #[test]
    fn unknown_keys_and_bad_toml_are_errors() {
        let err = parse("[jira]\nbase-url = \"https://j\"").unwrap_err();
        assert!(matches!(err, ConfigError::Parse { .. }));
        assert!(err.to_string().contains("base-url"), "{err}");
        assert!(matches!(parse("[jira"), Err(ConfigError::Parse { .. })));
        assert!(matches!(
            parse("[jira-software]\n"),
            Err(ConfigError::Parse { .. })
        ));
    }

    #[test]
    fn removed_bamboo_section_is_rejected() {
        let err = parse("[bamboo]\nbase_url = \"https://bamboo.example.com\"").unwrap_err();
        assert!(matches!(err, ConfigError::Parse { .. }));
        assert!(err.to_string().contains("bamboo"), "{err}");
    }

    #[test]
    fn each_product_has_its_own_scope() {
        let cfg = format!(
            "{FULL}\n[jira.scope]\nprojects = [\"ABC\"]\nmax_results = 50\n\
             \n[bitbucket.scope]\nrepos = [\"PROJ/*\"]\nallow_raw = true\n"
        );
        let jira = resolve(&cfg, Section::Jira).unwrap().scope;
        assert!(jira.is_restricted());
        assert!(jira.allows_jira_project("ABC") && !jira.allows_jira_project("XYZ"));
        assert_eq!(jira.max_results(), Some(50));

        let bitbucket = resolve(&cfg, Section::Bitbucket).unwrap().scope;
        assert!(bitbucket.allows_bitbucket_repo("PROJ", "any"));
        assert!(!bitbucket.allows_bitbucket_repo("HR", "x"));
        assert_eq!(
            bitbucket.max_results(),
            Some(crate::scope::DEFAULT_MAX_RESULTS)
        );

        // Confluence has no scope table, so it isn't restricted.
        let confluence = resolve(&cfg, Section::Confluence).unwrap().scope;
        assert!(!confluence.is_restricted());
        assert!(confluence.allows_confluence_space("ANY"));
    }

    #[test]
    fn bad_scope_values_are_errors() {
        let err = parse("[bitbucket.scope]\nrepos = [\"PROJ\"]").unwrap_err();
        assert!(err.to_string().contains("PROJECT/repo"), "{err}");
        let err = parse("[jira.scope]\nspaces = [\"DEV\"]").unwrap_err();
        assert!(
            err.to_string().contains("[jira.scope] takes `projects`"),
            "{err}"
        );
        let err = parse("[jira.scope]\nproject = [\"ABC\"]").unwrap_err();
        assert!(err.to_string().contains("project"), "{err}");
        assert!(parse("[confluence.scope]\nmax_results = 0").is_err());
        // The old top-level [scope] table is no longer accepted.
        assert!(parse("[scope]\njira_projects = [\"ABC\"]").is_err());
    }

    #[test]
    fn log_defaults_on_in_home() {
        let r = resolve(FULL, Section::Jira).unwrap();
        let path = r.log.expect("logging is on by default");
        assert!(path.ends_with(DEFAULT_LOG_PATH));
    }

    #[test]
    fn log_path_and_switch() {
        let off = format!("{FULL}\n[log]\nenabled = false\n");
        assert_eq!(resolve(&off, Section::Jira).unwrap().log, None);
        let custom = format!("{FULL}\n[log]\npath = \"/var/log/atl.log\"\n");
        assert_eq!(
            resolve(&custom, Section::Jira).unwrap().log,
            Some(PathBuf::from("/var/log/atl.log"))
        );
        let home = format!("{FULL}\n[log]\npath = \"~/atl.log\"\n");
        let path = resolve(&home, Section::Jira).unwrap().log.unwrap();
        assert!(path.is_absolute() && path.ends_with("atl.log"), "{path:?}");
    }

    #[test]
    fn empty_file_is_valid_but_resolves_nothing() {
        assert_eq!(parse("").unwrap(), Config::default());
    }

    #[test]
    fn missing_file_is_an_error() {
        let err = Config::from_file(Path::new("/definitely/not/here.toml")).unwrap_err();
        assert!(matches!(err, ConfigError::Read { .. }));
        assert!(err.to_string().contains("/definitely/not/here.toml"));
    }

    #[test]
    fn reads_from_file() {
        let dir = std::env::temp_dir().join(format!("atl-config-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("atlassian.toml");
        std::fs::write(&path, FULL).unwrap();
        let cfg = Config::from_file(&path).unwrap();
        assert_eq!(
            cfg.bitbucket.base_url.as_deref(),
            Some("https://bitbucket.example.com")
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn debug_redacts_passwords() {
        let cfg = parse(FULL).unwrap();
        let dbg = format!("{cfg:?}");
        assert!(dbg.contains("alice"));
        assert!(!dbg.contains("pa:ss") && !dbg.contains("other\""), "{dbg}");
        let creds = resolve(FULL, Section::Jira).unwrap().credentials;
        assert!(!format!("{creds:?}").contains("pa:ss"));
    }
}
