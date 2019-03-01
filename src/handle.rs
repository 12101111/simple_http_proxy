use crate::config::Config;
use crate::http::Request;
use std::io::prelude::*;
use std::net::{TcpStream, ToSocketAddrs};
use std::result::Result;
use std::sync::Arc;
use std::time::Duration;

// size of buffer
const BUFFER_LEN: usize = 131072;
// timeout of client read operation
const CLIENT_TIMEOUT: u64 = 200;
// timeout of server read operation
const SERVER_TIMEOUT: u64 = 1000;

pub fn handle_client(mut stream: TcpStream, config: Arc<Config>) -> Result<(), String> {
    let peer_ip = stream.peer_addr().unwrap().ip();
    info!("incoming request: {}", peer_ip);
    // block client in blacklist
    for ip in &config.filter.ip {
        if format!("{}", peer_ip) == *ip {
            let strforbid =
                b"HTTP/1.1 403 Forbidden\r\n\r\n<h1>403 Forbidden</h1> You can't use this proxy!";
            stream
                .write(strforbid)
                .map_err(|e| format!("can't send 403 to client, {}", e))?;
            return Ok(());
        }
    }
    // set client read timeout to make loop end at timeout
    stream
        .set_read_timeout(Some(Duration::from_millis(CLIENT_TIMEOUT)))
        .map_err(|e| format!("can't set_read_timeout, {}", e))?;
    // this loop won't end untill any `return` or `Err`
    loop {
        // buffer for massage
        let mut req_buffer = [0u8; BUFFER_LEN];
        // number of bytes read from stream
        let req_bytes = stream.read(&mut req_buffer).unwrap_or(0);
        // Sometimes it read all blank content
        // 16 is the length of "GET / HTTP/1.1\r\n"
        if req_bytes < 16 {
            return Ok(());
        }
        // prase HTTP request
        let mut req = Request::parse(&mut req_buffer[..req_bytes])?;
        if req.method == "CONNECT" {
            return Ok(());
        }
        if req.method != "GET" && req.method != "POST" {
            return Err("Invalid or not support HTTP Method".to_owned());
        }
        // block website in blacklist
        for website in &config.filter.website {
            if req.host == website {
                let strforbid =
                        b"HTTP/1.1 451 Unavailable For Legal Reasons\r\n\r\n<h1>451 Unavailable For Legal Reasons</h1>";
                stream
                    .write(strforbid)
                    .map_err(|e| format!("can't send 451 to client, {}", e))?;
                return Ok(());
            }
        }
        // modify host for website in redirection list
        for i in 0..config.redirect.len() {
            if req.host == config.redirect[i].from {
                req.modify_host(&config.redirect[i].to);
            }
        }
        // log requset message
        info!("GOT HTTP REQUEST, size:{} bytes", req_bytes);
        trace!("{}", req);
        // to_socket_addrs() will resole host to ip address
        let host = format!("{}:80", req.host);
        let host = host
            .to_socket_addrs()
            .map_err(|e| format!("unable to resolve host, {}", e))?
            .next()
            .ok_or("can't resolve host, no result found")?;
        let mut server_stream = TcpStream::connect(&host)
            .map_err(|e| format!("Error to connect to Server {} : {}", host, e))?;
        req.write(&mut server_stream)
            .map_err(|e| format!("can't send message to remote server, {}", e))?;
        server_stream
            .set_read_timeout(Some(Duration::from_millis(SERVER_TIMEOUT)))
            .map_err(|e| format!("can't set_read_timeout, {}", e))?;
        loop {
            let mut res_buffer = [0u8; BUFFER_LEN];
            let bytes = server_stream.read(&mut res_buffer).unwrap_or(0);
            if bytes == 0 {
                return Ok(());
            }
            stream
                .write(&res_buffer[..bytes])
                .map_err(|e| format!("can't send message to client, {}", e))?;
            info!("GOT HTTP RESPONSE,size: {} bytes", bytes);
        }
    }
}
