use std::{env, fmt, net::SocketAddr, path::PathBuf, str::FromStr};

use sqlx::{ConnectOptions, postgres::PgConnectOptions};

pub const DEFAULT_BIND: &str = "127.0.0.1:4010";
pub const UNVERSIONED_BUILD: &str = "unversioned-development";

#[derive(Clone)]
pub struct Config {
    pub(crate) bind: SocketAddr,
    pub(crate) database: PgConnectOptions,
    pub(crate) build_revision: String,
    pub(crate) web_root: Option<PathBuf>,
    pub(crate) game_root: Option<PathBuf>,
    #[cfg(test)]
    pub(crate) game_test_fixture: bool,
}

impl fmt::Debug for Config {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Config")
            .field("bind", &self.bind)
            .field("database", &"[redacted]")
            .field("build_revision", &self.build_revision)
            .field("web_root", &self.web_root)
            .field("game_root", &self.game_root)
            .finish()
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("DATABASE_URL is required")]
    MissingDatabaseUrl,
    #[error("DATABASE_URL must be a PostgreSQL URL with an explicit database name")]
    InvalidDatabaseUrl,
    #[error("CLUBSCAPE_BIND must be an IP socket address")]
    InvalidBind,
    #[error("CLUBSCAPE_BIND must use a loopback IP address")]
    NonLoopbackBind,
    #[error("CLUBSCAPE_BUILD_REVISION must be nonblank, printable and at most 256 bytes")]
    InvalidBuildRevision,
    #[error("server configuration environment values must be valid Unicode")]
    NonUnicodeEnvironment,
    #[error("CLUBSCAPE_WEB_ROOT must be a nonempty path")]
    InvalidWebRoot,
    #[error("CLUBSCAPE_GAME_ROOT must be a nonempty printable path")]
    InvalidGameRoot,
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_environment(optional_env)
    }

    fn from_environment(
        mut read: impl FnMut(&str) -> Result<Option<String>, ConfigError>,
    ) -> Result<Self, ConfigError> {
        let database_url = read("DATABASE_URL")?.ok_or(ConfigError::MissingDatabaseUrl)?;
        let bind = read("CLUBSCAPE_BIND")?.unwrap_or_else(|| DEFAULT_BIND.to_owned());
        let revision = read("CLUBSCAPE_BUILD_REVISION")?;
        let mut config = Self::new(&database_url, &bind, revision.as_deref())?;
        if let Some(root) = read("CLUBSCAPE_WEB_ROOT")? {
            if root.trim().is_empty() || root.chars().any(char::is_control) {
                return Err(ConfigError::InvalidWebRoot);
            }
            config = config.with_web_root(PathBuf::from(root))?;
        }
        if let Some(root) = read("CLUBSCAPE_GAME_ROOT")? {
            config = config.with_game_root(PathBuf::from(root))?;
        }
        Ok(config)
    }

    pub fn new(
        database_url: &str,
        bind: &str,
        build_revision: Option<&str>,
    ) -> Result<Self, ConfigError> {
        let bind: SocketAddr = bind.parse().map_err(|_| ConfigError::InvalidBind)?;
        if !bind.ip().is_loopback() {
            return Err(ConfigError::NonLoopbackBind);
        }
        if !database_url.starts_with("postgres://") && !database_url.starts_with("postgresql://") {
            return Err(ConfigError::InvalidDatabaseUrl);
        }
        let database = PgConnectOptions::from_str(database_url)
            .map_err(|_| ConfigError::InvalidDatabaseUrl)?
            .disable_statement_logging()
            .application_name("clubscape-server");
        if database.get_database().is_none_or(str::is_empty) {
            return Err(ConfigError::InvalidDatabaseUrl);
        }
        let build_revision = build_revision.unwrap_or(UNVERSIONED_BUILD);
        if build_revision.trim().is_empty()
            || build_revision.len() > 256
            || build_revision.chars().any(char::is_control)
        {
            return Err(ConfigError::InvalidBuildRevision);
        }
        Ok(Self {
            bind,
            database,
            build_revision: build_revision.to_owned(),
            web_root: None,
            game_root: None,
            #[cfg(test)]
            game_test_fixture: false,
        })
    }

    pub fn bind_address(&self) -> SocketAddr {
        self.bind
    }

    pub fn build_revision(&self) -> &str {
        &self.build_revision
    }

    pub fn with_web_root(mut self, root: impl Into<PathBuf>) -> Result<Self, ConfigError> {
        let root = root.into();
        if root.as_os_str().is_empty() {
            return Err(ConfigError::InvalidWebRoot);
        }
        self.web_root = Some(root);
        Ok(self)
    }

    pub fn with_game_root(mut self, root: impl Into<PathBuf>) -> Result<Self, ConfigError> {
        let root = root.into();
        if root.as_os_str().is_empty()
            || root
                .to_str()
                .is_none_or(|value| value.trim().is_empty() || value.chars().any(char::is_control))
        {
            return Err(ConfigError::InvalidGameRoot);
        }
        self.game_root = Some(root);
        Ok(self)
    }
}

