use serde::Deserialize;
use std::num::ParseIntError;

#[derive(Deserialize)]
struct Config {
    server: Vec<RedisServer>,
    permission: Vec<PathPermission>,
}

#[derive(Deserialize)]
struct RedisServer {
    url: url::Url,
}

#[derive(Deserialize)]
struct PathPermission {
    user: Option<String>,
    group: Option<String>,
    // TODO figure out how to use parse_octal with this
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

pub fn parse_octal(src: &str) -> Result<u16, PermissionParsingError> {
    match u16::from_str_radix(src, 8) {
        Ok(parsed) => match parsed {
            0o000..=0o777 => Ok(parsed),
            _ => Err(PermissionParsingError::OutOfRange),
        },
        Err(e) => Err(PermissionParsingError::BadValue(e)),
    }
}
