#[macro_use]
extern crate serde_derive;
extern crate toml;
extern crate url;

#[derive(Deserialize)]
struct Config {
    server: Vec<RedisServer>,
}

#[derive(Deserialize)]
struct RedisServer {
    url: url::Url,
}
