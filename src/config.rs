use config::{Config, ConfigError, File};
use serde::Deserialize;

/// All settings for the server. Currently there are only application settings, but in the future
/// there may be e.g. database settings.
#[derive(Deserialize)]
pub struct Settings {
    /// Application settings.
    pub application: ApplicationSettings,
}

/// Application settings.
#[derive(Deserialize)]
pub struct ApplicationSettings {
    /// The port number on which the application will listen.
    pub port: u16,

    /// The hostname or IP address where the application will run.
    ///
    /// This is a `String` that specifies the network address at which the application is
    /// accessible. This could be a hostname like "localhost" or an IP address like
    /// "127.0.0.1".
    pub host: String,
}

/// Based on the `APP_ENVIRONMENT` environment variable, reads the corresponding configuration file
/// and returns the settings.
pub fn get_configuration() -> Result<Settings, ConfigError> {
    let base_path = std::env::current_dir().expect("Failed to get current directory");
    let config_dir = base_path.join("config");

    let environment: Environment = std::env::var("APP_ENVIRONMENT")
        .unwrap_or_else(|_| "local".into())
        .try_into()
        .expect("Failed to parse APP_ENVIRONMENT");
    let environment_filename = format!("{}.toml", environment.as_str());

    let settings = Config::builder()
        .add_source(File::from(config_dir.join(environment_filename)).required(false))
        .build()?;
    settings.try_deserialize()
}

/// The possible runtime environments for the application.
pub enum Environment {
    /// Local development environment.
    Local,
    /// Production environment.
    Production,
}

impl Environment {
    /// Returns the environment as a string.
    pub fn as_str(&self) -> &str {
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
                "{other} is not a supported environment. Must be `local` or `production"
            )),
        }
    }
}
