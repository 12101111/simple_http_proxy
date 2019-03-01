#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate log;
mod config;
mod handle;
mod http;
mod threadpool;
use crate::config::Config;
use crate::handle::handle_client;
use crate::threadpool::ThreadPool;
use simplelog::*;
use std::fs::File;
use std::io;
use std::net::TcpListener;
use std::sync::Arc;

fn main() -> io::Result<()> {
    // open config file config.toml, panic if failed to open or prase
    // use crate serde and toml to prase this toml file
    let config = Config::open()?;

    // Pass config between threads using Arc.
    // Rust is a memory-safe and thread-safe language
    // Arc provides a immutable thread-safe reference counting pointer.
    // Using the same variable name will mask the previous one.
    let config = Arc::new(config);

    // setup logging, log to file and stderr
    let level = if config.verbose {
        LevelFilter::Trace
    } else {
        LevelFilter::Debug
    };
    CombinedLogger::init(vec![
        TermLogger::new(level, simplelog::Config::default()).unwrap(),
        WriteLogger::new(
            level,
            simplelog::Config::default(),
            File::create(&config.log).unwrap(),
        ),
    ])
    .unwrap();

    // start thread pool
    let pool = ThreadPool::new(config.thread);

    // bind to 0.0.0.0:8080. port can be changed in config.toml
    // In C, we use
    // socket = socket(AF_INET, SOCK_STREAM, 0);
    // bind(socket,&sockaddr,sizeof(sockaddr));
    // listen(socket,1024);
    // to do the same thing
    // Rust simplify that
    let listener = TcpListener::bind(format!("0.0.0.0:{}", config.port))?;

    // listener.incoming() returns an iterator over the connections being received on this listener.
    // Iterating over it is equivalent to calling accept in a loop.
    for stream in listener.incoming() {
        // This is how to handle errors in Rust.
        // Result is a enum that may contain result or error
        // use match to deal with it
        match stream {
            Ok(stream) => {
                // make another copy of Arc
                // keep in mind that Arc is just like a pointer
                let config = Arc::clone(&config);
                // pool.execute will send this closure to a thread worker
                // `move` will take the ownership of config to another thread
                pool.execute(move || {
                    // simple way to take Err of Result if you don't care Ok
                    if let Err(e) = handle_client(stream, config) {
                        error!("{}", e);
                    }
                })
            }
            Err(e) => {
                error!("{}", e);
            }
        }
    }

    // return without `return` keyword
    Ok(())
}
