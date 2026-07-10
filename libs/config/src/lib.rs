mod config;
mod error;

pub use config::*;
pub use error::*;

use config::{Config, Environment};

pub fn load() -> Result<AppConfig, ConfigError> {
    dotenvy::dotenv().ok();

    let config = Config::builder()
        .add_source(Environment::default().separator("_"))
        .build()?;

    Ok(config.try_deserialize()?)
}