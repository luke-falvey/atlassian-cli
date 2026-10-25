//! Basic-auth credentials read from `~/.atlassian_credentials`.
//!
//! The file holds a single line in the form `username:password`. Only the
//! first `:` separates the two, so passwords may themselves contain `:`.

use std::fmt;
use std::path::{Path, PathBuf};

use thiserror::Error;

/// File name looked up in the user's home directory.
pub const DEFAULT_FILE_NAME: &str = ".atlassian_credentials";

#[derive(Debug, Error)]
pub enum CredentialsError {
    #[error("could not determine the home directory; pass --credentials-file")]
    NoHomeDir,
    #[error("could not read credentials file {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("credentials file {0} is empty; expected `username:password`")]
    Empty(PathBuf),
    #[error("credentials file {0} is malformed; expected `username:password`")]
    MissingSeparator(PathBuf),
    #[error("credentials file {0} has an empty username")]
    EmptyUsername(PathBuf),
}

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

impl Credentials {
    /// Path of the default credentials file, `~/.atlassian_credentials`.
    pub fn default_path() -> Result<PathBuf, CredentialsError> {
        dirs::home_dir()
            .map(|home| home.join(DEFAULT_FILE_NAME))
            .ok_or(CredentialsError::NoHomeDir)
    }

    /// Read credentials from `path`, warning on stderr if the file is
    /// readable by other users.
    pub fn from_file(path: &Path) -> Result<Self, CredentialsError> {
        let contents = std::fs::read_to_string(path).map_err(|source| CredentialsError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        warn_if_world_readable(path);
        Self::parse(&contents, path)
    }

    /// Parse the contents of a credentials file. `path` is only used in errors.
    pub fn parse(contents: &str, path: &Path) -> Result<Self, CredentialsError> {
        // Use the first non-blank line; trailing newlines (LF or CRLF) are ignored.
        let line = contents
            .lines()
            .map(|l| l.trim_end_matches('\r'))
            .find(|l| !l.trim().is_empty())
            .ok_or_else(|| CredentialsError::Empty(path.to_path_buf()))?;
        let (username, password) = line
            .split_once(':')
            .ok_or_else(|| CredentialsError::MissingSeparator(path.to_path_buf()))?;
        let username = username.trim();
        if username.is_empty() {
            return Err(CredentialsError::EmptyUsername(path.to_path_buf()));
        }
        Ok(Self {
            username: username.to_string(),
            password: password.to_string(),
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

    fn parse(s: &str) -> Result<Credentials, CredentialsError> {
        Credentials::parse(s, Path::new("creds"))
    }

    #[test]
    fn parses_username_and_password() {
        let c = parse("alice:s3cret").unwrap();
        assert_eq!(c.username, "alice");
        assert_eq!(c.password, "s3cret");
    }

    #[test]
    fn password_may_contain_colons() {
        let c = parse("alice:pa:ss:word").unwrap();
        assert_eq!(c.username, "alice");
        assert_eq!(c.password, "pa:ss:word");
    }

    #[test]
    fn trailing_newlines_are_ignored() {
        assert_eq!(parse("alice:pw\n").unwrap().password, "pw");
        assert_eq!(parse("alice:pw\r\n").unwrap().password, "pw");
        assert_eq!(parse("\n\nalice:pw\n\n").unwrap().username, "alice");
    }

    #[test]
    fn password_whitespace_is_preserved() {
        assert_eq!(parse("alice: pw ").unwrap().password, " pw ");
    }

    #[test]
    fn empty_password_is_allowed() {
        assert_eq!(parse("alice:").unwrap().password, "");
    }

    #[test]
    fn missing_separator_is_an_error() {
        assert!(matches!(
            parse("alice"),
            Err(CredentialsError::MissingSeparator(_))
        ));
    }

    #[test]
    fn empty_file_is_an_error() {
        assert!(matches!(parse(""), Err(CredentialsError::Empty(_))));
        assert!(matches!(parse("\n \n"), Err(CredentialsError::Empty(_))));
    }

    #[test]
    fn empty_username_is_an_error() {
        assert!(matches!(
            parse(":pw"),
            Err(CredentialsError::EmptyUsername(_))
        ));
    }

    #[test]
    fn missing_file_is_an_error() {
        let err = Credentials::from_file(Path::new("/definitely/not/here")).unwrap_err();
        assert!(matches!(err, CredentialsError::Read { .. }));
        assert!(err.to_string().contains("/definitely/not/here"));
    }

    #[test]
    fn reads_from_file() {
        let dir = std::env::temp_dir().join(format!("atl-creds-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("creds");
        std::fs::write(&path, "bob:hunter2\n").unwrap();
        let c = Credentials::from_file(&path).unwrap();
        assert_eq!(c.username, "bob");
        assert_eq!(c.password, "hunter2");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn debug_redacts_password() {
        let c = parse("alice:topsecret").unwrap();
        let dbg = format!("{c:?}");
        assert!(dbg.contains("alice"));
        assert!(!dbg.contains("topsecret"));
    }
}
