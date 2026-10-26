//! Startup resolves one home so files and model choices agree for every connection.

use std::collections::HashMap;
use std::ffi::OsString;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use axum_extra::extract::cookie::Key;

/// Settings shared by every HTTP route.
#[derive(Clone)]
pub struct Settings {
    pub home: PathBuf,
    /// The actual bound address, including the port resolved by the listener.
    pub listen: SocketAddr,
    pub password: Option<String>,
    pub mcp_token: Option<String>,
    pub trusted_integration: bool,
    pub session_key: Key,
    pub upload_limit: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("could not resolve the home folder: {0}")]
    Home(#[source] std::io::Error),
    #[error("could not read settings from {path}: {source}")]
    DotenvRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not parse settings from {path}")]
    DotenvParse { path: PathBuf },
    #[error("invalid {name}: {reason}")]
    Invalid {
        name: &'static str,
        reason: &'static str,
    },
    #[error("QUANTY_PASSWORD is required when listening outside loopback")]
    PasswordRequired,
    #[error("trusted integration requires a loopback listen address")]
    TrustedListen,
    #[error("trusted integration cannot be combined with a password or MCP token")]
    TrustedCredentials,
}

impl Settings {
    /// Environment values take precedence over the home's `.env`.
    ///
    /// # Errors
    /// Returns an error for unreadable settings or invalid values.
    pub fn from_sources(
        home: &Path,
        environment: impl Fn(&str) -> Option<String>,
    ) -> Result<Self, SettingsError> {
        let home = std::path::absolute(home).map_err(SettingsError::Home)?;
        let path = home.join(".env");
        let mut file = HashMap::new();
        if path.exists() {
            let entries =
                dotenvy::from_path_iter(&path).map_err(|error| dotenv_error(&path, error))?;
            for entry in entries {
                let (name, value) = entry.map_err(|error| dotenv_error(&path, error))?;
                file.insert(name, value);
            }
        }
        let value = |name: &str| {
            environment(name)
                .filter(|v| !v.is_empty())
                .or_else(|| file.get(name).filter(|v| !v.is_empty()).cloned())
        };
        let invalid = |name, reason| SettingsError::Invalid { name, reason };
        let listen = value("QUANTY_LISTEN")
            .unwrap_or_else(|| "127.0.0.1:7878".into())
            .parse()
            .map_err(|_| invalid("QUANTY_LISTEN", "expected an IP address and port"))?;
        let session_key = match value("QUANTY_SESSION_KEY") {
            None => Key::generate(),
            Some(hex) => {
                let bytes: Option<Vec<u8>> = (hex.len() == 64)
                    .then(|| {
                        (0..32)
                            .map(|i| u8::from_str_radix(hex.get(2 * i..2 * i + 2)?, 16).ok())
                            .collect()
                    })
                    .flatten();
                let bytes = bytes
                    .ok_or_else(|| invalid("QUANTY_SESSION_KEY", "expected 64 hex characters"))?;
                Key::derive_from(&bytes)
            }
        };
        let megabytes = value("QUANTY_UPLOAD_LIMIT_MB")
            .unwrap_or_else(|| "1024".into())
            .parse::<usize>()
            .map_err(|_| invalid("QUANTY_UPLOAD_LIMIT_MB", "expected a positive integer"))?;
        let upload_limit = megabytes
            .checked_mul(1024 * 1024)
            .filter(|v| *v > 0)
            .ok_or_else(|| {
                invalid(
                    "QUANTY_UPLOAD_LIMIT_MB",
                    "expected a positive size that fits in memory",
                )
            })?;
        Ok(Self {
            home,
            listen,
            password: value("QUANTY_PASSWORD"),
            mcp_token: value("QUANTY_MCP_TOKEN"),
            trusted_integration: false,
            session_key,
            upload_limit,
        })
    }

    /// Flags are applied before this final safety check.
    ///
    /// # Errors
    /// Refuses an unauthenticated server outside loopback.
    pub fn validate(&self) -> Result<(), SettingsError> {
        if self.trusted_integration {
            if !self.listen.ip().is_loopback() {
                return Err(SettingsError::TrustedListen);
            }
            if self.password.is_some() || self.mcp_token.is_some() {
                return Err(SettingsError::TrustedCredentials);
            }
        }
        if !self.listen.ip().is_loopback() && self.password.as_ref().is_none_or(String::is_empty) {
            return Err(SettingsError::PasswordRequired);
        }
        Ok(())
    }
}

/// User tool folders come last so an existing PATH keeps its precedence.
pub fn search_path() -> OsString {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let mut folders: Vec<_> = std::env::split_paths(&path).collect();
    let extra = std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".local/bin"))
        .into_iter()
        .chain([
            PathBuf::from("/opt/homebrew/bin"),
            PathBuf::from("/usr/local/bin"),
        ]);
    for folder in extra {
        if folder.is_dir() && !folders.contains(&folder) {
            folders.push(folder);
        }
    }
    std::env::join_paths(folders).unwrap_or(path)
}

fn dotenv_error(path: &Path, error: dotenvy::Error) -> SettingsError {
    match error {
        dotenvy::Error::Io(source) => SettingsError::DotenvRead {
            path: path.to_path_buf(),
            source,
        },
        // Parse errors contain the original line, which can hold a password or token.
        _ => SettingsError::DotenvParse {
            path: path.to_path_buf(),
        },
    }
}
