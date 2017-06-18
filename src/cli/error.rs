extern crate micro_sf_client;
extern crate serde_json;

use std::error::Error;
use std::fmt;
use std::io;

use self::micro_sf_client::SFClientError;

#[derive(Debug)]
pub enum CLIError {
    InvalidConfig,
    ConfigStorageFailure(io::Error),
    Format(serde_json::error::Error),
    Network(SFClientError),
}

impl fmt::Display for CLIError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            CLIError::InvalidConfig => {
                write!(
                    f,
                    "Supplied config.toml could not be understood. Try checking for a \
                        misspelled or missing property."
                )
            }
            CLIError::ConfigStorageFailure(ref err) => err.fmt(f),
            CLIError::Format(_) => write!(f, "Failure to format response."),
            CLIError::Network(ref err) => err.fmt(f),
        }
    }
}

impl Error for CLIError {
    fn description(&self) -> &str {
        match *self {
            CLIError::InvalidConfig => {
                "Supplied config.toml could not be understood. Try checking for a misspelled or \
                 missing property."
            }
            CLIError::ConfigStorageFailure(ref err) => err.description(),
            CLIError::Format(_) => "Unable to format the response from the server.",
            CLIError::Network(ref err) => err.description(),
        }
    }

    fn cause(&self) -> Option<&Error> {
        match *self {
            CLIError::InvalidConfig => None,
            CLIError::ConfigStorageFailure(ref err) => Some(err),
            CLIError::Format(ref err) => Some(err),
            CLIError::Network(ref err) => Some(err),
        }
    }
}

impl From<io::Error> for CLIError {
    fn from(err: io::Error) -> CLIError {
        CLIError::ConfigStorageFailure(err)
    }
}

impl From<serde_json::error::Error> for CLIError {
    fn from(err: serde_json::error::Error) -> CLIError {
        CLIError::Format(err)
    }
}

impl From<micro_sf_client::SFClientError> for CLIError {
    fn from(err: micro_sf_client::SFClientError) -> CLIError {
        CLIError::Network(err)
    }
}
