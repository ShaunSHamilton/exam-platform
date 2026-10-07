use std::{fmt, str::FromStr};

use crate::ConfigError;

/// Deployment environment. Read from `ENVIRONMENT`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    Development,
    Staging,
    Production,
}

impl Environment {
    /// `ENVIRONMENT`, defaulting to `development` in debug builds only: a release build
    /// must say where it runs, so a missing variable never silently disables Sentry.
    pub fn from_env() -> Result<Self, ConfigError> {
        match crate::var("ENVIRONMENT") {
            Some(v) => v.parse(),
            None if cfg!(debug_assertions) => Ok(Self::Development),
            None => Err(ConfigError::new(
                "ENVIRONMENT must be set in release builds (development, staging or production)",
            )),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Development => "development",
            Self::Staging => "staging",
            Self::Production => "production",
        }
    }

    pub const fn is_development(self) -> bool {
        matches!(self, Self::Development)
    }
}

impl FromStr for Environment {
    type Err = ConfigError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "development" => Ok(Self::Development),
            "staging" => Ok(Self::Staging),
            "production" => Ok(Self::Production),
            other => Err(ConfigError::new(format!(
                "ENVIRONMENT must be development, staging or production, found {other:?}"
            ))),
        }
    }
}

impl fmt::Display for Environment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_known_values_only() {
        assert_eq!(
            "development".parse::<Environment>().unwrap(),
            Environment::Development
        );
        assert_eq!(
            "staging".parse::<Environment>().unwrap(),
            Environment::Staging
        );
        assert_eq!(
            "production".parse::<Environment>().unwrap(),
            Environment::Production
        );
        assert!("prod".parse::<Environment>().is_err());
        assert!("Production".parse::<Environment>().is_err());
    }

    #[test]
    fn round_trips_through_as_str() {
        for env in [
            Environment::Development,
            Environment::Staging,
            Environment::Production,
        ] {
            assert_eq!(env.as_str().parse::<Environment>().unwrap(), env);
        }
    }
}
