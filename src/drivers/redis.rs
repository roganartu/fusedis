use crate::fuse;

use redis;
use redis::Commands;
use std::error::Error;

const INO_CACHE_KEY: &str = "__fusekv_ino_cache__";

macro_rules! get_conn {
    ($client:expr) => {
        match $client.get_connection() {
            Ok(c) => c,
            Err(e) => {
                log::debug!("Error getting redis connection: {}", e);
                return Err(Box::new(e));
            }
        }
    };
}

macro_rules! redis_cmd {
    ($con:expr, $cmd:expr$(, $arg:expr)*) => {
        match redis::cmd($cmd)$(.arg($arg))*.query(&mut $con) {
            Ok(v) => v,
            Err(e) => {
                log::debug!("Error querying redis: {}", e);
                return Err(Box::new(e))
            }
        }
    };
}

#[derive(Debug, Clone)]
pub struct RedisDriver {
    // TODO add a box for the connection
    // TODO keep track of ino mappings locally to avoid Redis lookup?
    client: redis::Client,
}

impl fuse::KVReader for RedisDriver {
    fn get_by_name(&self, name: String, ino: u64) -> Result<Option<fuse::KVEntry>, Box<dyn Error>> {
        // We have a name, so we can just look directly into redis
        let mut conn = get_conn!(self.client);
        // TODO not sure if this is the best idea, it reads the whole value into
        // memory which might cause problems with large values.
        let value: String = match redis_cmd!(conn, "GET", &name) {
            Some(v) => v,
            None => return Ok(None),
        };
        // Insert ino into redis cache so we can lookup the name of the key later
        // in get_by_ino.
        // TODO make the key configurable?
        match conn.hset::<&str, &str, u64, u64>(INO_CACHE_KEY, &name, ino) {
            Err(e) => log::error!("Error updating ino cache {}.", e),
            _ => {}
        };
        Ok(Some(fuse::KVEntry::new(ino, name, value)))
    }
    fn get_by_ino(&self, ino: u64) -> Result<Option<fuse::KVEntry>, Box<dyn Error>> {
        // TODO impl
        Ok(None)
    }
    fn list_keys(&self, offset: i64) -> Result<Vec<fuse::KVRef>, Box<dyn Error>> {
        // TODO impl
        Ok(vec![])
    }
    fn read(&self, ino: u64, fh: u64, offset: i64) -> Result<Vec<u8>, Box<dyn Error>> {
        // TODO impl
        Ok(vec![])
    }
}

impl RedisDriver {
    pub fn new(client: redis::Client) -> RedisDriver {
        RedisDriver { client: client }
    }
}
