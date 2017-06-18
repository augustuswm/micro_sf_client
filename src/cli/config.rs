extern crate toml;

use std::fs::File;
use std::io::Read;

use error::CLIError;

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub login_url: String,
    pub version: String,
    pub username: String,
    pub password: String,
    pub client_id: String,
    pub client_secret: String,
}

impl Config {
    pub fn parse_config(path: &str) -> Result<Config, CLIError> {
        let mut config_toml = String::new();

        File::open(path)
            .map_err(CLIError::ConfigStorageFailure)
            .and_then(|mut file| {
                file.read_to_string(&mut config_toml).unwrap_or_else(
                    |err| {
                        panic!("Error while reading config: [{}]", err)
                    },
                );

                toml::from_str(&config_toml).or(Err(CLIError::InvalidConfig))
            })
    }
}
