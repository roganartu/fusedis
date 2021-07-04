use serde::Deserialize;
use std::fs;
use std::num::ParseIntError;
use std::path::PathBuf;
use toml;
use validator::Validate;

#[derive(Debug, Deserialize)]
pub struct Config {
    server: Option<Vec<RedisServer>>,
    permission: Option<Vec<PathPermission>>,
}

#[derive(Debug, Deserialize)]
pub struct RedisServer {
    // TODO add validation function that verifies:
    //   - scheme is either redis or rediss
    //   - host exists
    //   - port exists
    url: url::Url,
}

#[derive(Debug, Validate, Deserialize)]
pub struct PathPermission {
    user: Option<String>,
    group: Option<String>,
    #[validate(range(
        min = 0o000,
        max = 0o777,
        message = "Value must be between 000 and 777 (octal)"
    ))]
    chmod: Option<u16>,
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

pub fn load_file(src: PathBuf) -> Result<Config, ConfigError> {
    let f = match fs::read_to_string(src) {
        Ok(f) => f,
        Err(e) => return Err(ConfigError::Io(e)),
    };
    let config: Config = toml::from_str(&f).unwrap();
    Ok(config)
}
