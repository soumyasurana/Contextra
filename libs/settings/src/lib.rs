mod error;
mod settings;

pub use error::*;
pub use settings::*;

use config::{Config, Environment};

pub fn load() -> Result<AppConfig, ConfigError> {
    dotenvy::dotenv().ok();

    let config = Config::builder()
        .add_source(Environment::default().separator("_"))
        .build()?;

    Ok(config.try_deserialize()?)
}
