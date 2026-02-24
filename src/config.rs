use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;
use std::env;

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8080,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "postgresql://postgres@localhost:5432/cairn".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct RedisConfig {
    pub url: String,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: "redis://localhost:6379".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct JwtConfig {
    pub secret: String,
    pub expiry_minutes: i64,
    pub refresh_secret: String,
    pub refresh_expiry_days: i64,
}

impl Default for JwtConfig {
    fn default() -> Self {
        Self {
            secret: "change-me-in-production".to_string(),
            expiry_minutes: 15,
            refresh_secret: "change-me-too-in-production".to_string(),
            refresh_expiry_days: 7,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct OAuthConfig {
    pub google_client_id: Option<String>,
    pub google_client_secret: Option<String>,
    pub github_client_id: Option<String>,
    pub github_client_secret: Option<String>,
    pub redirect_base: String,
}

impl Default for OAuthConfig {
    fn default() -> Self {
        Self {
            google_client_id: None,
            google_client_secret: None,
            github_client_id: None,
            github_client_secret: None,
            redirect_base: "http://localhost:8080".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct InviteConfig {
    pub salt: String,
    pub require_invite: bool,
}

impl Default for InviteConfig {
    fn default() -> Self {
        Self {
            salt: "default-salt-change-me".to_string(),
            require_invite: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub from_email: String,
}

impl Default for SmtpConfig {
    fn default() -> Self {
        Self {
            host: "smtp.example.com".to_string(),
            port: 587,
            user: "".to_string(),
            password: "".to_string(),
            from_email: "noreply@cairn.local".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct RateLimitConfig {
    pub login_per_minute: u32,
    pub register_per_hour: u32,
    pub forgot_password_per_hour: u32,
    pub invite_validate_per_minute: u32,
    pub authenticated_per_minute: u32,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            login_per_minute: 5,
            register_per_hour: 3,
            forgot_password_per_hour: 3,
            invite_validate_per_minute: 10,
            authenticated_per_minute: 60,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct AppConfig {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub database: DatabaseConfig,
    #[serde(default)]
    pub redis: RedisConfig,
    #[serde(default)]
    pub jwt: JwtConfig,
    #[serde(default)]
    pub oauth: OAuthConfig,
    #[serde(default)]
    pub invite: InviteConfig,
    #[serde(default)]
    pub smtp: SmtpConfig,
    #[serde(default)]
    pub rate_limit: RateLimitConfig,
}

impl AppConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        let run_mode = env::var("RUN_MODE").unwrap_or_else(|_| "development".into());

        let config = Config::builder()
            .add_source(File::with_name("config/default").required(false))
            .add_source(File::with_name(&format!("config/{}", run_mode)).required(false))
            .add_source(Environment::with_prefix("CAIRN").separator("__"))
            .build()?;

        config.try_deserialize()
    }

    pub fn new() -> Result<Self, ConfigError> {
        Self::from_env()
    }
}
