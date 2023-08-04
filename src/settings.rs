use deadpool_redis::ConnectionInfo;
use serde::Deserialize;
use serde_aux::prelude::deserialize_number_from_string;
use serde_with::{serde_as, DurationSeconds};
use std::time::Duration;

#[derive(Debug, Deserialize)]
pub struct Settings {
    pub redis: RedisSettings,
    pub app: AppSettings,
    pub todo_handler: TodoHandlerSettings,
}

#[serde_as]
#[derive(Debug, Deserialize)]
pub struct TodoHandlerSettings {
    #[serde_as(as = "DurationSeconds<u64>")]
    pub store_interval: Duration,
}

#[derive(Debug, Deserialize)]
pub struct RedisSettings {
    pub host: String,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    pub username: String,
    pub password: String,
    pub db: i64,
}

impl From<RedisSettings> for ConnectionInfo {
    fn from(value: RedisSettings) -> Self {
        use deadpool_redis::{ConnectionAddr, RedisConnectionInfo};

        let addr = ConnectionAddr::Tcp(value.host, value.port);
        let connection_info = RedisConnectionInfo {
            db: value.db,
            username: Some(value.username),
            password: Some(value.password),
        };

        Self {
            redis: connection_info,
            addr,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct AppSettings {
    pub host: String,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
}

pub fn get_settings() -> Result<Settings, config::ConfigError> {
    let base_path = std::env::current_dir().expect("Failed to determine the current directory");
    let configuration_directory = base_path.join("config");

    // Detect the running environment.
    // Default to `local` if unspecified.
    let environment: Environment = std::env::var("APP_ENVIRONMENT")
        .unwrap_or_else(|_| "local".into())
        .try_into()
        .expect("Failed to parse APP_ENVIRONMENT.");
    let environment_filename = format!("{}.yaml", environment.as_str());
    let settings = config::Config::builder()
        .add_source(config::File::from(
            configuration_directory.join("base.yaml"),
        ))
        .add_source(config::File::from(
            configuration_directory.join(environment_filename),
        ))
        // Add in settings from environment variables (with a prefix of APP and '__' as separator)
        // E.g. `APP_APPLICATION__PORT=5001 would set `Settings.application.port`
        .add_source(
            config::Environment::with_prefix("APP")
                .prefix_separator("_")
                .separator("__"),
        )
        .build()?;

    settings.try_deserialize::<Settings>()
}

pub enum Environment {
    Local,
    Production,
}

impl Environment {
    pub fn as_str(&self) -> &'static str {
        match self {
            Environment::Local => "local",
            Environment::Production => "production",
        }
    }
}

impl TryFrom<String> for Environment {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        match s.to_lowercase().as_str() {
            "local" => Ok(Self::Local),
            "production" => Ok(Self::Production),
            other => Err(format!(
                "{} is not a supported environment. Use either `local` or `production`.",
                other
            )),
        }
    }
}