fn optional_env(name: &str) -> Result<Option<String>, ConfigError> {
    match env::var(name) {
        Ok(value) => Ok(Some(value)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(env::VarError::NotUnicode(_)) => Err(ConfigError::NonUnicodeEnvironment),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DATABASE: &str = "postgres://localhost/clubscape_config_unit_test";

    #[test]
    fn game_and_web_environment_roots_are_independent() {
        for (web, game) in [
            (None, None),
            (Some("public-web"), None),
            (None, Some("source-game")),
            (Some("public-web"), Some("source-game")),
        ] {
            let config = Config::from_environment(|name| {
                Ok(match name {
                    "DATABASE_URL" => Some(DATABASE.to_owned()),
                    "CLUBSCAPE_WEB_ROOT" => web.map(str::to_owned),
                    "CLUBSCAPE_GAME_ROOT" => game.map(str::to_owned),
                    _ => None,
                })
            })
            .unwrap();
            assert_eq!(config.web_root, web.map(PathBuf::from));
            assert_eq!(config.game_root, game.map(PathBuf::from));
        }
    }

    #[test]
    fn invalid_game_environment_is_not_ignored_when_web_root_is_absent() {
        for root in ["", "   ", "bad\nroot", "bad\0root"] {
            let result = Config::from_environment(|name| {
                Ok(match name {
                    "DATABASE_URL" => Some(DATABASE.to_owned()),
                    "CLUBSCAPE_GAME_ROOT" => Some(root.to_owned()),
                    _ => None,
                })
            });
            assert_eq!(result.unwrap_err(), ConfigError::InvalidGameRoot);
        }
        let result = Config::from_environment(|name| match name {
            "DATABASE_URL" => Ok(Some(DATABASE.to_owned())),
            "CLUBSCAPE_GAME_ROOT" => Err(ConfigError::NonUnicodeEnvironment),
            _ => Ok(None),
        });
        assert_eq!(result.unwrap_err(), ConfigError::NonUnicodeEnvironment);
    }

    #[test]
    fn binds_only_literal_loopback_addresses_and_allows_ephemeral_ports() {
        for address in ["127.0.0.1:4010", "127.0.0.2:0", "[::1]:0"] {
            let config = Config::new(DATABASE, address, None).unwrap();
            assert!(config.bind_address().ip().is_loopback());
        }
        for address in ["0.0.0.0:4010", "[::]:4010", "192.0.2.1:4010"] {
            assert_eq!(
                Config::new(DATABASE, address, None).unwrap_err(),
                ConfigError::NonLoopbackBind
            );
        }
        for address in ["localhost:4010", "127.0.0.1", "127.0.0.1:65536"] {
            assert_eq!(
                Config::new(DATABASE, address, None).unwrap_err(),
                ConfigError::InvalidBind
            );
        }
    }

    #[test]
    fn reports_the_exact_revision_or_an_explicit_development_marker() {
        assert_eq!(
            Config::new(DATABASE, DEFAULT_BIND, None)
                .unwrap()
                .build_revision(),
            UNVERSIONED_BUILD
        );
        let revision = "exact-review-revision+dirty ";
        assert_eq!(
            Config::new(DATABASE, DEFAULT_BIND, Some(revision))
                .unwrap()
                .build_revision(),
            revision
        );
        for revision in [
            "",
            "  ",
            "bad\nrevision",
            "bad\u{0}revision",
            &"a".repeat(257),
        ] {
            assert_eq!(
                Config::new(DATABASE, DEFAULT_BIND, Some(revision)).unwrap_err(),
                ConfigError::InvalidBuildRevision
            );
        }
    }

    #[test]
    fn rejects_missing_database_names_and_non_postgres_urls_without_echoing_input() {
        for url in ["", "sqlite://secret", "postgres://localhost", "not-a-url"] {
            let error = Config::new(url, DEFAULT_BIND, None).unwrap_err();
            assert_eq!(error, ConfigError::InvalidDatabaseUrl);
            assert!(!format!("{error:?} {error}").contains("secret"));
        }
        let config = Config::new(
            "postgres://synthetic:never-log-this@localhost/clubscape_config_unit_test",
            DEFAULT_BIND,
            None,
        )
        .unwrap();
        assert!(!format!("{config:?}").contains("never-log-this"));
        assert!(!format!("{config:?}").contains("synthetic"));
    }
}
