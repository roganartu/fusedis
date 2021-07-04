use serde::Deserialize;
use std::fs;
use std::num::ParseIntError;
use std::path::PathBuf;
use toml;
use validator::Validate;

#[derive(Debug, Validate, Deserialize, Default)]
pub struct ConfigFile {
    pub server: Option<Vec<RedisServer>>,
    pub permission: Option<Vec<PathPermission>>,
    pub disable_raw: Option<bool>,
    pub read_only: Option<bool>,
    pub allow_other: Option<bool>,
    pub user: Option<String>,
    pub group: Option<String>,
    #[validate(range(
        min = 0o000,
        max = 0o777,
        message = "Value must be between 000 and 777 (octal)"
    ))]
    pub chmod: Option<u16>,
    pub max_results: Option<i64>,
}

#[derive(Debug, Validate, Default, Clone)]
pub struct Config {
    pub server: Vec<RedisServer>,
    pub permission: Vec<PathPermission>,
    pub disable_raw: bool,
    pub read_only: bool,
    pub allow_other: bool,
    pub user: String,
    pub group: String,
    #[validate(range(
        min = 0o000,
        max = 0o777,
        message = "Value must be between 000 and 777 (octal)"
    ))]
    pub chmod: u16,
    pub max_results: i64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RedisServer {
    // TODO add validation function that verifies:
    //   - scheme is either redis or rediss
    //   - host exists
    //   - port exists
    pub url: url::Url,
}

#[derive(Debug, Validate, Deserialize, Clone)]
pub struct PathPermission {
    pub pattern: String,
    // TODO validate that at least one of user, group, or chmod is provided.
    pub user: Option<String>,
    pub group: Option<String>,
    #[validate(range(
        min = 0o000,
        max = 0o777,
        message = "Value must be between 000 and 777 (octal)"
    ))]
    pub chmod: Option<u16>,
}

quick_error! {
    #[derive(Debug)]
    pub enum PermissionParsingError {
        BadValue(err: ParseIntError) {
            source(err)
            display("Error parsing permission string: {}. Value must be between 000 and 777 (octal)", err)
        }
        OutOfRange {
            display("Value must be between 000 and 777 (octal)")
        }
    }
}

quick_error! {
    #[derive(Debug)]
    pub enum ConfigError {
        Io(err: std::io::Error) {
            source(err)
        }
    }
}

pub fn parse_octal(src: &str) -> Result<u16, PermissionParsingError> {
    match u16::from_str_radix(src, 8) {
        Ok(parsed) => match parsed {
            0o000..=0o777 => Ok(parsed),
            _ => Err(PermissionParsingError::OutOfRange),
        },
        Err(e) => Err(PermissionParsingError::BadValue(e)),
    }
}

pub fn load_file(src: PathBuf) -> Result<ConfigFile, ConfigError> {
    let f = match fs::read_to_string(src) {
        Ok(f) => f,
        Err(e) => return Err(ConfigError::Io(e)),
    };
    let config: ConfigFile = toml::from_str(&f).unwrap();
    Ok(config)
}
