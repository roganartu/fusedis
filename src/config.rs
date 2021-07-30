use serde::Deserialize;
use std::fmt;
use std::fs;
use std::num::ParseIntError;
use std::path::PathBuf;
use toml;
use validator::Validate;

#[derive(Debug, Validate, Deserialize, Default)]
pub struct ConfigFile {
    pub cluster_mode: Option<bool>,
    pub redis: Option<RedisServer>,
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
    // TODO allow configuring r2d2 connection pooling
}

#[derive(Debug, Validate, Default, Clone)]
pub struct Config {
    pub cluster_mode: bool,
    pub redis: Option<RedisServer>,
    pub permission: Vec<PathPermission>,
    pub disable_raw: bool,
    pub read_only: bool,
    pub allow_other: bool,
    pub uid: u32,
    pub gid: u32,
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
    // TODO validate with https://docs.rs/redis/0.20.2/redis/fn.parse_redis_url.html
    pub url: url::Url,
}

impl fmt::Display for RedisServer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.url.to_string())
    }
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
        UserNotFound {
            display("User not found.")
        }
        GroupNotFound {
            display("Group not found.")
        }
        NoDriver {
            display("No driver provided in config file.")
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
