use std::fs::File;
use std::io;
use std::io::prelude::*;

// Config field
// #[derive(Deserialize)] is a Procedural Macros
// Without write code,we can deserialize this struct
#[derive(Deserialize)]
pub struct Config {
    pub port: String,
    pub log: String,
    pub verbose: bool,
    pub thread: usize,
    pub filter: Filter,
    pub redirect: Vec<Redirect>,
}

// sub item
#[derive(Deserialize)]
pub struct Filter {
    pub website: Vec<String>,
    pub ip: Vec<String>,
}

// sub item
#[derive(Deserialize)]
pub struct Redirect {
    pub from: String,
    pub to: String,
}

impl Config {
    pub fn open() -> io::Result<Config> {
        let mut config_file = File::open("config.toml")?;
        let mut config_str = String::new();
        config_file.read_to_string(&mut config_str)?;
        toml::from_str(&config_str).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }
}
